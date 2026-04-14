import Foundation
import HummingbirdWebSocket
import Logging

actor WebSocketHub {
    final class Client: Hashable, Sendable {
        let id: UUID
        let outbound: WebSocketOutboundWriter
        let sourceTag: String?

        init(id: UUID = UUID(), outbound: WebSocketOutboundWriter, sourceTag: String? = nil) {
            self.id = id
            self.outbound = outbound
            self.sourceTag = sourceTag
        }

        static func == (lhs: Client, rhs: Client) -> Bool { lhs.id == rhs.id }
        func hash(into hasher: inout Hasher) { hasher.combine(id) }
    }

    private var clients: Set<Client> = []
    private var logger: Logger

    init(logger: Logger = Logger(label: "clipsync.ws.hub")) {
        self.logger = logger
    }

    var clientCount: Int { clients.count }

    func register(_ client: Client) {
        clients.insert(client)
        logger.info("WebSocket client connected", metadata: [
            "id": .string(client.id.uuidString),
            "clients": .stringConvertible(clients.count),
        ])
    }

    func unregister(_ client: Client) {
        clients.remove(client)
        logger.info("WebSocket client disconnected", metadata: [
            "id": .string(client.id.uuidString),
            "clients": .stringConvertible(clients.count),
        ])
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
        for client in snapshot {
            do {
                try await client.outbound.write(.text(text))
            } catch {
                logger.debug("Dropping client after write failure", metadata: [
                    "id": .string(client.id.uuidString),
                    "error": .string(String(describing: error)),
                ])
                clients.remove(client)
            }
        }
    }
}
