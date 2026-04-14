import Foundation
import HTTPTypes
import Hummingbird
import HummingbirdWebSocket
import Logging

final class ClipServer {
    private let config: ServerConfig
    private var logger: Logger
    private let hub: WebSocketHub
    private let injector: PasteboardInjector
    private var runTask: Task<Void, Never>?

    init(config: ServerConfig = .default,
         hub: WebSocketHub,
         injector: PasteboardInjector) {
        self.config = config
        var logger = Logger(label: "clipsync.server")
        logger.logLevel = config.logLevel
        self.logger = logger
        self.hub = hub
        self.injector = injector
    }

    func start() {
        guard runTask == nil else { return }
        let config = self.config
        let logger = self.logger
        let hub = self.hub
        let injector = self.injector

        runTask = Task.detached(priority: .userInitiated) {
            do {
                let router = Self.makeRouter(hub: hub, injector: injector, logger: logger)
                let app = Application(
                    router: router,
                    server: .http1WebSocketUpgrade { request, _, logger in
                        guard request.path == "/ws" else { return .dontUpgrade }
                        return .upgrade([:]) { inbound, outbound, _ in
                            let client = WebSocketHub.Client(outbound: outbound)
                            await hub.register(client)
                            // autoPing in WebSocketServerConfiguration (30s default) supplies the heartbeats.
                            do {
                                for try await _ in inbound { }
                            } catch {
                                logger.debug("WebSocket ended: \(error)")
                            }
                            await hub.unregister(client)
                        }
                    },
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

    private static let version = "0.1.0"
    private static let platform = "macos"

    private static func makeRouter(hub: WebSocketHub,
                                   injector: PasteboardInjector,
                                   logger: Logger) -> Router<BasicRequestContext> {
        let router = Router()
        router.get("/health") { _, _ -> HealthResponse in
            HealthResponse(ok: true, version: version, platform: platform)
        }
        router.post("/inject") { request, context -> InjectResponse in
            let sourceTag = request.headers[HTTPField.Name("X-ClipSync-Source")!]
            let payload = try await request.decode(as: ClipPayload.self, context: context)
            context.logger.info("inject received", metadata: [
                "source": .string(sourceTag ?? "unknown"),
                "type": .string(payload.type.rawValue),
                "nonce": .string(payload.nonce),
            ])
            do {
                try injector.inject(payload)
            } catch {
                context.logger.error("inject failed: \(error)")
                throw HTTPError(.badRequest, message: String(describing: error))
            }
            await hub.broadcast(payload)
            return InjectResponse(ok: true, nonce: payload.nonce)
        }
        return router
    }

    private struct HealthResponse: ResponseEncodable, Sendable {
        let ok: Bool
        let version: String
        let platform: String
    }

    private struct InjectResponse: ResponseEncodable, Sendable {
        let ok: Bool
        let nonce: String
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
