import Foundation
import Combine
import JCodeKit

#if canImport(UIKit)
import UIKit
#endif

@MainActor
final class AppModel: ObservableObject {
    let notifications = NotificationBridge()
    private let mobileCore = MobileCoreBridge()

    enum ConnectionState: Equatable {
        case disconnected
        case connecting
        case connected
    }

    struct ChatEntry: Identifiable, Equatable {
        let id: UUID
        let role: Role
        var text: String
        var toolCalls: [ToolCallInfo]
        var images: [(String, String)]

        enum Role: String {
            case user
            case assistant
            case system
        }

        init(id: UUID = UUID(), role: Role, text: String, toolCalls: [ToolCallInfo] = [], images: [(String, String)] = []) {
            self.id = id
            self.role = role
            self.text = text
            self.toolCalls = toolCalls
            self.images = images
        }

        static func == (lhs: ChatEntry, rhs: ChatEntry) -> Bool {
            lhs.id == rhs.id && lhs.text == rhs.text && lhs.toolCalls.count == rhs.toolCalls.count
        }
    }

    private func imagesEqual(_ lhs: [(String, String)], _ rhs: [(String, String)]) -> Bool {
        guard lhs.count == rhs.count else { return false }
        return zip(lhs, rhs).allSatisfy { left, right in
            left.0 == right.0 && left.1 == right.1
        }
    }

    @Published var connectionState: ConnectionState = .disconnected
    @Published var isProcessing: Bool = false
    @Published var availableModels: [String] = []
    @Published var savedServers: [ServerCredential] = []
    @Published var selectedServer: ServerCredential? {
        didSet {
            guard let selectedServer else {
                UserDefaults.standard.removeObject(forKey: "jcode.selected.host")
                UserDefaults.standard.removeObject(forKey: "jcode.selected.port")
                return
            }
            UserDefaults.standard.set(selectedServer.host, forKey: "jcode.selected.host")
            UserDefaults.standard.set(Int(selectedServer.port), forKey: "jcode.selected.port")

            if connectionState == .disconnected {
                hostInput = selectedServer.host
                portInput = String(selectedServer.port)
            }
        }
    }

    @Published var hostInput: String = ""
    @Published var portInput: String = "7643"
    @Published var pairCodeInput: String = ""
    @Published var deviceNameInput: String = {
        #if canImport(UIKit)
        return UIDevice.current.name
        #else
        return Host.current().localizedName ?? "Mac"
        #endif
    }()

    @Published var statusMessage: String?
    @Published var errorMessage: String?

    @Published var messages: [ChatEntry] = []
    @Published var draftMessage: String = ""
    @Published var activeSessionId: String = ""
    @Published var sessions: [String] = []
    @Published var serverName: String = ""
    @Published var serverVersion: String = ""
    @Published var modelName: String = ""
    @Published var gatewayHealthStatus: String = "Not checked"
    @Published var gatewayHealthVersion: String = ""
    @Published var gatewayHealthCheckedAt: Date?
    @Published var rustCoreReducerStatus: String = "Not checked"
    @Published var pendingApprovals: [MobileCoreApproval] = []

    private let credentialStore = CredentialStore()
    private var client: JCodeClient?
    private var clientDelegate: ClientDelegate?
    private var reconnecting = false
    private var shouldAutoReconnect = false
    private var connectionGeneration: UInt64 = 0
    private var reconnectAttempt: Int = 0
    private let maxReconnectBackoff: TimeInterval = 30

    private var lastAssistantMessageId: UUID?
    private var lastAssistantIndex: Int?
    private var inFlightTools: [String: ToolCallInfo] = [:]
    private var lastToolId: String?
    private var toolMessageIndex: [String: Int] = [:]
    private var toolSubIndex: [String: Int] = [:]

    private let deviceId: String = {
        if let existing = UserDefaults.standard.string(forKey: "jcode.device.id") {
            return existing
        }
        let generated = "ios-" + UUID().uuidString.lowercased()
        UserDefaults.standard.set(generated, forKey: "jcode.device.id")
        return generated
    }()

    private func activeSessionDefaultsKey(for credential: ServerCredential? = nil) -> String? {
        guard let credential = credential ?? selectedServer else { return nil }
        return "jcode.selected.session.\(credential.host).\(credential.port)"
    }

    private func rememberedSessionId(for credential: ServerCredential? = nil) -> String? {
        guard let key = activeSessionDefaultsKey(for: credential) else { return nil }
        let value = UserDefaults.standard.string(forKey: key)?.trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
        return value.isEmpty ? nil : value
    }

