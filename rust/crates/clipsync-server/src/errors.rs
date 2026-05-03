//! HTTP error types for the `/inject` endpoint.
//!
//! All 4xx responses from `/inject` use a single, standardized JSON body shape:
//!
//! ```json
//! { "error": "<machine-code>", "message": "<human-readable>" }
//! ```
//!
//! The `error` field is one of the machine codes in [`InjectErrorCode`].
//! See `docs/plans/master-plan-rust-fork.md` Phase 1.3.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;

use clipsync_core::protocol::PayloadValidationError;

/// Machine-readable error codes returned by `/inject` 4xx responses.
///
/// This is the exhaustive enum of failures that map to a 400 Bad Request
/// (or 413 in the case of `PayloadTooLarge` — see `IntoResponse`).
#[derive(Debug)]
pub enum InjectError {
    /// JSON parse / shape mismatch — body is not a valid `ClipPayload`.
    DecodeError(String),
    /// Payload `ts` field is outside the 5-minute clock-skew window.
    TimestampOutOfRange(String),
    /// Body size exceeded `MAX_PAYLOAD_BYTES`.
    PayloadTooLarge(String),
    /// `type` field present but not one of the known [`ClipType`] variants.
    UnsupportedKind(String),
}

impl InjectError {
    /// Stable machine-readable code, used as the `error` field in the body.
    pub fn code(&self) -> &'static str {
        match self {
            Self::DecodeError(_) => "decode_error",
            Self::TimestampOutOfRange(_) => "timestamp_out_of_range",
            Self::PayloadTooLarge(_) => "payload_too_large",
            Self::UnsupportedKind(_) => "unsupported_kind",
        }
    }

    /// Human-readable detail, used as the `message` field in the body.
    pub fn message(&self) -> &str {
        match self {
            Self::DecodeError(m)
            | Self::TimestampOutOfRange(m)
            | Self::PayloadTooLarge(m)
            | Self::UnsupportedKind(m) => m,
        }
    }

    /// HTTP status code for this error variant.
    ///
    /// All variants currently return `400 Bad Request` for parity with the
    /// Mac (Hummingbird) implementation, which reports the same shape on
    /// `HTTPError(.badRequest, message:)`.
    pub fn status(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
}

impl From<PayloadValidationError> for InjectError {
    fn from(err: PayloadValidationError) -> Self {
        match err {
            PayloadValidationError::TimestampOutOfRange { delta_ms, max_ms } => {
                Self::TimestampOutOfRange(format!(
                    "payload ts deviates by {delta_ms}ms (max {max_ms}ms)"
                ))
            }
        }
    }
}

#[derive(Serialize)]
struct InjectErrorBody<'a> {
    error: &'a str,
    message: &'a str,
}

impl IntoResponse for InjectError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = InjectErrorBody {
            error: self.code(),
            message: self.message(),
        };
        (status, Json(body)).into_response()
    }
}

/// Inspect a raw JSON body and classify the failure.
///
/// We do a two-stage parse so we can tell `unsupported_kind` apart from
/// generic `decode_error`:
///
/// 1. Parse to `serde_json::Value`. If this fails, it's `decode_error`.
/// 2. Inspect the `type` field. If it's a string but not in the known
///    set, return `unsupported_kind`.
/// 3. Otherwise, attempt the strongly-typed parse and report the
///    serde error as `decode_error`.
pub fn classify_decode_failure(body: &[u8]) -> InjectError {
    let value: serde_json::Value = match serde_json::from_slice(body) {
        Ok(v) => v,
        Err(e) => return InjectError::DecodeError(format!("invalid JSON: {e}")),
    };

    if let Some(kind) = value.get("type").and_then(|v| v.as_str()) {
        // Mirrors clipsync_core::protocol::ClipType (lowercase).
        const KNOWN_KINDS: &[&str] = &["text", "image", "file"];
        if !KNOWN_KINDS.contains(&kind) {
            return InjectError::UnsupportedKind(format!(
                "unknown clip type \"{kind}\" (expected one of: text, image, file)"
            ));
        }
    }

    // Re-parse to surface the structural mismatch (missing field, wrong type, etc.).
    match serde_json::from_value::<clipsync_core::protocol::ClipPayload>(value) {
        Ok(_) => InjectError::DecodeError("payload failed to decode (no further detail)".into()),
        Err(e) => InjectError::DecodeError(format!("payload shape invalid: {e}")),
    }
}
