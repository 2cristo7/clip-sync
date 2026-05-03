import Foundation
import HTTPTypes
import Hummingbird
import HummingbirdCore
import HummingbirdWebSocket
import HummingbirdTLS
import NIOSSL
import Logging

extension PairingResponse: ResponseEncodable {}

final class ClipServer {
    private let config: ServerConfig
    private var logger: Logger
    private let hub: WebSocketHub
    private let injector: PasteboardInjector
    private let pairing: PairingManager
    private let tokenStore: TokenStore
    private let hmacValidator: HMACValidator
    private let tlsConfiguration: TLSConfiguration?
    private var runTask: Task<Void, Never>?
    private let errorStore: ErrorStore
    let rateLimiter = RateLimiter()

    init(config: ServerConfig = .default,
         hub: WebSocketHub,
         injector: PasteboardInjector,
         pairing: PairingManager,
         tokenStore: TokenStore,
         hmacValidator: HMACValidator,
         tlsConfiguration: TLSConfiguration? = nil,
         errorStore: ErrorStore) {
        self.config = config
        var logger = Logger(label: "clipsync.server")
        logger.logLevel = config.logLevel
        self.logger = logger
        self.hub = hub
        self.injector = injector
        self.pairing = pairing
        self.tokenStore = tokenStore
        self.hmacValidator = hmacValidator
        self.tlsConfiguration = tlsConfiguration
        self.errorStore = errorStore
    }

    /// Runs the server until it exits or throws. Propagates errors to the caller.
    func run() async throws {
        let router = Self.makeRouter(
            hub: hub,
            injector: injector,
            pairing: pairing,
            tokenStore: tokenStore,
            hmacValidator: hmacValidator,
            rateLimiter: rateLimiter,
            logger: logger
        )

        let wsBuilder = HTTPServerBuilder.http1WebSocketUpgrade { [tokenStore, hub] request, _, logger in
            guard request.path == "/ws" else { return .dontUpgrade }
            // Enforce Bearer auth on the WS upgrade handshake.
            let authHeader = request.headerFields[HTTPField.Name("Authorization")!]
            guard let token = AuthMiddleware<BasicRequestContext>.extractBearer(authHeader),
                  (try? await tokenStore.validate(tokenPlain: token)) != nil else {
                logger.info("WS upgrade rejected: missing or invalid bearer")
                return .dontUpgrade
            }
            return .upgrade([:]) { inbound, outbound, _ in
                let client = WebSocketHub.Client(outbound: outbound)
                await hub.register(client)
                do {
                    for try await _ in inbound { }
                } catch {
                    logger.debug("WebSocket ended: \(error)")
                }
                await hub.unregister(client)
            }
        }

        let serverBuilder: HTTPServerBuilder
        if let tlsConfiguration {
            serverBuilder = try .tls(wsBuilder, tlsConfiguration: tlsConfiguration)
        } else {
            serverBuilder = wsBuilder
        }

        let app = Application(
            router: router,
            server: serverBuilder,
            configuration: .init(
                address: .hostname(config.host, port: config.port),
                serverName: "ClipSync"
            ),
            logger: logger
        )
        logger.info("ClipSync server starting on \(config.host):\(config.port)", metadata: [
            "tls": .stringConvertible(tlsConfiguration != nil),
        ])
        await hub.startPingLoop()
        try await app.runService()
    }

    func start() {
        guard runTask == nil else { return }
        let config = self.config
        let logger = self.logger
        let errorStore = self.errorStore

        runTask = Task.detached(priority: .userInitiated) { [weak self] in
            guard let self else { return }
            do {
                try await self.run()
            } catch {
                Self.logStartupError(error, config: config, logger: logger)
                let description = String(describing: error)
                let isPortInUse = description.contains("addressInUse")
                    || description.contains("EADDRINUSE")
                    || description.localizedCaseInsensitiveContains("address already in use")
                await MainActor.run {
                    if isPortInUse {
                        errorStore.appendAndNotify(AppError(
                            severity: .error,
                            summary: "Port \(config.port) already in use",
                            detail: error.localizedDescription,
                            suggestion: "Close other ClipSync instances or change the port."
                        ))
                    } else {
                        errorStore.appendAndNotify(AppError(
                            severity: .error,
                            summary: "Server failed to start",
                            detail: error.localizedDescription,
                            suggestion: "Check the logs and restart ClipSync."
                        ))
                    }
                }
            }
        }
    }

