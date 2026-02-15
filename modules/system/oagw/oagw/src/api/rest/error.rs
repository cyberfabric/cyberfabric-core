use axum::response::{IntoResponse, Response};
use http::StatusCode;
use serde::{Deserialize, Serialize};

use crate::domain::error::DomainError;

// ---------------------------------------------------------------------------
// RFC 9457 Problem Details
// ---------------------------------------------------------------------------

/// RFC 9457 Problem Details for HTTP APIs.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ProblemDetails {
    /// GTS error type identifier.
    #[serde(rename = "type")]
    pub error_type: String,
    /// Human-readable summary.
    pub title: String,
    /// HTTP status code.
    pub status: u16,
    /// Occurrence-specific explanation.
    pub detail: String,
    /// Request URI.
    pub instance: String,
}

// ---------------------------------------------------------------------------
// GTS error type constants
// ---------------------------------------------------------------------------

pub(crate) const ERR_VALIDATION: &str = "gts.x.core.errors.err.v1~x.oagw.validation.error.v1";
pub(crate) const ERR_MISSING_TARGET_HOST: &str =
    "gts.x.core.errors.err.v1~x.oagw.routing.missing_target_host.v1";
pub(crate) const ERR_INVALID_TARGET_HOST: &str =
    "gts.x.core.errors.err.v1~x.oagw.routing.invalid_target_host.v1";
pub(crate) const ERR_UNKNOWN_TARGET_HOST: &str =
    "gts.x.core.errors.err.v1~x.oagw.routing.unknown_target_host.v1";
pub(crate) const ERR_AUTH_FAILED: &str = "gts.x.core.errors.err.v1~x.oagw.auth.failed.v1";
pub(crate) const ERR_ROUTE_NOT_FOUND: &str = "gts.x.core.errors.err.v1~x.oagw.route.not_found.v1";
pub(crate) const ERR_PAYLOAD_TOO_LARGE: &str =
    "gts.x.core.errors.err.v1~x.oagw.payload.too_large.v1";
pub(crate) const ERR_RATE_LIMIT_EXCEEDED: &str =
    "gts.x.core.errors.err.v1~x.oagw.rate_limit.exceeded.v1";
pub(crate) const ERR_SECRET_NOT_FOUND: &str =
    "gts.x.core.errors.err.v1~x.oagw.secret.not_found.v1";
pub(crate) const ERR_DOWNSTREAM: &str = "gts.x.core.errors.err.v1~x.oagw.downstream.error.v1";
pub(crate) const ERR_PROTOCOL: &str = "gts.x.core.errors.err.v1~x.oagw.protocol.error.v1";
pub(crate) const ERR_UPSTREAM_DISABLED: &str =
    "gts.x.core.errors.err.v1~x.oagw.routing.upstream_disabled.v1";
pub(crate) const ERR_CONNECTION_TIMEOUT: &str =
    "gts.x.core.errors.err.v1~x.oagw.timeout.connection.v1";
pub(crate) const ERR_REQUEST_TIMEOUT: &str =
    "gts.x.core.errors.err.v1~x.oagw.timeout.request.v1";

// ---------------------------------------------------------------------------
// Error-to-ProblemDetails conversion
// ---------------------------------------------------------------------------

fn gts_type(err: &DomainError) -> &str {
    match err {
        DomainError::Validation { .. } | DomainError::Conflict { .. } => ERR_VALIDATION,
        DomainError::MissingTargetHost { .. } => ERR_MISSING_TARGET_HOST,
        DomainError::InvalidTargetHost { .. } => ERR_INVALID_TARGET_HOST,
        DomainError::UnknownTargetHost { .. } => ERR_UNKNOWN_TARGET_HOST,
        DomainError::AuthenticationFailed { .. } => ERR_AUTH_FAILED,
        DomainError::NotFound { .. } => ERR_ROUTE_NOT_FOUND,
        DomainError::PayloadTooLarge { .. } => ERR_PAYLOAD_TOO_LARGE,
        DomainError::RateLimitExceeded { .. } => ERR_RATE_LIMIT_EXCEEDED,
        DomainError::SecretNotFound { .. } => ERR_SECRET_NOT_FOUND,
        DomainError::DownstreamError { .. } | DomainError::Internal { .. } => ERR_DOWNSTREAM,
        DomainError::ProtocolError { .. } => ERR_PROTOCOL,
        DomainError::UpstreamDisabled { .. } => ERR_UPSTREAM_DISABLED,
        DomainError::ConnectionTimeout { .. } => ERR_CONNECTION_TIMEOUT,
        DomainError::RequestTimeout { .. } => ERR_REQUEST_TIMEOUT,
    }
}

