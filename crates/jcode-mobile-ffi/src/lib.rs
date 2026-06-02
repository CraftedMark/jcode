use jcode_mobile_core::{ScenarioName, SimulatorAction, SimulatorState, SimulatorStore};
use serde::Serialize;
use serde_json::Value;
use std::ffi::{CStr, CString, c_char};
use std::ptr;

pub struct MobileAppHandle {
    store: SimulatorStore,
}

#[derive(Debug, Serialize)]
struct BridgeResponse<T: Serialize> {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct InitialStateConfig {
    #[serde(default)]
    scenario: Option<String>,
    #[serde(default)]
    state: Option<SimulatorState>,
}

impl MobileAppHandle {
    fn new(initial_json: Option<&str>) -> Result<Self, String> {
        let state = match initial_json
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            Some(json) => initial_state_from_json(json)?,
            None => SimulatorState::default(),
        };
        Ok(Self {
            store: SimulatorStore::new(state),
        })
    }

    fn dispatch_json(&mut self, action_json: &str) -> Result<Value, String> {
        let action: SimulatorAction = serde_json::from_str(action_json)
            .map_err(|error| format!("invalid action json: {error}"))?;
        let report = self.store.dispatch(action);
        serde_json::to_value(report)
            .map_err(|error| format!("dispatch serialization failed: {error}"))
    }

    fn state_json(&self) -> Result<Value, String> {
        serde_json::to_value(self.store.state())
            .map_err(|error| format!("state serialization failed: {error}"))
    }

    fn tree_json(&self) -> Result<Value, String> {
        serde_json::to_value(self.store.semantic_tree())
            .map_err(|error| format!("tree serialization failed: {error}"))
    }

    fn logs_json(&self, limit: usize) -> Result<Value, String> {
        let transitions = self.store.transition_log();
        let start = transitions.len().saturating_sub(limit);
        serde_json::to_value(&transitions[start..])
            .map_err(|error| format!("log serialization failed: {error}"))
    }
}

fn initial_state_from_json(json: &str) -> Result<SimulatorState, String> {
    let config: InitialStateConfig = serde_json::from_str(json)
        .map_err(|error| format!("invalid initial state json: {error}"))?;

    if let Some(state) = config.state {
        return Ok(state);
    }

    if let Some(scenario) = config.scenario {
        let scenario = ScenarioName::parse(&scenario)
            .ok_or_else(|| format!("unknown mobile scenario: {scenario}"))?;
        return Ok(SimulatorState::for_scenario(scenario));
    }

    Ok(SimulatorState::default())
}

