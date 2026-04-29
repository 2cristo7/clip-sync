import AppKit
import Combine
import Foundation

@MainActor
final class MenuBarController: NSObject {
    private var statusItem: NSStatusItem?
    private let hub: WebSocketHub
    let errorStore: ErrorStore
    private let onStartPairing: () -> Void
    private let onTailscale: () -> Void
    private let onQuit: () -> Void
    private var refreshTask: Task<Void, Never>?
    private var currentClients: [ClipClientInfo] = []
    private var cancellables = Set<AnyCancellable>()

    init(hub: WebSocketHub,
         errorStore: ErrorStore,
         onStartPairing: @escaping () -> Void,
         onTailscale: @escaping () -> Void,
         onQuit: @escaping () -> Void) {
        self.hub = hub
        self.errorStore = errorStore
        self.onStartPairing = onStartPairing
        self.onTailscale = onTailscale
        self.onQuit = onQuit
        super.init()
        errorStore.$errors
            .receive(on: DispatchQueue.main)
            .sink { [weak self] _ in self?.rebuildMenu() }
            .store(in: &cancellables)
    }

    func install() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        statusItem = item
        rebuildMenu()
        startObservingHub()
    }

    private func updateStatusIcon() {
        guard let button = statusItem?.button else { return }
        if errorStore.hasErrors {
            let image = NSImage(
                systemSymbolName: "exclamationmark.circle.fill",
                accessibilityDescription: "ClipSync — Error"
            )
            image?.isTemplate = false
            button.image = image
            button.contentTintColor = .systemRed
        } else if errorStore.hasWarnings {
            let image = NSImage(
                systemSymbolName: "exclamationmark.circle.fill",
                accessibilityDescription: "ClipSync — Warning"
            )
            image?.isTemplate = false
            button.image = image
            button.contentTintColor = .systemOrange
        } else {
            let image = NSImage(
                systemSymbolName: "doc.on.clipboard",
                accessibilityDescription: "ClipSync"
            )
            image?.isTemplate = true
            button.image = image
            button.contentTintColor = nil
        }
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
        updateStatusIcon()
        let menu = NSMenu()

        // Error/warning items at the top
        let currentErrors = errorStore.errors
        if !currentErrors.isEmpty {
            for appError in currentErrors {
                let prefix = appError.severity == .error ? "🔴 " : "⚠ "
                let item = NSMenuItem(title: prefix + appError.summary, action: nil, keyEquivalent: "")
                item.isEnabled = false
                menu.addItem(item)
            }
            menu.addItem(.separator())
        }

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

        let tailscaleTitle = TailscaleHelper.isInstalled
            ? "Tailscale Pairing…"
            : "Setup Tailscale…"
        let tailscaleItem = NSMenuItem(
            title: tailscaleTitle,
            action: #selector(handleTailscale),
            keyEquivalent: "t"
        )
        tailscaleItem.target = self
        menu.addItem(tailscaleItem)

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

    @objc private func handleTailscale() {
        onTailscale()
    }

    @objc private func handleQuit() {
        onQuit()
    }
}
