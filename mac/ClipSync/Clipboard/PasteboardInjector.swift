import AppKit
import Foundation
import Logging

protocol PasteboardWriting: AnyObject {
    @discardableResult
    func clearContents() -> Int
    @discardableResult
    func setData(_ data: Data?, forType dataType: NSPasteboard.PasteboardType) -> Bool
    @discardableResult
    func setString(_ string: String, forType dataType: NSPasteboard.PasteboardType) -> Bool
}

extension NSPasteboard: PasteboardWriting {}

enum PasteboardInjectionError: Error, Equatable {
    case invalidBase64
    case unsupportedMime(String)
    case writeFailed
}

final class PasteboardInjector: @unchecked Sendable {
    private let pasteboard: PasteboardWriting
    private weak var watcher: PasteboardWatcher?
    private var logger: Logger

    init(pasteboard: PasteboardWriting = NSPasteboard.general,
         watcher: PasteboardWatcher? = nil,
         logger: Logger = Logger(label: "clipsync.pasteboard.injector")) {
        self.pasteboard = pasteboard
        self.watcher = watcher
        self.logger = logger
    }

    func bind(watcher: PasteboardWatcher) {
        self.watcher = watcher
    }

    func inject(_ payload: ClipPayload) throws {
        guard let data = payload.rawData else {
            throw PasteboardInjectionError.invalidBase64
        }
        // Tell the watcher to ignore the echo *before* touching the pasteboard so a racy
        // tick on another queue still sees the suppression.
        watcher?.suppressNextMatching(payload)
        pasteboard.clearContents()
        switch payload.type {
        case .text:
            guard let text = String(data: data, encoding: .utf8) else {
                throw PasteboardInjectionError.invalidBase64
            }
            guard pasteboard.setString(text, forType: .string) else {
                throw PasteboardInjectionError.writeFailed
            }
        case .image:
            let type = Self.pasteboardType(forMime: payload.mime)
            guard let type else { throw PasteboardInjectionError.unsupportedMime(payload.mime) }
            let imageData = Self.convertToPngIfNeeded(data: data, mime: payload.mime) ?? data
            guard pasteboard.setData(imageData, forType: type) else {
                throw PasteboardInjectionError.writeFailed
            }
        }
        logger.debug("Injected payload", metadata: [
            "type": .string(payload.type.rawValue),
            "mime": .string(payload.mime),
            "bytes": .stringConvertible(data.count),
        ])
    }

    private static func pasteboardType(forMime mime: String) -> NSPasteboard.PasteboardType? {
        switch mime.lowercased() {
        case "image/png": return .png
        case "image/tiff": return .tiff
        case "image/jpeg", "image/jpg": return .png  // JPEG data will be converted to PNG
        default: return nil
        }
    }

    /// Convert JPEG data to PNG for pasteboard injection.
    /// NSPasteboard handles PNG natively; JPEG needs conversion.
    private static func convertToPngIfNeeded(data: Data, mime: String) -> Data? {
        let lower = mime.lowercased()
        guard lower == "image/jpeg" || lower == "image/jpg" else { return data }
        guard let image = NSImage(data: data) else { return nil }
        guard let tiffRep = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffRep),
              let pngData = bitmap.representation(using: .png, properties: [:]) else {
            return nil
        }
        return pngData
    }
}
