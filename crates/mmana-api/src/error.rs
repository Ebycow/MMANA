//! Errors as RFC 9457 `application/problem+json` responses.

use axum::extract::rejection::JsonRejection;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use mmana_core::CalcError;
use mmana_core::optimize::OptimizeError;
use serde::Serialize;
use utoipa::ToSchema;

/// RFC 9457 problem details.
#[derive(Debug, Serialize, ToSchema)]
pub struct Problem {
    /// Stable identifier of the error kind, e.g. `calculation-failed`.
    #[serde(rename = "type")]
    pub kind: String,
    pub title: String,
    pub status: u16,
    pub detail: String,
}

#[derive(Debug)]
pub enum ApiError {
    /// The request body is not valid JSON for the endpoint.
    BadRequest(String),
    /// The request body is over the size limit (sent without a length, so
    /// the limit is only met while reading it).
    TooLarge(String),
    /// The input is well-formed but cannot be calculated.
    Calculation(CalcError),
    /// A request limit (points, pulses...) was exceeded.
    LimitExceeded(String),
    /// The optimisation setup cannot run.
    Optimize(OptimizeError),
    Internal(String),
}

impl ApiError {
    fn parts(&self) -> (StatusCode, &'static str, &'static str, String) {
        match self {
            ApiError::BadRequest(d) => (StatusCode::BAD_REQUEST, "invalid-request", "Invalid request", d.clone()),
            ApiError::TooLarge(d) => {
                (StatusCode::PAYLOAD_TOO_LARGE, "payload-too-large", "Request body too large", d.clone())
            }
            ApiError::Calculation(e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "calculation-failed",
                "The antenna cannot be calculated",
                e.to_string(),
            ),
            ApiError::Optimize(e) => (
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid-optimizer-setup",
                "The optimisation setup cannot run",
                e.to_string(),
            ),
            ApiError::LimitExceeded(d) => {
                (StatusCode::UNPROCESSABLE_ENTITY, "limit-exceeded", "Request limit exceeded", d.clone())
            }
            ApiError::Internal(d) => (StatusCode::INTERNAL_SERVER_ERROR, "internal", "Internal error", d.clone()),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, kind, title, detail) = self.parts();
        if status.is_server_error() {
            tracing::error!(%detail, "request failed");
        }
        let body = Problem { kind: kind.into(), title: title.into(), status: status.as_u16(), detail };
        (status, [(header::CONTENT_TYPE, "application/problem+json")], serde_json::to_string(&body).unwrap_or_default())
            .into_response()
    }
}

impl From<CalcError> for ApiError {
    fn from(e: CalcError) -> Self {
        ApiError::Calculation(e)
    }
}

impl From<OptimizeError> for ApiError {
    fn from(e: OptimizeError) -> Self {
        ApiError::Optimize(e)
    }
}

impl From<JsonRejection> for ApiError {
    fn from(r: JsonRejection) -> Self {
        if r.status() == StatusCode::PAYLOAD_TOO_LARGE {
            ApiError::TooLarge(r.body_text())
        } else {
            ApiError::BadRequest(r.body_text())
        }
    }
}
