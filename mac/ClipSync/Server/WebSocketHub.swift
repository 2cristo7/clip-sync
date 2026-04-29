import Foundation
import HummingbirdWebSocket
import Logging

struct ClipClientInfo: Sendable, Hashable {
    let id: UUID
    let remoteAddress: String?
    let connectedAt: Date
    let lastSeen: Date
}

actor WebSocketHub {
    final class Client: Hashable, @unchecked Sendable {
        let id: UUID
        let outbound: WebSocketOutboundWriter
        let sourceTag: String?
        let remoteAddress: String?
        let connectedAt: Date
        var lastSeen: Date

        init(id: UUID = UUID(),
             outbound: WebSocketOutboundWriter,
             sourceTag: String? = nil,
             remoteAddress: String? = nil,
             connectedAt: Date = Date()) {
            self.id = id
            self.outbound = outbound
            self.sourceTag = sourceTag
            self.remoteAddress = remoteAddress
            self.connectedAt = connectedAt
            self.lastSeen = connectedAt
        }

        static func == (lhs: Client, rhs: Client) -> Bool { lhs.id == rhs.id }
        func hash(into hasher: inout Hasher) { hasher.combine(id) }
    }

    private var clients: Set<Client> = []
    private var continuations: [UUID: AsyncStream<[ClipClientInfo]>.Continuation] = [:]
    private var logger: Logger
    let errorStore: ErrorStore
    private var lastDisconnectErrorTime: Date?

    init(logger: Logger = Logger(label: "clipsync.ws.hub"),
         errorStore: ErrorStore) {
        self.logger = logger
        self.errorStore = errorStore
    }

    var clientCount: Int { clients.count }

    func snapshot() -> [ClipClientInfo] {
        clients
            .map { ClipClientInfo(id: $0.id, remoteAddress: $0.remoteAddress, connectedAt: $0.connectedAt, lastSeen: $0.lastSeen) }
            .sorted { $0.connectedAt < $1.connectedAt }
    }

    func events() -> AsyncStream<[ClipClientInfo]> {
        AsyncStream { continuation in
            let id = UUID()
            continuations[id] = continuation
            continuation.yield(snapshot())
            continuation.onTermination = { [weak self] _ in
                guard let self else { return }
                Task { await self.removeContinuation(id) }
            }
        }
    }

    private func removeContinuation(_ id: UUID) {
        continuations.removeValue(forKey: id)
    }

    func register(_ client: Client) {
        clients.insert(client)
        logger.info("WebSocket client connected", metadata: [
            "id": .string(client.id.uuidString),
            "clients": .stringConvertible(clients.count),
        ])
        notifyChange()
    }

    func unregister(_ client: Client) {
        clients.remove(client)
        logger.info("WebSocket client disconnected", metadata: [
            "id": .string(client.id.uuidString),
            "clients": .stringConvertible(clients.count),
        ])
        notifyChange()
    }

    func touch(_ client: Client) {
        guard let existing = clients.first(where: { $0 == client }) else { return }
        existing.lastSeen = Date()
        notifyChange()
    }

    private func notifyChange() {
        let snap = snapshot()
        for continuation in continuations.values {
            continuation.yield(snap)
        }
    }

    func broadcast(_ payload: ClipPayload) async {
        guard !clients.isEmpty else { return }
        let text: String
        do {
            let data = try JSONEncoder().encode(payload)
            guard let encoded = String(data: data, encoding: .utf8) else { return }
            text = encoded
        } catch {
            logger.error("Failed to encode payload: \(error)")
            return
        }

        let snapshot = clients
        var didChange = false
        for client in snapshot {
            do {
                try await client.outbound.write(.text(text))
                client.lastSeen = Date()
                didChange = true
            } catch {
                logger.debug("Dropping client after write failure", metadata: [
                    "id": .string(client.id.uuidString),
                    "error": .string(String(describing: error)),
                ])
                clients.remove(client)
                didChange = true
                let now = Date()
                if lastDisconnectErrorTime == nil || now.timeIntervalSince(lastDisconnectErrorTime!) > 5 {
                    lastDisconnectErrorTime = now
                    let store = errorStore
                    Task { @MainActor in
                        store.append(AppError(
                            severity: .warning,
                            summary: "Device disconnected",
                            detail: "WebSocket write failed for client",
                            suggestion: "The device will reconnect automatically."
                        ))
                    }
                }
            }
        }
        if didChange { notifyChange() }
    }
}
