import AppKit
import SwiftUI

struct TailscaleSetupView: View {
    let onStartPairing: () -> Void

    @State private var tailscaleIP: String?
    @State private var isInstalled = false

    var body: some View {
        VStack(spacing: 20) {
            Text("Tailscale Setup")
                .font(.title2.weight(.semibold))

            if isInstalled, let ip = tailscaleIP {
                installedView(ip: ip)
            } else if isInstalled {
                runningButNoIPView
            } else {
                notInstalledView
            }
        }
        .padding(28)
        .frame(width: 380, height: 340)
        .onAppear { refresh() }
    }

    private func installedView(ip: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 48))
                .foregroundStyle(.green)
            Text("Tailscale is connected")
                .font(.headline)
            Text("Your Tailscale IP:")
                .font(.subheadline)
                .foregroundStyle(.secondary)
            Text(ip)
                .font(.system(size: 36, weight: .semibold, design: .monospaced))
                .textSelection(.enabled)
            Text("Enter this IP on your Android device, then pair.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            Button("Start Pairing…") {
                onStartPairing()
            }
            .buttonStyle(.borderedProminent)
        }
    }

    private var runningButNoIPView: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 48))
                .foregroundStyle(.orange)
            Text("Tailscale is installed but not connected")
                .font(.headline)
            Text("Open Tailscale and log in to get your IP address.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            Button("Open Tailscale") {
                NSWorkspace.shared.open(URL(fileURLWithPath: "/Applications/Tailscale.app"))
            }
            Button("Refresh") { refresh() }
                .buttonStyle(.bordered)
        }
    }

    private var notInstalledView: some View {
        VStack(spacing: 16) {
            Image(systemName: "arrow.down.circle.fill")
                .font(.system(size: 48))
                .foregroundStyle(.blue)
            Text("Tailscale not installed")
                .font(.headline)
            Text("Tailscale lets you sync your clipboard from anywhere — not just your home WiFi.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
            Button("Install from App Store") {
                TailscaleHelper.openDownloadPage()
            }
            .buttonStyle(.borderedProminent)
            Button("Refresh") { refresh() }
                .buttonStyle(.bordered)
        }
    }

    private func refresh() {
        isInstalled = TailscaleHelper.isInstalled
        if isInstalled {
            tailscaleIP = TailscaleHelper.ipv4()
        }
    }
}

@MainActor
final class TailscaleWindowController {
    private var window: NSWindow?

    private static let windowSize = NSSize(width: 380, height: 340)

    func show(onStartPairing: @escaping () -> Void) {
        if let window {
            window.makeKeyAndOrderFront(nil)
            NSApp.activate(ignoringOtherApps: true)
            return
        }
        let view = TailscaleSetupView(onStartPairing: { [weak self] in
            self?.close()
            onStartPairing()
        })
        let hosting = NSHostingController(rootView: view)
        hosting.sizingOptions = []
        let win = NSWindow(contentViewController: hosting)
        win.title = "Tailscale Setup"
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
    }
}
