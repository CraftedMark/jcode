import Foundation

public struct RustMobileCoreResponse: Decodable, Equatable {
    public let ok: Bool
    public let value: JSONValue?
    public let error: String?
}

public enum JSONValue: Decodable, Equatable {
    case string(String)
    case number(Double)
    case bool(Bool)
    case object([String: JSONValue])
    case array([JSONValue])
    case null

    public init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if container.decodeNil() {
            self = .null
        } else if let value = try? container.decode(Bool.self) {
            self = .bool(value)
        } else if let value = try? container.decode(Double.self) {
            self = .number(value)
        } else if let value = try? container.decode(String.self) {
            self = .string(value)
        } else if let value = try? container.decode([String: JSONValue].self) {
            self = .object(value)
        } else {
            self = .array(try container.decode([JSONValue].self))
        }
    }
}

public enum RustMobileCoreError: Error, Equatable, LocalizedError {
    case unavailable
    case creationFailed
    case callFailed(String)
    case invalidUTF8
    case invalidResponse

    public var errorDescription: String? {
        switch self {
        case .unavailable:
            return "Rust mobile core is not linked into this build."
        case .creationFailed:
            return "Rust mobile core could not create an app handle."
        case .callFailed(let message):
            return message
        case .invalidUTF8:
            return "Rust mobile core returned invalid UTF-8."
        case .invalidResponse:
            return "Rust mobile core returned an invalid response."
        }
    }
}

public final class RustMobileCore {
    public static var isLinked: Bool {
        #if canImport(JCodeMobileCore)
        true
        #else
        false
        #endif
    }

    #if canImport(JCodeMobileCore)
    private var handle: OpaquePointer?
    #else
    private var handle: OpaquePointer?
    #endif

    public init(initialJSON: String? = nil) throws {
        #if canImport(JCodeMobileCore)
        handle = initialJSON.withOptionalCString { pointer in
            jcode_mobile_app_new(pointer)
        }
        guard handle != nil else {
            throw RustMobileCoreError.creationFailed
        }
        #else
        _ = initialJSON
        throw RustMobileCoreError.unavailable
        #endif
    }

    deinit {
        #if canImport(JCodeMobileCore)
        if let handle {
            jcode_mobile_app_free(handle)
        }
        #endif
    }

    public func dispatch(actionJSON: String) throws -> RustMobileCoreResponse {
        #if canImport(JCodeMobileCore)
        guard let handle else { throw RustMobileCoreError.creationFailed }
        return try decodeBridgeResponse(jcode_mobile_dispatch(handle, actionJSON))
        #else
        _ = actionJSON
        throw RustMobileCoreError.unavailable
        #endif
    }

    public func state() throws -> RustMobileCoreResponse {
        #if canImport(JCodeMobileCore)
        guard let handle else { throw RustMobileCoreError.creationFailed }
        return try decodeBridgeResponse(jcode_mobile_state(handle))
        #else
        throw RustMobileCoreError.unavailable
        #endif
    }

    public func tree() throws -> RustMobileCoreResponse {
        #if canImport(JCodeMobileCore)
        guard let handle else { throw RustMobileCoreError.creationFailed }
        return try decodeBridgeResponse(jcode_mobile_tree(handle))
        #else
        throw RustMobileCoreError.unavailable
        #endif
    }

    public func logs(limit: UInt32 = 50) throws -> RustMobileCoreResponse {
        #if canImport(JCodeMobileCore)
        guard let handle else { throw RustMobileCoreError.creationFailed }
        return try decodeBridgeResponse(jcode_mobile_logs(handle, limit))
        #else
        _ = limit
        throw RustMobileCoreError.unavailable
        #endif
    }
}

#if canImport(JCodeMobileCore)
import JCodeMobileCore

private func decodeBridgeResponse(_ pointer: UnsafeMutablePointer<CChar>?) throws -> RustMobileCoreResponse {
    guard let pointer else {
        throw RustMobileCoreError.invalidResponse
    }
    defer {
        jcode_mobile_string_free(pointer)
    }
    guard let string = String(validatingUTF8: pointer) else {
        throw RustMobileCoreError.invalidUTF8
    }
    let response = try JSONDecoder().decode(RustMobileCoreResponse.self, from: Data(string.utf8))
    if !response.ok {
        throw RustMobileCoreError.callFailed(response.error ?? "Rust mobile core call failed.")
    }
    return response
}
#endif

private extension Optional where Wrapped == String {
    func withOptionalCString<R>(_ body: (UnsafePointer<CChar>?) -> R) -> R {
        switch self {
        case .some(let value):
            return value.withCString(body)
        case .none:
            return body(nil)
        }
    }
}
