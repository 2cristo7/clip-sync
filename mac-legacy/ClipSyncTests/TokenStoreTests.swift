import XCTest
@testable import ClipSync

final class TokenStoreTests: XCTestCase {
    private var service: String = ""
    private var keychain: Keychain!
    private var store: TokenStore!

    override func setUp() {
        super.setUp()
        service = "com.clipsync.tests.token.\(UUID().uuidString)"
        keychain = Keychain(service: service)
        store = TokenStore(keychain: keychain, account: "tokens")
    }

    override func tearDown() {
        try? keychain.delete(account: "tokens")
        super.tearDown()
    }

    private func skipIfNoKeychain(_ block: () async throws -> Void) async throws {
        do {
            try await block()
        } catch KeychainError.unexpectedStatus(let status) where status == errSecMissingEntitlement {
            throw XCTSkip("Keychain access unavailable (status=\(status))")
        }
    }

    func testIssueValidateTouchRoundtrip() async throws {
        try await skipIfNoKeychain {
            let (id, plain) = try await store.issue(deviceLabel: "pixel-7")
            XCTAssertFalse(id.isEmpty)
            XCTAssertFalse(plain.isEmpty)

            let record = try await store.validate(tokenPlain: plain)
            XCTAssertNotNil(record)
            XCTAssertEqual(record?.id, id)
            XCTAssertEqual(record?.deviceLabel, "pixel-7")

            let before = record!.lastSeenAt
            try await Task.sleep(nanoseconds: 10_000_000)
            try await store.touch(id: id, at: Date())
            let list = try await store.list()
            XCTAssertEqual(list.count, 1)
            XCTAssertGreaterThanOrEqual(list[0].lastSeenAt, before)
        }
    }

    func testRevokeRemovesToken() async throws {
        try await skipIfNoKeychain {
            let (id, plain) = try await store.issue(deviceLabel: "pixel-7")
            try await store.revoke(id: id)
            let record = try await store.validate(tokenPlain: plain)
            XCTAssertNil(record)
        }
    }

    func testValidateUnknownTokenReturnsNil() async throws {
        try await skipIfNoKeychain {
            let record = try await store.validate(tokenPlain: "not-a-real-token")
            XCTAssertNil(record)
        }
    }

    func testRegisterStoresHashNotPlaintext() async throws {
        try await skipIfNoKeychain {
            let plain = "alpha-bravo-charlie"
            let rec = try await store.register(tokenPlain: plain, deviceLabel: "phone")
            XCTAssertNotEqual(rec.tokenHash, plain)
            XCTAssertEqual(rec.tokenHash, TokenStore.hashHex(plain))
            let found = try await store.validate(tokenPlain: plain)
            XCTAssertEqual(found?.id, rec.id)
        }
    }

    func testPersistenceAcrossInstances() async throws {
        try await skipIfNoKeychain {
            let (_, plain) = try await store.issue(deviceLabel: "device")
            let store2 = TokenStore(keychain: keychain, account: "tokens")
            let record = try await store2.validate(tokenPlain: plain)
            XCTAssertNotNil(record)
        }
    }
}
