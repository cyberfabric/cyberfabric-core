use axum::response::{IntoResponse, Response};
use http::{HeaderValue, StatusCode};
use modkit::api::problem::Problem;

use crate::domain::error::DomainError;
use oagw_sdk::api::ErrorSource;

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
pub(crate) const ERR_NOT_FOUND: &str = "gts.x.core.errors.err.v1~x.oagw.resource.not_found.v1";
pub(crate) const ERR_ROUTE_NOT_FOUND: &str = "gts.x.core.errors.err.v1~x.oagw.route.not_found.v1";
pub(crate) const ERR_PAYLOAD_TOO_LARGE: &str =
    "gts.x.core.errors.err.v1~x.oagw.payload.too_large.v1";
pub(crate) const ERR_RATE_LIMIT_EXCEEDED: &str =
    "gts.x.core.errors.err.v1~x.oagw.rate_limit.exceeded.v1";
pub(crate) const ERR_SECRET_NOT_FOUND: &str = "gts.x.core.errors.err.v1~x.oagw.secret.not_found.v1";
pub(crate) const ERR_DOWNSTREAM: &str = "gts.x.core.errors.err.v1~x.oagw.downstream.error.v1";
pub(crate) const ERR_PROTOCOL: &str = "gts.x.core.errors.err.v1~x.oagw.protocol.error.v1";
pub(crate) const ERR_UPSTREAM_DISABLED: &str =
    "gts.x.core.errors.err.v1~x.oagw.routing.upstream_disabled.v1";
pub(crate) const ERR_CONNECTION_TIMEOUT: &str =
    "gts.x.core.errors.err.v1~x.oagw.timeout.connection.v1";
pub(crate) const ERR_REQUEST_TIMEOUT: &str = "gts.x.core.errors.err.v1~x.oagw.timeout.request.v1";
pub(crate) const ERR_FORBIDDEN: &str = "gts.x.core.errors.err.v1~x.oagw.authz.forbidden.v1";

// ---------------------------------------------------------------------------
// DomainError → Problem helpers
// ---------------------------------------------------------------------------

fn gts_type(err: &DomainError) -> &str {
    match err {
        DomainError::Validation { .. } | DomainError::Conflict { .. } => ERR_VALIDATION,
        DomainError::MissingTargetHost { .. } => ERR_MISSING_TARGET_HOST,
        DomainError::InvalidTargetHost { .. } => ERR_INVALID_TARGET_HOST,
        DomainError::UnknownTargetHost { .. } => ERR_UNKNOWN_TARGET_HOST,
        DomainError::AuthenticationFailed { .. } => ERR_AUTH_FAILED,
        DomainError::NotFound {
            entity: "route", ..
        } => ERR_ROUTE_NOT_FOUND,
        DomainError::NotFound { .. } => ERR_NOT_FOUND,
        DomainError::PayloadTooLarge { .. } => ERR_PAYLOAD_TOO_LARGE,
        DomainError::RateLimitExceeded { .. } => ERR_RATE_LIMIT_EXCEEDED,
        DomainError::SecretNotFound { .. } => ERR_SECRET_NOT_FOUND,
        DomainError::DownstreamError { .. } | DomainError::Internal { .. } => ERR_DOWNSTREAM,
        DomainError::ProtocolError { .. } => ERR_PROTOCOL,
        DomainError::UpstreamDisabled { .. } => ERR_UPSTREAM_DISABLED,
        DomainError::ConnectionTimeout { .. } => ERR_CONNECTION_TIMEOUT,
        DomainError::RequestTimeout { .. } => ERR_REQUEST_TIMEOUT,
        DomainError::Forbidden { .. } => ERR_FORBIDDEN,
    }
}

fn http_status_code(err: &DomainError) -> StatusCode {
    match err {
        DomainError::Validation { .. }
        | DomainError::MissingTargetHost { .. }
        | DomainError::InvalidTargetHost { .. }
        | DomainError::UnknownTargetHost { .. } => StatusCode::BAD_REQUEST,
        DomainError::Conflict { .. } => StatusCode::CONFLICT,
        DomainError::AuthenticationFailed { .. } => StatusCode::UNAUTHORIZED,
        DomainError::NotFound { .. } => StatusCode::NOT_FOUND,
        DomainError::PayloadTooLarge { .. } => StatusCode::PAYLOAD_TOO_LARGE,
        DomainError::RateLimitExceeded { .. } => StatusCode::TOO_MANY_REQUESTS,
        DomainError::SecretNotFound { .. } | DomainError::Internal { .. } => {
            StatusCode::INTERNAL_SERVER_ERROR
        }
        DomainError::DownstreamError { .. } | DomainError::ProtocolError { .. } => {
            StatusCode::BAD_GATEWAY
        }
        DomainError::UpstreamDisabled { .. } => StatusCode::SERVICE_UNAVAILABLE,
        DomainError::ConnectionTimeout { .. } | DomainError::RequestTimeout { .. } => {
            StatusCode::GATEWAY_TIMEOUT
        }
        DomainError::Forbidden { .. } => StatusCode::FORBIDDEN,
    }
}

fn error_title(err: &DomainError) -> &str {
    match err {
        DomainError::Validation { .. } => "Validation Error",
        DomainError::Conflict { .. } => "Conflict",
        DomainError::MissingTargetHost { .. } => "Missing Target Host",
        DomainError::InvalidTargetHost { .. } => "Invalid Target Host",
        DomainError::UnknownTargetHost { .. } => "Unknown Target Host",
        DomainError::AuthenticationFailed { .. } => "Authentication Failed",
        DomainError::NotFound { .. } => "Not Found",
        DomainError::PayloadTooLarge { .. } => "Payload Too Large",
        DomainError::RateLimitExceeded { .. } => "Rate Limit Exceeded",
        DomainError::SecretNotFound { .. } => "Secret Not Found",
        DomainError::DownstreamError { .. } | DomainError::Internal { .. } => "Downstream Error",
        DomainError::ProtocolError { .. } => "Protocol Error",
        DomainError::UpstreamDisabled { .. } => "Upstream Disabled",
        DomainError::ConnectionTimeout { .. } => "Connection Timeout",
        DomainError::RequestTimeout { .. } => "Request Timeout",
        DomainError::Forbidden { .. } => "Forbidden",
    }
}

