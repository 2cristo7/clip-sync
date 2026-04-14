import Foundation
import CryptoKit
import Logging

enum PairingError: Error, Equatable {
    case notStarted
    case invalid
    case expired
    case consumed
    case randomFailure
}

struct PairingResponse: Codable, Sendable {
    let token: String
    let sig: String
}

struct PairingSession: Sendable {
    let code: String
    let expiresAt: Date
}

protocol PairingClock: Sendable {
    func now() -> Date
}

struct SystemPairingClock: PairingClock {
    func now() -> Date { Date() }
}

actor PairingManager {
    private struct ActiveCode {
        let code: String
        let createdAt: Date
        var consumed: Bool = false
    }

    private let secret: Data
    private let ttl: TimeInterval
    private let clock: PairingClock
    private var active: ActiveCode?
    private var logger: Logger

    init(secret: Data,
         ttl: TimeInterval = 300,
         clock: PairingClock = SystemPairingClock(),
         logger: Logger = Logger(label: "clipsync.pairing")) {
        self.secret = secret
        self.ttl = ttl
        self.clock = clock
        self.logger = logger
    }

    func startPairing() throws -> PairingSession {
        let code = try Self.generate6DigitCode()
        let createdAt = clock.now()
        active = ActiveCode(code: code, createdAt: createdAt)
        let session = PairingSession(code: code, expiresAt: createdAt.addingTimeInterval(ttl))
        logger.info("Pairing code generated", metadata: ["ttl": .stringConvertible(Int(ttl))])
        return session
    }

    func cancel() {
        active = nil
    }

    func currentSession() -> PairingSession? {
        guard let a = active, !a.consumed else { return nil }
        if clock.now().timeIntervalSince(a.createdAt) > ttl { return nil }
        return PairingSession(code: a.code, expiresAt: a.createdAt.addingTimeInterval(ttl))
    }

    func consume(code: String) throws -> PairingResponse {
        guard let a = active else { throw PairingError.notStarted }
        if clock.now().timeIntervalSince(a.createdAt) > ttl {
            active = nil
            throw PairingError.expired
        }
        guard !a.consumed else { throw PairingError.consumed }
        guard a.code == code else { throw PairingError.invalid }
        active?.consumed = true

        let tokenBytes = try Self.randomBytes(count: 32)
        let signature = HMAC<SHA256>.authenticationCode(
            for: tokenBytes,
            using: SymmetricKey(data: secret)
        )
        logger.info("Pairing code consumed")
        return PairingResponse(
            token: tokenBytes.base64EncodedString(),
            sig: Data(signature).base64EncodedString()
        )
    }

    static func generate6DigitCode() throws -> String {
        var digits = ""
        while digits.count < 6 {
            var byte: UInt8 = 0
            let status = withUnsafeMutableBytes(of: &byte) { buffer -> Int32 in
                guard let base = buffer.baseAddress else { return errSecParam }
                return SecRandomCopyBytes(kSecRandomDefault, 1, base)
            }
            guard status == errSecSuccess else { throw PairingError.randomFailure }
            if byte < 250 {
                digits += String(byte % 10)
            }
        }
        return digits
    }

    static func randomBytes(count: Int) throws -> Data {
        var buffer = [UInt8](repeating: 0, count: count)
        let status = SecRandomCopyBytes(kSecRandomDefault, count, &buffer)
        guard status == errSecSuccess else { throw PairingError.randomFailure }
        return Data(buffer)
    }

    static func fingerprint(of secret: Data, hexLength: Int = 16) -> String {
        let digest = SHA256.hash(data: secret)
        let hex = digest.map { String(format: "%02x", $0) }.joined()
        return String(hex.prefix(hexLength))
    }
}
