import Foundation
import UserNotifications

enum ErrorSeverity {
    case warning, error
}

struct AppError: Identifiable {
    let id = UUID()
    let severity: ErrorSeverity
    let summary: String
    let detail: String
    let suggestion: String?
    let timestamp = Date()
}

@MainActor
final class ErrorStore: ObservableObject {
    @Published private(set) var errors: [AppError] = []

    func append(_ error: AppError) {
        errors.append(error)
        if error.severity == .warning {
            let errorId = error.id
            Task {
                try? await Task.sleep(for: .seconds(300))
                dismiss(errorId)
            }
        }
    }

    func appendAndNotify(_ error: AppError) {
        append(error)
        if error.severity == .error {
            let content = UNMutableNotificationContent()
            content.title = "ClipSync"
            content.body = error.summary
            content.sound = .default
            let request = UNNotificationRequest(
                identifier: error.id.uuidString,
                content: content,
                trigger: nil
            )
            UNUserNotificationCenter.current().add(request)
        }
    }

    func dismiss(_ id: UUID) {
        errors.removeAll { $0.id == id }
    }

    func dismissAll() {
        errors.removeAll()
    }

    var hasErrors: Bool { errors.contains { $0.severity == .error } }
    var hasWarnings: Bool { errors.contains { $0.severity == .warning } }
}
