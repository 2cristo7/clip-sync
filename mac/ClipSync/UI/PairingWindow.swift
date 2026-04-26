import AppKit
import CoreImage
import CoreImage.CIFilterBuiltins
import Foundation
import SwiftUI

struct PairingView: View {
    @State private var code: String
    @State private var expiresAt: Date
    let hostname: String
    let port: Int
    let onRefresh: () async -> PairingSession?

    @State private var remaining: TimeInterval = 0
    @State private var cachedQRImage: NSImage?
    @State private var isRefreshing = false

    init(code: String, expiresAt: Date, hostname: String, port: Int,
         onRefresh: @escaping () async -> PairingSession?) {
        self._code = State(initialValue: code)
        self._expiresAt = State(initialValue: expiresAt)
        self.hostname = hostname
        self.port = port
        self.onRefresh = onRefresh
    }

    var body: some View {
        VStack(spacing: 20) {
            Text("Pair Device")
                .font(.title2.weight(.semibold))
            Text(formattedCode)
                .font(.system(size: 52, weight: .semibold, design: .monospaced))
                .tracking(6)
            if let qr = cachedQRImage {
                Image(nsImage: qr)
                    .interpolation(.none)
                    .resizable()
                    .frame(width: 220, height: 220)
            }
            Text(remaining > 0
                 ? String(format: "Expires in %d:%02d", Int(remaining) / 60, Int(remaining) % 60)
                 : "Expired")
                .font(.system(.title3, design: .monospaced))
                .foregroundStyle(remaining <= 30 ? .red : .secondary)
            Text(pairingURL)
                .font(.caption.monospaced())
                .foregroundStyle(.secondary)
                .textSelection(.enabled)
            Button {
                Task { await refresh() }
            } label: {
                Label("New Code", systemImage: "arrow.clockwise")
            }
            .disabled(isRefreshing)
        }
        .padding(28)
        .frame(width: 380, height: 510)
        .onAppear {
            cachedQRImage = Self.generateQRImage(for: pairingURL)
            updateRemaining()
        }
        .onReceive(Timer.publish(every: 1, on: .main, in: .common).autoconnect()) { _ in
            updateRemaining()
        }
    }

    private func refresh() async {
        isRefreshing = true
        defer { isRefreshing = false }
        guard let session = await onRefresh() else { return }
        code = session.code
        expiresAt = session.expiresAt
        cachedQRImage = Self.generateQRImage(for: pairingURL)
        updateRemaining()
    }

    private var formattedCode: String {
        guard code.count == 6 else { return code }
        let half = code.index(code.startIndex, offsetBy: 3)
        return "\(code[..<half]) \(code[half...])"
    }

    private var pairingURL: String {
        "clipsync://pair?host=\(hostname)&port=\(port)&code=\(code)"
    }

    private func updateRemaining() {
        remaining = max(0, expiresAt.timeIntervalSinceNow)
    }

    private static func generateQRImage(for string: String) -> NSImage? {
        let filter = CIFilter.qrCodeGenerator()
        filter.message = Data(string.utf8)
        filter.correctionLevel = "M"
        guard let output = filter.outputImage else { return nil }
        let scaled = output.transformed(by: CGAffineTransform(scaleX: 8, y: 8))
        let context = CIContext()
        guard let cg = context.createCGImage(scaled, from: scaled.extent) else { return nil }
        let size = NSSize(width: scaled.extent.width, height: scaled.extent.height)
        return NSImage(cgImage: cg, size: size)
    }
}

@MainActor
final class PairingWindowController {
    private var window: NSWindow?
    private var shownCode: String?

    private static let windowSize = NSSize(width: 380, height: 510)

    func show(code: String, expiresAt: Date, hostname: String, port: Int,
              onRefresh: @escaping () async -> PairingSession?) {
        if let window, shownCode == code {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }
        // Code changed (new session) — close stale window and open fresh one.
        window?.close()
        window = nil
        shownCode = code

        let view = PairingView(
            code: code,
            expiresAt: expiresAt,
            hostname: hostname,
            port: port,
            onRefresh: onRefresh
        )
        let hosting = NSHostingController(rootView: view)
        hosting.sizingOptions = []
        let win = NSWindow(contentViewController: hosting)
        win.title = "Pair Device"
        win.styleMask = [.titled, .closable]
        win.isReleasedWhenClosed = false
        win.setContentSize(Self.windowSize)
        win.contentMinSize = Self.windowSize
        win.contentMaxSize = Self.windowSize
        win.center()
        win.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        window = win
    }

    func close() {
        window?.close()
        window = nil
        shownCode = nil
    }
}
