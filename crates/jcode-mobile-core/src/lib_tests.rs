use super::*;

#[test]
fn pairing_flow_reaches_connected_chat() {
    let mut store = SimulatorStore::default();
    store.dispatch(SimulatorAction::SetHost {
        value: "devbox.tailnet.ts.net".to_string(),
    });
    store.dispatch(SimulatorAction::SetPairCode {
        value: "123456".to_string(),
    });
    let report = store.dispatch(SimulatorAction::TapNode {
        node_id: "pair.submit".to_string(),
    });

    assert!(!report.transitions.is_empty());
    assert_eq!(store.state().connection_state, ConnectionState::Connected);
    assert_eq!(store.state().screen, Screen::Chat);
}

#[test]
fn pairing_submit_validates_empty_host_with_swift_parity_message() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::PairingReady));
    store.dispatch(SimulatorAction::SetHost {
        value: "   ".to_string(),
    });
    let report = store.dispatch(SimulatorAction::TapNode {
        node_id: "pair.submit".to_string(),
    });

    assert_eq!(
        store.state().connection_state,
        ConnectionState::Disconnected
    );
    assert_eq!(store.state().screen, Screen::Onboarding);
    assert_eq!(
        store.state().error_message.as_deref(),
        Some("Host cannot be empty.")
    );
    assert!(report.effect_records.is_empty());
}

#[test]
fn pairing_submit_validates_empty_code_with_swift_parity_message() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::PairingReady));
    store.dispatch(SimulatorAction::SetPairCode {
        value: "   ".to_string(),
    });
    let report = store.dispatch(SimulatorAction::TapNode {
        node_id: "pair.submit".to_string(),
    });

    assert_eq!(
        store.state().connection_state,
        ConnectionState::Disconnected
    );
    assert_eq!(
        store.state().error_message.as_deref(),
        Some("Enter the 6-digit pairing code from jcode pair.")
    );
    assert!(report.effect_records.is_empty());
}

#[test]
fn pairing_submit_validates_port_before_effect() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::PairingReady));
    store.dispatch(SimulatorAction::SetPort {
        value: "70000".to_string(),
    });
    let report = store.dispatch(SimulatorAction::TapNode {
        node_id: "pair.submit".to_string(),
    });

    assert_eq!(
        store.state().error_message.as_deref(),
        Some("Port must be a number from 0 to 65535.")
    );
    assert!(report.effect_records.is_empty());
}

#[test]
fn pairing_submit_emits_normalized_pairing_effect() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::PairingReady));
    store.dispatch(SimulatorAction::SetHost {
        value: "  devbox.tailnet.ts.net  ".to_string(),
    });
    store.dispatch(SimulatorAction::SetPort {
        value: " 7643 ".to_string(),
    });
    store.dispatch(SimulatorAction::SetPairCode {
        value: " 123456 ".to_string(),
    });
    store.dispatch(SimulatorAction::SetDeviceName {
        value: "  Mark's iPhone  ".to_string(),
    });
    let report = store.dispatch(SimulatorAction::TapNode {
        node_id: "pair.submit".to_string(),
    });

    assert_eq!(
        report.effect_records.first().map(|record| &record.effect),
        Some(&SimulatorEffect::PairAndConnect {
            host: "devbox.tailnet.ts.net".to_string(),
            port: "7643".to_string(),
            pair_code: "123456".to_string(),
            device_name: "Mark's iPhone".to_string(),
        })
    );
    assert_eq!(
        store
            .state()
            .selected_server
            .as_ref()
            .map(|server| server.host.as_str()),
        Some("devbox.tailnet.ts.net")
    );
}

#[test]
fn sending_message_creates_assistant_reply() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::SetDraft {
        value: "hello simulator".to_string(),
    });
    store.dispatch(SimulatorAction::TapNode {
        node_id: "chat.send".to_string(),
    });

    let last = store.state().messages.last();
    assert!(last.is_some(), "assistant reply present");
    let Some(last) = last else {
        return;
    };
    assert_eq!(last.role, MessageRole::Assistant);
    assert!(last.text.contains("hello simulator"));
    assert!(!store.state().is_processing);
}

