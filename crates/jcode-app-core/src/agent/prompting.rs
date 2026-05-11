use super::Agent;
use crate::logging;
use crate::message::{Message, ToolDefinition};

impl Agent {
    pub(super) fn log_prompt_prefix_accounting(
        &self,
        split: &crate::prompt::SplitSystemPrompt,
        tools: &[ToolDefinition],
    ) {
        let system_tokens = split.estimated_tokens();
        let tool_tokens = ToolDefinition::aggregate_prompt_token_estimate(tools);
        let prefix_tokens = system_tokens + tool_tokens;
        logging::info(&format!(
            "Prompt prefix estimate: total={} tokens (system={} tools={})",
            prefix_tokens, system_tokens, tool_tokens
        ));
    }

    pub(super) async fn build_memory_prompt_nonblocking_shared(
        &self,
        messages: std::sync::Arc<[Message]>,
        memory_event_tx: Option<crate::memory::MemoryEventSink>,
    ) -> Option<crate::memory::PendingMemory> {
        if !self.memory_enabled {
            return None;
        }

        let session_id = &self.session.id;

        let pending = if crate::message::ends_with_fresh_user_turn(&messages) {
            crate::memory::take_pending_memory(session_id)
        } else {
            None
        };

        // Use the persistent memory-agent pipeline as the single source of truth
        // for pre-warming the next turn. This is fire-and-forget.
        crate::memory_agent::update_context_sync_with_dir(
            session_id,
            std::sync::Arc::clone(&messages),
            self.session.working_dir.clone(),
        );

        if pending.is_some() {
            return pending;
        }

        // First-turn (or always-block) path: briefly wait for a memory fetch so
        // the very first prompt actually sees memory context.
        let cfg = &crate::config::config().memory;
        let should_block = match cfg.block_mode {
            crate::config::MemoryBlockMode::Never => false,
            crate::config::MemoryBlockMode::FirstTurnOnly => Self::is_first_user_turn(&messages),
            crate::config::MemoryBlockMode::Always => true,
        };
        if !should_block || !crate::message::ends_with_fresh_user_turn(&messages) {
            return None;
        }

        let timeout = std::time::Duration::from_millis(cfg.block_timeout_ms.max(1));
        let manager = self
            .session
            .working_dir
            .as_deref()
            .map(|dir| crate::memory::MemoryManager::new().with_project_dir(dir))
            .unwrap_or_default();

        crate::logging::info(&format!(
            "Blocking on memory fetch for first turn (timeout={}ms)",
            timeout.as_millis()
        ));
        manager
            .fetch_relevant_blocking(session_id, &messages, timeout, memory_event_tx)
            .await
    }

    /// Heuristic: true if `messages` is the user's first turn in this session
    /// (i.e. no assistant message has been produced yet).
    fn is_first_user_turn(messages: &[Message]) -> bool {
        !messages
            .iter()
            .any(|m| matches!(m.role, crate::message::Role::Assistant))
    }

    fn append_current_turn_system_reminder(&self, split: &mut crate::prompt::SplitSystemPrompt) {
        let Some(reminder) = self
            .current_turn_system_reminder
            .as_ref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        else {
            return;
        };

        if !split.dynamic_part.is_empty() {
            split.dynamic_part.push_str("\n\n");
        }
        split.dynamic_part.push_str("# System Reminder\n\n");
        split.dynamic_part.push_str(reminder);
    }

    /// Build split system prompt for better caching
    /// Returns static (cacheable) and dynamic (not cached) parts separately
    pub(super) fn build_system_prompt_split(
        &self,
        memory_prompt: Option<&str>,
    ) -> crate::prompt::SplitSystemPrompt {
        if let Some(ref override_prompt) = self.system_prompt_override {
            return crate::prompt::SplitSystemPrompt {
                static_part: override_prompt.clone(),
                dynamic_part: String::new(),
            };
        }

        let skills = self.current_skills_snapshot();
        let skill_prompt = self
            .active_skill
            .as_ref()
            .and_then(|name| skills.get(name).map(|skill| skill.get_prompt().to_string()));

        let available_skills: Vec<crate::prompt::SkillInfo> = self
            .current_skills_snapshot()
            .list()
            .iter()
            .map(|skill| crate::prompt::SkillInfo {
                name: skill.name.clone(),
                description: skill.description.clone(),
            })
            .collect();

        let working_dir = self
            .session
            .working_dir
            .as_ref()
            .map(std::path::PathBuf::from);

        let (mut split, _context_info) = crate::prompt::build_system_prompt_split(
            skill_prompt.as_deref(),
            &available_skills,
            self.session.is_canary,
            memory_prompt,
            working_dir.as_deref(),
        );

        self.append_current_turn_system_reminder(&mut split);

        split
    }

    /// Non-blocking memory prompt - takes pending result and spawns check for next turn.
    /// On the first user turn (or when configured to always block), waits briefly
    /// for a fresh memory fetch so the very first prompt actually sees memory.
    pub(super) async fn build_memory_prompt_nonblocking(
        &self,
        messages: &[Message],
        memory_event_tx: Option<crate::memory::MemoryEventSink>,
    ) -> Option<crate::memory::PendingMemory> {
        self.build_memory_prompt_nonblocking_shared(messages.to_vec().into(), memory_event_tx)
            .await
    }
}
