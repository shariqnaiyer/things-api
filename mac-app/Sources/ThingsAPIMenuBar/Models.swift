import Foundation

/// Persisted account state. Must stay structurally identical to the Rust `config::Account` struct
/// in src/config.rs — both sides read and write the same JSON file.
struct Account: Codable, Equatable {
    var username: String
    var url: String
    var tunnelToken: String
    var controlPlaneUrl: String
    var createdAt: String

    enum CodingKeys: String, CodingKey {
        case username
        case url
        case tunnelToken = "tunnel_token"
        case controlPlaneUrl = "control_plane_url"
        case createdAt = "created_at"
    }
}

/// POST /signup payload sent to the control plane.
struct SignupRequest: Encodable {
    let username: String
    let email: String?
}

/// 201 response from the control plane.
struct SignupResponse: Decodable {
    let username: String
    let url: String
    let tunnelToken: String

    enum CodingKeys: String, CodingKey {
        case username
        case url
        case tunnelToken = "tunnel_token"
    }
}

/// 4xx/5xx error envelope.
struct ServerError: Decodable {
    let error: String
}

/// /info response from the local Rust server.
struct AppInfo: Decodable {
    let name: String
    let version: String
    let frontmost: Bool
    let currentListName: String?
    let currentListUrl: String?

    enum CodingKeys: String, CodingKey {
        case name, version, frontmost
        case currentListName = "current_list_name"
        case currentListUrl = "current_list_url"
    }
}