    func stop() {
        runTask?.cancel()
        runTask = nil
        Task { await hub.stop() }
    }

    private static let version = "0.1.0"
    private static let platform = "macos"

    static func makeRouter(hub: WebSocketHub,
                           injector: PasteboardInjector,
                           pairing: PairingManager,
                           tokenStore: TokenStore,
                           hmacValidator: HMACValidator,
                           rateLimiter: RateLimiter,
                           logger: Logger) -> Router<BasicRequestContext> {
        let router = Router()
        router.add(middleware: RateLimitMiddleware<BasicRequestContext>(rateLimiter: rateLimiter))
        router.add(middleware: AuthMiddleware<BasicRequestContext>(
            tokenStore: tokenStore,
            hmacValidator: hmacValidator
        ))
        router.get("/health") { _, _ -> HealthResponse in
            HealthResponse(ok: true, version: version, platform: platform)
        }
        router.post("/inject") { request, context -> InjectResponse in
            let sourceTag = request.headers[HTTPField.Name("X-ClipSync-Source")!]
            // Decode manually instead of request.decode() — the default
            // BasicRequestContext.maxUploadSize is 2 MB which is too small
            // for images. AuthMiddleware already collected the body with its
            // own (higher) limit, so we just re-collect the buffered bytes.
            var req = request
            let buffer = try await req.collectBody(upTo: 25 * 1024 * 1024)
            let estimatedSize = buffer.readableBytes * 3 / 4
            guard estimatedSize <= 20 * 1024 * 1024 else {
                throw HTTPError(.contentTooLarge)
            }
            let payload: ClipPayload
            do {
                payload = try JSONDecoder().decode(ClipPayload.self, from: buffer)
                try payload.validate()
            } catch {
                context.logger.warning("inject decode/validate failed: \(error)")
                throw HTTPError(.badRequest, message: String(describing: error))
            }
            context.logger.info("inject received", metadata: [
                "source": .string(sourceTag ?? "unknown"),
                "type": .string(payload.type.rawValue),
                "nonce": .string(payload.nonce),
            ])
            do {
                // NSPasteboard must be driven from the main thread; dispatch explicitly
                // even though Apple marks it as thread-safe, calling it from an NIO
                // event-loop thread in a .accessory menu-bar app returns false silently.
                try await MainActor.run { try injector.inject(payload) }
            } catch {
                context.logger.error("inject failed: \(error)")
                throw HTTPError(.badRequest, message: String(describing: error))
            }
            await hub.broadcast(payload)
            return InjectResponse(ok: true, nonce: payload.nonce)
        }
        router.get("/pair") { request, context -> PairingResponse in
            let clientIP = request.headers[HTTPField.Name("X-Forwarded-For")!] ?? "unknown"
            guard await rateLimiter.allow(key: "pair:\(clientIP)", maxRequests: 5, windowSeconds: 60) else {
                throw HTTPError(.tooManyRequests)
            }
            guard let raw = request.uri.queryParameters["code"] else {
                throw HTTPError(.badRequest, message: "missing code")
            }
            let code = String(raw)
            do {
                let response = try await pairing.consume(code: code)
                // Register the issued token in TokenStore so subsequent requests
                // can authenticate with `Authorization: Bearer <token>`.
                let deviceLabel = request.headers[HTTPField.Name("X-ClipSync-Device")!] ?? "paired-device"
                _ = try await tokenStore.register(tokenPlain: response.token, deviceLabel: deviceLabel)
                return response
            } catch let error as PairingError {
                context.logger.info("pair rejected", metadata: [
                    "reason": .string(String(describing: error)),
                ])
                throw HTTPError(.unauthorized, message: String(describing: error))
            } catch {
                context.logger.error("pair failed: \(error)")
                throw HTTPError(.internalServerError, message: String(describing: error))
            }
        }
        return router
    }

    struct HealthResponse: ResponseEncodable, Sendable {
        let ok: Bool
        let version: String
        let platform: String
    }

    struct InjectResponse: ResponseEncodable, Sendable {
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
