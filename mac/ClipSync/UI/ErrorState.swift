import Foundation

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