fn http_status(err: &DomainError) -> u16 {
    match err {
        DomainError::Validation { .. }
        | DomainError::Conflict { .. }
        | DomainError::MissingTargetHost { .. }
        | DomainError::InvalidTargetHost { .. }
        | DomainError::UnknownTargetHost { .. } => 400,
        DomainError::AuthenticationFailed { .. } => 401,
        DomainError::NotFound { .. } => 404,
        DomainError::PayloadTooLarge { .. } => 413,
        DomainError::RateLimitExceeded { .. } => 429,
        DomainError::SecretNotFound { .. } | DomainError::Internal { .. } => 500,
        DomainError::DownstreamError { .. } | DomainError::ProtocolError { .. } => 502,
        DomainError::UpstreamDisabled { .. } => 503,
        DomainError::ConnectionTimeout { .. } | DomainError::RequestTimeout { .. } => 504,
    }
}

fn title(err: &DomainError) -> &str {
    match err {
        DomainError::Validation { .. } | DomainError::Conflict { .. } => "Validation Error",
        DomainError::MissingTargetHost { .. } => "Missing Target Host",
        DomainError::InvalidTargetHost { .. } => "Invalid Target Host",
        DomainError::UnknownTargetHost { .. } => "Unknown Target Host",
        DomainError::AuthenticationFailed { .. } => "Authentication Failed",
        DomainError::NotFound { .. } => "Route Not Found",
        DomainError::PayloadTooLarge { .. } => "Payload Too Large",
        DomainError::RateLimitExceeded { .. } => "Rate Limit Exceeded",
        DomainError::SecretNotFound { .. } => "Secret Not Found",
        DomainError::DownstreamError { .. } | DomainError::Internal { .. } => "Downstream Error",
        DomainError::ProtocolError { .. } => "Protocol Error",
        DomainError::UpstreamDisabled { .. } => "Upstream Disabled",
        DomainError::ConnectionTimeout { .. } => "Connection Timeout",
        DomainError::RequestTimeout { .. } => "Request Timeout",
    }
}

fn instance(err: &DomainError) -> &str {
    match err {
        DomainError::Validation { instance, .. }
        | DomainError::MissingTargetHost { instance, .. }
        | DomainError::InvalidTargetHost { instance, .. }
        | DomainError::UnknownTargetHost { instance, .. }
        | DomainError::AuthenticationFailed { instance, .. }
        | DomainError::PayloadTooLarge { instance, .. }
        | DomainError::RateLimitExceeded { instance, .. }
        | DomainError::SecretNotFound { instance, .. }
        | DomainError::DownstreamError { instance, .. }
        | DomainError::ProtocolError { instance, .. }
        | DomainError::ConnectionTimeout { instance, .. }
        | DomainError::RequestTimeout { instance, .. } => instance,
        DomainError::NotFound { .. }
        | DomainError::Conflict { .. }
        | DomainError::UpstreamDisabled { .. }
        | DomainError::Internal { .. } => "",
    }
}

fn to_problem_details(err: &DomainError) -> ProblemDetails {
    ProblemDetails {
        error_type: gts_type(err).to_string(),
        title: title(err).to_string(),
        status: http_status(err),
        detail: err.to_string(),
        instance: instance(err).to_string(),
    }
}

// ---------------------------------------------------------------------------
// Axum error response
// ---------------------------------------------------------------------------

/// Convert a `DomainError` into an axum `Response` with RFC 9457 Problem Details.
///
/// Injects the provided `instance` URI for variants that don't carry their own.
pub fn domain_error_response(err: DomainError, instance: &str) -> Response {
    let mut pd = to_problem_details(&err);
    // Override instance for variants that don't carry their own.
    if pd.instance.is_empty() {
        pd.instance = instance.to_string();
    }
    build_response(&err, pd)
}

/// Convert a `DomainError` into an axum `Response` with RFC 9457 Problem Details.
pub fn error_response(err: DomainError) -> Response {
    let pd = to_problem_details(&err);
    build_response(&err, pd)
}