#[test]
fn chat_send_transition_appends_user_and_assistant_placeholder() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::SetDraft {
        value: "  hello core  ".to_string(),
    });
    let report = store.dispatch(SimulatorAction::TapNode {
        node_id: "chat.send".to_string(),
    });

    let send_transition = report
        .transitions
        .iter()
        .find(|transition| {
            matches!(transition.action, SimulatorAction::TapNode { ref node_id } if node_id == "chat.send")
        })
        .expect("chat.send transition");
    let messages = &send_transition.after.messages;
    assert_eq!(messages[messages.len() - 2].role, MessageRole::User);
    assert_eq!(messages[messages.len() - 2].text, "hello core");
    assert_eq!(messages[messages.len() - 1].role, MessageRole::Assistant);
    assert_eq!(messages[messages.len() - 1].text, "");
    assert!(send_transition.after.is_processing);
}

#[test]
fn assistant_text_delta_updates_latest_assistant_message() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::AppendAssistantText {
        text: "hello".to_string(),
    });
    store.dispatch(SimulatorAction::AppendAssistantText {
        text: " world".to_string(),
    });

    let assistants: Vec<_> = store
        .state()
        .messages
        .iter()
        .filter(|message| message.role == MessageRole::Assistant)
        .collect();
    assert_eq!(assistants.len(), 1);
    assert_eq!(
        assistants.last().map(|message| message.text.as_str()),
        Some(
            "The simulator is headless-first, automation-first, and shares state semantics with the future iOS app.hello world"
        )
    );
    assert!(store.state().is_processing);
}

#[test]
fn assistant_text_replace_updates_latest_assistant_message() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ReplaceAssistantText {
        text: "replacement".to_string(),
    });

    let assistants: Vec<_> = store
        .state()
        .messages
        .iter()
        .filter(|message| message.role == MessageRole::Assistant)
        .collect();
    assert_eq!(assistants.len(), 1);
    assert_eq!(
        assistants.last().map(|message| message.text.as_str()),
        Some("replacement")
    );
}

#[test]
fn chat_send_empty_draft_does_not_emit_effect() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::SetDraft {
        value: "   ".to_string(),
    });
    let report = store.dispatch(SimulatorAction::TapNode {
        node_id: "chat.send".to_string(),
    });

    assert_eq!(
        store.state().error_message.as_deref(),
        Some("Draft is empty.")
    );
    assert!(report.effect_records.is_empty());
}

#[test]
fn tool_start_attaches_tool_to_latest_assistant_message() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ToolStart {
        id: "tool-1".to_string(),
        name: "bash".to_string(),
    });

    let assistant = store
        .state()
        .messages
        .iter()
        .rev()
        .find(|message| message.role == MessageRole::Assistant)
        .expect("assistant message");
    assert_eq!(assistant.tool_calls.len(), 1);
    assert_eq!(assistant.tool_calls[0].id, "tool-1");
    assert_eq!(assistant.tool_calls[0].name, "bash");
    assert_eq!(assistant.tool_calls[0].state, ToolCallState::Streaming);
    assert_eq!(store.state().active_tool_id.as_deref(), Some("tool-1"));
    assert!(store.state().is_processing);
}

#[test]
fn tool_input_accumulates_on_active_tool() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ToolStart {
        id: "tool-1".to_string(),
        name: "bash".to_string(),
    });
    store.dispatch(SimulatorAction::ToolInput {
        delta: "cargo ".to_string(),
    });
    store.dispatch(SimulatorAction::ToolInput {
        delta: "test".to_string(),
    });

    let tool = latest_tool(&store).expect("latest tool");
    assert_eq!(tool.input, "cargo test");
    assert_eq!(tool.state, ToolCallState::Streaming);
}

#[test]
fn tool_exec_and_done_update_tool_state() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ToolStart {
        id: "tool-1".to_string(),
        name: "bash".to_string(),
    });
    store.dispatch(SimulatorAction::ToolExec {
        id: "tool-1".to_string(),
        name: "bash".to_string(),
    });

    let executing = latest_tool(&store).expect("executing tool");
    assert_eq!(executing.state, ToolCallState::Executing);

    store.dispatch(SimulatorAction::ToolDone {
        id: "tool-1".to_string(),
        name: "bash".to_string(),
        output: "ok".to_string(),
        error: None,
    });

    let done = latest_tool(&store).expect("done tool");
    assert_eq!(done.state, ToolCallState::Done);
    assert_eq!(done.output.as_deref(), Some("ok"));
    assert_eq!(done.error, None);
}

