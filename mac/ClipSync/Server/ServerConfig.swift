import Foundation
import Logging

struct ServerConfig {
    let host: String
    let port: Int
    let logLevel: Logger.Level

    static var defaultPort: Int {
        if let envPort = ProcessInfo.processInfo.environment["CLIPSYNC_PORT"],
           let port = Int(envPort) {
            return port
        }
        return 7010
    }

    static var `default`: ServerConfig {
        ServerConfig(
            host: "0.0.0.0",
            port: defaultPort,
            logLevel: .info
        )
    }
}
