# Mobile Chat Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Move the next chat behavior slice into `jcode-mobile-core` so Swift can later render/call shared behavior instead of owning it twice.

**Architecture:** Rust remains the source of truth for mobile product behavior. This slice keeps the existing simulator/fake-backend shape, but makes chat send and assistant streaming semantics match the current Swift `AppModel` behavior closely enough to bridge later.

**Tech Stack:** Rust, `jcode-mobile-core`, `jcode-mobile-sim`, existing replay/golden trace tests.

---

### Task 1: Chat Placeholder And Stream Reducer

**Files:**
- Modify: `crates/jcode-mobile-core/src/lib.rs`
- Modify: `crates/jcode-mobile-core/src/lib_tests.rs`
- Modify if required: `crates/jcode-mobile-core/tests/golden/pairing_ready_chat_send.json`

- [x] **Step 1: Write failing tests**

Add tests in `crates/jcode-mobile-core/src/lib_tests.rs` asserting:

```rust
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
        .find(|transition| matches!(transition.action, SimulatorAction::TapNode { ref node_id } if node_id == "chat.send"))
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
    assert_eq!(assistants.last().map(|message| message.text.as_str()), Some("The simulator is headless-first, automation-first, and shares state semantics with the future iOS app.hello world"));
}

#[test]
fn assistant_text_replace_updates_latest_assistant_message() {
    let mut store = SimulatorStore::new(SimulatorState::for_scenario(ScenarioName::ConnectedChat));
    store.dispatch(SimulatorAction::ReplaceAssistantText {
        text: "replacement".to_string(),
    });

    assert_eq!(
        store.state().messages.last().map(|message| message.text.as_str()),
        Some("replacement")
    );
}
```

- [x] **Step 2: Verify the tests fail**

Run:

```bash
rtk cargo test -p jcode-mobile-core chat_send_transition assistant_text -- --nocapture
```

Expected: failures because `chat.send` does not create an assistant placeholder and `ReplaceAssistantText` does not exist yet.

- [x] **Step 3: Implement minimal reducer changes**

In `crates/jcode-mobile-core/src/lib.rs`:

- Add `ReplaceAssistantText { text: String }` to `SimulatorAction`.
- On `chat.send`, append a user message and an empty assistant message before emitting `SendMessage`.
- Change `AppendAssistantText` to append to the latest assistant message, creating one only if no assistant exists.
- Implement `ReplaceAssistantText` to replace the latest assistant message, creating one only if no assistant exists.
- Keep `FinishTurn` as the processing cleanup action.

- [x] **Step 4: Verify mobile core and replay behavior**

Run:

```bash
rtk cargo test -p jcode-mobile-core
```

Expected: all core tests pass, except the golden replay may fail if the intentional placeholder transition changes the serialized trace.

- [x] **Step 5: Refresh golden replay if needed**

If `golden_replay_trace_matches_core_behavior` fails because the new reducer state is intentional, update `crates/jcode-mobile-core/tests/golden/pairing_ready_chat_send.json` from a freshly recorded trace.

- [x] **Step 6: Run simulator and smoke checks**

Run:

```bash
rtk cargo test -p jcode-mobile-sim
rtk scripts/mobile_simulator_smoke.sh
rtk cargo check -p jcode-mobile-core -p jcode-mobile-sim
rtk git diff --check
```

Expected: all pass.

- [x] **Step 7: Commit and push**

Run:

```bash
rtk git add crates/jcode-mobile-core/src/lib.rs crates/jcode-mobile-core/src/lib_tests.rs crates/jcode-mobile-core/tests/golden/pairing_ready_chat_send.json docs/superpowers/plans/2026-05-17-mobile-chat-core.md
rtk git commit -m "Move mobile chat streaming into core"
rtk git push origin ios-app
```