#[test]
fn tool_done_with_error_marks_tool_failed() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ToolStart {
        id: "tool-1".to_string(),
        name: "bash".to_string(),
    });
    store.dispatch(SimulatorAction::ToolDone {
        id: "tool-1".to_string(),
        name: "bash".to_string(),
        output: "stderr".to_string(),
        error: Some("exit status 1".to_string()),
    });

    let failed = latest_tool(&store).expect("failed tool");
    assert_eq!(failed.state, ToolCallState::Failed);
    assert_eq!(failed.output.as_deref(), Some("stderr"));
    assert_eq!(failed.error.as_deref(), Some("exit status 1"));
}

#[test]
fn finish_turn_clears_active_tool_tracking() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ToolStart {
        id: "tool-1".to_string(),
        name: "bash".to_string(),
    });
    store.dispatch(SimulatorAction::FinishTurn);

    assert_eq!(store.state().active_tool_id, None);
    assert!(!store.state().is_processing);
}

#[test]
fn history_event_updates_session_model_and_messages() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ApplyServerEvent {
        event: protocol::MobileServerEvent::History(protocol::HistoryPayload {
            session_id: "session_sim_2".to_string(),
            messages: vec![
                protocol::HistoryMessage {
                    role: "user".to_string(),
                    content: "run tests".to_string(),
                    tool_data: None,
                },
                protocol::HistoryMessage {
                    role: "assistant".to_string(),
                    content: "done".to_string(),
                    tool_data: Some(protocol::HistoryToolData {
                        id: Some("tool-9".to_string()),
                        name: Some("bash".to_string()),
                        input: Some("cargo test".to_string()),
                        output: Some("ok".to_string()),
                    }),
                },
            ],
            server_name: Some("jcode".to_string()),
            server_icon: None,
            server_version: Some("v-test".to_string()),
            provider_name: Some("openai".to_string()),
            provider_model: Some("gpt-5.1".to_string()),
            connection_type: Some("simulator".to_string()),
            available_models: vec!["gpt-5.1".to_string(), "claude-sonnet-4".to_string()],
            all_sessions: vec!["session_sim_1".to_string(), "session_sim_2".to_string()],
            is_canary: None,
            was_interrupted: None,
            total_tokens: None,
        }),
    });

    assert_eq!(
        store.state().active_session_id.as_deref(),
        Some("session_sim_2")
    );
    assert_eq!(store.state().model_name.as_deref(), Some("gpt-5.1"));
    assert_eq!(store.state().messages.len(), 2);
    let assistant = store.state().messages.last().expect("assistant history");
    assert_eq!(assistant.role, MessageRole::Assistant);
    assert_eq!(assistant.tool_calls[0].state, ToolCallState::Done);
    assert_eq!(assistant.tool_calls[0].input, "cargo test");
}

#[test]
fn switch_session_emits_resume_effect_and_waits_for_history() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    let report = store.dispatch(SimulatorAction::SwitchSession {
        session_id: "session_sim_2".to_string(),
    });

    let switch_transition = report
        .transitions
        .iter()
        .find(|transition| matches!(transition.action, SimulatorAction::SwitchSession { .. }))
        .expect("switch transition");
    assert_eq!(
        switch_transition.after.pending_session_id.as_deref(),
        Some("session_sim_2")
    );
    assert!(switch_transition.after.messages.is_empty());
    assert_eq!(
        report.effect_records.first().map(|record| &record.effect),
        Some(&SimulatorEffect::ResumeSession {
            session_id: "session_sim_2".to_string(),
        })
    );
    assert_eq!(
        store.state().active_session_id.as_deref(),
        Some("session_sim_2")
    );
    assert_eq!(store.state().pending_session_id, None);
}

#[test]
fn model_change_waits_for_server_confirmation() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    let report = store.dispatch(SimulatorAction::SetModel {
        model: "claude-sonnet-4".to_string(),
    });

    let request_transition = report
        .transitions
        .iter()
        .find(|transition| matches!(transition.action, SimulatorAction::SetModel { .. }))
        .expect("model request transition");
    assert_eq!(
        request_transition.after.pending_model_name.as_deref(),
        Some("claude-sonnet-4")
    );
    assert_eq!(
        request_transition.after.model_name.as_deref(),
        Some("gpt-5")
    );
    assert_eq!(store.state().model_name.as_deref(), Some("claude-sonnet-4"));
    assert_eq!(store.state().pending_model_name, None);
}

