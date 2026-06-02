import Foundation

public struct PairResponse: Codable, Sendable {
    public let token: String
    public let serverName: String
    public let serverVersion: String

    enum CodingKeys: String, CodingKey {
        case token
        case serverName = "server_name"
        case serverVersion = "server_version"
    }
}

public struct PairError: Codable, Sendable {
    public let error: String
}

public struct HealthResponse: Codable, Sendable {
    public let status: String
    public let version: String
    public let gateway: Bool
}

public struct PairingClient: Sendable {
    public let host: String
    public let port: UInt16

    public init(host: String, port: UInt16 = 7643) {
        let endpoint = GatewayEndpoint(host: host, port: port)
        self.host = endpoint.host
        self.port = endpoint.port
    }

    private var baseURL: URL {
        var components = URLComponents()
        components.scheme = "http"
        components.host = host
        components.port = Int(port)
        return components.url!
    }

    private static let insecureSession: URLSession = {
        let config = URLSessionConfiguration.default
        return URLSession(configuration: config, delegate: InsecureDelegate.shared, delegateQueue: nil)
    }()

    public func checkHealth() async throws -> HealthResponse {
        let url = baseURL.appendingPathComponent("health")
        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await Self.insecureSession.data(from: url)
        } catch {
            throw PairingError.gatewayUnavailable(
                GatewayDiagnosticFailure.classify(error, phase: .health, host: host, port: port)
            )
        }
        guard let http = response as? HTTPURLResponse else {
            throw PairingError.gatewayUnavailable(
                GatewayDiagnosticFailure(
                    kind: .invalidResponse,
                    phase: .health,
                    host: host,
                    port: port
                )
            )
        }
        guard http.statusCode == 200 else {
            throw PairingError.gatewayUnavailable(
                GatewayDiagnosticFailure.httpStatus(
                    http.statusCode,
                    phase: .health,
                    host: host,
                    port: port
                )
            )
        }
        return try JSONDecoder().decode(HealthResponse.self, from: data)
    }

    public func pair(
        code: String,
        deviceId: String,
        deviceName: String,
        apnsToken: String? = nil
    ) async throws -> PairResponse {
        let url = baseURL.appendingPathComponent("pair")
        var request = URLRequest(url: url)
        request.httpMethod = "POST"
        request.setValue("application/json", forHTTPHeaderField: "Content-Type")

        var body: [String: String] = [
            "code": code,
            "device_id": deviceId,
            "device_name": deviceName,
        ]
        if let apns = apnsToken {
            body["apns_token"] = apns
        }
        request.httpBody = try JSONEncoder().encode(body)

        let data: Data
        let response: URLResponse
        do {
            (data, response) = try await Self.insecureSession.data(for: request)
        } catch {
            throw PairingError.gatewayUnavailable(
                GatewayDiagnosticFailure.classify(error, phase: .pairing, host: host, port: port)
            )
        }
        guard let http = response as? HTTPURLResponse else {
            throw PairingError.gatewayUnavailable(
                GatewayDiagnosticFailure(
                    kind: .invalidResponse,
                    phase: .pairing,
                    host: host,
                    port: port
                )
            )
        }

        switch http.statusCode {
        case 200:
            return try JSONDecoder().decode(PairResponse.self, from: data)
        case 401:
            let err = try? JSONDecoder().decode(PairError.self, from: data)
            throw PairingError.invalidCode(err?.error ?? "Invalid or expired pairing code")
        default:
            let err = try? JSONDecoder().decode(PairError.self, from: data)
            throw PairingError.serverError(err?.error ?? "HTTP \(http.statusCode)")
        }
    }
}

final class InsecureDelegate: NSObject, URLSessionDelegate, Sendable {
    static let shared = InsecureDelegate()
    func urlSession(
        _ session: URLSession,
        didReceive challenge: URLAuthenticationChallenge
    ) async -> (URLSession.AuthChallengeDisposition, URLCredential?) {
        (.useCredential, URLCredential(trust: challenge.protectionSpace.serverTrust!))
    }
}

public enum PairingError: Error, Sendable {
    case serverUnreachable
    case gatewayUnavailable(GatewayDiagnosticFailure)
    case invalidCode(String)
    case serverError(String)
}
