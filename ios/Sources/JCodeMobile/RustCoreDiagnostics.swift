import Foundation

#if canImport(JCodeMobileCore)
import JCodeMobileCore

enum RustCoreDiagnostics {
    static func smoke() -> String {
        let initial = #"{"scenario":"connected_chat"}"#
        guard let handle = initial.withCString({ jcode_mobile_app_new($0) }) else {
            return "Rust core unavailable"
        }
        defer {
            jcode_mobile_app_free(handle)
        }

        let action = #"{"type":"set_model","model":"claude-sonnet-4"}"#
        guard let response = action.withCString({ jcode_mobile_dispatch(handle, $0) }) else {
            return "Rust dispatch returned no response"
        }
        defer {
            jcode_mobile_string_free(response)
        }

        guard let text = String(validatingCString: response) else {
            return "Rust dispatch returned invalid UTF-8"
        }

        if text.contains(#""ok":true"#), text.contains("claude-sonnet-4") {
            return "Rust core linked"
        }
        return "Rust core response unexpected"
    }
}
#else
enum RustCoreDiagnostics {
    static func smoke() -> String {
        "Rust core not linked"
    }
}
#endif
