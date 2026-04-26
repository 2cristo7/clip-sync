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
            // Always inject as PNG — universally supported by macOS pasteboard.
            // toPng() handles JPEG, WebP, GIF, TIFF, BMP, and any format NSImage can decode.
            let pngData: Data
            if payload.mime.lowercased() == "image/png" {
                pngData = data
            } else {
                guard let converted = Self.toPng(data) else {
                    throw PasteboardInjectionError.unsupportedMime(payload.mime)
                }
                pngData = converted
            }
            guard pasteboard.setData(pngData, forType: .png) else {
                throw PasteboardInjectionError.writeFailed
            }
        }
        logger.debug("Injected payload", metadata: [
            "type": .string(payload.type.rawValue),
            "mime": .string(payload.mime),
            "bytes": .stringConvertible(data.count),
        ])
    }

    /// Convert any image data to PNG via NSImage. Handles JPEG, WebP, GIF,
    /// TIFF, BMP — anything NSImage can decode. Returns nil only if NSImage
    /// cannot parse the data at all.
    private static func toPng(_ data: Data) -> Data? {
        guard let image = NSImage(data: data),
              let tiffRep = image.tiffRepresentation,
              let bitmap = NSBitmapImageRep(data: tiffRep),
              let pngData = bitmap.representation(using: .png, properties: [:]) else {
            return nil
        }
        return pngData
    }
}
