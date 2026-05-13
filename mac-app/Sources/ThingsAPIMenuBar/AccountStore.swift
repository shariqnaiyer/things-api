import Foundation

/// Mirrors the layout in src/config.rs on the Rust side. Both processes operate on these files;
/// the Swift app should never assume it owns them exclusively.
///
///   macOS:  ~/Library/Application Support/things-api/
///                 - auth_token       (single line, the API bearer)
///                 - account.json     (Account JSON, from signup)
@MainActor
final class AccountStore: ObservableObject {
    @Published private(set) var account: Account?
    @Published private(set) var authToken: String?

    static let configDir: URL = {
        // FileManager.default.urls(for: .applicationSupportDirectory) returns
        // ~/Library/Application Support on macOS — matches the Rust `dirs::config_dir()`
        // fallback chain.
        let base = FileManager.default
            .urls(for: .applicationSupportDirectory, in: .userDomainMask)
            .first
            ?? URL(fileURLWithPath: NSHomeDirectory())
        let dir = base.appendingPathComponent("things-api", isDirectory: true)
        try? FileManager.default.createDirectory(
            at: dir,
            withIntermediateDirectories: true
        )
        return dir
    }()

    static var accountFile: URL { configDir.appendingPathComponent("account.json") }
    static var authTokenFile: URL { configDir.appendingPathComponent("auth_token") }

    init() {
        reload()
    }

    func reload() {
        account = Self.readAccount()
        authToken = Self.readAuthToken()
    }

    // MARK: - Account

    static func readAccount() -> Account? {
        guard let data = try? Data(contentsOf: accountFile) else { return nil }
        return try? JSONDecoder().decode(Account.self, from: data)
    }

    @discardableResult
    func writeAccount(_ account: Account) throws -> Account {
        let encoder = JSONEncoder()
        encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        let data = try encoder.encode(account)
        try data.write(to: Self.accountFile, options: [.atomic])
        setRestrictivePermissions(on: Self.accountFile)
        self.account = account
        return account
    }

    func clearAccount() {
        try? FileManager.default.removeItem(at: Self.accountFile)
        account = nil
    }

    // MARK: - Auth token

    static func readAuthToken() -> String? {
        guard let raw = try? String(contentsOf: authTokenFile, encoding: .utf8) else {
            return nil
        }
        let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
        return trimmed.isEmpty ? nil : trimmed
    }

    /// Generate a new bearer token in the same `thingsapi_<hex>` format the Rust binary uses
    /// (48 hex chars from 24 random bytes).
    @discardableResult
    func rotateToken() throws -> String {
        var bytes = [UInt8](repeating: 0, count: 24)
        let result = SecRandomCopyBytes(kSecRandomDefault, bytes.count, &bytes)
        guard result == errSecSuccess else {
            throw NSError(
                domain: "AccountStore",
                code: Int(result),
                userInfo: [NSLocalizedDescriptionKey: "SecRandomCopyBytes failed"]
            )
        }
        let hex = bytes.map { String(format: "%02x", $0) }.joined()
        let token = "thingsapi_\(hex)"
        try token.write(to: Self.authTokenFile, atomically: true, encoding: .utf8)
        setRestrictivePermissions(on: Self.authTokenFile)
        authToken = token
        return token
    }

    /// Ensure a token exists; create one only if missing.
    @discardableResult
    func ensureAuthToken() throws -> String {
        if let t = authToken { return t }
        return try rotateToken()
    }

    private func setRestrictivePermissions(on url: URL) {
        // 0o600 — matches the Rust side's chmod after writing token files.
        try? FileManager.default.setAttributes(
            [.posixPermissions: 0o600],
            ofItemAtPath: url.path
        )
    }
}
