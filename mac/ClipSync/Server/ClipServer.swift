import Foundation
import Hummingbird
import Logging

final class ClipServer {
    private let config: ServerConfig
    private var logger: Logger
    private var runTask: Task<Void, Never>?

    init(config: ServerConfig = .default) {
        self.config = config
        var logger = Logger(label: "clipsync.server")
        logger.logLevel = config.logLevel
        self.logger = logger
    }

    func start() {
        guard runTask == nil else { return }
        let config = self.config
        let logger = self.logger

        runTask = Task.detached(priority: .userInitiated) {
            do {
                let router = Self.makeRouter()
                let app = Application(
                    router: router,
                    configuration: .init(
                        address: .hostname(config.host, port: config.port),
                        serverName: "ClipSync"
                    ),
                    logger: logger
                )
                logger.info("ClipSync server starting on \(config.host):\(config.port)")
                try await app.runService()
            } catch {
                Self.logStartupError(error, config: config, logger: logger)
            }
        }
    }

    func stop() {
        runTask?.cancel()
        runTask = nil
    }

    private static func makeRouter() -> Router<BasicRequestContext> {
        Router()
    }

    private static func logStartupError(
        _ error: Error,
        config: ServerConfig,
        logger: Logger
    ) {
        let description = String(describing: error)
        if description.contains("addressInUse")
            || description.contains("EADDRINUSE")
            || description.localizedCaseInsensitiveContains("address already in use")
        {
            logger.error(
                """
                Port \(config.port) is already in use on \(config.host). \
                Another ClipSync instance or a different process is holding it. \
                Stop the other process and relaunch.
                """
            )
        } else {
            logger.error("ClipSync server failed to start on \(config.host):\(config.port): \(error)")
        }
    }
}
