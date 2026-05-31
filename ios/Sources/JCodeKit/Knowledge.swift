import Foundation

public enum KnowledgeScope: String, Codable, Sendable, CaseIterable {
  case identity
  case wiki

  public static let allCases: [KnowledgeScope] = [.identity, .wiki]

  public var displayName: String {
    switch self {
    case .identity: "Identity"
    case .wiki: "Wiki"
    }
  }
}

public struct KnowledgeFileSummary: Codable, Identifiable, Sendable, Equatable {
  public var id: String { "\(scope.rawValue):\(path)" }
  public let scope: KnowledgeScope
  public let path: String
  public let title: String
  public let byteLen: UInt64
  public let modifiedUnixSecs: UInt64?
  public let sha256: String

  enum CodingKeys: String, CodingKey {
    case scope
    case path
    case title
    case byteLen = "byte_len"
    case modifiedUnixSecs = "modified_unix_secs"
    case sha256
  }
}

public struct KnowledgeFileListResponse: Codable, Sendable {
  public let files: [KnowledgeFileSummary]
}

public struct KnowledgeFileReadResponse: Codable, Sendable {
  public let scope: KnowledgeScope
  public let path: String
  public let title: String
  public let content: String
  public let sha256: String
  public let byteLen: UInt64
  public let modifiedUnixSecs: UInt64?

  enum CodingKeys: String, CodingKey {
    case scope
    case path
    case title
    case content
    case sha256
    case byteLen = "byte_len"
    case modifiedUnixSecs = "modified_unix_secs"
  }
}

public struct KnowledgeWriteResponse: Codable, Sendable {
  public let scope: KnowledgeScope
  public let path: String
  public let sha256: String
  public let backupPath: String?

  enum CodingKeys: String, CodingKey {
    case scope
    case path
    case sha256
    case backupPath = "backup_path"
  }
}

public struct KnowledgeClient: Sendable {
  public let host: String
  public let port: UInt16
  private let authToken: String

  public init(host: String, port: UInt16 = 7643, authToken: String) {
    let endpoint = GatewayEndpoint(host: host, port: port)
    self.host = endpoint.host
    self.port = endpoint.port
    self.authToken = authToken
  }

  private var baseURL: URL {
    var components = URLComponents()
    components.scheme = "http"
    components.host = host
    components.port = Int(port)
    return components.url!
  }

  public func listFiles(scope: KnowledgeScope) async throws -> [KnowledgeFileSummary] {
    var components = URLComponents(
      url: baseURL.appendingPathComponent("knowledge/files"), resolvingAgainstBaseURL: false)!
    components.queryItems = [URLQueryItem(name: "scope", value: scope.rawValue)]
    let response: KnowledgeFileListResponse = try await sendJSON(url: components.url!)
    return response.files
  }

  public func readFile(scope: KnowledgeScope, path: String) async throws
    -> KnowledgeFileReadResponse
  {
    var components = URLComponents(
      url: baseURL.appendingPathComponent("knowledge/file"), resolvingAgainstBaseURL: false)!
    components.queryItems = [
      URLQueryItem(name: "scope", value: scope.rawValue),
      URLQueryItem(name: "path", value: path),
    ]
    return try await sendJSON(url: components.url!)
  }

  public func writeFile(
    scope: KnowledgeScope,
    path: String,
    content: String,
    baseSHA256: String?
  ) async throws -> KnowledgeWriteResponse {
    let url = baseURL.appendingPathComponent("knowledge/file")
    var request = URLRequest(url: url)
    request.httpMethod = "POST"
    request.setValue("application/json", forHTTPHeaderField: "Content-Type")
    request.setValue("Bearer \(authToken)", forHTTPHeaderField: "Authorization")
    let body = KnowledgeWriteRequest(
      scope: scope,
      path: path,
      content: content,
      baseSHA256: baseSHA256
    )
    request.httpBody = try JSONEncoder().encode(body)
    return try await sendJSON(request: request)
  }

  private func sendJSON<T: Decodable>(url: URL) async throws -> T {
    var request = URLRequest(url: url)
    request.setValue("Bearer \(authToken)", forHTTPHeaderField: "Authorization")
    return try await sendJSON(request: request)
  }

  private func sendJSON<T: Decodable>(request: URLRequest) async throws -> T {
    let (data, response) = try await URLSession(
      configuration: .default, delegate: InsecureDelegate.shared, delegateQueue: nil
    ).data(for: request)
    guard let http = response as? HTTPURLResponse else {
      throw KnowledgeError.serverUnreachable
    }
    guard (200..<300).contains(http.statusCode) else {
      let error = try? JSONDecoder().decode(PairError.self, from: data)
      throw KnowledgeError.serverError(error?.error ?? "HTTP \(http.statusCode)")
    }
    return try JSONDecoder().decode(T.self, from: data)
  }
}

public enum KnowledgeError: Error, LocalizedError, Sendable {
  case serverUnreachable
  case serverError(String)

  public var errorDescription: String? {
    switch self {
    case .serverUnreachable:
      "Knowledge gateway is unreachable."
    case .serverError(let message):
      message
    }
  }
}

private struct KnowledgeWriteRequest: Encodable {
  let scope: KnowledgeScope
  let path: String
  let content: String
  let baseSHA256: String?

  enum CodingKeys: String, CodingKey {
    case scope
    case path
    case content
    case baseSHA256 = "base_sha256"
  }
}
