import XCTest
@testable import ClipSync

final class KeychainTests: XCTestCase {
    private var service: String = ""
    private var keychain: Keychain!

    override func setUp() {
        super.setUp()
        service = "com.clipsync.tests.\(UUID().uuidString)"
        keychain = Keychain(service: service)
    }

    override func tearDown() {
        try? keychain.delete()
        super.tearDown()
    }

    func testSaveLoadDeleteRoundtrip() throws {
        let payload = Data([0xDE, 0xAD, 0xBE, 0xEF])
        do {
            try keychain.save(payload)
        } catch KeychainError.unexpectedStatus(let status) where status == errSecMissingEntitlement {
            throw XCTSkip("Keychain access unavailable in this test environment (status=\(status))")
        }

        let loaded = try keychain.load()
        XCTAssertEqual(loaded, payload)

        let updated = Data([0xCA, 0xFE])
        try keychain.save(updated)
        XCTAssertEqual(try keychain.load(), updated)

        try keychain.delete()
        XCTAssertThrowsError(try keychain.load()) { error in
            XCTAssertEqual(error as? KeychainError, .notFound)
        }
    }

    func testLoadOrCreateSecretIsStable() throws {
        let first: Data
        do {
            first = try keychain.loadOrCreateSecret()
        } catch KeychainError.unexpectedStatus(let status) where status == errSecMissingEntitlement {
            throw XCTSkip("Keychain access unavailable in this test environment (status=\(status))")
        }
        XCTAssertEqual(first.count, 32)
        let second = try keychain.loadOrCreateSecret()
        XCTAssertEqual(first, second)
    }
}
