import AppKit
import SwiftUI

@main
struct ClipSyncApp: App {
    @NSApplicationDelegateAdaptor(AppDelegate.self) private var appDelegate

    var body: some Scene {
        Settings {
            EmptyView()
        }
    }
}

final class AppDelegate: NSObject, NSApplicationDelegate {
    private var statusItem: NSStatusItem?
    private let hub = WebSocketHub()
    private let watcher = PasteboardWatcher()
    private lazy var server = ClipServer(hub: hub)
    private var broadcastTask: Task<Void, Never>?

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        installStatusItem()
        startPipeline()
    }

    func applicationWillTerminate(_ notification: Notification) {
        broadcastTask?.cancel()
        watcher.stop()
        server.stop()
    }

    private func startPipeline() {
        let hub = self.hub
        let stream = watcher.events()
        broadcastTask = Task.detached(priority: .utility) {
            for await payload in stream {
                await hub.broadcast(payload)
            }
        }
        watcher.start()
        server.start()
    }

    private func installStatusItem() {
        let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let button = item.button {
            let image = NSImage(
                systemSymbolName: "doc.on.clipboard",
                accessibilityDescription: "ClipSync"
            )
            image?.isTemplate = true
            button.image = image
        }

        let menu = NSMenu()
        menu.addItem(
            NSMenuItem(
                title: "Quit ClipSync",
                action: #selector(quit),
                keyEquivalent: "q"
            )
        )
        menu.items.last?.target = self
        item.menu = menu
        self.statusItem = item
    }

    @objc private func quit() {
        NSApp.terminate(nil)
    }
}