    private func rememberSessionId(_ sessionId: String, for credential: ServerCredential? = nil) {
        let trimmed = sessionId.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty, let key = activeSessionDefaultsKey(for: credential) else { return }
        UserDefaults.standard.set(trimmed, forKey: key)
    }

    func loadSavedServers() async {
        await notifications.refreshAuthorizationStatus()
        syncRustCoreDiagnostics()
        let all = await credentialStore.all()
        let creds = all.sorted {
            if $0.host == $1.host {
                return $0.port < $1.port
            }
            return $0.host < $1.host
        }

        savedServers = creds

        let rememberedHost = UserDefaults.standard.string(forKey: "jcode.selected.host")
        let rememberedPort = UserDefaults.standard.integer(forKey: "jcode.selected.port")

        if let selected = selectedServer {
            let exists = creds.contains(where: { $0.host == selected.host && $0.port == selected.port })
            if !exists {
                selectedServer = nil
            }
        }

        if selectedServer == nil,
           let rememberedHost,
           rememberedPort > 0,
           let remembered = creds.first(where: { $0.host == rememberedHost && Int($0.port) == rememberedPort }) {
            selectedServer = remembered
        }

        if selectedServer == nil {
            selectedServer = creds.first
        }
    }

    func parsePort() -> UInt16? {
        guard let value = UInt16(portInput.trimmingCharacters(in: .whitespacesAndNewlines)) else {
            return nil
        }
        return value
    }

    func probeServer() async {
        clearTransientMessages()

        guard let port = parsePort() else {
            errorMessage = "Port must be a number from 0 to 65535."
            return
        }

        let host = hostInput.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !host.isEmpty else {
            errorMessage = "Host cannot be empty."
            return
        }

        let client = PairingClient(host: host, port: port)
        do {
            let response = try await client.checkHealth()
            statusMessage = "Server reachable: \(response.version)"
            errorMessage = nil
        } catch {
            errorMessage = "Unable to reach server. Verify Tailscale and gateway settings."
        }
    }

    func refreshGatewayDiagnostics() async {
        let targetHost: String
        let targetPort: UInt16

        if let selectedServer {
            targetHost = selectedServer.host
            targetPort = selectedServer.port
        } else {
            guard let port = parsePort() else {
                gatewayHealthStatus = "Port must be a number from 0 to 65535."
                gatewayHealthVersion = ""
                gatewayHealthCheckedAt = Date()
                return
            }
            let host = hostInput.trimmingCharacters(in: .whitespacesAndNewlines)
            guard !host.isEmpty else {
                gatewayHealthStatus = "Host cannot be empty."
                gatewayHealthVersion = ""
                gatewayHealthCheckedAt = Date()
                return
            }
            targetHost = host
            targetPort = port
        }

        gatewayHealthStatus = "Checking \(targetHost):\(targetPort)..."
        gatewayHealthVersion = ""
        gatewayHealthCheckedAt = Date()

        do {
            let response = try await PairingClient(host: targetHost, port: targetPort).checkHealth()
            gatewayHealthStatus = response.gateway
                ? "Gateway reachable"
                : "Server reachable, gateway flag missing"
            gatewayHealthVersion = response.version
            gatewayHealthCheckedAt = Date()
        } catch {
            gatewayHealthStatus = "Gateway unreachable. Check host, port, network, and jcode serve."
            gatewayHealthVersion = ""
            gatewayHealthCheckedAt = Date()
        }
    }

    func pairAndSave() async {
        clearTransientMessages()

        guard let port = parsePort() else {
            errorMessage = "Port must be a number from 0 to 65535."
            return
        }

        let host = hostInput.trimmingCharacters(in: .whitespacesAndNewlines)
        let code = pairCodeInput.trimmingCharacters(in: .whitespacesAndNewlines)
        let deviceName = deviceNameInput.trimmingCharacters(in: .whitespacesAndNewlines)

        guard !host.isEmpty else {
            errorMessage = "Host cannot be empty."
            return
        }
        guard !code.isEmpty else {
            errorMessage = "Enter the 6-digit pairing code from jcode pair."
            return
        }
        guard !deviceName.isEmpty else {
            errorMessage = "Device name cannot be empty."
            return
        }

        do {
            let pairClient = PairingClient(host: host, port: port)
            let response = try await pairClient.pair(code: code, deviceId: deviceId, deviceName: deviceName)

            let credential = ServerCredential(
                host: host,
                port: port,
                authToken: response.token,
                serverName: response.serverName,
                serverVersion: response.serverVersion,
                deviceId: deviceId,
                pairedAt: Date()
            )

            try await credentialStore.save(credential)
            await loadSavedServers()
            selectedServer = credential
            statusMessage = "Paired with \(response.serverName) (\(response.serverVersion)). Connecting..."
            pairCodeInput = ""
            errorMessage = nil
            await connectSelected()
        } catch let error as PairingError {
            switch error {
            case .serverUnreachable:
                errorMessage = "Server unreachable. Confirm host/port and gateway status."
            case .invalidCode(let message):
                errorMessage = message
            case .serverError(let message):
                errorMessage = message
            }
        } catch {
            errorMessage = "Pairing failed: \(error.localizedDescription)"
        }
    }