#[test]
fn reconnect_effect_preserves_active_session() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    let report = store.dispatch(SimulatorAction::Disconnected {
        message: Some("socket closed".to_string()),
        should_reconnect: true,
    });

    let reconnect_transition = report
        .transitions
        .iter()
        .find(|transition| {
            matches!(
                transition.action,
                SimulatorAction::Disconnected {
                    should_reconnect: true,
                    ..
                }
            )
        })
        .expect("reconnect transition");
    assert_eq!(
        reconnect_transition.after.connection_state,
        ConnectionState::Connecting
    );
    assert_eq!(
        report.effect_records.first().map(|record| &record.effect),
        Some(&SimulatorEffect::Reconnect {
            host: "devbox.tailnet.ts.net".to_string(),
            port: "7643".to_string(),
            session_id: Some("session_sim_1".to_string()),
            attempt: 0,
        })
    );
    assert_eq!(store.state().connection_state, ConnectionState::Connected);
    assert_eq!(store.state().reconnect_attempt, 0);
}

#[test]
fn interrupted_event_removes_empty_assistant_placeholder() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ReplaceAssistantText {
        text: String::new(),
    });
    store.dispatch(SimulatorAction::ApplyServerEvent {
        event: protocol::MobileServerEvent::Interrupted,
    });

    assert!(!store.state().is_processing);
    assert!(
        !store
            .state()
            .messages
            .iter()
            .any(|message| message.role == MessageRole::Assistant && message.text.is_empty())
    );
    assert_eq!(
        store.state().messages.last().map(|message| message.role),
        Some(MessageRole::System)
    );
}

#[test]
fn approval_request_is_rust_owned_state() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ApprovalRequested {
        request: ApprovalRequest {
            id: "approval-42".to_string(),
            command_summary: "bash: cargo test -p jcode-mobile-core".to_string(),
            workspace: Some("/workspace/jcode".to_string()),
            risk: ApprovalRisk::Medium,
            timeout_seconds: Some(300),
            reason: Some("Regression run".to_string()),
        },
    });

    assert_eq!(store.state().pending_approvals.len(), 1);
    assert_eq!(store.state().pending_approvals[0].id, "approval-42");
    assert!(store.state().is_processing);
    assert!(
        store
            .state()
            .messages
            .iter()
            .any(|message| message.text.contains("Approval required"))
    );
}

#[test]
fn approval_decisions_emit_scoped_effects() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ApprovalRequested {
        request: ApprovalRequest {
            id: "approval-42".to_string(),
            command_summary: "bash: cargo test".to_string(),
            workspace: None,
            risk: ApprovalRisk::Low,
            timeout_seconds: None,
            reason: None,
        },
    });
    let report = store.dispatch(SimulatorAction::ApproveApproval {
        request_id: "approval-42".to_string(),
    });

    assert!(store.state().pending_approvals.is_empty());
    assert_eq!(
        report.effect_records.first().map(|record| &record.effect),
        Some(&SimulatorEffect::SubmitApproval {
            request_id: "approval-42".to_string(),
            approved: true,
            reason: None,
        })
    );
}

#[test]
fn approval_expiry_prevents_late_decision() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ApprovalRequested {
        request: ApprovalRequest {
            id: "approval-42".to_string(),
            command_summary: "bash: rm -rf target".to_string(),
            workspace: None,
            risk: ApprovalRisk::High,
            timeout_seconds: Some(1),
            reason: None,
        },
    });
    store.dispatch(SimulatorAction::ApprovalExpired {
        request_id: "approval-42".to_string(),
    });
    let report = store.dispatch(SimulatorAction::DenyApproval {
        request_id: "approval-42".to_string(),
        reason: Some("Too late".to_string()),
    });

    assert!(store.state().pending_approvals.is_empty());
    assert!(report.effect_records.is_empty());
    assert_eq!(
        store.state().error_message.as_deref(),
        Some("Approval request is no longer pending.")
    );
}

fn latest_tool(store: &SimulatorStore) -> Option<&ToolCall> {
    store
        .state()
        .messages
        .iter()
        .rev()
        .find_map(|message| message.tool_calls.last())
}

#[test]
fn semantic_tree_reflects_current_screen() {
    let store = SimulatorStore::default();
    let tree = store.semantic_tree();
    assert_eq!(tree.screen, Screen::Onboarding);
    assert!(
        tree.root
            .children
            .iter()
            .any(|node| node.id == "pair.submit")
    );
}

