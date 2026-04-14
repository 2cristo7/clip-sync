import AppKit
import XCTest
@testable import ClipSync

final class FakePasteboard: PasteboardReading, PasteboardWriting {
    private(set) var changeCount: Int = 0
    private var storage: [NSPasteboard.PasteboardType: Data] = [:]

    func types() -> [NSPasteboard.PasteboardType]? {
        Array(storage.keys)
    }

    func string(forType type: NSPasteboard.PasteboardType) -> String? {
        guard let data = storage[type] else { return nil }
        return String(data: data, encoding: .utf8)
    }

    func data(forType type: NSPasteboard.PasteboardType) -> Data? {
        storage[type]
    }

    @discardableResult
    func clearContents() -> Int {
        storage.removeAll()
        changeCount += 1
        return changeCount
    }

    @discardableResult
    func setData(_ data: Data?, forType dataType: NSPasteboard.PasteboardType) -> Bool {
        guard let data else { return false }
        storage[dataType] = data
        changeCount += 1
        return true
    }

    @discardableResult
    func setString(_ string: String, forType dataType: NSPasteboard.PasteboardType) -> Bool {
        storage[dataType] = Data(string.utf8)
        changeCount += 1
        return true
    }

    /// Simulate an external writer (e.g. user copies in another app).
    func externalWriteText(_ text: String) {
        clearContents()
        _ = setString(text, forType: .string)
    }

    func externalWriteImage(_ data: Data, type: NSPasteboard.PasteboardType = .png) {
        clearContents()
        _ = setData(data, forType: type)
    }
}

final class PasteboardRoundtripTests: XCTestCase {
    func testWatcherEmitsTextOnChange() throws {
        let pb = FakePasteboard()
        let watcher = PasteboardWatcher(pasteboard: pb, intervalMillis: 50)

        var received: [ClipPayload] = []
        let stream = watcher.events()
        let iterator = Task { () -> ClipPayload? in
            for await payload in stream {
                return payload
            }
            return nil
        }

        // Give the AsyncStream's continuation time to register on the watcher queue.
        Thread.sleep(forTimeInterval: 0.05)
        pb.externalWriteText("hola mundo")
        watcher.pollNow()

        let payload = try XCTUnwrap(runAsyncAndWait(iterator, timeout: 1.0).flatMap { $0 })
        received.append(payload)
        iterator.cancel()

        XCTAssertEqual(received.first?.type, .text)
        XCTAssertEqual(received.first?.rawData.flatMap { String(data: $0, encoding: .utf8) }, "hola mundo")
    }

    func testInjectorRoundtripText() throws {
        let pb = FakePasteboard()
        let injector = PasteboardInjector(pasteboard: pb)

        let payload = ClipPayload.text("via inject")
        try injector.inject(payload)

        XCTAssertEqual(pb.string(forType: .string), "via inject")
    }

    func testInjectorRoundtripImage() throws {
        let pb = FakePasteboard()
        let injector = PasteboardInjector(pasteboard: pb)
        let bytes = Data([0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x01, 0x02, 0x03])

        let payload = ClipPayload.image(bytes, mime: "image/png")
        try injector.inject(payload)

        XCTAssertEqual(pb.data(forType: .png), bytes)
    }

    func testAntiLoopSuppressesEchoTick() throws {
        let pb = FakePasteboard()
        let watcher = PasteboardWatcher(pasteboard: pb, intervalMillis: 50)
        let injector = PasteboardInjector(pasteboard: pb, watcher: watcher)

        var received: [ClipPayload] = []
        let collector = Task {
            for await payload in watcher.events() {
                received.append(payload)
            }
        }

        // Allow the continuation to register.
        Thread.sleep(forTimeInterval: 0.05)

        // Inject three times; the watcher should emit none because each write is suppressed.
        for value in ["one", "two", "three"] {
            try injector.inject(ClipPayload.text(value))
            watcher.pollNow()
        }
        XCTAssertEqual(received.count, 0, "Injected payloads must not be re-broadcast")

        // Now simulate a genuine external change — must be emitted.
        pb.externalWriteText("external")
        watcher.pollNow()

        let deadline = Date().addingTimeInterval(1.0)
        while received.isEmpty && Date() < deadline {
            Thread.sleep(forTimeInterval: 0.02)
        }
        collector.cancel()

        XCTAssertEqual(received.count, 1, "External change must produce exactly one broadcast")
        XCTAssertEqual(received.first?.rawData.flatMap { String(data: $0, encoding: .utf8) }, "external")
    }

    // MARK: - Helpers

    private func runAsyncAndWait<T>(_ task: Task<T, Never>, timeout: TimeInterval) -> T? {
        let expectation = expectation(description: "async task")
        var result: T?
        Task {
            result = await task.value
            expectation.fulfill()
        }
        wait(for: [expectation], timeout: timeout)
        return result
    }
}
