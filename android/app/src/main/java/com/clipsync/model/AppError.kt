package com.clipsync.model

import java.util.UUID

enum class ErrorSeverity { WARNING, ERROR }

sealed class ErrorAction {
    data object Retry : ErrorAction()
    data object Repair : ErrorAction()
    data class OpenUrl(val url: String) : ErrorAction()
}

data class AppError(
    val id: String = UUID.randomUUID().toString(),
    val severity: ErrorSeverity,
    val summary: String,
    val detail: String? = null,
    val suggestion: String? = null,
    val action: ErrorAction? = null
)
