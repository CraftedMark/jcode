use anyhow::{Result, anyhow};
use clap::Parser;
use jcode::id::new_id;
use jcode::message::{Message, ToolDefinition};
use jcode::provider::{EventStream, Provider};
use jcode::tool::{Registry, ToolContext, ToolExecutionMode};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "jcode-harness")]
#[command(about = "Run a deterministic tool harness smoke test")]
struct Args {
    /// Use an explicit working directory (defaults to a temp folder).
    #[arg(long)]
    cwd: Option<String>,

    /// Include network-backed tools (webfetch/websearch/codesearch).
    #[arg(long)]
    include_network: bool,
}

struct NoopProvider;

#[async_trait::async_trait]
impl Provider for NoopProvider {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &[ToolDefinition],
        _system: &str,
        _resume_session_id: Option<&str>,
    ) -> Result<EventStream> {
        anyhow::bail!("Noop provider - tool harness does not invoke models.")
    }

    fn name(&self) -> &str {
        "noop"
    }

    fn fork(&self) -> Arc<dyn Provider> {
        Arc::new(NoopProvider)
    }

    fn available_models_display(&self) -> Vec<String> {
        vec![]
    }

    async fn prefetch_models(&self) -> Result<()> {
        Ok(())
    }
}

struct ToolCase {
    name: &'static str,
    input: serde_json::Value,
    label: &'static str,
    check: ToolCheck,
}

