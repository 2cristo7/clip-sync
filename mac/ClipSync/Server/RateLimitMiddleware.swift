import Foundation
import HTTPTypes
import Hummingbird

struct RateLimitMiddleware<Context: RequestContext>: RouterMiddleware {
    let rateLimiter: RateLimiter
    let maxInjectBodyBytes: Int

    init(rateLimiter: RateLimiter, maxInjectBodyBytes: Int = 20 * 1024 * 1024) {
        self.rateLimiter = rateLimiter
        self.maxInjectBodyBytes = maxInjectBodyBytes
    }

    func handle(_ request: Request,
                context: Context,
                next: (Request, Context) async throws -> Response) async throws -> Response {
        guard request.uri.path == "/inject" else {
            return try await next(request, context)
        }
        let clientIP = request.headers[HTTPField.Name("X-Forwarded-For")!] ?? "unknown"
        guard await rateLimiter.allow(key: "inject:\(clientIP)", maxRequests: 10, windowSeconds: 1) else {
            throw HTTPError(.tooManyRequests)
        }
        if let lengthStr = request.headers[.contentLength],
           let length = Int(lengthStr),
           length > maxInjectBodyBytes {
            throw HTTPError(.contentTooLarge)
        }
        return try await next(request, context)
    }
}
