import Foundation

enum ClipKind: String, Codable, Sendable {
    case text
    case image
}

struct ClipPayload: Codable, Sendable, Equatable {
    let type: ClipKind
    let mime: String
    let dataBase64: String
    let ts: Int64
    let nonce: String

    // Wire protocol uses "data" as the key; the Swift property is named
    // dataBase64 for clarity. CodingKeys bridges the two.
    enum CodingKeys: String, CodingKey {
        case type
        case mime
        case dataBase64 = "data"
        case ts
        case nonce
    }

    init(type: ClipKind, mime: String, dataBase64: String, ts: Int64, nonce: String) {
        self.type = type
        self.mime = mime
        self.dataBase64 = dataBase64
        self.ts = ts
        self.nonce = nonce
    }

    static func currentTimestampMillis() -> Int64 {
        Int64(Date().timeIntervalSince1970 * 1000)
    }

    static func newNonce() -> String {
        UUID().uuidString
    }

    static func text(_ value: String, ts: Int64 = ClipPayload.currentTimestampMillis()) -> ClipPayload {
        let data = Data(value.utf8)
        return ClipPayload(
            type: .text,
            mime: "text/plain",
            dataBase64: data.base64EncodedString(),
            ts: ts,
            nonce: newNonce()
        )
    }

    static func image(_ data: Data, mime: String = "image/png", ts: Int64 = ClipPayload.currentTimestampMillis()) -> ClipPayload {
        ClipPayload(
            type: .image,
            mime: mime,
            dataBase64: data.base64EncodedString(),
            ts: ts,
            nonce: newNonce()
        )
    }

    var rawData: Data? {
        Data(base64Encoded: dataBase64)
    }
}