#[derive(Clone, Copy)]
enum ToolCheck {
    OutputContains(&'static str),
    OutputContainsAll(&'static [&'static str]),
    OutputContainsWorkspacePath,
    WorkspaceFileEquals {
        path: &'static str,
        content: &'static str,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let workspace = resolve_workspace(args.cwd)?;

    std::fs::create_dir_all(&workspace)?;
    std::env::set_current_dir(&workspace)?;
    eprintln!("Harness workspace: {}", workspace.display());

    let provider: Arc<dyn Provider> = Arc::new(NoopProvider);
    let registry = Registry::new(provider).await;

    let session_id = new_id("harness");
    let base_ctx = ToolContext {
        session_id: session_id.clone(),
        message_id: session_id.clone(),
        tool_call_id: String::new(),
        working_dir: Some(workspace.clone()),
        stdin_request_tx: None,
        graceful_shutdown_signal: None,
        execution_mode: ToolExecutionMode::Direct,
    };

    let mut cases = Vec::new();
    cases.push(ToolCase {
        name: "write",
        label: "write sample.txt",
        input: json!({"file_path": "sample.txt", "content": "alpha\nbeta\n"}),
        check: ToolCheck::WorkspaceFileEquals {
            path: "sample.txt",
            content: "alpha\nbeta\n",
        },
    });
    cases.push(ToolCase {
        name: "read",
        label: "read sample.txt",
        input: json!({"file_path": "sample.txt"}),
        check: ToolCheck::OutputContainsAll(&["alpha", "beta"]),
    });
    cases.push(ToolCase {
        name: "edit",
        label: "edit sample.txt (alpha -> alpha1)",
        input: json!({"file_path": "sample.txt", "old_string": "alpha", "new_string": "alpha1"}),
        check: ToolCheck::WorkspaceFileEquals {
            path: "sample.txt",
            content: "alpha1\nbeta\n",
        },
    });
    cases.push(ToolCase {
        name: "multiedit",
        label: "multiedit sample.txt",
        input: json!({
            "file_path": "sample.txt",
            "edits": [
                {"old_string": "alpha1", "new_string": "alpha2"},
                {"old_string": "beta", "new_string": "beta1"}
            ]
        }),
        check: ToolCheck::WorkspaceFileEquals {
            path: "sample.txt",
            content: "alpha2\nbeta1\n",
        },
    });
    cases.push(ToolCase {
        name: "patch",
        label: "patch sample.txt",
        input: json!({"patch_text": "--- a/sample.txt\n+++ b/sample.txt\n@@ -1,2 +1,3 @@\n alpha2\n beta1\n+gamma\n"}),
        check: ToolCheck::WorkspaceFileEquals {
            path: "sample.txt",
            content: "alpha2\nbeta1\ngamma\n",
        },
    });
    cases.push(ToolCase {
        name: "apply_patch",
        label: "apply_patch add file",
        input: json!({"patch_text": "*** Begin Patch\n*** Add File: added.txt\n+added\n*** End Patch\n"}),
        check: ToolCheck::WorkspaceFileEquals {
            path: "added.txt",
            content: "added\n",
        },
    });
    cases.push(ToolCase {
        name: "ls",
        label: "ls .",
        input: json!({"path": "."}),
        check: ToolCheck::OutputContainsAll(&["added.txt", "sample.txt"]),
    });
    cases.push(ToolCase {
        name: "glob",
        label: "glob *.txt",
        input: json!({"pattern": "*.txt"}),
        check: ToolCheck::OutputContainsAll(&["added.txt", "sample.txt"]),
    });
    cases.push(ToolCase {
        name: "grep",
        label: "grep gamma",
        input: json!({"pattern": "gamma", "path": "."}),
        check: ToolCheck::OutputContains("gamma"),
    });
    cases.push(ToolCase {
        name: "bash",
        label: "bash pwd",
        input: json!({"command": "pwd"}),
        check: ToolCheck::OutputContainsWorkspacePath,
    });
    cases.push(ToolCase {
        name: "invalid",
        label: "invalid tool call",
        input: json!({"tool": "unknown", "error": "missing required field"}),
        check: ToolCheck::OutputContains("Invalid tool invocation for 'unknown'"),
    });
    cases.push(ToolCase {
        name: "todo",
        label: "todo write",
        input: json!({"todos": [{"content": "harness task", "status": "pending", "priority": "low", "id": "1"}]}),
        check: ToolCheck::OutputContains("harness task"),
    });
    cases.push(ToolCase {
        name: "todo",
        label: "todo read",
        input: json!({}),
        check: ToolCheck::OutputContains("harness task"),
    });
    cases.push(ToolCase {
        name: "batch",
        label: "batch ls + read",
        input: json!({
            "tool_calls": [
                {"tool": "ls", "parameters": {"path": "."}},
                {"tool": "read", "parameters": {"file_path": "sample.txt"}}
            ]
        }),
        check: ToolCheck::OutputContainsAll(&["Completed: 2 succeeded, 0 failed", "gamma"]),
    });

    if args.include_network {
        cases.push(ToolCase {
            name: "webfetch",
            label: "webfetch example.com",
            input: json!({"url": "https://example.com", "format": "text"}),
            check: ToolCheck::OutputContains("Example Domain"),
        });
        cases.push(ToolCase {
            name: "websearch",
            label: "websearch rust async",
            input: json!({"query": "rust async await"}),
            check: ToolCheck::OutputContains("rust"),
        });
        cases.push(ToolCase {
            name: "codesearch",
            label: "codesearch tokio spawn",
            input: json!({"query": "tokio::spawn"}),
            check: ToolCheck::OutputContains("tokio"),
        });
    }

    let mut failures = Vec::new();

    for (idx, case) in cases.iter().enumerate() {
        let ctx = ToolContext {
            tool_call_id: format!("harness-{}", idx + 1),
            ..base_ctx.clone()
        };
        println!("\n== {} ({}) ==", case.name, case.label);
        match registry.execute(case.name, case.input.clone(), ctx).await {
            Ok(output) => {
                if let Some(title) = output.title {
                    println!("[title] {}", title);
                }
                println!("{}", output.output);
                if let Err(err) = verify_case(case, &output.output, &workspace) {
                    println!("[check] FAIL: {}", err);
                    failures.push(format!("{}: {}", case.label, err));
                } else {
                    println!("[check] ok");
                }
            }
            Err(err) => {
                println!("[error] {}", err);
                failures.push(format!("{}: tool returned error: {}", case.label, err));
            }
        }
    }

    if !failures.is_empty() {
        println!("\nHarness failed {} check(s):", failures.len());
        for failure in failures {
            println!(" - {}", failure);
        }
        return Err(anyhow!("tool harness checks failed"));
    }

    println!("\nHarness passed {} check(s).", cases.len());
    Ok(())
}

fn resolve_workspace(cwd: Option<String>) -> Result<PathBuf> {
    if let Some(cwd) = cwd {
        absolutize_path(PathBuf::from(cwd))
    } else {
        create_temp_workspace()
    }
}

fn create_temp_workspace() -> Result<PathBuf> {
    let mut path = std::env::temp_dir();
    path.push(format!("jcode-harness-{}", new_id("run")));
    absolutize_path(path)
}

fn absolutize_path(path: PathBuf) -> Result<PathBuf> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

fn verify_case(case: &ToolCase, output: &str, workspace: &std::path::Path) -> Result<()> {
    match case.check {
        ToolCheck::OutputContains(expected) => {
            if output.contains(expected) {
                Ok(())
            } else {
                Err(anyhow!("output did not contain {:?}", expected))
            }
        }
        ToolCheck::OutputContainsAll(expected_values) => {
            let missing: Vec<&str> = expected_values
                .iter()
                .copied()
                .filter(|expected| !output.contains(expected))
                .collect();
            if missing.is_empty() {
                Ok(())
            } else {
                Err(anyhow!("output missing {:?}", missing))
            }
        }
        ToolCheck::OutputContainsWorkspacePath => {
            let expected = workspace.display().to_string();
            if output.contains(&expected) {
                Ok(())
            } else {
                Err(anyhow!(
                    "output did not contain workspace path {}",
                    expected
                ))
            }
        }
        ToolCheck::WorkspaceFileEquals { path, content } => {
            let actual = std::fs::read_to_string(workspace.join(path))
                .map_err(|err| anyhow!("failed to read {}: {}", path, err))?;
            if actual == content {
                Ok(())
            } else {
                Err(anyhow!(
                    "{} content mismatch: expected {:?}, got {:?}",
                    path,
                    content,
                    actual
                ))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolutize_path_preserves_absolute_paths() {
        let path = PathBuf::from("/tmp/jcode-harness-test");

        assert_eq!(absolutize_path(path.clone()).unwrap(), path);
    }

    #[test]
    fn absolutize_path_resolves_relative_paths_from_current_dir() {
        let path = PathBuf::from(".context/harness-test");
        let expected = std::env::current_dir().unwrap().join(&path);

        assert_eq!(absolutize_path(path).unwrap(), expected);
    }
}
