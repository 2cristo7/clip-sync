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
        if payload.type == .file {
            try saveFile(data: data, payload: payload)
            return
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
        case .file:
            break
        case .image:
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
            try saveFile(data: pngData, payload: ClipPayload.file(pngData, name: imageFileName(payload), mime: "image/png"))
        }
        logger.debug("Injected payload", metadata: [
            "type": .string(payload.type.rawValue),
            "mime": .string(payload.mime),
            "bytes": .stringConvertible(data.count),
        ])
    }

    private func saveFile(data: Data, payload: ClipPayload) throws {
        let documentsURL = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first!
        let clipSyncDir = documentsURL.appendingPathComponent("ClipSync")
        if !FileManager.default.fileExists(atPath: clipSyncDir.path) {
            try FileManager.default.createDirectory(at: clipSyncDir, withIntermediateDirectories: true)
        }
        let fileName = payload.name ?? "clipsync_\(payload.nonce)"
        var destURL = clipSyncDir.appendingPathComponent(fileName)
        var counter = 1
        let baseName = destURL.deletingPathExtension().lastPathComponent
        let ext = destURL.pathExtension
        while FileManager.default.fileExists(atPath: destURL.path) {
            let newName = ext.isEmpty ? "\(baseName) (\(counter))" : "\(baseName) (\(counter)).\(ext)"
            destURL = clipSyncDir.appendingPathComponent(newName)
            counter += 1
        }
        do {
            try data.write(to: destURL)
        } catch {
            throw PasteboardInjectionError.writeFailed
        }
        logger.info("File saved to Documents/ClipSync: \(destURL.lastPathComponent)")
        showFileSavedNotification(fileURL: destURL)
    }

    private func showFileSavedNotification(fileURL: URL) {
        DispatchQueue.main.async {
            let alert = NSAlert()
            alert.messageText = "ClipSync"
            alert.informativeText = "File saved to Documents/ClipSync: \(fileURL.lastPathComponent)"
            alert.alertStyle = .informational
            alert.addButton(withTitle: "OK")
            alert.addButton(withTitle: "Show in Finder")
            let response = alert.runModal()
            if response == .alertSecondButtonReturn {
                NSWorkspace.shared.activateFileViewerSelecting([fileURL])
            }
        }
    }

    private func imageFileName(_ payload: ClipPayload) -> String {
        if let name = payload.name { return name }
        let formatter = DateFormatter()
        formatter.dateFormat = "yyyy-MM-dd_HH-mm-ss"
        return "ClipSync_\(formatter.string(from: Date())).png"
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
