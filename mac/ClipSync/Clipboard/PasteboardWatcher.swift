import AppKit
import CryptoKit
import Dispatch
import Foundation
import Logging

protocol PasteboardReading: AnyObject {
    var changeCount: Int { get }
    func string(forType type: NSPasteboard.PasteboardType) -> String?
    func data(forType type: NSPasteboard.PasteboardType) -> Data?
    func types() -> [NSPasteboard.PasteboardType]?
}

extension NSPasteboard: PasteboardReading {
    func types() -> [NSPasteboard.PasteboardType]? {
        self.types
    }
}

final class PasteboardWatcher: @unchecked Sendable {
    typealias Snapshot = ClipPayload

    private let pasteboard: PasteboardReading
    private let interval: DispatchTimeInterval
    private let queue = DispatchQueue(label: "clipsync.pasteboard.watcher")
    private var timer: DispatchSourceTimer?
    private var lastChangeCount: Int
    private var suppressedDigests: [String] = []
    private var logger: Logger

    private var continuations: [UUID: AsyncStream<Snapshot>.Continuation] = [:]

    init(pasteboard: PasteboardReading = NSPasteboard.general,
         intervalMillis: Int = 500,
         logger: Logger = Logger(label: "clipsync.pasteboard.watcher")) {
        self.pasteboard = pasteboard
        self.interval = .milliseconds(intervalMillis)
        self.logger = logger
        self.lastChangeCount = pasteboard.changeCount
    }

    func start() {
        queue.async { [weak self] in
            guard let self, self.timer == nil else { return }
            let timer = DispatchSource.makeTimerSource(queue: self.queue)
            timer.schedule(deadline: .now() + self.interval, repeating: self.interval)
            timer.setEventHandler { [weak self] in
                self?.tick()
            }
            self.timer = timer
            timer.resume()
        }
    }

    func stop() {
        queue.async { [weak self] in
            self?.timer?.cancel()
            self?.timer = nil
        }
    }

    func events() -> AsyncStream<Snapshot> {
        AsyncStream { continuation in
            let id = UUID()
            self.queue.async {
                self.continuations[id] = continuation
            }
            continuation.onTermination = { [weak self] _ in
                self?.queue.async {
                    self?.continuations.removeValue(forKey: id)
                }
            }
        }
    }

    /// Register a payload hash so the next matching watcher tick is ignored.
    func suppressNextMatching(_ payload: ClipPayload) {
        let digest = Self.digest(for: payload)
        queue.async { [weak self] in
            guard let self else { return }
            self.suppressedDigests.append(digest)
            // Keep a small bounded window.
            if self.suppressedDigests.count > 8 {
                self.suppressedDigests.removeFirst(self.suppressedDigests.count - 8)
            }
            // Baseline the change count so the injection that just happened is skipped.
            self.lastChangeCount = self.pasteboard.changeCount
        }
    }

    /// For tests — force a synchronous tick.
    func pollNow() {
        queue.sync { self.tick() }
    }

    private func tick() {
        dispatchPrecondition(condition: .onQueue(queue))
        let current = pasteboard.changeCount
        guard current != lastChangeCount else { return }
        lastChangeCount = current

        guard let payload = capturePayload() else { return }
        let digest = Self.digest(for: payload)
        if let index = suppressedDigests.firstIndex(of: digest) {
            suppressedDigests.remove(at: index)
            logger.debug("Suppressed echoed payload", metadata: ["digest": .string(digest)])
            return
        }
        for continuation in continuations.values {
            continuation.yield(payload)
        }
    }

    private func capturePayload() -> ClipPayload? {
        let types = pasteboard.types() ?? []
        if types.contains(.png), let data = pasteboard.data(forType: .png) {
            return ClipPayload.image(data, mime: "image/png")
        }
        if types.contains(.tiff), let data = pasteboard.data(forType: .tiff) {
            return ClipPayload.image(data, mime: "image/tiff")
        }
        if types.contains(.string), let value = pasteboard.string(forType: .string), !value.isEmpty {
            return ClipPayload.text(value)
        }
        return nil
    }

    /// Hash payload content ignoring ts/nonce so echoes are detected regardless of timestamp.
    static func digest(for payload: ClipPayload) -> String {
        var hasher = SHA256()
        hasher.update(data: Data(payload.type.rawValue.utf8))
        hasher.update(data: Data(payload.mime.utf8))
        hasher.update(data: Data(payload.dataBase64.utf8))
        let digest = hasher.finalize()
        return digest.map { String(format: "%02x", $0) }.joined()
    }
}