fn build_response(err: &DomainError, pd: ProblemDetails) -> Response {
    let status = StatusCode::from_u16(pd.status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let body = serde_json::to_string(&pd).unwrap_or_default();

    let mut response = (
        status,
        [(http::header::CONTENT_TYPE, "application/problem+json")],
        body,
    )
        .into_response();

    response
        .headers_mut()
        .insert("x-oagw-error-source", "gateway".parse().unwrap());

    // Add Retry-After header for 429 responses.
    if let DomainError::RateLimitExceeded {
        retry_after_secs: Some(secs),
        ..
    } = err
        && let Ok(v) = secs.to_string().parse()
    {
        response.headers_mut().insert("retry-after", v);
    }

    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_error_produces_correct_problem_details() {
        let err = DomainError::Validation {
            detail: "missing required field 'server'".into(),
            instance: "/oagw/v1/upstreams".into(),
        };
        let pd = to_problem_details(&err);
        assert_eq!(pd.status, 400);
        assert_eq!(pd.error_type, ERR_VALIDATION);
        assert_eq!(pd.title, "Validation Error");
        assert!(pd.detail.contains("missing required field"));
        assert_eq!(pd.instance, "/oagw/v1/upstreams");
    }

    #[test]
    fn rate_limit_exceeded_produces_429() {
        let err = DomainError::RateLimitExceeded {
            detail: "rate limit exceeded for upstream".into(),
            instance: "/oagw/v1/proxy/api.openai.com/v1/chat/completions".into(),
            retry_after_secs: Some(30),
        };
        let pd = to_problem_details(&err);
        assert_eq!(pd.status, 429);
        assert_eq!(pd.error_type, ERR_RATE_LIMIT_EXCEEDED);
    }

    #[test]
    fn not_found_produces_404() {
        let err = DomainError::NotFound {
            entity: "route",
            id: uuid::Uuid::nil(),
        };
        let pd = to_problem_details(&err);
        assert_eq!(pd.status, 404);
        assert_eq!(pd.error_type, ERR_ROUTE_NOT_FOUND);
    }

    #[test]
    fn all_error_types_produce_valid_json() {
        let errors: Vec<DomainError> = vec![
            DomainError::Validation {
                detail: "test".into(),
                instance: "/test".into(),
            },
            DomainError::MissingTargetHost {
                instance: "/test".into(),
            },
            DomainError::InvalidTargetHost {
                instance: "/test".into(),
            },
            DomainError::UnknownTargetHost {
                detail: "test".into(),
                instance: "/test".into(),
            },
            DomainError::AuthenticationFailed {
                detail: "test".into(),
                instance: "/test".into(),
            },
            DomainError::NotFound {
                entity: "route",
                id: uuid::Uuid::nil(),
            },
            DomainError::PayloadTooLarge {
                detail: "test".into(),
                instance: "/test".into(),
            },
            DomainError::RateLimitExceeded {
                detail: "test".into(),
                instance: "/test".into(),
                retry_after_secs: None,
            },
            DomainError::SecretNotFound {
                detail: "test".into(),
                instance: "/test".into(),
            },
            DomainError::DownstreamError {
                detail: "test".into(),
                instance: "/test".into(),
            },
            DomainError::ProtocolError {
                detail: "test".into(),
                instance: "/test".into(),
            },
            DomainError::UpstreamDisabled {
                alias: "test".into(),
            },
            DomainError::ConnectionTimeout {
                detail: "test".into(),
                instance: "/test".into(),
            },
            DomainError::RequestTimeout {
                detail: "test".into(),
                instance: "/test".into(),
            },
        ];
        for err in &errors {
            let pd = to_problem_details(err);
            let json = serde_json::to_string(&pd).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.get("type").is_some(), "missing 'type' for {err:?}");
            assert!(
                parsed.get("status").is_some(),
                "missing 'status' for {err:?}"
            );
            assert!(parsed.get("title").is_some(), "missing 'title' for {err:?}");
            assert!(
                parsed.get("detail").is_some(),
                "missing 'detail' for {err:?}"
            );
            assert!(
                parsed.get("instance").is_some(),
                "missing 'instance' for {err:?}"
            );
        }
    }

    #[test]
    fn problem_details_serde_round_trip() {
        let pd = ProblemDetails {
            error_type: ERR_VALIDATION.into(),
            title: "Validation Error".into(),
            status: 400,
            detail: "test detail".into(),
            instance: "/test".into(),
        };
        let json = serde_json::to_string(&pd).unwrap();
        let pd2: ProblemDetails = serde_json::from_str(&json).unwrap();
        assert_eq!(pd, pd2);
    }
}
