//! User-extension hook system, compatible with Claude Code / Codex `hooks.json`.
//!
//! Reads two files (merged, project takes precedence):
//!   1. `~/.jcode/hooks.json`              (global, per-user)
//!   2. `<working_dir>/.jcode/hooks.json`  (per-project)
//!
//! Schema (same as Claude Code / Codex):
//!
//! ```json
//! {
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "matcher": "Bash",
//!         "hooks": [
//!           { "type": "command", "command": "rtk hook claude" }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! Currently implements `PreToolUse` only. Hook commands receive a JSON object
//! on stdin (`{"tool_name": ..., "tool_input": ...}`) and may print a
//! `hookSpecificOutput` JSON object on stdout. If `updatedInput` is present
//! in the response, the tool input is rewritten before execution.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::process::Command as TokioCommand;

/// Default per-hook timeout. Hook commands that exceed this are killed and
/// treated as no-ops (failing closed — the original tool input is used).
const HOOK_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct HooksFile {
    #[serde(default)]
    hooks: HooksMap,
}

/// Map of event name → list of matcher-groups.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
struct HooksMap {
    #[serde(rename = "PreToolUse", default)]
    pre_tool_use: Vec<MatcherGroup>,
    // Future: PostToolUse, UserPromptSubmit, etc.
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct MatcherGroup {
    /// Pattern to match the tool name. `*` (or empty/missing) matches any tool.
    /// Substring match against `tool_name`.
    #[serde(default)]
    matcher: Option<String>,
    #[serde(default)]
    hooks: Vec<HookSpec>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct HookSpec {
    /// Currently always "command". Reserved for future hook kinds.
    #[serde(rename = "type", default)]
    kind: Option<String>,
    /// Shell command line to run. Receives the event JSON on stdin.
    command: String,
    /// Optional per-hook timeout override in milliseconds.
    #[serde(default)]
    timeout_ms: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookOutput {
    #[serde(default)]
    hook_specific_output: Option<HookSpecificOutput>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct HookSpecificOutput {
    /// If present, replaces the tool input for the upcoming tool call.
    #[serde(default)]
    updated_input: Option<Value>,
    /// Reserved for future use: permission decisions, additional context, etc.
    #[serde(flatten, default)]
    _other: serde_json::Map<String, Value>,
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

static GLOBAL_HOOKS_PATH: OnceLock<PathBuf> = OnceLock::new();

fn global_hooks_path() -> &'static Path {
    GLOBAL_HOOKS_PATH.get_or_init(|| {
        // `JCODE_HOOKS_FILE` lets tests and power users override the global
        // hooks file location. Setting it to an empty string or a non-existent
        // path effectively disables global hooks.
        if let Ok(p) = std::env::var("JCODE_HOOKS_FILE") {
            return PathBuf::from(p);
        }
        crate::storage::user_home_path("hooks.json").unwrap_or_else(|_| PathBuf::from(""))
    })
}

fn project_hooks_path(working_dir: Option<&Path>) -> Option<PathBuf> {
    let wd = working_dir?;
    Some(wd.join(".jcode").join("hooks.json"))
}

fn load_one(path: &Path) -> Option<HooksFile> {
    if !path.exists() {
        return None;
    }
    match std::fs::read_to_string(path) {
        Ok(text) => match serde_json::from_str::<HooksFile>(&text) {
            Ok(parsed) => Some(parsed),
            Err(e) => {
                crate::logging::warn(&format!(
                    "Ignoring malformed hooks file {}: {}",
                    path.display(),
                    e
                ));
                None
            }
        },
        Err(e) => {
            crate::logging::warn(&format!(
                "Failed to read hooks file {}: {}",
                path.display(),
                e
            ));
            None
        }
    }
}

fn load_effective_hooks(working_dir: Option<&Path>) -> HooksFile {
    let mut merged = HooksFile::default();

    if let Some(g) = load_one(global_hooks_path()) {
        merged.hooks.pre_tool_use.extend(g.hooks.pre_tool_use);
    }
    if let Some(pp) = project_hooks_path(working_dir)
        && let Some(p) = load_one(&pp)
    {
        // Project hooks run *after* global ones so they can override.
        merged.hooks.pre_tool_use.extend(p.hooks.pre_tool_use);
    }

    merged
}

// ---------------------------------------------------------------------------
// Matching
// ---------------------------------------------------------------------------

fn matcher_matches(matcher: Option<&str>, tool_name: &str) -> bool {
    match matcher.map(str::trim) {
        None | Some("") | Some("*") => true,
        Some(pat) => {
            // Same convention as Claude / Codex: substring (case-sensitive)
            // against the tool name.
            tool_name.contains(pat)
        }
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Run all `PreToolUse` hooks that match `tool_name` and return a (possibly
/// rewritten) tool input.
///
/// Failure modes (logged but non-fatal):
///   * Hook file missing or malformed → original `tool_input` returned.
///   * Hook command non-zero / timeout / non-JSON output → that hook is
///     skipped; remaining hooks still run.
///
/// Hook commands are executed in `working_dir` if provided, else inherit the
/// current process's CWD.
pub async fn apply_pre_tool_use(
    tool_name: &str,
    tool_input: Value,
    working_dir: Option<&Path>,
) -> Value {
    let cfg = load_effective_hooks(working_dir);
    if cfg.hooks.pre_tool_use.is_empty() {
        return tool_input;
    }

    let mut current_input = tool_input;
    for group in &cfg.hooks.pre_tool_use {
        if !matcher_matches(group.matcher.as_deref(), tool_name) {
            continue;
        }
        for hook in &group.hooks {
            if let Some(updated) = run_one(hook, tool_name, &current_input, working_dir).await {
                current_input = updated;
            }
        }
    }
    current_input
}

/// Run a single hook command and return `Some(updated_input)` if it produced
/// one, else `None`.
async fn run_one(
    hook: &HookSpec,
    tool_name: &str,
    current_input: &Value,
    working_dir: Option<&Path>,
) -> Option<Value> {
    let cmd = hook.command.trim();
    if cmd.is_empty() {
        return None;
    }
    let timeout = Duration::from_millis(hook.timeout_ms.unwrap_or(HOOK_TIMEOUT_MS));

    // Compose stdin payload in the Claude/Codex format.
    let payload = serde_json::json!({
        "tool_name": tool_name,
        "tool_input": current_input,
        // hook_event_name is conventional; Claude/Codex send it too.
        "hook_event_name": "PreToolUse",
    });
    let stdin_bytes = payload.to_string();

    let mut command = TokioCommand::new("sh");
    command.arg("-c").arg(cmd);
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.kill_on_drop(true);
    if let Some(wd) = working_dir {
        command.current_dir(wd);
    }

    let mut child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            crate::logging::warn(&format!("Hook spawn failed (cmd={}): {}", cmd, e));
            return None;
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(stdin_bytes.as_bytes()).await;
        // Drop stdin to signal EOF.
        drop(stdin);
    }

    let output = match tokio::time::timeout(timeout, child.wait_with_output()).await {
        Ok(Ok(out)) => out,
        Ok(Err(e)) => {
            crate::logging::warn(&format!("Hook wait failed (cmd={}): {}", cmd, e));
            return None;
        }
        Err(_) => {
            crate::logging::warn(&format!(
                "Hook timed out after {}ms (cmd={})",
                timeout.as_millis(),
                cmd
            ));
            return None;
        }
    };

    if !output.status.success() {
        // Non-zero exit is allowed: many hooks signal "no rewrite" by exiting
        // 0 with empty stdout; only the JSON-output path is checked. We still
        // log the stderr for visibility.
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if !stderr.is_empty() {
            crate::logging::info(&format!(
                "Hook non-zero exit (cmd={} status={}): {}",
                cmd, output.status, stderr
            ));
        }
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stdout.is_empty() {
        return None;
    }

    match serde_json::from_str::<HookOutput>(&stdout) {
        Ok(parsed) => {
            let updated = parsed
                .hook_specific_output
                .and_then(|hso| hso.updated_input);
            if updated.is_some() {
                crate::logging::info(&format!("Hook rewrote tool input (cmd={})", cmd));
            }
            updated
        }
        Err(_) => {
            // Tolerate non-JSON output silently — many hooks just want to log.
            None
        }
    }
}

// Re-export for tests
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn matcher_wildcard_matches_anything() {
        assert!(matcher_matches(None, "bash"));
        assert!(matcher_matches(Some(""), "bash"));
        assert!(matcher_matches(Some("*"), "bash"));
        assert!(matcher_matches(Some("*"), "edit"));
    }

    #[test]
    fn matcher_substring_match() {
        assert!(matcher_matches(Some("Bash"), "Bash"));
        assert!(matcher_matches(Some("ash"), "bash"));
        assert!(!matcher_matches(Some("Bash"), "edit"));
    }

    #[tokio::test]
    async fn apply_pre_tool_use_no_hooks_is_passthrough() {
        // No global file path can resolve to anything meaningful in tests
        // without a real ~/.jcode/hooks.json; treat missing as no-op.
        let input = json!({"command": "ls"});
        let out = apply_pre_tool_use("bash", input.clone(), None).await;
        assert_eq!(out, input);
    }

    #[tokio::test]
    async fn run_one_rewrites_via_hook_command() {
        // Use a tiny shell pipeline as the "hook" — it ignores stdin and
        // emits a valid HookOutput rewriting the command to `rtk ls`.
        let hook = HookSpec {
            kind: Some("command".to_string()),
            command: r#"printf '{"hookSpecificOutput":{"updatedInput":{"command":"rtk ls"}}}'"#
                .to_string(),
            timeout_ms: Some(2_000),
        };
        let updated = run_one(&hook, "bash", &json!({"command": "ls"}), None).await;
        assert_eq!(updated, Some(json!({"command": "rtk ls"})));
    }

    #[tokio::test]
    async fn run_one_returns_none_on_empty_stdout() {
        let hook = HookSpec {
            kind: Some("command".to_string()),
            command: "true".to_string(),
            timeout_ms: Some(2_000),
        };
        let updated = run_one(&hook, "bash", &json!({"command": "ls"}), None).await;
        assert_eq!(updated, None);
    }

    #[tokio::test]
    async fn run_one_times_out_gracefully() {
        let hook = HookSpec {
            kind: Some("command".to_string()),
            command: "sleep 5".to_string(),
            timeout_ms: Some(100),
        };
        let updated = run_one(&hook, "bash", &json!({"command": "ls"}), None).await;
        assert_eq!(updated, None);
    }
}
