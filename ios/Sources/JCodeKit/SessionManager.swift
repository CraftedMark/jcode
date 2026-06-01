import Foundation

public struct SessionInfo: Sendable {
    public let sessionId: String
    public let friendlyName: String?
}

public actor SessionManager {
    private let connection: JCodeConnection
    private var currentSessionId: String?
    private var allSessions: [String] = []
    private var sessionSummaries: [SessionSummary] = []

    public init(connection: JCodeConnection) {
        self.connection = connection
    }

    public var activeSessionId: String? { currentSessionId }
    public var sessions: [String] { allSessions }
    public var summaries: [SessionSummary] { sessionSummaries }

    public func setActiveSession(_ sessionId: String) {
        currentSessionId = sessionId
    }

    public func updateSessions(from payload: HistoryPayload) {
        currentSessionId = payload.sessionId
        allSessions = payload.allSessions
        sessionSummaries = payload.sessionSummaries
    }

    public func switchSession(_ sessionId: String) async throws {
        try await connection.resumeSession(sessionId)
        currentSessionId = sessionId
    }
}
