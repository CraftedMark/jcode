import Foundation

#if canImport(JCodeMobileCore)
import JCodeMobileCore

@MainActor
final class MobileCoreBridge {
    private var handle: OpaquePointer?

    init() {
        let initial = #"{"scenario":"onboarding"}"#
        handle = initial.withCString { jcode_mobile_app_new($0) }
    }

    deinit {
        if let handle {
            jcode_mobile_app_free(handle)
        }
    }

    var isLinked: Bool {
        handle != nil
    }

    @discardableResult
    func dispatch(_ json: String) -> MobileCoreSnapshot? {
        guard let handle else { return nil }
        guard let responsePointer = json.withCString({ jcode_mobile_dispatch(handle, $0) }) else {
            return nil
        }
        jcode_mobile_string_free(responsePointer)
        return state()
    }

    func state() -> MobileCoreSnapshot? {
        guard let handle, let responsePointer = jcode_mobile_state(handle) else {
            return nil
        }
        defer {
            jcode_mobile_string_free(responsePointer)
        }
        guard let response = String(validatingUTF8: responsePointer),
              let data = response.data(using: .utf8),
              let envelope = try? JSONDecoder().decode(MobileCoreStateEnvelope.self, from: data),
              envelope.ok else {
            return nil
        }
        return MobileCoreSnapshot(state: envelope.value)
    }
}
#else
@MainActor
final class MobileCoreBridge {
    var isLinked: Bool { false }
    func dispatch(_: String) -> MobileCoreSnapshot? { nil }
    func state() -> MobileCoreSnapshot? { nil }
}
#endif

struct MobileCoreStateEnvelope: Decodable {
    let ok: Bool
    let value: MobileCoreState
}

struct MobileCoreState: Decodable {
    let screen: String
    let connectionState: String
    let statusMessage: String?
    let errorMessage: String?
    let messages: [MobileCoreMessage]
    let activeSessionId: String?
    let sessions: [String]
    let availableModels: [String]
    let modelName: String?
    let isProcessing: Bool
    let pendingApprovals: [MobileCoreApproval]

    enum CodingKeys: String, CodingKey {
        case screen
        case connectionState = "connection_state"
        case statusMessage = "status_message"
        case errorMessage = "error_message"
        case messages
        case activeSessionId = "active_session_id"
        case sessions
        case availableModels = "available_models"
        case modelName = "model_name"
        case isProcessing = "is_processing"
        case pendingApprovals = "pending_approvals"
    }
}

struct MobileCoreMessage: Decodable {
    let id: String
    let role: String
    let text: String
}

struct MobileCoreApproval: Decodable, Equatable, Identifiable {
    let id: String
    let commandSummary: String
    let risk: String

    enum CodingKeys: String, CodingKey {
        case id
        case commandSummary = "command_summary"
        case risk
    }
}

struct MobileCoreSnapshot {
    let state: MobileCoreState

    var summary: String {
        [
            state.connectionState,
            "\(state.messages.count) messages",
            state.modelName ?? "no model",
            "\(state.pendingApprovals.count) approvals",
        ].joined(separator: " - ")
    }
}

extension String {
    var jsonEscapedForMobileCore: String {
        let data = try? JSONEncoder().encode(self)
        return data.flatMap { String(data: $0, encoding: .utf8) } ?? #""""#
    }
}
