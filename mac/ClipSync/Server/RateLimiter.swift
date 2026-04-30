import Foundation

actor RateLimiter {
    private var requests: [String: [Date]] = [:]

    func allow(key: String, maxRequests: Int, windowSeconds: TimeInterval) -> Bool {
        let now = Date()
        let cutoff = now.addingTimeInterval(-windowSeconds)
        var timestamps = requests[key, default: []].filter { $0 > cutoff }
        guard timestamps.count < maxRequests else { return false }
        timestamps.append(now)
        requests[key] = timestamps
        return true
    }

    func reset(key: String) {
        requests.removeValue(forKey: key)
    }
}
