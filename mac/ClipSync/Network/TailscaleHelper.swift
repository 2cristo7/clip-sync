import AppKit
import Foundation
import Logging

enum TailscaleState: Equatable {
    case notInstalled
    case daemonDown
    case notLoggedIn
    case disconnected
    case connected(ip: String)
}

struct TailscaleHelper {
    private static let logger = Logger(label: "clipsync.tailscale")
    private static let cliPath = "/Applications/Tailscale.app/Contents/MacOS/Tailscale"

    static var isInstalled: Bool {
        FileManager.default.fileExists(atPath: cliPath)
    }

    private static func debugLog(_ msg: String) {
        let line = "\(Date()): \(msg)\n"
        let path = "/tmp/clipsync-tailscale-debug.log"
        if let fh = FileHandle(forWritingAtPath: path) {
            fh.seekToEndOfFile()
            fh.write(line.data(using: .utf8)!)
            fh.closeFile()
        } else {
            FileManager.default.createFile(atPath: path, contents: line.data(using: .utf8))
        }
    }

    static func detect() -> TailscaleState {
        guard isInstalled else {
            debugLog("CLI not found at \(cliPath)")
            return .notInstalled
        }

        let (statusOut, statusErr, statusCode) = runCLIFull(["status", "--json"])
        debugLog("exitCode=\(statusCode) stderrLen=\(statusErr?.count ?? -1) stdoutLen=\(statusOut?.count ?? -1)")
        debugLog("stderr=\(statusErr ?? "nil")")
        debugLog("stdout prefix=\(String((statusOut ?? "").prefix(300)))")

        if isDaemonError(stdout: statusOut, stderr: statusErr, exitCode: statusCode) {
            debugLog("isDaemonError=true")
            return .daemonDown
        }

        guard statusCode == 0, let out = statusOut,
              let data = out.data(using: .utf8),
              let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
              let backend = json["BackendState"] as? String else {
            debugLog("guard failed — code=\(statusCode) hasOut=\(statusOut != nil)")
            return .daemonDown
        }
        debugLog("BackendState=\(backend)")

        switch backend {
        case "NeedsLogin":
            return .notLoggedIn
        case "Running":
            let selfNode = json["Self"] as? [String: Any]
            let ips = selfNode?["TailscaleIPs"] as? [String] ?? []
            if let ipv4 = ips.first(where: { $0.contains(".") && !$0.contains(":") }) {
                return .connected(ip: ipv4)
            }
            // Fall back to ip -4 CLI if JSON lacks IPs
            if let ip = ipFromCLI() {
                return .connected(ip: ip)
            }
            return .connected(ip: "")
        case "Stopped", "NoState", "Offline":
            return .disconnected
        default:
            return .disconnected
        }
    }

    static func openDownloadPage() {
        NSWorkspace.shared.open(URL(string: "https://apps.apple.com/app/tailscale/id1475387142")!)
    }

    static func openApp() {
        NSWorkspace.shared.open(URL(fileURLWithPath: "/Applications/Tailscale.app"))
    }

    static func openNetworkExtensionSettings() {
        let url = URL(string: "x-apple.systempreferences:com.apple.preference.security?Privacy_NetworkExtensions")!
        NSWorkspace.shared.open(url)
    }

    // MARK: - Private

    private static func ipFromCLI() -> String? {
        let (out, _, code) = runCLIFull(["ip", "-4"])
        guard code == 0 else { return nil }
        return extractIPv4(from: out ?? "")
    }

    private static func extractIPv4(from output: String) -> String? {
        for line in output.components(separatedBy: .newlines) {
            let s = line.trimmingCharacters(in: .whitespaces)
            let parts = s.split(separator: ".")
            guard parts.count == 4 else { continue }
            let valid = parts.allSatisfy { p in (Int(p).map { $0 >= 0 && $0 <= 255 }) ?? false }
            if valid { return s }
        }
        return nil
    }

    private static func isDaemonError(stdout: String?, stderr: String?, exitCode: Int32) -> Bool {
        if let out = stdout {
            let lower = out.lowercased()
            if lower.contains("clierror") || lower.contains("gui failed to start") || lower.contains("failed to start") {
                return true
            }
        }
        guard exitCode != 0 else { return false }
        guard let err = stderr else { return exitCode == 3 }
        let lower = err.lowercased()
        return lower.contains("failed to connect") ||
               lower.contains("is tailscale running") ||
               lower.contains("socket") ||
               lower.contains("tailscaled") ||
               lower.contains("not running") ||
               exitCode == 3
    }

    private static func runCLIFull(_ args: [String]) -> (stdout: String?, stderr: String?, code: Int32) {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: cliPath)
        process.arguments = args
        let outPipe = Pipe()
        let errPipe = Pipe()
        process.standardOutput = outPipe
        process.standardError = errPipe
        do {
            try process.run()
            process.waitUntilExit()
            let out = String(data: outPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)
            let err = String(data: errPipe.fileHandleForReading.readDataToEndOfFile(), encoding: .utf8)
            return (out, err, process.terminationStatus)
        } catch {
            logger.warning("Tailscale CLI launch failed: \(error)")
            return (nil, nil, -1)
        }
    }
}
