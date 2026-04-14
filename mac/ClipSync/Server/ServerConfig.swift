import Foundation
import Logging

struct ServerConfig {
    let host: String
    let port: Int
    let logLevel: Logger.Level

    static let `default` = ServerConfig(
        host: "0.0.0.0",
        port: 7010,
        logLevel: .info
    )
}