#[test]
fn semantic_tree_exposes_agent_metadata() {
    let store = SimulatorStore::default();
    let tree = store.semantic_tree();

    let pair_submit = tree
        .root
        .children
        .iter()
        .find(|node| node.id == "pair.submit");
    assert!(pair_submit.is_some(), "pair submit node");
    let Some(pair_submit) = pair_submit else {
        return;
    };
    assert_eq!(
        pair_submit.accessibility_label.as_deref(),
        Some("Pair & Connect")
    );
    assert!(pair_submit.supported_actions.contains(&UiNodeAction::Tap));

    let pair_host = tree
        .root
        .children
        .iter()
        .find(|node| node.id == "pair.host");
    assert!(pair_host.is_some(), "pair host node");
    let Some(pair_host) = pair_host else {
        return;
    };
    assert!(pair_host.supported_actions.contains(&UiNodeAction::SetText));
    assert!(
        pair_host
            .supported_actions
            .contains(&UiNodeAction::TypeText)
    );
}

#[test]
fn all_scenarios_parse_round_trip() {
    for scenario in ScenarioName::ALL {
        assert_eq!(ScenarioName::parse(scenario.as_str()), Some(*scenario));
    }
}

#[test]
fn scenario_fixtures_cover_error_processing_and_offline_states() {
    let invalid = SimulatorState::for_scenario(ScenarioName::PairingInvalidCode);
    assert!(
        invalid
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid")
    );

    let streaming = SimulatorState::for_scenario(ScenarioName::ChatStreaming);
    assert!(streaming.is_processing);
    assert_eq!(streaming.screen, Screen::Chat);

    let offline = SimulatorState::for_scenario(ScenarioName::OfflineQueuedMessage);
    assert_eq!(offline.connection_state, ConnectionState::Disconnected);
    assert!(offline.draft_message.contains("Queued"));
}

#[test]
fn fake_backend_rejects_invalid_pairing_code() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::PairingReady));
    store.dispatch(SimulatorAction::SetPairCode {
        value: "000000".to_string(),
    });
    store.dispatch(SimulatorAction::TapNode {
        node_id: "pair.submit".to_string(),
    });

    assert_eq!(
        store.state().connection_state,
        ConnectionState::Disconnected
    );
    assert!(
        store
            .state()
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid")
    );
}

#[test]
fn fake_backend_reports_unreachable_host() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::PairingReady));
    store.dispatch(SimulatorAction::SetHost {
        value: "offline.tailnet.ts.net".to_string(),
    });
    store.dispatch(SimulatorAction::TapNode {
        node_id: "pair.submit".to_string(),
    });

    assert_eq!(
        store.state().connection_state,
        ConnectionState::Disconnected
    );
    assert!(
        store
            .state()
            .error_message
            .as_deref()
            .unwrap_or_default()
            .contains("unreachable")
    );
}

#[test]
fn replay_trace_records_and_replays_deterministically() -> anyhow::Result<()> {
    let actions = vec![
        SimulatorAction::TapNode {
            node_id: "pair.submit".to_string(),
        },
        SimulatorAction::SetDraft {
            value: "hello replay".to_string(),
        },
        SimulatorAction::TapNode {
            node_id: "chat.send".to_string(),
        },
    ];
    let trace = ReplayTrace::record(
        "pairing-ready-chat-send",
        SimulatorState::for_scenario(ScenarioName::PairingReady),
        actions,
    );
    trace.assert_replays()?;
    assert_eq!(trace.actions.len(), 3);
    assert_eq!(trace.transitions.len(), 7);
    assert_eq!(trace.effects.len(), 2);
    assert_eq!(trace.final_state.screen, Screen::Chat);
    assert!(
        trace
            .final_state
            .messages
            .iter()
            .any(|message| message.text.contains("hello replay"))
    );
    Ok(())
}

#[test]
fn golden_replay_trace_matches_core_behavior() -> anyhow::Result<()> {
    let golden = include_str!("../tests/golden/pairing_ready_chat_send.json");
    let trace: ReplayTrace = serde_json::from_str(golden)?;
    trace.assert_replays()?;
    Ok(())
}

