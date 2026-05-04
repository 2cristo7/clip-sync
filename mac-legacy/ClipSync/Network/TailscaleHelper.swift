import AppKit
import Darwin
import Foundation
import Logging

struct TailscaleHelper {
    private static let logger = Logger(label: "clipsync.tailscale")

    static var isInstalled: Bool {
        FileManager.default.fileExists(atPath: "/Applications/Tailscale.app")
    }

    private static let cliCandidates = [
        "/Applications/Tailscale.app/Contents/MacOS/Tailscale",
        "/usr/local/bin/tailscale",
        "/opt/homebrew/bin/tailscale",
    ]

    static func ipv4() -> String? {
        // Primary: CLI detection
        if let ip = ipv4ViaCLI() { return ip }
        // Fallback: scan network interfaces for Tailscale CGNAT range (100.64/10)
        return ipv4ViaNetworkInterfaces()
    }

    private static func ipv4ViaCLI() -> String? {
        for path in cliCandidates {
            guard FileManager.default.fileExists(atPath: path) else { continue }
            let process = Process()
            process.executableURL = URL(fileURLWithPath: path)
            process.arguments = ["ip", "-4"]
            let pipe = Pipe()
            process.standardOutput = pipe
            process.standardError = Pipe()
            do {
                try process.run()
                process.waitUntilExit()
                guard process.terminationStatus == 0 else { continue }
                let data = pipe.fileHandleForReading.readDataToEndOfFile()
                if let ip = String(data: data, encoding: .utf8)?
                    .trimmingCharacters(in: .whitespacesAndNewlines),
                   ip.range(of: #"^\d{1,3}(\.\d{1,3}){3}$"#, options: .regularExpression) != nil {
                    return ip
                }
            } catch {
                logger.warning("Tailscale CLI at \(path) failed: \(error)")
            }
        }
        return nil
    }

    // Detect Tailscale IP by scanning interfaces for 100.64.0.0/10 addresses.
    // Works when the VPN tunnel is up but the CLI path is wrong or daemon socket is unreachable.
    private static func ipv4ViaNetworkInterfaces() -> String? {
        var ifaddr: UnsafeMutablePointer<ifaddrs>?
        guard getifaddrs(&ifaddr) == 0, let head = ifaddr else { return nil }
        defer { freeifaddrs(head) }
        var ptr: UnsafeMutablePointer<ifaddrs>? = head
        while let current = ptr {
            let addr = current.pointee
            if addr.ifa_addr.pointee.sa_family == UInt8(AF_INET) {
                var hostname = [CChar](repeating: 0, count: Int(NI_MAXHOST))
                let rc = getnameinfo(
                    addr.ifa_addr,
                    socklen_t(MemoryLayout<sockaddr_in>.size),
                    &hostname, socklen_t(hostname.count),
                    nil, 0, NI_NUMERICHOST
                )
                if rc == 0 {
                    let ip = String(cString: hostname)
                    let parts = ip.split(separator: ".")
                    if parts.count == 4,
                       parts[0] == "100",
                       let second = Int(parts[1]),
                       second >= 64 && second <= 127 {
                        logger.info("Tailscale IP found via ifaddrs: \(ip)")
                        return ip
                    }
                }
            }
            ptr = addr.ifa_next
        }
        return nil
    }

    static var isVpnActive: Bool {
        ipv4() != nil
    }

    static func status() -> String? {
        let path = cliCandidates.first { FileManager.default.fileExists(atPath: $0) }
            ?? cliCandidates[0]
        let process = Process()
        process.executableURL = URL(fileURLWithPath: path)
        process.arguments = ["status", "--json"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        do {
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else { return nil }
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            return String(data: data, encoding: .utf8)
        } catch {
            return nil
        }
    }

    static func openDownloadPage() {
        if let url = URL(string: "https://apps.apple.com/app/tailscale/id1475387142") {
            NSWorkspace.shared.open(url)
        }
    }
}
