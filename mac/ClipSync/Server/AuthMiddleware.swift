import Foundation
import HTTPTypes
import Hummingbird
import NIOCore
import Logging

/// Middleware that enforces:
///   - Bearer auth on `/inject` (token must exist in `TokenStore`).
///   - HMAC-SHA256 signature on `/inject` bodies (via `HMACValidator`).
/// It lets `/health` and `/pair` through unauthenticated.
///
/// Because validating HMAC requires the full body, this middleware collects the
/// body and replaces `request.body` with the same bytes so the downstream route
/// handler can still decode it.
struct AuthMiddleware<Context: RequestContext>: RouterMiddleware {
    let tokenStore: TokenStore
    let hmacValidator: HMACValidator
    let maxBodySize: Int

    init(tokenStore: TokenStore,
         hmacValidator: HMACValidator,
         maxBodySize: Int = 4 * 1024 * 1024) {
        self.tokenStore = tokenStore
        self.hmacValidator = hmacValidator
        self.maxBodySize = maxBodySize
    }

    func handle(_ request: Request,
                context: Context,
                next: (Request, Context) async throws -> Response) async throws -> Response {
        let path = request.uri.path
        // Open endpoints: /health and /pair do not require auth.
        if path == "/health" || path.hasPrefix("/pair") {
            return try await next(request, context)
        }

        // All other routes need Bearer auth.
        let authorization = request.headers[.authorization]
        guard let tokenPlain = Self.extractBearer(authorization) else {
            return Response(status: .unauthorized, headers: [.wwwAuthenticate: "Bearer"], body: .init())
        }
        guard let record = try await tokenStore.validate(tokenPlain: tokenPlain) else {
            return Response(status: .unauthorized, headers: [.wwwAuthenticate: "Bearer"], body: .init())
        }

        if path == "/inject" {
            let sigField = HTTPField.Name("X-ClipSync-Signature")!
            let sigHeader = request.headers[sigField]
            var mutableRequest = request
            let body = try await mutableRequest.collectBody(upTo: maxBodySize)
            let bodyData = Data(buffer: body)
            do {
                try hmacValidator.validate(headerValue: sigHeader, body: bodyData)
            } catch {
                context.logger.info("HMAC rejected", metadata: [
                    "reason": .string(String(describing: error)),
                ])
                return Response(status: .unauthorized, body: .init())
            }

            try await tokenStore.touch(id: record.id)
            return try await next(mutableRequest, context)
        }

        try await tokenStore.touch(id: record.id)
        return try await next(request, context)
    }

    static func extractBearer(_ header: String?) -> String? {
        guard let header = header?.trimmingCharacters(in: .whitespaces), !header.isEmpty else {
            return nil
        }
        let prefix = "Bearer "
        guard header.lowercased().hasPrefix(prefix.lowercased()) else { return nil }
        let token = String(header.dropFirst(prefix.count)).trimmingCharacters(in: .whitespaces)
        return token.isEmpty ? nil : token
    }
}
