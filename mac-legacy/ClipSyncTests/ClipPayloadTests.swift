import XCTest
@testable import ClipSync

final class ClipPayloadTests: XCTestCase {
    func testTextFactoryEncodesUTF8Base64() throws {
        let payload = ClipPayload.text("hola")
        XCTAssertEqual(payload.type, .text)
        XCTAssertEqual(payload.mime, "text/plain")
        XCTAssertEqual(payload.dataBase64, Data("hola".utf8).base64EncodedString())
        XCTAssertFalse(payload.nonce.isEmpty)
    }

    func testCodableRoundtrip() throws {
        let payload = ClipPayload.text("round trip")
        let data = try JSONEncoder().encode(payload)
        let decoded = try JSONDecoder().decode(ClipPayload.self, from: data)
        XCTAssertEqual(decoded, payload)
    }

    func testDigestIgnoresTsAndNonce() {
        let a = ClipPayload(type: .text, mime: "text/plain", dataBase64: "aGk=", ts: 1, nonce: "A")
        let b = ClipPayload(type: .text, mime: "text/plain", dataBase64: "aGk=", ts: 999, nonce: "B")
        XCTAssertEqual(PasteboardWatcher.digest(for: a), PasteboardWatcher.digest(for: b))
    }
}
