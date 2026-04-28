import AppKit
import SwiftUI

struct TailscaleSetupView: View {
    let onStartPairing: () -> Void

    @State private var state: TailscaleState = .notInstalled
    @State private var isRefreshing = false

    var body: some View {
        VStack(spacing: 20) {
            Text("Tailscale Setup")
                .font(.title2.weight(.semibold))
            stateView
        }
        .padding(28)
        .frame(width: 380, height: 380)
        .onAppear { refresh() }
    }

    @ViewBuilder
    private var stateView: some View {
        switch state {
        case .notInstalled:
            notInstalledView
        case .daemonDown:
            daemonDownView
        case .notLoggedIn:
            notLoggedInView
        case .disconnected:
            disconnectedView
        case .connected(let ip):
            connectedView(ip: ip)
        }
    }

    private func connectedView(ip: String) -> some View {
        VStack(spacing: 16) {
            Image(systemName: "checkmark.circle.fill")
                .font(.system(size: 48))
                .foregroundStyle(.green)
            Text("Tailscale is connected")
                .font(.headline)
            VStack(spacing: 4) {
                Text("Your Tailscale IP")
                    .font(.subheadline)
                    .foregroundStyle(.secondary)
                Text(ip)
                    .font(.system(size: 32, weight: .semibold, design: .monospaced))
                    .textSelection(.enabled)
                    .padding(.horizontal, 12)
                    .padding(.vertical, 6)
                    .background(.quaternary, in: RoundedRectangle(cornerRadius: 8))
            }
            Text("On your Android:\nTailscale section → \"Switch to Tailscale\"\n→ enter this IP → Pair.")
                .font(.caption)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            Divider()
            HStack(spacing: 12) {
                Button("Pair locally (WiFi)") { onStartPairing() }
                    .buttonStyle(.bordered)
                Button("Pair via Tailscale") { onStartPairing() }
                    .buttonStyle(.borderedProminent)
            }
        }
    }

    private var disconnectedView: some View {
        VStack(spacing: 16) {
            Image(systemName: "wifi.exclamationmark")
                .font(.system(size: 48))
                .foregroundStyle(.orange)
            Text("Tailscale VPN not connected")
                .font(.headline)
            Text("Tailscale is installed but the VPN is off.\nOpen Tailscale and connect to continue.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 12) {
                Button("Open Tailscale") { TailscaleHelper.openApp() }
                    .buttonStyle(.borderedProminent)
                Button("Refresh") { refresh() }
                    .buttonStyle(.bordered)
                    .disabled(isRefreshing)
            }
            Text("Refreshes automatically every 3 s.")
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
        .onReceive(Timer.publish(every: 3, on: .main, in: .common).autoconnect()) { _ in
            refresh()
        }
    }

    private var daemonDownView: some View {
        VStack(spacing: 16) {
            Image(systemName: "exclamationmark.triangle.fill")
                .font(.system(size: 48))
                .foregroundStyle(.yellow)
            Text("Tailscale daemon not running")
                .font(.headline)
            Text("The Tailscale network extension is not active.\nOpen Tailscale to start it — or enable it in\nSystem Settings → Privacy & Security → Network Extensions.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 12) {
                Button("Open Tailscale") { TailscaleHelper.openApp() }
                    .buttonStyle(.borderedProminent)
                Button("System Settings") { TailscaleHelper.openNetworkExtensionSettings() }
                    .buttonStyle(.bordered)
            }
            Button("Refresh") { refresh() }
                .buttonStyle(.bordered)
                .disabled(isRefreshing)
        }
        .onReceive(Timer.publish(every: 3, on: .main, in: .common).autoconnect()) { _ in
            refresh()
        }
    }

    private var notLoggedInView: some View {
        VStack(spacing: 16) {
            Image(systemName: "person.crop.circle.badge.exclamationmark")
                .font(.system(size: 48))
                .foregroundStyle(.orange)
            Text("Not logged in to Tailscale")
                .font(.headline)
            Text("Open Tailscale and log in to your account to connect your devices.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 12) {
                Button("Open Tailscale") { TailscaleHelper.openApp() }
                    .buttonStyle(.borderedProminent)
                Button("Refresh") { refresh() }
                    .buttonStyle(.bordered)
                    .disabled(isRefreshing)
            }
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
            Button("Install from App Store") { TailscaleHelper.openDownloadPage() }
                .buttonStyle(.borderedProminent)
            Button("Refresh") { refresh() }
                .buttonStyle(.bordered)
                .disabled(isRefreshing)
        }
    }

    private func refresh() {
        guard !isRefreshing else { return }
        isRefreshing = true
        Task.detached(priority: .utility) {
            let detected = TailscaleHelper.detect()
            await MainActor.run {
                state = detected
                isRefreshing = false
            }
        }
    }
}

@MainActor
final class TailscaleWindowController {
    private var window: NSWindow?
    private static let windowSize = NSSize(width: 380, height: 380)

    func show(onStartPairing: @escaping () -> Void) {
        window?.close()
        window = nil
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