    func deleteServer(_ credential: ServerCredential) async {
        if connectionState != .disconnected,
           selectedServer?.host == credential.host,
           selectedServer?.port == credential.port {
            await disconnect()
        }

        do {
            try await credentialStore.remove(host: credential.host, port: credential.port)
            if selectedServer?.host == credential.host && selectedServer?.port == credential.port {
                selectedServer = nil
            }
            await loadSavedServers()
            statusMessage = "Removed \(credential.host):\(credential.port)"
        } catch {
            errorMessage = "Failed to remove server: \(error.localizedDescription)"
        }
    }

    fileprivate func markNewGeneration() -> UInt64 {
        connectionGeneration &+= 1
        return connectionGeneration
    }

    fileprivate func isCurrentGeneration(_ generation: UInt64) -> Bool {
        connectionGeneration == generation
    }

    func connectSelected() async {
        clearTransientMessages()

        guard let credential = selectedServer else {
            errorMessage = "Select a paired server first."
            return
        }

        let generation = markNewGeneration()

        if let current = client {
            shouldAutoReconnect = false
            reconnecting = false
            await current.disconnect()
        }
        client = nil
        clientDelegate = nil

        connectionState = .connecting
        shouldAutoReconnect = true
        reconnecting = false
        dispatchCoreConnectedIntent()

        let sessionToResume = activeSessionId.isEmpty ? rememberedSessionId(for: credential) : activeSessionId

        // Keep the existing transcript visible while reconnecting. iOS can suspend the
        // socket when the app backgrounds; clearing here makes it look like the phone
        // forgot the conversation before the server has a chance to replay history.
        inFlightTools.removeAll()
        lastToolId = nil
        lastAssistantMessageId = nil
        lastAssistantIndex = nil
        toolMessageIndex.removeAll()
        toolSubIndex.removeAll()
        if let sessionToResume {
            activeSessionId = sessionToResume
        }
        serverName = credential.serverName
        serverVersion = credential.serverVersion
        modelName = ""

        let newClient = JCodeClient(host: credential.host, port: credential.port, authToken: credential.authToken)
        let delegate = ClientDelegate(model: self, generation: generation)
        clientDelegate = delegate
        await newClient.setDelegate(delegate)

        do {
            try await newClient.connect(resumeSessionId: sessionToResume)
            client = newClient
            connectionState = .connected
            reconnecting = false
            statusMessage = "Connected to \(credential.host):\(credential.port)"
            dispatchCore(action: #"{"type":"connected","session_id":"\#(activeSessionId.isEmpty ? "pending" : activeSessionId)"}"#)
            try? await newClient.refreshApprovals()
        } catch {
            connectionState = .disconnected
            shouldAutoReconnect = false
            reconnecting = false
            clientDelegate = nil
            errorMessage = "Connect failed: \(error.localizedDescription)"
            dispatchCore(action: #"{"type":"connection_failed","message":\#(error.localizedDescription.jsonEscapedForMobileCore)}"#)
        }
    }

    func disconnect() async {
        _ = markNewGeneration()
        shouldAutoReconnect = false
        reconnecting = false

        guard let client else {
            connectionState = .disconnected
            return
        }

        await client.disconnect()
        self.client = nil
        self.clientDelegate = nil
        connectionState = .disconnected
        statusMessage = "Disconnected"
        dispatchCore(action: #"{"type":"disconnected","message":null,"should_reconnect":false}"#)
    }

    @discardableResult
    func sendDraft(images: [(String, String)] = []) async -> Bool {
        clearTransientMessages()

        if connectionState != .connected {
            guard selectedServer != nil else {
                errorMessage = "Not connected."
                return false
            }
            statusMessage = "Reconnecting before send..."
            await connectSelected()
        }

        guard connectionState == .connected else {
            errorMessage = "Not connected. Tap Connect in Settings or check the Mac gateway."
            return false
        }

        let trimmed = draftMessage.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !trimmed.isEmpty || !images.isEmpty else {
            return false
        }
        dispatchCore(action: #"{"type":"set_draft","value":\#(trimmed.jsonEscapedForMobileCore)}"#)

        guard let client else {
            errorMessage = "Not connected."
            return false
        }

        let isInterleaving = isProcessing

        if isInterleaving && !images.isEmpty {
            errorMessage = "Please wait for the current run to finish before sending images."
            return false
        }

        if isInterleaving && trimmed.isEmpty {
            errorMessage = "Type a message to send an update while the agent is running."
            return false
        }

        do {
            messages.append(ChatEntry(role: .user, text: trimmed, images: images))
            draftMessage = ""

            if isInterleaving {
                statusMessage = "Will update the current run at the next safe point."
                try await client.interrupt(trimmed)
            } else {
                let assistantPlaceholder = ChatEntry(role: .assistant, text: "")
                messages.append(assistantPlaceholder)
                lastAssistantMessageId = assistantPlaceholder.id

                if images.isEmpty {
                    dispatchCore(action: #"{"type":"tap_node","node_id":"chat.send"}"#)
                    try await client.send(trimmed)
                } else {
                    dispatchCore(action: #"{"type":"tap_node","node_id":"chat.send"}"#)
                    try await client.send(trimmed, images: images)
                }
            }
            return true
        } catch {
            if !isInterleaving {
                if let id = lastAssistantMessageId,
                   let idx = messages.firstIndex(where: { $0.id == id }) {
                    messages.remove(at: idx)
                }
                lastAssistantMessageId = messages.last(where: { $0.role == .assistant })?.id
            }
            if let idx = messages.lastIndex(where: { $0.role == .user && $0.text == trimmed && imagesEqual($0.images, images) }) {
                messages.remove(at: idx)
            }
            errorMessage = isInterleaving
                ? "Could not update the running agent: \(error.localizedDescription)"
                : "Send failed: \(error.localizedDescription)"
            dispatchCore(action: #"{"type":"connection_failed","message":\#(errorMessage?.jsonEscapedForMobileCore ?? #""""#)}"#)
            return false
        }
    }

    func refreshHistory() async {
        guard let client else {
            errorMessage = "Not connected."
            return
        }

        do {
            try await client.refreshHistory()
        } catch {
            errorMessage = "Could not refresh history: \(error.localizedDescription)"
        }
    }

    func cancelGeneration() async {
        guard let client else { return }
        do {
            try await client.cancel()
        } catch {
            errorMessage = "Cancel failed: \(error.localizedDescription)"
        }
    }

    func interruptAgent(_ message: String, urgent: Bool = false) async {
        guard let client else { return }
        do {
            try await client.interrupt(message, urgent: urgent)
        } catch {
            errorMessage = "Interrupt failed: \(error.localizedDescription)"
        }
    }

    func submitApproval(_ approval: MobileCoreApproval, approved: Bool) async {
        guard let client else {
            errorMessage = "Not connected."
            return
        }

        let decisionNode = approved ? "approve" : "deny"
        let nodeId = "approval.\(approval.id).\(decisionNode)"
        dispatchCore(action: #"{"type":"tap_node","node_id":\#(nodeId.jsonEscapedForMobileCore)}"#)

        do {
            try await client.submitApproval(
                requestId: approval.id,
                approved: approved,
                reason: approved ? nil : "Denied from iPhone"
            )
            statusMessage = approved ? "Approval sent." : "Approval denied."
            errorMessage = nil
            try? await client.refreshApprovals()
        } catch {
            errorMessage = "Approval failed: \(error.localizedDescription)"
        }
    }

    func changeModel(_ model: String) async {
        guard let client else { return }
        dispatchCore(action: #"{"type":"set_model","model":\#(model.jsonEscapedForMobileCore)}"#)
        do {
            try await client.changeModel(model)
        } catch {
            errorMessage = "Model change failed: \(error.localizedDescription)"
        }
    }

    func switchToSession(_ sessionId: String) async {
        guard !sessionId.isEmpty else {
            return
        }

        guard let client else {
            errorMessage = "Not connected."
            return
        }

        do {
            dispatchCore(action: #"{"type":"switch_session","session_id":\#(sessionId.jsonEscapedForMobileCore)}"#)
            try await client.switchSession(sessionId)
            activeSessionId = sessionId
            rememberSessionId(sessionId)
            statusMessage = "Switched to \(sessionId)"
            // History will be refreshed by server event.
        } catch {
            errorMessage = "Session switch failed: \(error.localizedDescription)"
        }
    }

    private func applyConnectedServerInfo(_ info: ServerInfo) {
        activeSessionId = info.sessionId
        rememberSessionId(info.sessionId)
        sessions = info.allSessions
        serverName = info.serverName ?? "jcode"
        serverVersion = info.serverVersion ?? ""
        modelName = info.providerModel ?? ""
        availableModels = info.availableModels
    }

    private func applyHistory(_ history: [HistoryMessage]) {
        var mapped: [ChatEntry] = []
        mapped.reserveCapacity(history.count)
        for item in history {
            let role: ChatEntry.Role
            switch item.role {
            case "assistant":
                role = .assistant
            case "system":
                role = .system
            default:
                role = .user
            }

            var toolCalls: [ToolCallInfo] = []
            if let tool = item.toolData, let id = tool.id, let name = tool.name {
                var info = ToolCallInfo(id: id, name: name)
                info.input = tool.input ?? ""
                info.output = tool.output
                info.state = .done
                toolCalls.append(info)
            }

            mapped.append(ChatEntry(role: role, text: item.content, toolCalls: toolCalls))
        }
        messages = mapped
        lastAssistantMessageId = messages.last(where: { $0.role == .assistant })?.id
        if let id = lastAssistantMessageId {
            lastAssistantIndex = messages.lastIndex(where: { $0.id == id })
        } else {
            lastAssistantIndex = nil
        }
        inFlightTools.removeAll()
        lastToolId = nil
        toolMessageIndex.removeAll()
        toolSubIndex.removeAll()
    }

    private func appendAssistantChunk(_ delta: String) {
        if let idx = lastAssistantIndex, idx < messages.count,
           messages[idx].id == lastAssistantMessageId {
            messages[idx].text += delta
            return
        }
        if let id = lastAssistantMessageId,
           let idx = messages.firstIndex(where: { $0.id == id }) {
            lastAssistantIndex = idx
            messages[idx].text += delta
            return
        }

        let entry = ChatEntry(role: .assistant, text: delta)
        messages.append(entry)
        lastAssistantMessageId = entry.id
        lastAssistantIndex = messages.count - 1
    }

    private func replaceAssistantText(_ text: String) {
        if let idx = lastAssistantIndex, idx < messages.count,
           messages[idx].id == lastAssistantMessageId {
            messages[idx].text = text
            return
        }
        if let id = lastAssistantMessageId,
           let idx = messages.firstIndex(where: { $0.id == id }) {
            lastAssistantIndex = idx
            messages[idx].text = text
            return
        }

        let entry = ChatEntry(role: .assistant, text: text)
        messages.append(entry)
        lastAssistantMessageId = entry.id
        lastAssistantIndex = messages.count - 1
    }

    private func attachTool(_ tool: ToolCallInfo) {
        inFlightTools[tool.id] = tool
        lastToolId = tool.id

        if let idx = lastAssistantIndex, idx < messages.count,
           messages[idx].id == lastAssistantMessageId {
            messages[idx].toolCalls.append(tool)
            toolMessageIndex[tool.id] = idx
            toolSubIndex[tool.id] = messages[idx].toolCalls.count - 1
        } else if let id = lastAssistantMessageId,
                  let idx = messages.firstIndex(where: { $0.id == id }) {
            lastAssistantIndex = idx
            messages[idx].toolCalls.append(tool)
            toolMessageIndex[tool.id] = idx
            toolSubIndex[tool.id] = messages[idx].toolCalls.count - 1
        } else {
            let entry = ChatEntry(role: .assistant, text: "", toolCalls: [tool])
            messages.append(entry)
            lastAssistantMessageId = entry.id
            lastAssistantIndex = messages.count - 1
            toolMessageIndex[tool.id] = messages.count - 1
            toolSubIndex[tool.id] = 0
        }
    }

    private func updateLatestTool(_ toolId: String, _ mutate: (inout ToolCallInfo) -> Void) {
        guard var tool = inFlightTools[toolId] else {
            return
        }

        mutate(&tool)
        inFlightTools[toolId] = tool

        if let msgIdx = toolMessageIndex[toolId], msgIdx < messages.count,
           let tIdx = toolSubIndex[toolId], tIdx < messages[msgIdx].toolCalls.count,
           messages[msgIdx].toolCalls[tIdx].id == toolId {
            messages[msgIdx].toolCalls[tIdx] = tool
            return
        }

        for msgIdx in messages.indices {
            if let toolIdx = messages[msgIdx].toolCalls.firstIndex(where: { $0.id == toolId }) {
                toolMessageIndex[toolId] = msgIdx
                toolSubIndex[toolId] = toolIdx
                messages[msgIdx].toolCalls[toolIdx] = tool
                break
            }
        }
    }

    private func clearTransientMessages() {
        statusMessage = nil
        errorMessage = nil
    }

    private func syncRustCoreDiagnostics() {
        guard mobileCore.isLinked else {
            rustCoreReducerStatus = "Rust reducer not linked"
            pendingApprovals = []
            return
        }
        if let snapshot = mobileCore.state() {
            applyCoreSnapshot(snapshot)
        } else {
            rustCoreReducerStatus = "Rust reducer state unavailable"
        }
    }

    private func dispatchCore(action: String) {
        guard mobileCore.isLinked else {
            rustCoreReducerStatus = "Rust reducer not linked"
            pendingApprovals = []
            return
        }
        if let snapshot = mobileCore.dispatch(action) {
            applyCoreSnapshot(snapshot)
        } else {
            rustCoreReducerStatus = "Rust reducer dispatch failed"
        }
    }

    private func applyCoreSnapshot(_ snapshot: MobileCoreSnapshot) {
        rustCoreReducerStatus = snapshot.summary
        pendingApprovals = snapshot.state.pendingApprovals
    }

    private func dispatchCoreConnectedIntent() {
        guard let credential = selectedServer else { return }
        dispatchCore(action: #"{"type":"set_host","value":\#(credential.host.jsonEscapedForMobileCore)}"#)
        dispatchCore(action: #"{"type":"set_port","value":\#(String(credential.port).jsonEscapedForMobileCore)}"#)
    }

    fileprivate func onConnected(_ info: ServerInfo) {
        connectionState = .connected
        reconnecting = false
        reconnectAttempt = 0
        applyConnectedServerInfo(info)
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"history","session_id":\#(info.sessionId.jsonEscapedForMobileCore),"messages":[],"server_name":\#((info.serverName ?? "jcode").jsonEscapedForMobileCore),"server_version":\#((info.serverVersion ?? "").jsonEscapedForMobileCore),"provider_model":\#((info.providerModel ?? "").jsonEscapedForMobileCore),"available_models":\#(jsonArray(info.availableModels)),"all_sessions":\#(jsonArray(info.allSessions))}}"#)
    }

    fileprivate func onDisconnected(error: String?) {
        client = nil
        clientDelegate = nil
        connectionState = .disconnected
        inFlightTools.removeAll()
        lastToolId = nil
        lastAssistantMessageId = nil
        lastAssistantIndex = nil
        toolMessageIndex.removeAll()
        toolSubIndex.removeAll()

        if let error, !error.isEmpty {
            errorMessage = error
        }
        dispatchCore(action: #"{"type":"disconnected","message":\#((error ?? "").isEmpty ? "null" : error!.jsonEscapedForMobileCore),"should_reconnect":\#(shouldAutoReconnect ? "true" : "false")}"#)

        guard shouldAutoReconnect else {
            reconnecting = false
            return
        }

        if reconnecting {
            return
        }

        reconnecting = true
        let attempt = reconnectAttempt
        reconnectAttempt += 1
        let baseDelay = min(pow(2.0, Double(attempt)), maxReconnectBackoff)
        let jitter = Double.random(in: 0...1)
        let delay = baseDelay + jitter
        statusMessage = "Reconnecting in \(Int(delay))s..."

        Task {
            try? await Task.sleep(for: .seconds(delay))
            guard reconnecting, shouldAutoReconnect else { return }
            await connectSelected()
        }
    }

    fileprivate func onTextDelta(_ text: String) {
        isProcessing = true
        appendAssistantChunk(text)
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"text_delta","text":\#(text.jsonEscapedForMobileCore)}}}"#)
    }

    fileprivate func onTextReplace(_ text: String) {
        replaceAssistantText(text)
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"text_replace","text":\#(text.jsonEscapedForMobileCore)}}}"#)
    }

    fileprivate func onInterrupted(_ interrupt: InterruptInfo) {
        isProcessing = false

        if let id = lastAssistantMessageId,
           let idx = messages.firstIndex(where: { $0.id == id }),
           messages[idx].text.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty,
           messages[idx].toolCalls.isEmpty {
            messages.remove(at: idx)
        }

        messages.append(ChatEntry(role: .system, text: interrupt.message))
        notifications.notify(
            title: "jcode interrupted",
            body: interrupt.message,
            identifier: "jcode.interrupted.\(UUID().uuidString)"
        )

        inFlightTools.removeAll()
        lastToolId = nil
        lastAssistantMessageId = nil
    }

    fileprivate func onSoftInterruptInjected(_ info: SoftInterruptInjectionInfo) {
        if let skipped = info.toolsSkipped, skipped > 0 {
            statusMessage = "Updated current run. Skipped \(skipped) remaining tool(s)."
        } else {
            statusMessage = "Updated current run."
        }
    }

    fileprivate func onReloading(newSocket _: String?) {
        isProcessing = false
        statusMessage = "Server reloading. Reconnecting..."
        errorMessage = nil
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"reloading"}}"#)
    }

    fileprivate func onReloadProgress(step: String, message: String, success: Bool?, output: String?) {
        if success == false {
            errorMessage = message
        } else {
            statusMessage = message
        }

        var fields = [
            #""type":"reload_progress""#,
            #""step":\#(step.jsonEscapedForMobileCore)"#,
            #""message":\#(message.jsonEscapedForMobileCore)"#,
        ]
        if let success {
            fields.append(#""success":\#(success ? "true" : "false")"#)
        }
        if let output {
            fields.append(#""output":\#(output.jsonEscapedForMobileCore)"#)
        }
        dispatchCore(action: #"{"type":"apply_server_event","event":{\#(fields.joined(separator: ","))}}"#)
    }

    fileprivate func onToolStart(_ tool: ToolCallInfo) {
        isProcessing = true
        attachTool(tool)
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"tool_start","id":\#(tool.id.jsonEscapedForMobileCore),"name":\#(tool.name.jsonEscapedForMobileCore)}}}"#)
    }

    fileprivate func onToolInput(_ delta: String) {
        guard let toolId = lastToolId else {
            return
        }
        updateLatestTool(toolId) { tool in
            tool.input += delta
            tool.state = .streaming
        }
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"tool_input","delta":\#(delta.jsonEscapedForMobileCore)}}}"#)
    }

    fileprivate func onToolExec(id: String, name _: String) {
        updateLatestTool(id) { tool in
            tool.state = .executing
        }
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"tool_exec","id":\#(id.jsonEscapedForMobileCore),"name":"tool"}}"#)
    }

    fileprivate func onToolDone(id: String, name _: String, output: String, error: String?) {
        updateLatestTool(id) { tool in
            tool.output = output
            tool.error = error
            tool.state = error == nil ? .done : .failed
        }
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"tool_done","id":\#(id.jsonEscapedForMobileCore),"name":"tool","output":\#(output.jsonEscapedForMobileCore),"error":\#(error?.jsonEscapedForMobileCore ?? "null")}}}"#)
    }

    fileprivate func onTurnDone(id _: UInt64) {
        isProcessing = false
        inFlightTools.removeAll()
        lastToolId = nil
        lastAssistantMessageId = nil
        lastAssistantIndex = nil
        toolMessageIndex.removeAll()
        toolSubIndex.removeAll()
        notifications.notify(
            title: "jcode finished",
            body: activeSessionId.isEmpty ? "The current run finished." : "Session \(activeSessionId) finished.",
            identifier: "jcode.done.\(UUID().uuidString)"
        )
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"done","id":0}}"#)
    }

    fileprivate func onServerError(id _: UInt64, message: String) {
        errorMessage = message
        notifications.notify(
            title: "jcode needs attention",
            body: message,
            identifier: "jcode.error.\(UUID().uuidString)"
        )
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"error","id":0,"message":\#(message.jsonEscapedForMobileCore)}}}"#)
    }

    fileprivate func onModelChanged(model: String, provider _: String?) {
        modelName = model
        statusMessage = "Model: \(model)"
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"model_changed","id":0,"model":\#(model.jsonEscapedForMobileCore)}}}"#)
    }

    fileprivate func onHistory(_ history: [HistoryMessage]) {
        applyHistory(history)
        let messageJson = history.map { item in
            #"{"role":\#(item.role.jsonEscapedForMobileCore),"content":\#(item.content.jsonEscapedForMobileCore)}"#
        }.joined(separator: ",")
        dispatchCore(action: #"{"type":"apply_server_event","event":{"type":"history","session_id":\#(activeSessionId.jsonEscapedForMobileCore),"messages":[\#(messageJson)],"available_models":\#(jsonArray(availableModels)),"all_sessions":\#(jsonArray(sessions))}}"#)
    }

    fileprivate func onApprovals(_ approvals: [ApprovalRequestPayload]) {
        pendingApprovals = approvals.map(MobileCoreApproval.init(payload:))
        for approval in approvals {
            let risk = approval.risk.lowercased()
            let coreRisk: String
            switch risk {
            case "high": coreRisk = "high"
            case "low": coreRisk = "low"
            default: coreRisk = "medium"
            }
            dispatchCore(action: #"{"type":"approval_requested","request":{"id":\#(approval.id.jsonEscapedForMobileCore),"command_summary":\#(approval.commandSummary.jsonEscapedForMobileCore),"risk":"\#(coreRisk)"}}"#)
        }
    }
}

private func jsonArray(_ values: [String]) -> String {
    "[" + values.map(\.jsonEscapedForMobileCore).joined(separator: ",") + "]"
}

@MainActor
private final class ClientDelegate: JCodeClientDelegate {
    unowned let model: AppModel
    let generation: UInt64

    init(model: AppModel, generation: UInt64) {
        self.model = model
        self.generation = generation
    }

    private func guardCurrent() -> Bool {
        model.isCurrentGeneration(generation)
    }

    func clientDidConnect(serverInfo: ServerInfo) {
        guard guardCurrent() else { return }
        model.onConnected(serverInfo)
    }

    func clientDidDisconnect(error: String?) {
        guard guardCurrent() else { return }
        model.onDisconnected(error: error)
    }

    func clientDidReceiveText(_ text: String) {
        guard guardCurrent() else { return }
        model.onTextDelta(text)
    }

    func clientDidReplaceText(_ text: String) {
        guard guardCurrent() else { return }
        model.onTextReplace(text)
    }

    func clientDidStartTool(_ tool: ToolCallInfo) {
        guard guardCurrent() else { return }
        model.onToolStart(tool)
    }

    func clientDidReceiveToolInput(_ delta: String) {
        guard guardCurrent() else { return }
        model.onToolInput(delta)
    }

    func clientDidExecuteTool(id: String, name: String) {
        guard guardCurrent() else { return }
        model.onToolExec(id: id, name: name)
    }

    func clientDidFinishTool(id: String, name: String, output: String, error: String?) {
        guard guardCurrent() else { return }
        model.onToolDone(id: id, name: name, output: output, error: error)
    }

    func clientDidFinishTurn(id: UInt64) {
        guard guardCurrent() else { return }
        model.onTurnDone(id: id)
    }

    func clientDidReceiveError(id: UInt64, message: String) {
        guard guardCurrent() else { return }
        model.onServerError(id: id, message: message)
    }

    func clientDidUpdateTokens(_ update: TokenUpdate) {
        guard guardCurrent() else { return }
        _ = update
    }

    func clientDidChangeModel(model: String, provider: String?) {
        guard guardCurrent() else { return }
        self.model.onModelChanged(model: model, provider: provider)
    }

    func clientDidReceiveHistory(messages: [HistoryMessage]) {
        guard guardCurrent() else { return }
        model.onHistory(messages)
    }

    func clientDidInterrupt(_ interrupt: InterruptInfo) {
        guard guardCurrent() else { return }
        model.onInterrupted(interrupt)
    }

    func clientDidInjectSoftInterrupt(_ info: SoftInterruptInjectionInfo) {
        guard guardCurrent() else { return }
        model.onSoftInterruptInjected(info)
    }

    func clientDidUpdateApprovals(_ approvals: [ApprovalRequestPayload]) {
        guard guardCurrent() else { return }
        model.onApprovals(approvals)
    }

    func clientDidStartReload(newSocket: String?) {
        guard guardCurrent() else { return }
        model.onReloading(newSocket: newSocket)
    }

    func clientDidUpdateReloadProgress(step: String, message: String, success: Bool?, output: String?) {
        guard guardCurrent() else { return }
        model.onReloadProgress(step: step, message: message, success: success, output: output)
    }
}
