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

    static func detect() -> TailscaleState {
        guard isInstalled else { return .notInstalled }

        let (ipOut, ipErr, ipCode) = runCLIFull(["ip", "-4"])
        if ipCode == 0, let ip = ipOut?.trimmingCharacters(in: .whitespacesAndNewlines), !ip.isEmpty {
            return .connected(ip: ip)
        }

        if isDaemonError(stderr: ipErr, exitCode: ipCode) {
            return .daemonDown
        }

        let (statusOut, statusErr, statusCode) = runCLIFull(["status", "--json"])
        if isDaemonError(stderr: statusErr, exitCode: statusCode) {
            return .daemonDown
        }

        if statusCode == 0, let out = statusOut,
           let data = out.data(using: .utf8),
           let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
           let backend = json["BackendState"] as? String {
            switch backend {
            case "NeedsLogin": return .notLoggedIn
            case "Stopped", "NoState": return .disconnected
            default: break
            }
        }

        return .disconnected
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

    private static func isDaemonError(stderr: String?, exitCode: Int32) -> Bool {
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
