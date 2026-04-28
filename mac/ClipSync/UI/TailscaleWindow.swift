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
        .frame(width: 380, height: 380)
        .onAppear { refresh() }
    }

    private func installedView(ip: String) -> some View {
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
                Button("On same WiFi? Pair locally") {
                    onStartPairing()
                }
                .buttonStyle(.bordered)
                Button("Pair via Tailscale") {
                    onStartPairing()
                }
                .buttonStyle(.borderedProminent)
            }
        }
    }

    private var runningButNoIPView: some View {
        VStack(spacing: 16) {
            Image(systemName: "wifi.exclamationmark")
                .font(.system(size: 48))
                .foregroundStyle(.orange)
            Text("Tailscale VPN not connected")
                .font(.headline)
            Text("Tailscale is installed but the VPN is not active.\nConnect to the VPN to get your Tailscale IP.")
                .font(.subheadline)
                .foregroundStyle(.secondary)
                .multilineTextAlignment(.center)
                .fixedSize(horizontal: false, vertical: true)
            HStack(spacing: 12) {
                Button("Open Tailscale") {
                    NSWorkspace.shared.open(URL(fileURLWithPath: "/Applications/Tailscale.app"))
                }
                .buttonStyle(.borderedProminent)
                Button("Refresh") { refresh() }
                    .buttonStyle(.bordered)
            }
            Text("The view will refresh automatically once connected.")
                .font(.caption2)
                .foregroundStyle(.tertiary)
        }
        .onReceive(Timer.publish(every: 3, on: .main, in: .common).autoconnect()) { _ in
            refresh()
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
        guard isInstalled else { return }
        // ipv4() spawns a subprocess — never block the main thread with waitUntilExit().
        Task.detached(priority: .utility) {
            let ip = TailscaleHelper.ipv4()
            await MainActor.run { tailscaleIP = ip }
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
