import AppKit
import Foundation
import Logging
import NIOSSL
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

@MainActor
final class AppDelegate: NSObject, NSApplicationDelegate {
    private let hub = WebSocketHub()
    private let watcher = PasteboardWatcher()
    private lazy var injector = PasteboardInjector(watcher: watcher)
    private let keychain = Keychain()
    private var pairingSecret: Data = Data()
    private lazy var pairing = PairingManager(secret: pairingSecret)
    private let tokenStore = TokenStore()
    private let tlsManager = TLSManager()
    private lazy var hmacValidator = HMACValidator(secret: pairingSecret)
    private var server: ClipServer?
    private lazy var menuBar = MenuBarController(
        hub: hub,
        onStartPairing: { [weak self] in self?.startPairing() },
        onQuit: { NSApp.terminate(nil) }
    )
    private var advertiser: BonjourAdvertiser?
    private var reachabilityMonitor: ReachabilityMonitor?
    private let pairingWindow = PairingWindowController()
    private var broadcastTask: Task<Void, Never>?
    private var logger = Logger(label: "clipsync.app")

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        do {
            pairingSecret = try keychain.loadOrCreateSecret()
        } catch {
            logger.error("Failed to load/create pairing secret: \(error)")
            pairingSecret = Data(count: 32)
        }
        hmacValidator = HMACValidator(secret: pairingSecret)
        var tlsConfig: TLSConfiguration? = nil
        do {
            try tlsManager.loadOrCreate()
            tlsConfig = try tlsManager.makeServerTLSConfiguration()
        } catch {
            logger.error("Failed to initialise TLS identity: \(error)")
        }
        menuBar.install()
        let server = ClipServer(
            hub: hub,
            injector: injector,
            pairing: pairing,
            tokenStore: tokenStore,
            hmacValidator: hmacValidator,
            tlsConfiguration: tlsConfig
        )
        self.server = server
        startPipeline()
        startAdvertising()
    }

    func applicationWillTerminate(_ notification: Notification) {
        broadcastTask?.cancel()
        watcher.stop()
        server?.stop()
        reachabilityMonitor?.stop()
        advertiser?.stop()
        menuBar.tearDown()
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
        server?.start()
    }

    private func startAdvertising() {
        let fp = tlsManager.spkiFingerprint.isEmpty
            ? PairingManager.fingerprint(of: pairingSecret)
            : tlsManager.spkiFingerprint
        let name = Self.deviceName()
        let txt: [String: String] = [
            "version": "0.1.0",
            "name": name,
            "fp": fp,
        ]
        let advertiser = BonjourAdvertiser(
            port: Int32(ServerConfig.default.port),
            serviceName: name,
            txtRecord: txt
        )
        advertiser.start()
        self.advertiser = advertiser

        let reachability = ReachabilityMonitor(advertiser: advertiser)
        reachability.start()
        self.reachabilityMonitor = reachability
    }

    private static func deviceName() -> String {
        let raw = ProcessInfo.processInfo.hostName
        if let base = raw.components(separatedBy: ".").first, !base.isEmpty {
            return base
        }
        return "ClipSync"
    }

    private func startPairing() {
        Task { @MainActor in
            do {
                // Reuse an existing valid session so the window always shows the
                // same code the server is expecting. Without this, clicking
                // "Start Pairing…" a second time generates a new code on the
                // server while the already-open window keeps displaying the old
                // one, causing every correct entry to be rejected as "invalid".
                let session: PairingSession
                if let existing = await pairing.currentSession() {
                    session = existing
                } else {
                    session = try await pairing.startPairing()
                }
                let hostname = ProcessInfo.processInfo.hostName
                pairingWindow.show(
                    code: session.code,
                    expiresAt: session.expiresAt,
                    hostname: hostname,
                    port: ServerConfig.default.port,
                    onRefresh: { [weak self] in
                        guard let self else { return nil }
                        return try? await self.pairing.startPairing()
                    }
                )
            } catch {
                logger.error("Failed to start pairing: \(error)")
            }
        }
    }
}
