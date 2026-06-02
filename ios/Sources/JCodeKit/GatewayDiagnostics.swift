import Foundation

public enum GatewayDiagnosticPhase: String, Codable, Sendable, Equatable {
    case health
    case pairing
    case webSocket
    case knowledge
}

public enum GatewayDiagnosticFailureKind: String, Codable, Sendable, Equatable {
    case dns
    case portClosed
    case timedOut
    case offline
    case auth
    case server
    case invalidResponse
    case unknown
}

public struct GatewayDiagnosticFailure: Error, Sendable, Equatable, LocalizedError {
    public let kind: GatewayDiagnosticFailureKind
    public let phase: GatewayDiagnosticPhase
    public let host: String
    public let port: UInt16
    public let statusCode: Int?
    public let underlyingMessage: String?

    public init(
        kind: GatewayDiagnosticFailureKind,
        phase: GatewayDiagnosticPhase,
        host: String,
        port: UInt16,
        statusCode: Int? = nil,
        underlyingMessage: String? = nil
    ) {
        self.kind = kind
        self.phase = phase
        self.host = host
        self.port = port
        self.statusCode = statusCode
        self.underlyingMessage = underlyingMessage
    }

    public var errorDescription: String? {
        userMessage
    }

    public var userMessage: String {
        switch kind {
        case .dns:
            "Cannot resolve \(host). Check the host name, Tailscale, or LAN DNS."
        case .portClosed:
            "No gateway is listening at \(host):\(port). Start `jcode serve` or check the port."
        case .timedOut:
            "Timed out reaching \(host):\(port). Check Tailscale, Wi-Fi, VPN, or firewall."
        case .offline:
            "Network is offline. Reconnect Wi-Fi, cellular, or Tailscale and try again."
        case .auth:
            "Gateway rejected this device token. Re-pair the phone with `jcode pair`."
        case .server:
            if let statusCode {
                "Gateway returned HTTP \(statusCode). Check the desktop `jcode serve` logs."
            } else {
                "Gateway returned a server error. Check the desktop `jcode serve` logs."
            }
        case .invalidResponse:
            "Gateway responded, but not with the expected jcode health payload."
        case .unknown:
            underlyingMessage ?? "Gateway check failed for \(host):\(port)."
        }
    }

    public var shortLabel: String {
        switch kind {
        case .dns: "DNS / host failure"
        case .portClosed: "Port closed"
        case .timedOut: "Connection timed out"
        case .offline: "Network offline"
        case .auth: "Authentication failed"
        case .server: "Gateway server error"
        case .invalidResponse: "Unexpected gateway response"
        case .unknown: "Unknown gateway failure"
        }
    }

    public static func httpStatus(
        _ statusCode: Int,
        phase: GatewayDiagnosticPhase,
        host: String,
        port: UInt16,
        message: String? = nil
    ) -> GatewayDiagnosticFailure {
        let kind: GatewayDiagnosticFailureKind = switch statusCode {
        case 401, 403: .auth
        case 500...599: .server
        default: .invalidResponse
        }
        return GatewayDiagnosticFailure(
            kind: kind,
            phase: phase,
            host: host,
            port: port,
            statusCode: statusCode,
            underlyingMessage: message
        )
    }

    public static func classify(
        _ error: Error,
        phase: GatewayDiagnosticPhase,
        host: String,
        port: UInt16
    ) -> GatewayDiagnosticFailure {
        if let failure = error as? GatewayDiagnosticFailure {
            return failure
        }
        if let pairing = error as? PairingError {
            switch pairing {
            case .gatewayUnavailable(let failure):
                return failure
            case .serverUnreachable:
                return GatewayDiagnosticFailure(
                    kind: .unknown,
                    phase: phase,
                    host: host,
                    port: port,
                    underlyingMessage: "Gateway unreachable."
                )
            case .invalidCode(let message), .serverError(let message):
                return GatewayDiagnosticFailure(
                    kind: .server,
                    phase: phase,
                    host: host,
                    port: port,
                    underlyingMessage: message
                )
            }
        }
        if let urlError = error as? URLError {
            return classify(urlError, phase: phase, host: host, port: port)
        }

        let nsError = error as NSError
        if nsError.domain == NSURLErrorDomain {
            let code = URLError.Code(rawValue: nsError.code)
            return classify(URLError(code), phase: phase, host: host, port: port)
        }

        return GatewayDiagnosticFailure(
            kind: .unknown,
            phase: phase,
            host: host,
            port: port,
            underlyingMessage: error.localizedDescription
        )
    }

    private static func classify(
        _ error: URLError,
        phase: GatewayDiagnosticPhase,
        host: String,
        port: UInt16
    ) -> GatewayDiagnosticFailure {
        let kind: GatewayDiagnosticFailureKind = switch error.code {
        case .cannotFindHost, .dnsLookupFailed:
            .dns
        case .cannotConnectToHost:
            .portClosed
        case .timedOut:
            .timedOut
        case .notConnectedToInternet, .networkConnectionLost, .internationalRoamingOff, .dataNotAllowed:
            .offline
        case .userAuthenticationRequired, .userCancelledAuthentication, .clientCertificateRejected:
            .auth
        case .badServerResponse, .cannotParseResponse:
            .invalidResponse
        default:
            .unknown
        }
        return GatewayDiagnosticFailure(
            kind: kind,
            phase: phase,
            host: host,
            port: port,
            underlyingMessage: error.localizedDescription
        )
    }
}
