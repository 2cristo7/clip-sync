import Foundation
import Logging
import Network

/// Monitors network path changes via `NWPathMonitor` and re-announces
/// the Bonjour service when interfaces change (e.g. Wi-Fi ↔ Ethernet,
/// VPN tunnel up/down).
///
/// Usage:
/// ```swift
/// let monitor = ReachabilityMonitor(advertiser: bonjourAdvertiser)
/// monitor.start()
/// // later…
/// monitor.stop()
/// ```
final class ReachabilityMonitor {

    private let monitor: NWPathMonitor
    private let queue: DispatchQueue
    private let advertiser: BonjourAdvertiser
    private var logger: Logger
    private var lastInterfaceNames: Set<String> = []
    private var isRunning = false

    /// Called on the reachability queue whenever the set of active network
    /// interfaces changes. Use this to react in AppDelegate (e.g. verify
    /// server health after a network change).
    var onNetworkChange: (() -> Void)?

    init(
        advertiser: BonjourAdvertiser,
        pathMonitor: NWPathMonitor = NWPathMonitor(),
        queue: DispatchQueue = DispatchQueue(label: "clipsync.reachability", qos: .utility),
        logger: Logger = Logger(label: "clipsync.reachability")
    ) {
        self.advertiser = advertiser
        self.monitor = pathMonitor
        self.queue = queue
        self.logger = logger
    }

    /// Begin monitoring. Safe to call multiple times; subsequent calls are no-ops.
    func start() {
        guard !isRunning else { return }
        isRunning = true

        monitor.pathUpdateHandler = { [weak self] path in
            self?.handlePathUpdate(path)
        }
        monitor.start(queue: queue)
        logger.info("ReachabilityMonitor started")
    }

    /// Stop monitoring and release resources.
    func stop() {
        guard isRunning else { return }
        isRunning = false
        monitor.cancel()
        logger.info("ReachabilityMonitor stopped")
    }

    // MARK: - Internal (visible for testing)

    /// Returns the set of interface names from a path.
    static func interfaceNames(from path: NWPath) -> Set<String> {
        Set(path.availableInterfaces.map(\.name))
    }

    // MARK: - Private

    private func handlePathUpdate(_ path: NWPath) {
        let names = Self.interfaceNames(from: path)
        let status = path.status

        logger.info("Network path changed", metadata: [
            "status": .string(String(describing: status)),
            "interfaces": .string(names.sorted().joined(separator: ", ")),
        ])

        // Re-announce Bonjour when the set of available interfaces changes.
        // This handles scenarios like:
        //  - Wi-Fi → Ethernet switch
        //  - VPN/Tailscale utun interface appearing or disappearing
        //  - Waking from sleep with a different network
        if names != lastInterfaceNames {
            lastInterfaceNames = names
            if status == .satisfied {
                logger.info("Interfaces changed, re-announcing Bonjour service")
                advertiser.stop()
                // Delay start to avoid a race where stop() hasn't fully unwound
                // before start() schedules the new NetService (race R3).
                Task {
                    try? await Task.sleep(for: .milliseconds(500))
                    advertiser.start()
                }
                onNetworkChange?()
            }
        }
    }
}
