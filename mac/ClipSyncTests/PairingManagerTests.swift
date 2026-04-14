import XCTest
import CryptoKit
@testable import ClipSync

final class PairingManagerTests: XCTestCase {
    final class MutableClock: PairingClock, @unchecked Sendable {
        private let lock = NSLock()
        private var _current: Date
        init(_ start: Date) { _current = start }
        func now() -> Date {
            lock.lock(); defer { lock.unlock() }
            return _current
        }
        func advance(by interval: TimeInterval) {
            lock.lock(); defer { lock.unlock() }
            _current = _current.addingTimeInterval(interval)
        }
    }

    private let secret = Data((0..<32).map { _ in UInt8.random(in: 0...255) })

    func testCodeGeneratesSixDigitString() async throws {
        let clock = MutableClock(Date(timeIntervalSince1970: 0))
        let manager = PairingManager(secret: secret, ttl: 300, clock: clock)
        let session = try await manager.startPairing()
        XCTAssertEqual(session.code.count, 6)
        XCTAssertTrue(session.code.allSatisfy { $0.isNumber })
    }

    func testCodeIsSingleUse() async throws {
        let clock = MutableClock(Date(timeIntervalSince1970: 0))
        let manager = PairingManager(secret: secret, ttl: 300, clock: clock)
        let session = try await manager.startPairing()

        let response = try await manager.consume(code: session.code)
        XCTAssertFalse(response.token.isEmpty)
        XCTAssertFalse(response.sig.isEmpty)

        do {
            _ = try await manager.consume(code: session.code)
            XCTFail("second consume should fail")
        } catch PairingError.consumed {
            // expected
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testExpiredCodeFails() async throws {
        let clock = MutableClock(Date(timeIntervalSince1970: 0))
        let manager = PairingManager(secret: secret, ttl: 300, clock: clock)
        let session = try await manager.startPairing()
        clock.advance(by: 301)
        do {
            _ = try await manager.consume(code: session.code)
            XCTFail("expired code should fail")
        } catch PairingError.expired {
            // expected
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testWrongCodeReturnsInvalid() async throws {
        let clock = MutableClock(Date(timeIntervalSince1970: 0))
        let manager = PairingManager(secret: secret, ttl: 300, clock: clock)
        _ = try await manager.startPairing()
        do {
            _ = try await manager.consume(code: "000000")
            XCTFail("wrong code should fail")
        } catch PairingError.invalid {
            // expected
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testConsumeBeforeStartFails() async throws {
        let manager = PairingManager(secret: secret, ttl: 300)
        do {
            _ = try await manager.consume(code: "123456")
            XCTFail("no active code should fail")
        } catch PairingError.notStarted {
            // expected
        } catch {
            XCTFail("unexpected error: \(error)")
        }
    }

    func testSignatureValidatesWithSecret() async throws {
        let clock = MutableClock(Date(timeIntervalSince1970: 0))
        let manager = PairingManager(secret: secret, ttl: 300, clock: clock)
        let session = try await manager.startPairing()
        let response = try await manager.consume(code: session.code)

        guard let tokenBytes = Data(base64Encoded: response.token),
              let signatureBytes = Data(base64Encoded: response.sig) else {
            XCTFail("token/sig not valid base64")
            return
        }
        let key = SymmetricKey(data: secret)
        XCTAssertTrue(HMAC<SHA256>.isValidAuthenticationCode(
            signatureBytes,
            authenticating: tokenBytes,
            using: key
        ))
    }

    func testFingerprintIsStable() {
        let a = PairingManager.fingerprint(of: secret)
        let b = PairingManager.fingerprint(of: secret)
        XCTAssertEqual(a, b)
        XCTAssertEqual(a.count, 16)
    }
}
