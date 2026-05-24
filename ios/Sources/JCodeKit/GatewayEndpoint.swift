import Foundation

struct GatewayEndpoint: Sendable {
    let host: String
    let port: UInt16

    init(host rawHost: String, port fallbackPort: UInt16 = 7643) {
        let trimmed = rawHost.trimmingCharacters(in: .whitespacesAndNewlines)
        let withScheme: String
        if trimmed.contains("://") {
            withScheme = trimmed
        } else {
            withScheme = "jcode://\(trimmed)"
        }

        if let components = URLComponents(string: withScheme),
           let parsedHost = components.host,
           !parsedHost.isEmpty {
            host = parsedHost
            if let parsedPort = components.port, let normalizedPort = UInt16(exactly: parsedPort) {
                port = normalizedPort
            } else {
                port = fallbackPort
            }
            return
        }

        let parts = trimmed.split(separator: ":", maxSplits: 1).map(String.init)
        host = parts.first?.isEmpty == false ? parts[0] : trimmed
        if parts.count == 2, let parsedPort = UInt16(parts[1]) {
            port = parsedPort
        } else {
            port = fallbackPort
        }
    }
}
