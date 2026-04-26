import AppKit
import Foundation
import Logging

struct TailscaleHelper {
    private static let logger = Logger(label: "clipsync.tailscale")

    static var isInstalled: Bool {
        FileManager.default.fileExists(atPath: "/Applications/Tailscale.app")
    }

    private static let cliPath = "/Applications/Tailscale.app/Contents/MacOS/Tailscale"

    static func ipv4() -> String? {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: cliPath)
        process.arguments = ["ip", "-4"]
        let pipe = Pipe()
        process.standardOutput = pipe
        process.standardError = Pipe()
        do {
            try process.run()
            process.waitUntilExit()
            guard process.terminationStatus == 0 else { return nil }
            let data = pipe.fileHandleForReading.readDataToEndOfFile()
            return String(data: data, encoding: .utf8)?.trimmingCharacters(in: .whitespacesAndNewlines)
        } catch {
            logger.warning("Failed to get Tailscale IP: \(error)")
            return nil
        }
    }

    static func openDownloadPage() {
        if let url = URL(string: "https://apps.apple.com/app/tailscale/id1475387142") {
            NSWorkspace.shared.open(url)
        }
    }
}