fn response_json<T: Serialize>(result: Result<T, String>) -> String {
    let response = match result {
        Ok(value) => BridgeResponse {
            ok: true,
            value: Some(value),
            error: None,
        },
        Err(error) => BridgeResponse::<T> {
            ok: false,
            value: None,
            error: Some(error),
        },
    };
    serde_json::to_string(&response).unwrap_or_else(|error| {
        format!(r#"{{"ok":false,"error":"bridge response serialization failed: {error}"}}"#)
    })
}

fn c_string_to_str<'a>(ptr: *const c_char) -> Result<Option<&'a str>, String> {
    if ptr.is_null() {
        return Ok(None);
    }
    let c_str = unsafe { CStr::from_ptr(ptr) };
    c_str
        .to_str()
        .map(Some)
        .map_err(|error| format!("input was not valid UTF-8: {error}"))
}

fn into_c_string(json: String) -> *mut c_char {
    match CString::new(json) {
        Ok(value) => value.into_raw(),
        Err(error) => {
            let fallback = format!(
                r#"{{"ok":false,"error":"bridge response contained interior NUL: {error}"}}"#
            );
            CString::new(fallback)
                .expect("fallback bridge response does not contain NUL")
                .into_raw()
        }
    }
}

fn handle_mut<'a>(app: *mut MobileAppHandle) -> Result<&'a mut MobileAppHandle, String> {
    if app.is_null() {
        return Err("mobile app handle is null".to_string());
    }
    Ok(unsafe { &mut *app })
}

fn handle_ref<'a>(app: *const MobileAppHandle) -> Result<&'a MobileAppHandle, String> {
    if app.is_null() {
        return Err("mobile app handle is null".to_string());
    }
    Ok(unsafe { &*app })
}

#[unsafe(no_mangle)]
pub extern "C" fn jcode_mobile_app_new(initial_json: *const c_char) -> *mut MobileAppHandle {
    let initial = match c_string_to_str(initial_json) {
        Ok(value) => value,
        Err(_) => return ptr::null_mut(),
    };

    match MobileAppHandle::new(initial) {
        Ok(app) => Box::into_raw(Box::new(app)),
        Err(_) => ptr::null_mut(),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn jcode_mobile_app_free(app: *mut MobileAppHandle) {
    if app.is_null() {
        return;
    }
    unsafe {
        drop(Box::from_raw(app));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn jcode_mobile_dispatch(
    app: *mut MobileAppHandle,
    action_json: *const c_char,
) -> *mut c_char {
    let result = handle_mut(app).and_then(|app| {
        let action_json = c_string_to_str(action_json)?.unwrap_or("");
        app.dispatch_json(action_json)
    });
    into_c_string(response_json(result))
}

#[unsafe(no_mangle)]
pub extern "C" fn jcode_mobile_state(app: *const MobileAppHandle) -> *mut c_char {
    let result = handle_ref(app).and_then(MobileAppHandle::state_json);
    into_c_string(response_json(result))
}

#[unsafe(no_mangle)]
pub extern "C" fn jcode_mobile_tree(app: *const MobileAppHandle) -> *mut c_char {
    let result = handle_ref(app).and_then(MobileAppHandle::tree_json);
    into_c_string(response_json(result))
}

#[unsafe(no_mangle)]
pub extern "C" fn jcode_mobile_logs(app: *const MobileAppHandle, limit: u32) -> *mut c_char {
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let result = handle_ref(app).and_then(|app| app.logs_json(limit));
    into_c_string(response_json(result))
}

#[unsafe(no_mangle)]
pub extern "C" fn jcode_mobile_string_free(value: *mut c_char) {
    if value.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn creates_app_from_scenario_json() {
        let app = MobileAppHandle::new(Some(r#"{"scenario":"connected_chat"}"#));
        assert!(app.is_ok(), "scenario should create app");
        let app = app.expect("app");
        let value = app.state_json().expect("state json");
        assert_eq!(value["screen"], "chat");
        assert_eq!(value["active_session_id"], "session_sim_1");
    }

    #[test]
    fn dispatches_actions_through_json_boundary() {
        let mut app = MobileAppHandle::new(Some(r#"{"scenario":"connected_chat"}"#)).expect("app");
        let report = app
            .dispatch_json(r#"{"type":"set_model","model":"claude-sonnet-4"}"#)
            .expect("dispatch");
        assert_eq!(report["final_state"]["model_name"], "claude-sonnet-4");
        let logs = app.logs_json(1).expect("logs");
        assert_eq!(logs.as_array().map(Vec::len), Some(1));
    }

    #[test]
    fn invalid_action_returns_error_envelope() {
        let mut app = MobileAppHandle::new(None).expect("app");
        let json = response_json(app.dispatch_json(r#"{"type":"missing"}"#));
        let value: Value = serde_json::from_str(&json).expect("response json");
        assert_eq!(value["ok"], false);
        assert!(
            value["error"]
                .as_str()
                .unwrap_or_default()
                .contains("invalid action json")
        );
    }

    #[test]
    fn c_abi_state_round_trip_returns_owned_string() {
        let initial = CString::new(r#"{"scenario":"pairing_ready"}"#).expect("initial c string");
        let app = jcode_mobile_app_new(initial.as_ptr());
        assert!(!app.is_null());

        let state_ptr = jcode_mobile_state(app);
        assert!(!state_ptr.is_null());
        let state = unsafe { CStr::from_ptr(state_ptr) }
            .to_str()
            .expect("state utf8")
            .to_string();
        jcode_mobile_string_free(state_ptr);
        jcode_mobile_app_free(app);

        let value: Value = serde_json::from_str(&state).expect("state response");
        assert_eq!(value, json!({"ok":true,"value":value["value"]}));
        assert_eq!(value["value"]["pairing"]["host"], "devbox.tailnet.ts.net");
    }
}
