import XCTest
@testable import ClipSync

final class HMACValidatorTests: XCTestCase {
    final class FixedClock: HMACClock, @unchecked Sendable {
        let date: Date
        init(_ date: Date) { self.date = date }
        func now() -> Date { date }
    }

    private let secret = Data((0..<32).map { _ in UInt8.random(in: 0...255) })

    func testValidSignature() throws {
        let now = Date(timeIntervalSince1970: 1_700_000_000)
        let body = Data("{\"hello\":\"world\"}".utf8)
        let ts = Int(now.timeIntervalSince1970)
        let header = HMACValidator.sign(body: body, secret: secret, at: ts)
        let validator = HMACValidator(secret: secret, clock: FixedClock(now))
        XCTAssertNoThrow(try validator.validate(headerValue: header, body: body))
    }

    func testInvalidSignatureRejected() throws {
        let now = Date(timeIntervalSince1970: 1_700_000_000)
        let body = Data("body".utf8)
        let ts = Int(now.timeIntervalSince1970)
        let tampered = "t=\(ts), v1=deadbeef"
        let validator = HMACValidator(secret: secret, clock: FixedClock(now))
        do {
            try validator.validate(headerValue: tampered, body: body)
            XCTFail("expected failure")
        } catch HMACValidationError.invalidSignature {
            // expected
        }
    }

    func testMissingHeader() {
        let validator = HMACValidator(secret: secret, clock: FixedClock(Date()))
        XCTAssertThrowsError(try validator.validate(headerValue: nil, body: Data())) { err in
            XCTAssertEqual(err as? HMACValidationError, .missingHeader)
        }
    }

    func testReplayOldTimestampRejected() throws {
        let past = Date(timeIntervalSince1970: 1_700_000_000)
        let now = past.addingTimeInterval(120) // 2 min later
        let body = Data("body".utf8)
        let ts = Int(past.timeIntervalSince1970)
        let header = HMACValidator.sign(body: body, secret: secret, at: ts)
        let validator = HMACValidator(secret: secret, clock: FixedClock(now), skewSeconds: 60)
        XCTAssertThrowsError(try validator.validate(headerValue: header, body: body)) { err in
            XCTAssertEqual(err as? HMACValidationError, .replayOrSkew)
        }
    }

    func testFutureTimestampRejected() throws {
        let now = Date(timeIntervalSince1970: 1_700_000_000)
        let future = now.addingTimeInterval(120)
        let body = Data("body".utf8)
        let ts = Int(future.timeIntervalSince1970)
        let header = HMACValidator.sign(body: body, secret: secret, at: ts)
        let validator = HMACValidator(secret: secret, clock: FixedClock(now), skewSeconds: 60)
        XCTAssertThrowsError(try validator.validate(headerValue: header, body: body)) { err in
            XCTAssertEqual(err as? HMACValidationError, .replayOrSkew)
        }
    }

    func testMalformedHeaderRejected() {
        let validator = HMACValidator(secret: secret, clock: FixedClock(Date()))
        XCTAssertThrowsError(try validator.validate(headerValue: "nope", body: Data()))
    }
}