#[test]
fn layout_bounds_support_hit_testing() {
    let store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::PairingReady));
    let tree = store.semantic_tree();
    let submit = tree
        .root
        .children
        .iter()
        .find(|node| node.id == "pair.submit");
    assert!(submit.is_some(), "pair.submit node");
    let Some(submit) = submit else {
        return;
    };
    assert!(submit.bounds.is_some(), "pair.submit bounds");
    let Some(bounds) = submit.bounds else {
        return;
    };
    let (x, y) = bounds.center();
    assert_eq!(
        hit_test(&tree, x, y).map(|node| node.id.as_str()),
        Some("pair.submit")
    );
    assert_eq!(
        hit_test_actionable(&tree, x, y, UiNodeAction::Tap).map(|node| node.id.as_str()),
        Some("pair.submit")
    );
}

#[test]
fn chat_layout_hit_tests_send_button() {
    let store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    let tree = store.semantic_tree();
    assert_eq!(
        hit_test_actionable(&tree, 330, 788, UiNodeAction::Tap).map(|node| node.id.as_str()),
        Some("chat.send")
    );
}

#[test]
fn screenshot_snapshot_is_deterministic_svg_with_layout() {
    let store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    let tree = store.semantic_tree();
    let first = screenshot_snapshot(&tree);
    let second = screenshot_snapshot(&tree);

    assert_eq!(first, second);
    assert_eq!(first.width, DEFAULT_VIEWPORT_WIDTH);
    assert_eq!(first.height, DEFAULT_VIEWPORT_HEIGHT);
    assert!(first.hash.starts_with("fnv1a64:"));
    assert!(first.svg.contains("data-node=\"chat.send\""));
    assert_eq!(
        first.scene.as_ref().map(|scene| scene.schema_version),
        Some(1)
    );
    assert!(first.layout.root.bounds.is_some());
}

#[test]
fn visual_scene_is_rust_owned_backend_contract() {
    let store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    let scene = store.visual_scene();

    assert_eq!(scene.schema_version, VISUAL_SCENE_SCHEMA_VERSION);
    assert_eq!(scene.coordinate_space, "logical_points_top_left");
    assert_eq!(scene.viewport.width, DEFAULT_VIEWPORT_WIDTH);
    assert_eq!(scene.viewport.height, DEFAULT_VIEWPORT_HEIGHT);
    assert!(scene.layers.iter().any(|layer| layer.id == "background"));
    assert!(scene.layers.iter().any(|layer| layer.id == "chrome"));
    assert!(scene.layers.iter().any(|layer| layer.id == "content"));

    let content = scene.layers.iter().find(|layer| layer.id == "content");
    assert!(content.is_some(), "content layer");
    let Some(content) = content else {
        return;
    };
    assert!(content.primitives.iter().any(|primitive| matches!(
        primitive,
        VisualPrimitive::Rect(rect)
            if rect.semantic_node_id.as_deref() == Some("chat.send")
                && rect.bounds.x == DEFAULT_VIEWPORT_WIDTH - 110
    )));
    assert!(content.primitives.iter().any(|primitive| matches!(
        primitive,
        VisualPrimitive::Text(text)
            if text.semantic_node_id.as_deref() == Some("message.0")
                && text.text.contains("summarize")
    )));
}

#[test]
fn svg_backend_renders_from_visual_scene() {
    let store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::PairingReady));
    let scene = store.visual_scene();
    let svg = render_scene_svg(&scene);

    assert!(svg.contains("data-layer=\"background\""));
    assert!(svg.contains("data-layer=\"chrome\""));
    assert!(svg.contains("data-layer=\"content\""));
    assert!(svg.contains("data-primitive=\"pair.submit.rect\""));
    assert!(svg.contains("data-node=\"pair.submit\""));
    assert!(svg.contains("Pair &amp; Connect"));
}

#[test]
fn screenshot_diff_reports_mismatch() {
    let store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    let mut expected = screenshot_snapshot(&store.semantic_tree());
    let actual = expected.clone();
    expected.svg.push_str("<!-- changed -->");
    expected.hash = "fnv1a64:changed".to_string();

    let diff = diff_screenshots(&expected, &actual);
    assert!(!diff.matches);
    assert!(diff.first_difference.is_some());
}

#[test]
fn text_render_exposes_human_readable_layout() {
    let store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    let text = render_text(&store.semantic_tree());

    assert!(text.contains("jcode mobile simulator"));
    assert!(text.contains("screen: Chat"));
    assert!(text.contains("chat.send [Button]"));
    assert!(text.contains("@280,766 94x44"));
}