fn error_instance(err: &DomainError) -> &str {
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
        | DomainError::Internal { .. }
        | DomainError::Forbidden { .. } => "",
    }
}

// ---------------------------------------------------------------------------
// From<DomainError> for Problem
// ---------------------------------------------------------------------------

impl From<DomainError> for Problem {
    fn from(err: DomainError) -> Self {
        let gts = gts_type(&err).to_string();
        let inst = error_instance(&err).to_string();
        let status = http_status_code(&err);
        let t = error_title(&err).to_string();
        let detail = err.to_string();

        Problem::new(status, t, detail)
            .with_type(gts)
            .with_instance(inst)
    }
}

// ---------------------------------------------------------------------------
// Convenience functions for handlers
// ---------------------------------------------------------------------------

/// Convert a `DomainError` into a `Problem`, filling in `instance` for
/// variants that don't carry their own. Used by management API handlers.
pub(crate) fn domain_error_to_problem(err: DomainError, instance: &str) -> Problem {
    let mut p = Problem::from(err);
    if p.instance.is_empty() {
        p.instance = instance.to_string();
    }
    p
}

/// Convert a `DomainError` into an axum `Response` with the
/// `x-oagw-error-source: gateway` header. Used by the proxy handler.
pub fn error_response(err: DomainError) -> Response {
    let retry_after = match &err {
        DomainError::RateLimitExceeded {
            retry_after_secs: Some(secs),
            ..
        } => Some(*secs),
        _ => None,
    };

    let problem: Problem = err.into();
    let mut response = problem.into_response();

    response.headers_mut().insert(
        "x-oagw-error-source",
        HeaderValue::from_static(ErrorSource::Gateway.as_str()),
    );

    if let Some(secs) = retry_after
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
    fn validation_error_produces_correct_problem() {
        let err = DomainError::Validation {
            detail: "missing required field 'server'".into(),
            instance: "/oagw/v1/upstreams".into(),
        };
        let p: Problem = err.into();
        assert_eq!(p.status, StatusCode::BAD_REQUEST);
        assert_eq!(p.type_url, ERR_VALIDATION);
        assert_eq!(p.title, "Validation Error");
        assert!(p.detail.contains("missing required field"));
        assert_eq!(p.instance, "/oagw/v1/upstreams");
    }

    #[test]
    fn conflict_error_produces_409() {
        let err = DomainError::Conflict {
            detail: "alias already exists".into(),
        };
        let p: Problem = err.into();
        assert_eq!(p.status, StatusCode::CONFLICT);
        assert_eq!(p.type_url, ERR_VALIDATION);
        assert_eq!(p.title, "Conflict");
    }

    #[test]
    fn rate_limit_exceeded_produces_429() {
        let err = DomainError::RateLimitExceeded {
            detail: "rate limit exceeded for upstream".into(),
            instance: "/oagw/v1/proxy/api.openai.com/v1/chat/completions".into(),
            retry_after_secs: Some(30),
        };
        let p: Problem = err.into();
        assert_eq!(p.status, StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(p.type_url, ERR_RATE_LIMIT_EXCEEDED);
    }

    #[test]
    fn not_found_produces_404() {
        let err = DomainError::NotFound {
            entity: "route",
            id: uuid::Uuid::nil(),
        };
        let p: Problem = err.into();
        assert_eq!(p.status, StatusCode::NOT_FOUND);
        assert_eq!(p.type_url, ERR_ROUTE_NOT_FOUND);
    }

    #[test]
    fn all_error_types_produce_valid_json() {
        let errors: Vec<DomainError> = vec![
            DomainError::Validation {
                detail: "test".into(),
                instance: "/test".into(),
            },
            DomainError::Conflict {
                detail: "test".into(),
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
            DomainError::Internal {
                message: "test".into(),
            },
            DomainError::Forbidden {
                detail: "test".into(),
            },
        ];
        for err in errors {
            let p: Problem = err.into();
            let json = serde_json::to_string(&p).unwrap();
            let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert!(parsed.get("type").is_some());
            assert!(parsed.get("status").is_some());
            assert!(parsed.get("title").is_some());
            assert!(parsed.get("detail").is_some());
            assert!(parsed.get("instance").is_some());
        }
    }

    #[test]
    fn domain_error_to_problem_fills_missing_instance() {
        let err = DomainError::NotFound {
            entity: "upstream",
            id: uuid::Uuid::nil(),
        };
        let p = domain_error_to_problem(err, "/oagw/v1/upstreams/123");
        assert_eq!(p.instance, "/oagw/v1/upstreams/123");
    }

    #[test]
    fn domain_error_to_problem_preserves_existing_instance() {
        let err = DomainError::Validation {
            detail: "bad input".into(),
            instance: "/oagw/v1/upstreams".into(),
        };
        let p = domain_error_to_problem(err, "/fallback");
        assert_eq!(p.instance, "/oagw/v1/upstreams");
    }

    #[test]
    fn error_response_sets_gateway_header() {
        let err = DomainError::NotFound {
            entity: "route",
            id: uuid::Uuid::nil(),
        };
        let resp = error_response(err);
        assert_eq!(
            resp.headers().get("x-oagw-error-source").unwrap(),
            "gateway"
        );
    }
}
