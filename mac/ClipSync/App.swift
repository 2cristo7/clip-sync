import AppKit
import Foundation
import Logging
import NIOSSL
import SwiftUI
import UserNotifications

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
    private lazy var hub = WebSocketHub(errorStore: errorStore)
    private let watcher = PasteboardWatcher()
    private lazy var injector = PasteboardInjector(watcher: watcher)
    private let keychain = Keychain()
    private var pairingSecret: Data = Data()
    private lazy var pairing = PairingManager(secret: pairingSecret)
    private let tokenStore = TokenStore()
    private let tlsManager = TLSManager()
    private lazy var hmacValidator = HMACValidator(secret: pairingSecret)
    private var server: ClipServer?
    let errorStore = ErrorStore()
    private lazy var menuBar = MenuBarController(
        hub: hub,
        errorStore: errorStore,
        onStartPairing: { [weak self] in self?.startPairing() },
        onTailscale: { [weak self] in self?.showTailscale() },
        onToggleSync: { [weak self] in self?.toggleSync() },
        onQuit: { NSApp.terminate(nil) }
    )
    private var advertiser: BonjourAdvertiser?
    private var reachabilityMonitor: ReachabilityMonitor?
    private let pairingWindow = PairingWindowController()
    private let tailscaleWindow = TailscaleWindowController()
    private var broadcastTask: Task<Void, Never>?
    private var serverTask: Task<Void, Never>?
    private var logger = Logger(label: "clipsync.app")

    func applicationDidFinishLaunching(_ notification: Notification) {
        NSApp.setActivationPolicy(.accessory)
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert]) { _, _ in }
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
            errorStore.append(AppError(
                severity: .warning,
                summary: "Running without TLS encryption",
                detail: "TLS setup failed: \(error.localizedDescription)",
                suggestion: "Restart ClipSync. Clipboard data will be sent unencrypted on your network."
            ))
        }
        menuBar.install()
        let server = ClipServer(
            hub: hub,
            injector: injector,
            pairing: pairing,
            tokenStore: tokenStore,
            hmacValidator: hmacValidator,
            tlsConfiguration: tlsConfig,
            errorStore: errorStore
        )
        self.server = server
        startPipeline()
        startAdvertising()
    }

    func applicationWillTerminate(_ notification: Notification) {
        broadcastTask?.cancel()
        serverTask?.cancel()
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

        serverTask = Task.detached { [weak self] in
            guard let server = await self?.server else { return }
            var retries = 0
            let maxRetries = 3

            while retries < maxRetries {
                do {
                    try await server.run()
                    // run() returned normally — clean shutdown, stop retrying.
                    break
                } catch {
                    // Ignore cancellation — this is an intentional shutdown.
                    if Task.isCancelled { break }
                    let description = String(describing: error)
                    let isPortInUse = description.contains("addressInUse")
                        || description.contains("EADDRINUSE")
                        || description.localizedCaseInsensitiveContains("address already in use")
                    let logger = await self?.logger
                    let errorStore = self?.errorStore
                    let port = ServerConfig.defaultPort

                    if isPortInUse {
                        logger?.error("Port \(port) already in use — another ClipSync instance may be running")
                        await MainActor.run {
                            errorStore?.appendAndNotify(AppError(
                                severity: .error,
                                summary: "Port \(port) already in use",
                                detail: "Another ClipSync instance is already running.",
                                suggestion: "Quit the other instance from the menu bar."
                            ))
                        }
                        break  // No point retrying — port won't free itself
                    }

                    retries += 1
                    logger?.error("Server crashed (attempt \(retries)/\(maxRetries)): \(error)")

                    if retries < maxRetries {
                        logger?.info("Restarting server in 5s...")
                        try? await Task.sleep(for: .seconds(5))
                        if Task.isCancelled { break }
                    } else {
                        logger?.error("Server failed after \(maxRetries) attempts, giving up")
                        let detail = error.localizedDescription
                        await MainActor.run {
                            errorStore?.appendAndNotify(AppError(
                                severity: .error,
                                summary: "Server stopped unexpectedly",
                                detail: detail,
                                suggestion: "Restart ClipSync manually."
                            ))
                        }
                    }
                }
            }
        }
    }

    private func startAdvertising() {
        let name = Self.deviceName()
        var txt: [String: String] = [
            "version": "0.1.0",
            "name": name,
        ]
        if !tlsManager.spkiFingerprint.isEmpty {
            txt["fp"] = tlsManager.spkiFingerprint
        }
        let advertiser = BonjourAdvertiser(
            port: Int32(ServerConfig.default.port),
            serviceName: name,
            txtRecord: txt
        )
        advertiser.onPublishFailed = { [weak self] error in
            Task { @MainActor in
                self?.errorStore.append(AppError(
                    severity: .warning,
                    summary: "mDNS advertising failed",
                    detail: error.localizedDescription,
                    suggestion: "Devices on your network may not find this Mac automatically."
                ))
            }
        }
        advertiser.start()
        self.advertiser = advertiser

        let reachability = ReachabilityMonitor(advertiser: advertiser)
        reachability.onNetworkChange = { [weak self] in
            Task { @MainActor in
                self?.logger.info("Network changed — verifying server health")
            }
        }
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

    private func showTailscale() {
        tailscaleWindow.show(onStartPairing: { [weak self] in
            self?.startPairing()
        })
    }

    private func toggleSync() {
        let nowPaused = !menuBar.isSyncPaused
        if nowPaused {
            watcher.stop()
            logger.info("Sync paused by user")
        } else {
            watcher.start()
            logger.info("Sync resumed by user")
        }
        menuBar.setSyncPaused(nowPaused)
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
                let hostname = TLSManager.primaryIPv4Address() ?? ProcessInfo.processInfo.hostName
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
