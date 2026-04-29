import Foundation
import CryptoKit

/// Persistent store of issued pairing tokens.
///
/// Tokens are persisted as SHA-256 hashes (never plaintext). Plaintext is only
/// ever returned from `issue(...)` and passed into `validate(...)` for comparison.
actor TokenStore {
    struct Record: Codable, Equatable, Sendable {
        let id: String
        let tokenHash: String        // hex SHA-256 of plaintext token
        var createdAt: Date
        var lastSeenAt: Date
        var deviceLabel: String
    }

    private let keychain: Keychain
    private let account: String
    private var records: [String: Record] = [:] // id -> record
    private var loaded: Bool = false

    init(keychain: Keychain = Keychain(service: TokenStore.service),
         account: String = TokenStore.defaultAccount) {
        self.keychain = keychain
        self.account = account
    }

    static let service = "com.clipsync.token-store"
    static let defaultAccount = "tokens"

    // MARK: - Public API

    /// Registers an externally-generated plaintext token (e.g. one produced by
    /// `PairingManager.consume`) so it can be validated later.
    @discardableResult
    func register(tokenPlain: String,
                  deviceLabel: String,
                  now: Date = Date()) throws -> Record {
        try loadIfNeeded()
        let id = UUID().uuidString
        let tokenHash = Self.hashHex(tokenPlain)
        let record = Record(
            id: id,
            tokenHash: tokenHash,
            createdAt: now,
            lastSeenAt: now,
            deviceLabel: deviceLabel
        )
        records[id] = record
        try persist()
        return record
    }

    func issue(deviceLabel: String, now: Date = Date()) throws -> (id: String, tokenPlain: String) {
        try loadIfNeeded()
        let id = UUID().uuidString
        let tokenBytes = try Self.randomTokenBytes()
        let tokenPlain = tokenBytes.base64EncodedString()
        let tokenHash = Self.hashHex(tokenPlain)
        let record = Record(
            id: id,
            tokenHash: tokenHash,
            createdAt: now,
            lastSeenAt: now,
            deviceLabel: deviceLabel
        )
        records[id] = record
        try persist()
        return (id, tokenPlain)
    }

    func validate(tokenPlain: String) throws -> Record? {
        try loadIfNeeded()
        let hash = Self.hashHex(tokenPlain)
        return records.values.first(where: { $0.tokenHash == hash })
    }

    func touch(id: String, at date: Date = Date()) throws {
        try loadIfNeeded()
        guard var rec = records[id] else { return }
        rec.lastSeenAt = date
        records[id] = rec
        try persist()
    }

    func revoke(id: String) throws {
        try loadIfNeeded()
        records.removeValue(forKey: id)
        try persist()
    }

    func list() throws -> [Record] {
        try loadIfNeeded()
        return Array(records.values).sorted { $0.createdAt < $1.createdAt }
    }

    /// Clears in-memory and on-disk state. Exposed for tests.
    func reset() throws {
        records.removeAll()
        try keychain.delete(account: account)
        loaded = true
    }

    // MARK: - Persistence

    private func loadIfNeeded() throws {
        guard !loaded else { return }
        do {
            let data = try keychain.load(account: account)
            let decoded = try JSONDecoder().decode([Record].self, from: data)
            records = Dictionary(uniqueKeysWithValues: decoded.map { ($0.id, $0) })
        } catch KeychainError.notFound {
            records = [:]
        } catch {
            // Corrupted or unreadable data — start fresh rather than crash.
            records = [:]
        }
        loaded = true
    }

    private func persist() throws {
        let sorted = records.values.sorted { $0.createdAt < $1.createdAt }
        let data = try JSONEncoder().encode(Array(sorted))
        try keychain.save(data, account: account)
    }

    // MARK: - Crypto helpers

    static func hashHex(_ tokenPlain: String) -> String {
        let digest = SHA256.hash(data: Data(tokenPlain.utf8))
        return digest.map { String(format: "%02x", $0) }.joined()
    }

    static func randomTokenBytes(count: Int = 32) throws -> Data {
        var buffer = [UInt8](repeating: 0, count: count)
        let status = SecRandomCopyBytes(kSecRandomDefault, count, &buffer)
        guard status == errSecSuccess else {
            throw KeychainError.randomGenerationFailed(status)
        }
        return Data(buffer)
    }
}
