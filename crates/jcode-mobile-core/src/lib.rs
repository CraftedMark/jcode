use serde::{Deserialize, Serialize};

pub mod protocol;
mod visual;

pub use visual::*;

fn is_zero_u32(value: &u32) -> bool {
    *value == 0
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Screen {
    Onboarding,
    Pairing,
    Chat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MessageRole {
    User,
    Assistant,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolCallState {
    Streaming,
    Executing,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: String,
    pub output: Option<String>,
    pub error: Option<String>,
    pub state: ToolCallState,
}

impl ToolCall {
    pub fn new(id: String, name: String) -> Self {
        Self {
            id,
            name,
            input: String::new(),
            output: None,
            error: None,
            state: ToolCallState::Streaming,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: MessageRole,
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRequest {
    pub id: String,
    pub command_summary: String,
    pub workspace: Option<String>,
    pub risk: ApprovalRisk,
    pub timeout_seconds: Option<u32>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerSummary {
    pub host: String,
    pub port: String,
    pub server_name: String,
    pub server_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingForm {
    pub host: String,
    pub port: String,
    pub pair_code: String,
    pub device_name: String,
}

impl Default for PairingForm {
    fn default() -> Self {
        Self {
            host: String::new(),
            port: "7643".to_string(),
            pair_code: String::new(),
            device_name: "jcode simulator".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulatorState {
    pub screen: Screen,
    pub connection_state: ConnectionState,
    pub pairing: PairingForm,
    pub saved_servers: Vec<ServerSummary>,
    pub selected_server: Option<ServerSummary>,
    pub status_message: Option<String>,
    pub error_message: Option<String>,
    pub messages: Vec<ChatMessage>,
    pub draft_message: String,
    pub active_session_id: Option<String>,
    pub sessions: Vec<String>,
    #[serde(default)]
    pub session_summaries: Vec<protocol::MobileSessionSummary>,
    pub available_models: Vec<String>,
    pub model_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_transport: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connection_phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub status_detail: Option<String>,
    pub is_processing: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pending_approvals: Vec<ApprovalRequest>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_tool_id: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub reconnect_attempt: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_model_name: Option<String>,
}

pub type MobileAppState = SimulatorState;

impl Default for SimulatorState {
    fn default() -> Self {
        Self::for_scenario(ScenarioName::Onboarding)
    }
}

impl SimulatorState {
    pub fn for_scenario(scenario: ScenarioName) -> Self {
        match scenario {
            ScenarioName::Onboarding => Self {
                screen: Screen::Onboarding,
                connection_state: ConnectionState::Disconnected,
                pairing: PairingForm::default(),
                saved_servers: Vec::new(),
                selected_server: None,
                status_message: Some("Ready to pair with a jcode server.".to_string()),
                error_message: None,
                messages: Vec::new(),
                draft_message: String::new(),
                active_session_id: None,
                sessions: Vec::new(),
                session_summaries: Vec::new(),
                available_models: Vec::new(),
                model_name: None,
                provider_name: None,
                connection_transport: None,
                connection_phase: None,
                status_detail: None,
                is_processing: false,
                pending_approvals: Vec::new(),
                active_tool_id: None,
                reconnect_attempt: 0,
                pending_session_id: None,
                pending_model_name: None,
            },
            ScenarioName::PairingReady => Self {
                pairing: PairingForm {
                    host: "devbox.tailnet.ts.net".to_string(),
                    port: "7643".to_string(),
                    pair_code: "123456".to_string(),
                    device_name: "jcode simulator".to_string(),
                },
                status_message: Some("Fields prefilled for simulated pairing.".to_string()),
                ..Self::for_scenario(ScenarioName::Onboarding)
            },
            ScenarioName::ConnectedChat => {
                let server = ServerSummary {
                    host: "devbox.tailnet.ts.net".to_string(),
                    port: "7643".to_string(),
                    server_name: "jcode".to_string(),
                    server_version: env!("CARGO_PKG_VERSION").to_string(),
                };
                Self {
                    screen: Screen::Chat,
                    connection_state: ConnectionState::Connected,
                    pairing: PairingForm {
                        host: server.host.clone(),
                        port: server.port.clone(),
                        pair_code: String::new(),
                        device_name: "jcode simulator".to_string(),
                    },
                    saved_servers: vec![server.clone()],
                    selected_server: Some(server),
                    status_message: Some("Connected to simulated jcode server.".to_string()),
                    error_message: None,
                    messages: vec![
                        ChatMessage {
                            id: "msg-user-1".to_string(),
                            role: MessageRole::User,
                            text: "Can you summarize the simulator architecture?".to_string(),
                            tool_calls: Vec::new(),
                        },
                        ChatMessage {
                            id: "msg-assistant-1".to_string(),
                            role: MessageRole::Assistant,
                            text: "The simulator is headless-first, automation-first, and shares state semantics with the future iOS app.".to_string(),
                            tool_calls: Vec::new(),
                        },
                    ],
                    draft_message: String::new(),
                    active_session_id: Some("session_sim_1".to_string()),
                    sessions: vec!["session_sim_1".to_string(), "session_sim_2".to_string()],
                    session_summaries: vec![
                        simulated_session_summary("session_sim_1", "fox", Some("Simulator chat"), true),
                        simulated_session_summary("session_sim_2", "oak", Some("Release follow-up"), false),
                    ],
                    available_models: vec!["gpt-5".to_string(), "claude-sonnet-4".to_string()],
                    model_name: Some("gpt-5".to_string()),
                    provider_name: Some("openai".to_string()),
                    connection_transport: Some("simulator".to_string()),
                    connection_phase: Some("connected".to_string()),
                    status_detail: Some("Connected to simulated jcode server.".to_string()),
                    is_processing: false,
                    pending_approvals: Vec::new(),
                    active_tool_id: None,
                    reconnect_attempt: 0,
                    pending_session_id: None,
                    pending_model_name: None,
                }
            }
            ScenarioName::PairingInvalidCode => Self {
                pairing: PairingForm {
                    host: "devbox.tailnet.ts.net".to_string(),
                    port: "7643".to_string(),
                    pair_code: "000000".to_string(),
                    device_name: "jcode simulator".to_string(),
                },
                status_message: None,
                error_message: Some("Invalid or expired pairing code.".to_string()),
                ..Self::for_scenario(ScenarioName::Onboarding)
            },
            ScenarioName::ServerUnreachable => Self {
                pairing: PairingForm {
                    host: "offline.tailnet.ts.net".to_string(),
                    port: "7643".to_string(),
                    pair_code: "123456".to_string(),
                    device_name: "jcode simulator".to_string(),
                },
                status_message: None,
                error_message: Some(
                    "Server unreachable. Confirm host/port and gateway status.".to_string(),
                ),
                ..Self::for_scenario(ScenarioName::Onboarding)
            },
            ScenarioName::ConnectedEmptyChat => {
                let mut state = Self::for_scenario(ScenarioName::ConnectedChat);
                state.messages.clear();
                state.status_message = Some("Connected to simulated empty chat.".to_string());
                state
            }
            ScenarioName::ChatStreaming => {
                let mut state = Self::for_scenario(ScenarioName::ConnectedChat);
                state.messages.push(ChatMessage {
                    id: "msg-user-streaming".to_string(),
                    role: MessageRole::User,
                    text: "Run the mobile simulator smoke test.".to_string(),
                    tool_calls: Vec::new(),
                });
                state.messages.push(ChatMessage {
                    id: "msg-assistant-streaming".to_string(),
                    role: MessageRole::Assistant,
                    text: "Running the Linux-native simulator".to_string(),
                    tool_calls: Vec::new(),
                });
                state.status_message = Some("Assistant response is streaming.".to_string());
                state.is_processing = true;
                state
            }
            ScenarioName::ToolApprovalRequired => {
                let mut state = Self::for_scenario(ScenarioName::ConnectedChat);
                state.pending_approvals.push(ApprovalRequest {
                    id: "approval-1".to_string(),
                    command_summary: "bash: cargo test -p jcode-mobile-core".to_string(),
                    workspace: Some("/workspace/jcode".to_string()),
                    risk: ApprovalRisk::Medium,
                    timeout_seconds: Some(300),
                    reason: Some("Run the mobile core regression suite.".to_string()),
                });
                state.messages.push(ChatMessage {
                    id: "msg-tool-approval".to_string(),
                    role: MessageRole::System,
                    text: "Tool approval required: bash: cargo test -p jcode-mobile-core."
                        .to_string(),
                    tool_calls: Vec::new(),
                });
                state.status_message = Some("Waiting for simulated tool approval.".to_string());
                state.is_processing = true;
                state
            }
            ScenarioName::ToolFailed => {
                let mut state = Self::for_scenario(ScenarioName::ConnectedChat);
                state.messages.push(ChatMessage {
                    id: "msg-tool-failed".to_string(),
                    role: MessageRole::System,
                    text: "Simulated tool failed: exit status 1.".to_string(),
                    tool_calls: Vec::new(),
                });
                state.error_message = Some("Last simulated tool failed.".to_string());
                state
            }
            ScenarioName::NetworkReconnect => {
                let mut state = Self::for_scenario(ScenarioName::ConnectedChat);
                state.connection_state = ConnectionState::Connecting;
                state.status_message =
                    Some("Reconnecting to simulated jcode server...".to_string());
                state
            }
            ScenarioName::OfflineQueuedMessage => {
                let mut state = Self::for_scenario(ScenarioName::ConnectedChat);
                state.connection_state = ConnectionState::Disconnected;
                state.draft_message = "Queued while offline".to_string();
                state.status_message =
                    Some("Message queued until simulated reconnect.".to_string());
                state
            }
            ScenarioName::LongRunningTask => {
                let mut state = Self::for_scenario(ScenarioName::ConnectedChat);
                state.messages.push(ChatMessage {
                    id: "msg-long-running".to_string(),
                    role: MessageRole::Assistant,
                    text: "Long-running simulated task is still in progress.".to_string(),
                    tool_calls: Vec::new(),
                });
                state.status_message = Some("Long-running simulated task in progress.".to_string());
                state.is_processing = true;
                state
            }
        }
    }
}

fn simulated_session_summary(
    session_id: &str,
    display_name: &str,
    title: Option<&str>,
    is_active: bool,
) -> protocol::MobileSessionSummary {
    protocol::MobileSessionSummary {
        session_id: session_id.to_string(),
        display_name: display_name.to_string(),
        title: title.map(ToOwned::to_owned),
        working_dir: Some("/repo".to_string()),
        status: "active".to_string(),
        status_detail: None,
        updated_at: "2026-06-01T12:00:00Z".to_string(),
        last_active_at: Some("2026-06-01T12:00:00Z".to_string()),
        provider_key: Some("openai".to_string()),
        model: Some("gpt-5".to_string()),
        is_active,
        is_live: true,
        client_count: if is_active { 1 } else { 0 },
        activity: None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScenarioName {
    Onboarding,
    PairingReady,
    ConnectedChat,
    PairingInvalidCode,
    ServerUnreachable,
    ConnectedEmptyChat,
    ChatStreaming,
    ToolApprovalRequired,
    ToolFailed,
    NetworkReconnect,
    OfflineQueuedMessage,
    LongRunningTask,
}

impl ScenarioName {
    pub const ALL: &'static [Self] = &[
        Self::Onboarding,
        Self::PairingReady,
        Self::ConnectedChat,
        Self::PairingInvalidCode,
        Self::ServerUnreachable,
        Self::ConnectedEmptyChat,
        Self::ChatStreaming,
        Self::ToolApprovalRequired,
        Self::ToolFailed,
        Self::NetworkReconnect,
        Self::OfflineQueuedMessage,
        Self::LongRunningTask,
    ];

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "onboarding" => Some(Self::Onboarding),
            "pairing_ready" => Some(Self::PairingReady),
            "connected_chat" => Some(Self::ConnectedChat),
            "pairing_invalid_code" => Some(Self::PairingInvalidCode),
            "server_unreachable" => Some(Self::ServerUnreachable),
            "connected_empty_chat" => Some(Self::ConnectedEmptyChat),
            "chat_streaming" => Some(Self::ChatStreaming),
            "tool_approval_required" => Some(Self::ToolApprovalRequired),
            "tool_failed" => Some(Self::ToolFailed),
            "network_reconnect" => Some(Self::NetworkReconnect),
            "offline_queued_message" => Some(Self::OfflineQueuedMessage),
            "long_running_task" => Some(Self::LongRunningTask),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Onboarding => "onboarding",
            Self::PairingReady => "pairing_ready",
            Self::ConnectedChat => "connected_chat",
            Self::PairingInvalidCode => "pairing_invalid_code",
            Self::ServerUnreachable => "server_unreachable",
            Self::ConnectedEmptyChat => "connected_empty_chat",
            Self::ChatStreaming => "chat_streaming",
            Self::ToolApprovalRequired => "tool_approval_required",
            Self::ToolFailed => "tool_failed",
            Self::NetworkReconnect => "network_reconnect",
            Self::OfflineQueuedMessage => "offline_queued_message",
            Self::LongRunningTask => "long_running_task",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SimulatorAction {
    Reset,
    LoadScenario {
        scenario: ScenarioName,
    },
    SetHost {
        value: String,
    },
    SetPort {
        value: String,
    },
    SetPairCode {
        value: String,
    },
    SetDeviceName {
        value: String,
    },
    SetDraft {
        value: String,
    },
    TapNode {
        node_id: String,
    },
    PairingSucceeded {
        server_name: String,
        server_version: String,
    },
    PairingFailed {
        message: String,
    },
    Connected {
        session_id: String,
    },
    Disconnected {
        message: Option<String>,
        should_reconnect: bool,
    },
    ConnectionFailed {
        message: String,
    },
    SwitchSession {
        session_id: String,
    },
    SetModel {
        model: String,
    },
    ApplyServerEvent {
        event: protocol::MobileServerEvent,
    },
    ApprovalRequested {
        request: ApprovalRequest,
    },
    ApproveApproval {
        request_id: String,
    },
    DenyApproval {
        request_id: String,
        reason: Option<String>,
    },
    ApprovalExpired {
        request_id: String,
    },
    AppendAssistantText {
        text: String,
    },
    ReplaceAssistantText {
        text: String,
    },
    ToolStart {
        id: String,
        name: String,
    },
    ToolInput {
        delta: String,
    },
    ToolExec {
        id: String,
        name: String,
    },
    ToolDone {
        id: String,
        name: String,
        output: String,
        error: Option<String>,
    },
    FinishTurn,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SimulatorEffect {
    PairAndConnect {
        host: String,
        port: String,
        pair_code: String,
        device_name: String,
    },
    SendMessage {
        text: String,
    },
    Reconnect {
        host: String,
        port: String,
        session_id: Option<String>,
        attempt: u32,
    },
    ResumeSession {
        session_id: String,
    },
    SetModel {
        model: String,
    },
    SubmitApproval {
        request_id: String,
        approved: bool,
        reason: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRecord {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub action: SimulatorAction,
    pub before: SimulatorState,
    pub after: SimulatorState,
    pub effects: Vec<SimulatorEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectRecord {
    pub seq: u64,
    pub timestamp_ms: u64,
    pub effect: SimulatorEffect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DispatchReport {
    pub transitions: Vec<TransitionRecord>,
    pub effect_records: Vec<EffectRecord>,
    pub final_state: SimulatorState,
}

pub const REPLAY_TRACE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayTrace {
    pub schema_version: u32,
    pub name: String,
    pub initial_state: SimulatorState,
    pub actions: Vec<SimulatorAction>,
    pub transitions: Vec<TransitionRecord>,
    pub effects: Vec<EffectRecord>,
    pub final_state: SimulatorState,
}

impl ReplayTrace {
    pub fn record(
        name: impl Into<String>,
        initial_state: SimulatorState,
        actions: Vec<SimulatorAction>,
    ) -> Self {
        let mut store = SimulatorStore::new(initial_state.clone());
        for action in actions.iter().cloned() {
            store.dispatch(action);
        }
        Self {
            schema_version: REPLAY_TRACE_SCHEMA_VERSION,
            name: name.into(),
            initial_state,
            actions,
            transitions: store.transition_log().to_vec(),
            effects: store.effect_log().to_vec(),
            final_state: store.state().clone(),
        }
    }

    pub fn replay(&self) -> Self {
        Self::record(
            self.name.clone(),
            self.initial_state.clone(),
            self.actions.clone(),
        )
    }

    pub fn assert_replays(&self) -> anyhow::Result<()> {
        if self.schema_version != REPLAY_TRACE_SCHEMA_VERSION {
            anyhow::bail!(
                "unsupported replay trace schema version {}, expected {}",
                self.schema_version,
                REPLAY_TRACE_SCHEMA_VERSION
            );
        }
        let replayed = self.replay();
        if &replayed != self {
            anyhow::bail!(
                "replay trace mismatch for {}\nexpected:\n{}\nactual:\n{}",
                self.name,
                serde_json::to_string_pretty(self)?,
                serde_json::to_string_pretty(&replayed)?
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct SimulatorStore {
    initial_state: SimulatorState,
    state: SimulatorState,
    action_log: Vec<SimulatorAction>,
    transition_log: Vec<TransitionRecord>,
    effect_log: Vec<EffectRecord>,
    next_seq: u64,
    now_ms: u64,
}

impl Default for SimulatorStore {
    fn default() -> Self {
        Self::new(SimulatorState::default())
    }
}

impl SimulatorStore {
    pub fn new(initial_state: SimulatorState) -> Self {
        Self {
            initial_state: initial_state.clone(),
            state: initial_state,
            action_log: Vec::new(),
            transition_log: Vec::new(),
            effect_log: Vec::new(),
            next_seq: 1,
            now_ms: 0,
        }
    }

    pub fn state(&self) -> &SimulatorState {
        &self.state
    }

    pub fn transition_log(&self) -> &[TransitionRecord] {
        &self.transition_log
    }

    pub fn action_log(&self) -> &[SimulatorAction] {
        &self.action_log
    }

    pub fn effect_log(&self) -> &[EffectRecord] {
        &self.effect_log
    }

    pub fn semantic_tree(&self) -> UiTree {
        build_ui_tree(&self.state)
    }

    pub fn state_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(&self.state)?)
    }

    pub fn tree_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(&self.semantic_tree())?)
    }

    pub fn visual_scene(&self) -> VisualScene {
        visual_scene(&self.semantic_tree())
    }

    pub fn visual_scene_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(&self.visual_scene())?)
    }

    pub fn transition_log_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(&self.transition_log)?)
    }

    pub fn replay_trace(&self, name: impl Into<String>) -> ReplayTrace {
        ReplayTrace {
            schema_version: REPLAY_TRACE_SCHEMA_VERSION,
            name: name.into(),
            initial_state: self.initial_state.clone(),
            actions: self.action_log.clone(),
            transitions: self.transition_log.clone(),
            effects: self.effect_log.clone(),
            final_state: self.state.clone(),
        }
    }

    pub fn dispatch(&mut self, action: SimulatorAction) -> DispatchReport {
        self.action_log.push(action.clone());
        let mut pending = vec![action];
        let mut transitions = Vec::new();
        let mut effect_records = Vec::new();

        while let Some(action) = pending.pop() {
            let before = self.state.clone();
            let reduction = reduce(before.clone(), action.clone());
            self.state = reduction.after.clone();

            let seq = self.next_seq;
            self.next_seq += 1;
            self.now_ms += 1;

            let transition = TransitionRecord {
                seq,
                timestamp_ms: self.now_ms,
                action,
                before,
                after: reduction.after,
                effects: reduction.effects.clone(),
            };
            self.transition_log.push(transition.clone());
            transitions.push(transition);

            for effect in reduction.effects {
                self.now_ms += 1;
                let effect_record = EffectRecord {
                    seq,
                    timestamp_ms: self.now_ms,
                    effect: effect.clone(),
                };
                self.effect_log.push(effect_record.clone());
                effect_records.push(effect_record);
                let follow_ups = FakeJcodeBackend::default().handle_effect(effect);
                for next in follow_ups.into_iter().rev() {
                    pending.push(next);
                }
            }
        }

        DispatchReport {
            transitions,
            effect_records,
            final_state: self.state.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct Reduction {
    after: SimulatorState,
    effects: Vec<SimulatorEffect>,
}

fn reduce(mut state: SimulatorState, action: SimulatorAction) -> Reduction {
    let mut effects = Vec::new();
    match action {
        SimulatorAction::Reset => {
            state = SimulatorState::default();
        }
        SimulatorAction::LoadScenario { scenario } => {
            state = SimulatorState::for_scenario(scenario);
        }
        SimulatorAction::SetHost { value } => {
            state.pairing.host = value;
            state.error_message = None;
        }
        SimulatorAction::SetPort { value } => {
            state.pairing.port = value;
            state.error_message = None;
        }
        SimulatorAction::SetPairCode { value } => {
            state.pairing.pair_code = value;
            state.error_message = None;
        }
        SimulatorAction::SetDeviceName { value } => {
            state.pairing.device_name = value;
            state.error_message = None;
        }
        SimulatorAction::SetDraft { value } => {
            state.draft_message = value;
            state.error_message = None;
        }
        SimulatorAction::TapNode { node_id } => match node_id.as_str() {
            "pair.submit" => match validate_pairing_form(&state.pairing) {
                Ok(validated) => {
                    state.pairing.host = validated.host.clone();
                    state.pairing.port = validated.port.clone();
                    state.pairing.pair_code = validated.pair_code.clone();
                    state.pairing.device_name = validated.device_name.clone();

                    state.screen = Screen::Pairing;
                    state.connection_state = ConnectionState::Connecting;
                    state.status_message = Some(format!(
                        "Pairing to {}:{}...",
                        validated.host, validated.port
                    ));
                    state.error_message = None;
                    effects.push(SimulatorEffect::PairAndConnect {
                        host: validated.host,
                        port: validated.port,
                        pair_code: validated.pair_code,
                        device_name: validated.device_name,
                    });
                }
                Err(message) => {
                    state.error_message = Some(message);
                }
            },
            "chat.send" => {
                if state.connection_state != ConnectionState::Connected {
                    state.error_message = Some("Not connected.".to_string());
                } else if state.draft_message.trim().is_empty() {
                    state.error_message = Some("Draft is empty.".to_string());
                } else {
                    let text = state.draft_message.trim().to_string();
                    let next_user_id = state.messages.len() + 1;
                    state.messages.push(ChatMessage {
                        id: format!("msg-user-{next_user_id}"),
                        role: MessageRole::User,
                        text: text.clone(),
                        tool_calls: Vec::new(),
                    });
                    state.messages.push(ChatMessage {
                        id: format!("msg-assistant-{}", next_user_id + 1),
                        role: MessageRole::Assistant,
                        text: String::new(),
                        tool_calls: Vec::new(),
                    });
                    state.draft_message.clear();
                    state.status_message = Some("Sending simulated message...".to_string());
                    state.error_message = None;
                    state.is_processing = true;
                    effects.push(SimulatorEffect::SendMessage { text });
                }
            }
            "chat.interrupt" => {
                state.is_processing = false;
                state.status_message = Some("Interrupted simulated turn.".to_string());
            }
            _ if node_id.starts_with("approval.") => {
                if let Some((request_id, decision)) = parse_approval_node_id(&node_id) {
                    match decision {
                        ApprovalDecision::Approve => {
                            return reduce(state, SimulatorAction::ApproveApproval { request_id });
                        }
                        ApprovalDecision::Deny => {
                            return reduce(
                                state,
                                SimulatorAction::DenyApproval {
                                    request_id,
                                    reason: Some("Denied from mobile.".to_string()),
                                },
                            );
                        }
                    }
                } else {
                    state.error_message = Some(format!("Unknown approval node id: {node_id}"));
                }
            }
            node_id if node_id.starts_with("chat.session.") => {
                let session_id = node_id.trim_start_matches("chat.session.").to_string();
                if state
                    .sessions
                    .iter()
                    .any(|candidate| candidate == &session_id)
                {
                    state.active_session_id = Some(session_id.clone());
                    for summary in &mut state.session_summaries {
                        summary.is_active = summary.session_id == session_id;
                    }
                    state.status_message =
                        Some(format!("Switched to simulated session {session_id}."));
                } else {
                    state.error_message = Some(format!("Unknown simulated session: {session_id}"));
                }
            }
            _ => {
                state.error_message = Some(format!("Unknown node id: {node_id}"));
            }
        },
        SimulatorAction::PairingSucceeded {
            server_name,
            server_version,
        } => {
            let server = ServerSummary {
                host: state.pairing.host.clone(),
                port: state.pairing.port.clone(),
                server_name,
                server_version,
            };
            state
                .saved_servers
                .retain(|existing| existing.host != server.host || existing.port != server.port);
            state.saved_servers.push(server.clone());
            state.selected_server = Some(server);
            state.status_message = Some("Simulated pairing succeeded.".to_string());
            state.error_message = None;
        }
        SimulatorAction::PairingFailed { message }
        | SimulatorAction::ConnectionFailed { message } => {
            state.screen = Screen::Onboarding;
            state.connection_state = ConnectionState::Disconnected;
            state.status_message = None;
            state.error_message = Some(message);
            state.is_processing = false;
            state.active_tool_id = None;
        }
        SimulatorAction::Connected { session_id } => {
            state.screen = Screen::Chat;
            state.connection_state = ConnectionState::Connected;
            ensure_session(&mut state, session_id.clone());
            if state.session_summaries.is_empty() {
                state.session_summaries = vec![simulated_session_summary(
                    &session_id,
                    &session_id,
                    Some("Connected chat"),
                    true,
                )];
            } else {
                for summary in &mut state.session_summaries {
                    summary.is_active = summary.session_id == session_id;
                }
            }
            if state.available_models.is_empty() {
                state.available_models = vec!["gpt-5".to_string(), "claude-sonnet-4".to_string()];
            }
            if state.model_name.is_none() {
                state.model_name = Some("gpt-5".to_string());
            }
            state.reconnect_attempt = 0;
            state.pending_session_id = None;
            state.pending_model_name = None;
            state.status_message = Some("Connected to simulated jcode server.".to_string());
            state.error_message = None;
            if state.messages.is_empty() {
                state.messages.push(ChatMessage {
                    id: "msg-system-connected".to_string(),
                    role: MessageRole::System,
                    text: "Simulator connected. Send a message to begin.".to_string(),
                    tool_calls: Vec::new(),
                });
            }
        }
        SimulatorAction::Disconnected {
            message,
            should_reconnect,
        } => {
            state.connection_state = ConnectionState::Disconnected;
            state.connection_phase = Some("disconnected".to_string());
            state.is_processing = false;
            state.active_tool_id = None;
            state.pending_model_name = None;
            if let Some(message) = message {
                state.error_message = Some(message);
            }

            if should_reconnect {
                if let Some(server) = state.selected_server.clone() {
                    let attempt = state.reconnect_attempt;
                    state.reconnect_attempt = state.reconnect_attempt.saturating_add(1);
                    state.connection_state = ConnectionState::Connecting;
                    state.connection_phase = Some("reconnecting".to_string());
                    state.status_message = Some(format!(
                        "Reconnecting to {}:{} (attempt {})...",
                        server.host,
                        server.port,
                        attempt + 1
                    ));
                    effects.push(SimulatorEffect::Reconnect {
                        host: server.host,
                        port: server.port,
                        session_id: state.active_session_id.clone(),
                        attempt,
                    });
                } else {
                    state.status_message = None;
                    state.error_message = Some("Select a paired server first.".to_string());
                }
            } else {
                state.status_message = Some("Disconnected.".to_string());
            }
        }
        SimulatorAction::SwitchSession { session_id } => {
            let session_id = session_id.trim().to_string();
            if session_id.is_empty() {
                state.error_message = Some("Session id cannot be empty.".to_string());
            } else if state.connection_state != ConnectionState::Connected {
                state.error_message = Some("Not connected.".to_string());
            } else {
                ensure_session(&mut state, session_id.clone());
                state.pending_session_id = Some(session_id.clone());
                state.messages.clear();
                clear_active_turn_tracking(&mut state);
                state.status_message = Some(format!("Switching to {session_id}..."));
                state.error_message = None;
                effects.push(SimulatorEffect::ResumeSession { session_id });
            }
        }
        SimulatorAction::SetModel { model } => {
            let model = model.trim().to_string();
            if model.is_empty() {
                state.error_message = Some("Model cannot be empty.".to_string());
            } else if state.connection_state != ConnectionState::Connected {
                state.error_message = Some("Not connected.".to_string());
            } else {
                state.pending_model_name = Some(model.clone());
                state.status_message = Some(format!("Switching model to {model}..."));
                state.error_message = None;
                effects.push(SimulatorEffect::SetModel { model });
            }
        }
        SimulatorAction::ApplyServerEvent { event } => {
            apply_server_event(&mut state, event);
        }
        SimulatorAction::ApprovalRequested { request } => {
            state
                .pending_approvals
                .retain(|existing| existing.id != request.id);
            let summary = request.command_summary.clone();
            state.pending_approvals.push(request);
            state.status_message = Some("Approval required.".to_string());
            state.messages.push(ChatMessage {
                id: format!("msg-approval-{}", state.messages.len() + 1),
                role: MessageRole::System,
                text: format!("Approval required: {summary}"),
                tool_calls: Vec::new(),
            });
            state.is_processing = true;
        }
        SimulatorAction::ApproveApproval { request_id } => {
            if remove_pending_approval(&mut state, &request_id).is_some() {
                state.status_message = Some("Approval sent.".to_string());
                effects.push(SimulatorEffect::SubmitApproval {
                    request_id,
                    approved: true,
                    reason: None,
                });
            } else {
                state.error_message = Some("Approval request is no longer pending.".to_string());
            }
        }
        SimulatorAction::DenyApproval { request_id, reason } => {
            if remove_pending_approval(&mut state, &request_id).is_some() {
                state.status_message = Some("Denial sent.".to_string());
                effects.push(SimulatorEffect::SubmitApproval {
                    request_id,
                    approved: false,
                    reason,
                });
            } else {
                state.error_message = Some("Approval request is no longer pending.".to_string());
            }
        }
        SimulatorAction::ApprovalExpired { request_id } => {
            if remove_pending_approval(&mut state, &request_id).is_some() {
                state.status_message = Some("Approval expired.".to_string());
            }
        }
        SimulatorAction::AppendAssistantText { text } => {
            append_to_latest_assistant(&mut state, &text);
            state.is_processing = true;
        }
        SimulatorAction::ReplaceAssistantText { text } => {
            replace_latest_assistant(&mut state, text);
        }
        SimulatorAction::ToolStart { id, name } => {
            attach_tool_to_latest_assistant(&mut state, ToolCall::new(id.clone(), name));
            state.active_tool_id = Some(id);
            state.is_processing = true;
        }
        SimulatorAction::ToolInput { delta } => {
            if let Some(tool_id) = state.active_tool_id.clone() {
                update_tool(&mut state, &tool_id, |tool| {
                    tool.input.push_str(&delta);
                    tool.state = ToolCallState::Streaming;
                });
                state.is_processing = true;
            }
        }
        SimulatorAction::ToolExec { id, name: _ } => {
            update_tool(&mut state, &id, |tool| {
                tool.state = ToolCallState::Executing;
            });
            state.active_tool_id = Some(id);
            state.is_processing = true;
        }
        SimulatorAction::ToolDone {
            id,
            name: _,
            output,
            error,
        } => {
            update_tool(&mut state, &id, |tool| {
                tool.output = Some(output);
                tool.error = error;
                tool.state = if tool.error.is_some() {
                    ToolCallState::Failed
                } else {
                    ToolCallState::Done
                };
            });
            state.active_tool_id = Some(id);
        }
        SimulatorAction::FinishTurn => {
            state.is_processing = false;
            clear_active_turn_tracking(&mut state);
            state.status_message = Some("Simulated turn finished.".to_string());
        }
    }

    Reduction {
        after: state,
        effects,
    }
}

fn remove_pending_approval(
    state: &mut SimulatorState,
    request_id: &str,
) -> Option<ApprovalRequest> {
    let index = state
        .pending_approvals
        .iter()
        .position(|request| request.id == request_id)?;
    Some(state.pending_approvals.remove(index))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApprovalDecision {
    Approve,
    Deny,
}

fn parse_approval_node_id(node_id: &str) -> Option<(String, ApprovalDecision)> {
    let rest = node_id.strip_prefix("approval.")?;
    let (request_id, decision) = rest.rsplit_once('.')?;
    let decision = match decision {
        "approve" => ApprovalDecision::Approve,
        "deny" => ApprovalDecision::Deny,
        _ => return None,
    };
    if request_id.is_empty() {
        return None;
    }
    Some((request_id.to_string(), decision))
}

fn append_to_latest_assistant(state: &mut SimulatorState, text: &str) {
    if let Some(message) = state
        .messages
        .iter_mut()
        .rev()
        .find(|message| message.role == MessageRole::Assistant)
    {
        message.text.push_str(text);
        return;
    }

    state.messages.push(ChatMessage {
        id: format!("msg-assistant-{}", state.messages.len() + 1),
        role: MessageRole::Assistant,
        text: text.to_string(),
        tool_calls: Vec::new(),
    });
}

fn replace_latest_assistant(state: &mut SimulatorState, text: String) {
    if let Some(message) = state
        .messages
        .iter_mut()
        .rev()
        .find(|message| message.role == MessageRole::Assistant)
    {
        message.text = text;
        return;
    }

    state.messages.push(ChatMessage {
        id: format!("msg-assistant-{}", state.messages.len() + 1),
        role: MessageRole::Assistant,
        text,
        tool_calls: Vec::new(),
    });
}

fn attach_tool_to_latest_assistant(state: &mut SimulatorState, tool: ToolCall) {
    if let Some(message) = state
        .messages
        .iter_mut()
        .rev()
        .find(|message| message.role == MessageRole::Assistant)
    {
        message.tool_calls.push(tool);
        return;
    }

    state.messages.push(ChatMessage {
        id: format!("msg-assistant-{}", state.messages.len() + 1),
        role: MessageRole::Assistant,
        text: String::new(),
        tool_calls: vec![tool],
    });
}

fn update_tool(state: &mut SimulatorState, tool_id: &str, mutate: impl FnOnce(&mut ToolCall)) {
    for message in state.messages.iter_mut().rev() {
        if let Some(tool) = message
            .tool_calls
            .iter_mut()
            .rev()
            .find(|tool| tool.id == tool_id)
        {
            mutate(tool);
            return;
        }
    }
}

fn clear_active_turn_tracking(state: &mut SimulatorState) {
    state.active_tool_id = None;
}

fn ensure_session(state: &mut SimulatorState, session_id: String) {
    state.active_session_id = Some(session_id.clone());
    if !state.sessions.iter().any(|session| session == &session_id) {
        state.sessions.push(session_id);
    }
}

fn apply_server_event(state: &mut SimulatorState, event: protocol::MobileServerEvent) {
    match event {
        protocol::MobileServerEvent::Ack { .. } | protocol::MobileServerEvent::Pong { .. } => {}
        protocol::MobileServerEvent::TextDelta { text } => {
            append_to_latest_assistant(state, &text);
            state.is_processing = true;
        }
        protocol::MobileServerEvent::TextReplace { text } => {
            replace_latest_assistant(state, text);
        }
        protocol::MobileServerEvent::ToolStart { id, name } => {
            attach_tool_to_latest_assistant(state, ToolCall::new(id.clone(), name));
            state.active_tool_id = Some(id);
            state.is_processing = true;
        }
        protocol::MobileServerEvent::ToolInput { delta } => {
            if let Some(tool_id) = state.active_tool_id.clone() {
                update_tool(state, &tool_id, |tool| {
                    tool.input.push_str(&delta);
                    tool.state = ToolCallState::Streaming;
                });
            }
        }
        protocol::MobileServerEvent::ToolExec { id, .. } => {
            update_tool(state, &id, |tool| {
                tool.state = ToolCallState::Executing;
            });
            state.active_tool_id = Some(id);
            state.is_processing = true;
        }
        protocol::MobileServerEvent::ToolDone {
            id, output, error, ..
        } => {
            update_tool(state, &id, |tool| {
                tool.output = Some(output);
                tool.error = error;
                tool.state = if tool.error.is_some() {
                    ToolCallState::Failed
                } else {
                    ToolCallState::Done
                };
            });
            state.active_tool_id = Some(id);
        }
        protocol::MobileServerEvent::Done { .. } => {
            state.is_processing = false;
            clear_active_turn_tracking(state);
            state.status_message = Some("Turn finished.".to_string());
        }
        protocol::MobileServerEvent::Error { message, .. } => {
            state.error_message = Some(message);
            state.is_processing = false;
            clear_active_turn_tracking(state);
        }
        protocol::MobileServerEvent::State {
            session_id,
            is_processing,
            ..
        } => {
            state.screen = Screen::Chat;
            state.connection_state = ConnectionState::Connected;
            ensure_session(state, session_id);
            state.is_processing = is_processing;
            state.reconnect_attempt = 0;
            state.pending_session_id = None;
        }
        protocol::MobileServerEvent::SessionId { session_id } => {
            state.screen = Screen::Chat;
            state.connection_state = ConnectionState::Connected;
            ensure_session(state, session_id);
            state.reconnect_attempt = 0;
            state.pending_session_id = None;
        }
        protocol::MobileServerEvent::History(payload) => {
            apply_history_payload(state, payload);
        }
        protocol::MobileServerEvent::ModelChanged {
            model,
            provider_name,
            error,
            ..
        } => {
            state.pending_model_name = None;
            if let Some(error) = error {
                state.error_message = Some(error);
            } else {
                if let Some(provider_name) = provider_name {
                    state.provider_name = Some(provider_name);
                }
                if !state
                    .available_models
                    .iter()
                    .any(|existing| existing == &model)
                {
                    state.available_models.push(model.clone());
                }
                state.model_name = Some(model.clone());
                state.status_message = Some(format!("Model: {model}"));
                state.error_message = None;
            }
        }
        protocol::MobileServerEvent::Reloading { .. } => {
            state.connection_state = ConnectionState::Connecting;
            state.connection_phase = Some("server_reloading".to_string());
            state.status_message = Some("Server reloading. Reconnecting...".to_string());
            state.is_processing = false;
            clear_active_turn_tracking(state);
        }
        protocol::MobileServerEvent::ConnectionType { connection } => {
            state.connection_transport = Some(connection);
        }
        protocol::MobileServerEvent::ConnectionPhase { phase } => {
            state.connection_phase = Some(phase.clone());
            state.status_message = Some(format!("Connection: {phase}"));
        }
        protocol::MobileServerEvent::StatusDetail { detail } => {
            state.status_detail = Some(detail.clone());
            state.status_message = Some(detail);
        }
        protocol::MobileServerEvent::AvailableModelsUpdated {
            provider_name,
            provider_model,
            available_models,
        } => {
            if let Some(provider_name) = provider_name {
                state.provider_name = Some(provider_name);
            }
            if let Some(provider_model) = provider_model {
                state.model_name = Some(provider_model);
            }
            state.available_models = available_models;
            state.pending_model_name = None;
        }
        protocol::MobileServerEvent::ReloadProgress {
            message, success, ..
        } => {
            if success == Some(false) {
                state.error_message = Some(message);
            } else {
                state.status_message = Some(message);
            }
        }
        protocol::MobileServerEvent::Interrupted => {
            state.is_processing = false;
            clear_active_turn_tracking(state);
            remove_empty_latest_assistant(state);
            state.messages.push(ChatMessage {
                id: format!("msg-system-{}", state.messages.len() + 1),
                role: MessageRole::System,
                text: "Interrupted by server.".to_string(),
                tool_calls: Vec::new(),
            });
        }
        protocol::MobileServerEvent::SoftInterruptInjected { tools_skipped, .. } => {
            state.status_message = if let Some(skipped) = tools_skipped.filter(|count| *count > 0) {
                Some(format!(
                    "Updated current run. Skipped {skipped} remaining tool(s)."
                ))
            } else {
                Some("Updated current run.".to_string())
            };
        }
        protocol::MobileServerEvent::SplitResponse {
            new_session_id,
            new_session_name,
            ..
        } => {
            ensure_session(state, new_session_id);
            state.status_message = Some(format!("Created split session: {new_session_name}"));
        }
        protocol::MobileServerEvent::CompactResult {
            message, success, ..
        } => {
            if success {
                state.status_message = Some(message);
            } else {
                state.error_message = Some(message);
            }
        }
        protocol::MobileServerEvent::Notification(notification) => {
            let sender = notification
                .from_name
                .filter(|name| !name.trim().is_empty())
                .unwrap_or(notification.from_session);
            state.status_message = Some(format!("{sender}: {}", notification.message));
        }
        protocol::MobileServerEvent::StdinRequest { prompt, .. } => {
            state.messages.push(ChatMessage {
                id: format!("msg-system-{}", state.messages.len() + 1),
                role: MessageRole::System,
                text: prompt,
                tool_calls: Vec::new(),
            });
            state.is_processing = true;
        }
        protocol::MobileServerEvent::ApprovalRequests { requests, .. } => {
            state.pending_approvals = requests
                .into_iter()
                .map(|request| ApprovalRequest {
                    id: request.id,
                    command_summary: request.command_summary,
                    workspace: request.workspace,
                    risk: match request.risk.as_str() {
                        "high" => ApprovalRisk::High,
                        "low" => ApprovalRisk::Low,
                        _ => ApprovalRisk::Medium,
                    },
                    timeout_seconds: request
                        .timeout_seconds
                        .and_then(|value| u32::try_from(value).ok()),
                    reason: None,
                })
                .collect();
        }
        protocol::MobileServerEvent::TokenUsage { .. }
        | protocol::MobileServerEvent::UpstreamProvider { .. }
        | protocol::MobileServerEvent::SessionRenamed { .. }
        | protocol::MobileServerEvent::SwarmStatus { .. }
        | protocol::MobileServerEvent::McpStatus { .. }
        | protocol::MobileServerEvent::MemoryInjected { .. } => {}
    }
}

fn apply_history_payload(state: &mut SimulatorState, payload: protocol::HistoryPayload) {
    state.screen = Screen::Chat;
    state.connection_state = ConnectionState::Connected;
    state.active_session_id = Some(payload.session_id.clone());
    state.sessions = if payload.all_sessions.is_empty() {
        vec![payload.session_id.clone()]
    } else {
        payload.all_sessions
    };
    if !state
        .sessions
        .iter()
        .any(|session| session == &payload.session_id)
    {
        state.sessions.insert(0, payload.session_id.clone());
    }
    state.session_summaries = if payload.session_summaries.is_empty() {
        state
            .sessions
            .iter()
            .map(|session_id| {
                simulated_session_summary(
                    session_id,
                    session_id,
                    None,
                    session_id == &payload.session_id,
                )
            })
            .collect()
    } else {
        payload.session_summaries
    };
    state.available_models = payload.available_models;
    state.provider_name = payload.provider_name;
    state.model_name = payload.provider_model;
    state.connection_transport = payload.connection_type;
    state.reconnect_attempt = 0;
    state.pending_session_id = None;
    state.pending_model_name = None;

    if let Some(selected) = state.selected_server.as_mut() {
        if let Some(server_name) = payload.server_name.clone() {
            selected.server_name = server_name;
        }
        if let Some(server_version) = payload.server_version.clone() {
            selected.server_version = server_version;
        }
    }

    state.messages = payload
        .messages
        .into_iter()
        .enumerate()
        .map(|(idx, item)| history_message_to_chat(idx, item))
        .collect();
    clear_active_turn_tracking(state);
}

fn history_message_to_chat(idx: usize, item: protocol::HistoryMessage) -> ChatMessage {
    let tool_calls = item
        .tool_data
        .and_then(|tool| {
            let id = tool.id?;
            let name = tool.name?;
            Some(ToolCall {
                id,
                name,
                input: tool.input.unwrap_or_default(),
                output: tool.output,
                error: None,
                state: ToolCallState::Done,
            })
        })
        .into_iter()
        .collect();

    ChatMessage {
        id: format!("msg-history-{}", idx + 1),
        role: match item.role.as_str() {
            "assistant" => MessageRole::Assistant,
            "system" => MessageRole::System,
            _ => MessageRole::User,
        },
        text: item.content,
        tool_calls,
    }
}

fn remove_empty_latest_assistant(state: &mut SimulatorState) {
    if let Some(index) = state
        .messages
        .iter()
        .rposition(|message| message.role == MessageRole::Assistant)
    {
        let message = &state.messages[index];
        if message.text.trim().is_empty() && message.tool_calls.is_empty() {
            state.messages.remove(index);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ValidatedPairingForm {
    host: String,
    port: String,
    pair_code: String,
    device_name: String,
}

fn validate_pairing_form(form: &PairingForm) -> Result<ValidatedPairingForm, String> {
    let host = form.host.trim().to_string();
    if host.is_empty() {
        return Err("Host cannot be empty.".to_string());
    }

    let port = form.port.trim().to_string();
    if port.parse::<u16>().is_err() {
        return Err("Port must be a number from 0 to 65535.".to_string());
    }

    let pair_code = form.pair_code.trim().to_string();
    if pair_code.is_empty() {
        return Err("Enter the 6-digit pairing code from jcode pair.".to_string());
    }

    let device_name = form.device_name.trim().to_string();
    if device_name.is_empty() {
        return Err("Device name cannot be empty.".to_string());
    }

    Ok(ValidatedPairingForm {
        host,
        port,
        pair_code,
        device_name,
    })
}

#[derive(Debug, Clone, Default)]
pub struct FakeJcodeBackend;

impl FakeJcodeBackend {
    pub fn handle_effect(&self, effect: SimulatorEffect) -> Vec<SimulatorAction> {
        match effect {
            SimulatorEffect::PairAndConnect {
                host, pair_code, ..
            } => self.pair_and_connect(&host, &pair_code),
            SimulatorEffect::SendMessage { text } => self.send_message(&text),
            SimulatorEffect::Reconnect { session_id, .. } => vec![SimulatorAction::Connected {
                session_id: session_id.unwrap_or_else(|| "session_sim_1".to_string()),
            }],
            SimulatorEffect::ResumeSession { session_id } => {
                vec![SimulatorAction::ApplyServerEvent {
                    event: protocol::MobileServerEvent::History(protocol::HistoryPayload {
                        session_id,
                        messages: Vec::new(),
                        server_name: Some("jcode".to_string()),
                        server_icon: None,
                        server_version: Some(env!("CARGO_PKG_VERSION").to_string()),
                        provider_name: Some("openai".to_string()),
                        provider_model: Some("gpt-5".to_string()),
                        connection_type: Some("simulator".to_string()),
                        available_models: vec!["gpt-5".to_string(), "claude-sonnet-4".to_string()],
                        all_sessions: vec![
                            "session_sim_1".to_string(),
                            "session_sim_2".to_string(),
                        ],
                        session_summaries: Vec::new(),
                        is_canary: None,
                        was_interrupted: None,
                        total_tokens: None,
                    }),
                }]
            }
            SimulatorEffect::SetModel { model } => vec![SimulatorAction::ApplyServerEvent {
                event: protocol::MobileServerEvent::ModelChanged {
                    id: 0,
                    model,
                    provider_name: Some("simulator".to_string()),
                    error: None,
                },
            }],
            SimulatorEffect::SubmitApproval { .. } => Vec::new(),
        }
    }

    fn pair_and_connect(&self, host: &str, pair_code: &str) -> Vec<SimulatorAction> {
        if host.contains("offline") || host.contains("unreachable") {
            return vec![SimulatorAction::ConnectionFailed {
                message: "Server unreachable. Confirm host/port and gateway status.".to_string(),
            }];
        }

        if pair_code != "123456" {
            return vec![SimulatorAction::PairingFailed {
                message: "Invalid or expired pairing code.".to_string(),
            }];
        }

        vec![
            SimulatorAction::PairingSucceeded {
                server_name: "jcode".to_string(),
                server_version: env!("CARGO_PKG_VERSION").to_string(),
            },
            SimulatorAction::Connected {
                session_id: "session_sim_1".to_string(),
            },
        ]
    }

    fn send_message(&self, text: &str) -> Vec<SimulatorAction> {
        vec![
            SimulatorAction::AppendAssistantText {
                text: format!("Simulated response to: {text}"),
            },
            SimulatorAction::FinishTurn,
        ]
    }
}

fn build_ui_tree(state: &SimulatorState) -> UiTree {
    let mut children = Vec::new();

    if let Some(status) = &state.status_message {
        children.push(UiNode {
            id: "banner.status".to_string(),
            role: UiNodeRole::Banner,
            label: "Status".to_string(),
            value: Some(status.clone()),
            visible: true,
            enabled: true,
            focused: false,
            accessibility_label: None,
            accessibility_value: None,
            supported_actions: Vec::new(),
            bounds: None,
            children: Vec::new(),
        });
    }

    if let Some(error) = &state.error_message {
        children.push(UiNode {
            id: "banner.error".to_string(),
            role: UiNodeRole::Banner,
            label: "Error".to_string(),
            value: Some(error.clone()),
            visible: true,
            enabled: true,
            focused: false,
            accessibility_label: None,
            accessibility_value: None,
            supported_actions: Vec::new(),
            bounds: None,
            children: Vec::new(),
        });
    }

    match state.screen {
        Screen::Onboarding | Screen::Pairing => {
            children.extend([
                UiNode {
                    id: "pair.host".to_string(),
                    role: UiNodeRole::TextInput,
                    label: "Host".to_string(),
                    value: Some(state.pairing.host.clone()),
                    visible: true,
                    enabled: state.connection_state != ConnectionState::Connecting,
                    focused: false,
                    accessibility_label: None,
                    accessibility_value: None,
                    supported_actions: Vec::new(),
                    bounds: None,
                    children: Vec::new(),
                },
                UiNode {
                    id: "pair.port".to_string(),
                    role: UiNodeRole::TextInput,
                    label: "Port".to_string(),
                    value: Some(state.pairing.port.clone()),
                    visible: true,
                    enabled: state.connection_state != ConnectionState::Connecting,
                    focused: false,
                    accessibility_label: None,
                    accessibility_value: None,
                    supported_actions: Vec::new(),
                    bounds: None,
                    children: Vec::new(),
                },
                UiNode {
                    id: "pair.code".to_string(),
                    role: UiNodeRole::TextInput,
                    label: "Pair Code".to_string(),
                    value: Some(state.pairing.pair_code.clone()),
                    visible: true,
                    enabled: state.connection_state != ConnectionState::Connecting,
                    focused: false,
                    accessibility_label: None,
                    accessibility_value: None,
                    supported_actions: Vec::new(),
                    bounds: None,
                    children: Vec::new(),
                },
                UiNode {
                    id: "pair.device_name".to_string(),
                    role: UiNodeRole::TextInput,
                    label: "Device Name".to_string(),
                    value: Some(state.pairing.device_name.clone()),
                    visible: true,
                    enabled: state.connection_state != ConnectionState::Connecting,
                    focused: false,
                    accessibility_label: None,
                    accessibility_value: None,
                    supported_actions: Vec::new(),
                    bounds: None,
                    children: Vec::new(),
                },
                UiNode {
                    id: "pair.submit".to_string(),
                    role: UiNodeRole::Button,
                    label: "Pair & Connect".to_string(),
                    value: None,
                    visible: true,
                    enabled: state.connection_state != ConnectionState::Connecting,
                    focused: false,
                    accessibility_label: None,
                    accessibility_value: None,
                    supported_actions: Vec::new(),
                    bounds: None,
                    children: Vec::new(),
                },
            ]);
        }
        Screen::Chat => {
            if !state.pending_approvals.is_empty() {
                let approval_children = state
                    .pending_approvals
                    .iter()
                    .flat_map(|approval| {
                        [
                            UiNode {
                                id: format!("approval.{}.approve", approval.id),
                                role: UiNodeRole::Button,
                                label: "Approve".to_string(),
                                value: Some(approval.command_summary.clone()),
                                visible: true,
                                enabled: true,
                                focused: false,
                                accessibility_label: None,
                                accessibility_value: None,
                                supported_actions: Vec::new(),
                                bounds: None,
                                children: Vec::new(),
                            },
                            UiNode {
                                id: format!("approval.{}.deny", approval.id),
                                role: UiNodeRole::Button,
                                label: "Deny".to_string(),
                                value: Some(approval.command_summary.clone()),
                                visible: true,
                                enabled: true,
                                focused: false,
                                accessibility_label: None,
                                accessibility_value: None,
                                supported_actions: Vec::new(),
                                bounds: None,
                                children: Vec::new(),
                            },
                        ]
                    })
                    .collect();
                children.push(UiNode {
                    id: "approval.pending".to_string(),
                    role: UiNodeRole::Banner,
                    label: "Approval Required".to_string(),
                    value: Some(format!(
                        "{} pending approval(s)",
                        state.pending_approvals.len()
                    )),
                    visible: true,
                    enabled: true,
                    focused: false,
                    accessibility_label: None,
                    accessibility_value: None,
                    supported_actions: Vec::new(),
                    bounds: None,
                    children: approval_children,
                });
            }

            let session_children = state
                .session_summaries
                .iter()
                .map(|session| UiNode {
                    id: format!("chat.session.{}", session.session_id),
                    role: UiNodeRole::Button,
                    label: session
                        .title
                        .clone()
                        .unwrap_or_else(|| session.display_name.clone()),
                    value: Some(session.session_id.clone()),
                    visible: true,
                    enabled: true,
                    focused: session.is_active,
                    accessibility_label: Some(format!(
                        "Session {}",
                        session.title.as_deref().unwrap_or(&session.display_name)
                    )),
                    accessibility_value: Some(session.status.clone()),
                    supported_actions: Vec::new(),
                    bounds: None,
                    children: Vec::new(),
                })
                .collect();
            children.push(UiNode {
                id: "chat.sessions".to_string(),
                role: UiNodeRole::MessageList,
                label: "Sessions".to_string(),
                value: None,
                visible: true,
                enabled: true,
                focused: false,
                accessibility_label: Some("Sessions".to_string()),
                accessibility_value: None,
                supported_actions: Vec::new(),
                bounds: None,
                children: session_children,
            });
            let message_children = state
                .messages
                .iter()
                .enumerate()
                .map(|(idx, message)| UiNode {
                    id: format!("message.{idx}"),
                    role: UiNodeRole::Message,
                    label: format!("{:?} message", message.role),
                    value: Some(message.text.clone()),
                    visible: true,
                    enabled: true,
                    focused: false,
                    accessibility_label: None,
                    accessibility_value: None,
                    supported_actions: Vec::new(),
                    bounds: None,
                    children: Vec::new(),
                })
                .collect();
            children.push(UiNode {
                id: "chat.messages".to_string(),
                role: UiNodeRole::MessageList,
                label: "Messages".to_string(),
                value: None,
                visible: true,
                enabled: true,
                focused: false,
                accessibility_label: None,
                accessibility_value: None,
                supported_actions: Vec::new(),
                bounds: None,
                children: message_children,
            });
            children.push(UiNode {
                id: "chat.draft".to_string(),
                role: UiNodeRole::Composer,
                label: "Draft".to_string(),
                value: Some(state.draft_message.clone()),
                visible: true,
                enabled: true,
                focused: false,
                accessibility_label: None,
                accessibility_value: None,
                supported_actions: Vec::new(),
                bounds: None,
                children: Vec::new(),
            });
            children.push(UiNode {
                id: "chat.send".to_string(),
                role: UiNodeRole::Button,
                label: "Send".to_string(),
                value: None,
                visible: true,
                enabled: state.connection_state == ConnectionState::Connected,
                focused: false,
                accessibility_label: None,
                accessibility_value: None,
                supported_actions: Vec::new(),
                bounds: None,
                children: Vec::new(),
            });
            children.push(UiNode {
                id: "chat.interrupt".to_string(),
                role: UiNodeRole::Button,
                label: "Interrupt".to_string(),
                value: None,
                visible: true,
                enabled: state.is_processing,
                focused: false,
                accessibility_label: None,
                accessibility_value: None,
                supported_actions: Vec::new(),
                bounds: None,
                children: Vec::new(),
            });
        }
    }

    with_default_layout(with_agent_metadata(UiTree {
        screen: state.screen,
        root: UiNode {
            id: "root".to_string(),
            role: UiNodeRole::Screen,
            label: format!("{:?}", state.screen),
            value: None,
            visible: true,
            enabled: true,
            focused: false,
            accessibility_label: None,
            accessibility_value: None,
            supported_actions: Vec::new(),
            bounds: None,
            children,
        },
    }))
}

fn with_default_layout(mut tree: UiTree) -> UiTree {
    tree.root.bounds = Some(UiRect {
        x: 0,
        y: 0,
        width: DEFAULT_VIEWPORT_WIDTH,
        height: DEFAULT_VIEWPORT_HEIGHT,
    });

    let mut y = 16;
    for child in &mut tree.root.children {
        match child.id.as_str() {
            "banner.status" | "banner.error" => {
                child.bounds = Some(UiRect {
                    x: 16,
                    y,
                    width: DEFAULT_VIEWPORT_WIDTH - 32,
                    height: 44,
                });
                y += 56;
            }
            _ => {}
        }
    }

    match tree.screen {
        Screen::Onboarding | Screen::Pairing => layout_pairing_screen(&mut tree.root.children, y),
        Screen::Chat => layout_chat_screen(&mut tree.root.children, y),
    }

    tree
}

fn layout_pairing_screen(children: &mut [UiNode], mut y: i32) {
    for id in [
        "pair.host",
        "pair.port",
        "pair.code",
        "pair.device_name",
        "pair.submit",
    ] {
        if let Some(node) = children.iter_mut().find(|node| node.id == id) {
            node.bounds = Some(UiRect {
                x: 16,
                y,
                width: DEFAULT_VIEWPORT_WIDTH - 32,
                height: 52,
            });
            y += 64;
        }
    }
}

fn layout_chat_screen(children: &mut [UiNode], y: i32) {
    let mut content_y = y;
    if let Some(approval) = children
        .iter_mut()
        .find(|node| node.id == "approval.pending")
    {
        approval.bounds = Some(UiRect {
            x: 16,
            y: content_y,
            width: DEFAULT_VIEWPORT_WIDTH - 32,
            height: 76,
        });
        let mut button_x = DEFAULT_VIEWPORT_WIDTH - 190;
        for child in &mut approval.children {
            child.bounds = Some(UiRect {
                x: button_x,
                y: content_y + 16,
                width: 82,
                height: 44,
            });
            button_x += 90;
        }
        content_y += 88;
    }

    if let Some(messages) = children.iter_mut().find(|node| node.id == "chat.messages") {
        messages.bounds = Some(UiRect {
            x: 16,
            y: content_y,
            width: DEFAULT_VIEWPORT_WIDTH - 32,
            height: 610 - content_y,
        });
        let mut message_y = content_y + 8;
        for message in &mut messages.children {
            message.bounds = Some(UiRect {
                x: 24,
                y: message_y,
                width: DEFAULT_VIEWPORT_WIDTH - 48,
                height: 56,
            });
            message_y += 64;
        }
    }

    if let Some(draft) = children.iter_mut().find(|node| node.id == "chat.draft") {
        draft.bounds = Some(UiRect {
            x: 16,
            y: 690,
            width: DEFAULT_VIEWPORT_WIDTH - 32,
            height: 52,
        });
    }
    if let Some(send) = children.iter_mut().find(|node| node.id == "chat.send") {
        send.bounds = Some(UiRect {
            x: DEFAULT_VIEWPORT_WIDTH - 110,
            y: 766,
            width: 94,
            height: 44,
        });
    }
    if let Some(interrupt) = children.iter_mut().find(|node| node.id == "chat.interrupt") {
        interrupt.bounds = Some(UiRect {
            x: 16,
            y: 766,
            width: 120,
            height: 44,
        });
    }
}

fn with_agent_metadata(mut tree: UiTree) -> UiTree {
    annotate_node_for_agents(&mut tree.root);
    tree
}

fn annotate_node_for_agents(node: &mut UiNode) {
    if node.accessibility_label.is_none() {
        node.accessibility_label = Some(node.label.clone());
    }
    if node.accessibility_value.is_none() {
        node.accessibility_value = node.value.clone();
    }

    node.supported_actions = match node.role {
        UiNodeRole::TextInput | UiNodeRole::Composer if node.enabled => {
            vec![UiNodeAction::SetText, UiNodeAction::TypeText]
        }
        UiNodeRole::Button if node.enabled => vec![UiNodeAction::Tap],
        UiNodeRole::MessageList if node.enabled => vec![UiNodeAction::Scroll],
        _ => Vec::new(),
    };

    for child in &mut node.children {
        annotate_node_for_agents(child);
    }
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
