import AppKit
import Foundation

@MainActor
final class MenuBarController: NSObject {
    private var statusItem: NSStatusItem?
    private let hub: WebSocketHub
    private let onStartPairing: () -> Void
    private let onQuit: () -> Void
    private var refreshTask: Task<Void, Never>?
    private var currentClients: [ClipClientInfo] = []

    init(hub: WebSocketHub,
         onStartPairing: @escaping () -> Void,
         onQuit: @escaping () -> Void) {
        self.hub = hub
        self.onStartPairing = onStartPairing
        self.onQuit = onQuit
    }

    func install() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            let image = NSImage(
                systemSymbolName: "doc.on.clipboard",
                accessibilityDescription: "ClipSync"
            )
            image?.isTemplate = true
            button.image = image
        }
        statusItem = item
        rebuildMenu()
        startObservingHub()
    }

    func tearDown() {
        refreshTask?.cancel()
        refreshTask = nil
        if let item = statusItem {
            NSStatusBar.system.removeStatusItem(item)
            statusItem = nil
        }
    }

    private func startObservingHub() {
        let hub = self.hub
        refreshTask = Task { [weak self] in
            let stream = await hub.events()
            for await clients in stream {
                await MainActor.run {
                    self?.currentClients = clients
                    self?.rebuildMenu()
                }
            }
        }
    }

    private func rebuildMenu() {
        let menu = NSMenu()
        let statusText: String = currentClients.isEmpty
            ? "⚪️ Idle"
            : "🟢 Connected (\(currentClients.count))"
        let statusMenuItem = NSMenuItem(title: statusText, action: nil, keyEquivalent: "")
        statusMenuItem.isEnabled = false
        menu.addItem(statusMenuItem)

        let pairItem = NSMenuItem(
            title: "Start Pairing…",
            action: #selector(handleStartPairing),
            keyEquivalent: "p"
        )
        pairItem.target = self
        menu.addItem(pairItem)

        if !currentClients.isEmpty {
            let clientsItem = NSMenuItem(title: "Clients", action: nil, keyEquivalent: "")
            let submenu = NSMenu()
            let formatter = DateFormatter()
            formatter.dateStyle = .none
            formatter.timeStyle = .medium
            for client in currentClients {
                let address = client.remoteAddress ?? String(client.id.uuidString.prefix(8))
                let seen = formatter.string(from: client.lastSeen)
                let entry = NSMenuItem(
                    title: "\(address) — seen \(seen)",
                    action: nil,
                    keyEquivalent: ""
                )
                entry.isEnabled = false
                submenu.addItem(entry)
            }
            clientsItem.submenu = submenu
            menu.addItem(clientsItem)
        }

        menu.addItem(.separator())
        let quitItem = NSMenuItem(
            title: "Quit ClipSync",
            action: #selector(handleQuit),
            keyEquivalent: "q"
        )
        quitItem.target = self
        menu.addItem(quitItem)
        statusItem?.menu = menu
    }

    @objc private func handleStartPairing() {
        onStartPairing()
    }

    @objc private func handleQuit() {
        onQuit()
    }
}
