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

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        installStatusItem()
    }

    func applicationWillTerminate(_ notification: Notification) {}

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
