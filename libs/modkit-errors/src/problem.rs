//! RFC 9457 Problem Details for HTTP APIs (pure data model, no HTTP framework dependencies)

use http::StatusCode;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

// @cpt-constraint:cpt-cf-ues-constraint-rfc9457:p1
/// Content type for Problem Details as per RFC 9457.
pub const APPLICATION_PROBLEM_JSON: &str = "application/problem+json";

/// Custom serializer for `StatusCode` to u16
#[allow(clippy::trivially_copy_pass_by_ref)] // serde requires &T signature
fn serialize_status_code<S>(status: &StatusCode, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u16(status.as_u16())
}

/// Custom deserializer for `StatusCode` from u16
fn deserialize_status_code<'de, D>(deserializer: D) -> Result<StatusCode, D::Error>
where
    D: Deserializer<'de>,
{
    let code = u16::deserialize(deserializer)?;
    StatusCode::from_u16(code).map_err(serde::de::Error::custom)
}

// @cpt-interface:cpt-cf-ues-interface-problem:p1
/// RFC 9457 Problem Details for HTTP APIs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(
    feature = "utoipa",
    schema(
        title = "Problem",
        description = "RFC 9457 Problem Details for HTTP APIs"
    )
)]
#[must_use]
pub struct Problem {
    /// A URI reference that identifies the problem type.
    /// When dereferenced, it might provide human-readable documentation.
    #[serde(rename = "type")]
    pub type_url: String,
    /// A short, human-readable summary of the problem type.
    pub title: String,
    /// The HTTP status code for this occurrence of the problem.
    /// Serializes as u16 for RFC 9457 compatibility.
    #[serde(
        serialize_with = "serialize_status_code",
        deserialize_with = "deserialize_status_code"
    )]
    #[cfg_attr(feature = "utoipa", schema(value_type = u16))]
    pub status: StatusCode,
    /// A human-readable explanation specific to this occurrence of the problem.
    pub detail: String,
    /// A URI reference that identifies the specific occurrence of the problem.
    pub instance: String,
    /// Optional machine-readable error code defined by the application.
    pub code: String,
    /// Optional trace id useful for tracing.
    pub trace_id: Option<String>,
    /// Optional validation errors for 4xx problems.
    pub errors: Option<Vec<ValidationViolation>>,
}

/// Individual validation violation for a specific field or property.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(title = "ValidationViolation"))]
pub struct ValidationViolation {
    /// field path, e.g. "email" or "user.email"
    pub field: String,
    /// Human-readable message describing the validation error
    pub message: String,
    /// Optional machine-readable error code
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

/// Collection of validation errors for 422 responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(title = "ValidationError"))]
pub struct ValidationError {
    /// List of individual validation violations
    pub errors: Vec<ValidationViolation>,
}

/// Wrapper for `ValidationError` that can be used as a standalone response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[cfg_attr(feature = "utoipa", schema(title = "ValidationErrorResponse"))]
pub struct ValidationErrorResponse {
    /// The validation errors
    #[serde(flatten)]
    pub validation: ValidationError,
}

impl Problem {
    /// Create a new Problem with the given status, title, and detail.
    ///
    /// Note: This function accepts `http::StatusCode` for type safety.
    /// The status is serialized as `u16` for RFC 9457 compatibility.
    pub fn new(status: StatusCode, title: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            type_url: "about:blank".to_owned(),
            title: title.into(),
            status,
            detail: detail.into(),
            instance: String::new(),
            code: String::new(),
            trace_id: None,
            errors: None,
        }
    }

    pub fn with_type(mut self, type_url: impl Into<String>) -> Self {
        self.type_url = type_url.into();
        self
    }

    pub fn with_instance(mut self, uri: impl Into<String>) -> Self {
        self.instance = uri.into();
        self
    }

    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = code.into();
        self
    }

    pub fn with_trace_id(mut self, id: impl Into<String>) -> Self {
        self.trace_id = Some(id.into());
        self
    }

    pub fn with_errors(mut self, errors: Vec<ValidationViolation>) -> Self {
        self.errors = Some(errors);
        self
    }
}

/// Axum integration: make Problem directly usable as a response.
///
/// Automatically enriches the Problem with `trace_id` from the current
/// tracing span if not already set.
#[cfg(feature = "axum")]
impl axum::response::IntoResponse for Problem {
    fn into_response(self) -> axum::response::Response {
        use axum::http::HeaderValue;

        // Enrich with trace_id from current span if not already set
        let problem = if self.trace_id.is_none() {
            match tracing::Span::current().id() {
                Some(span_id) => self.with_trace_id(span_id.into_u64().to_string()),
                _ => self,
            }
        } else {
            self
        };

        let status = problem.status;
        let mut resp = axum::Json(problem).into_response();
        *resp.status_mut() = status;
        resp.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static(APPLICATION_PROBLEM_JSON),
        );
        resp
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn problem_builder_pattern() {
        let p = Problem::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "Validation Failed",
            "Input validation errors",
        )
        .with_code("VALIDATION_ERROR")
        .with_instance("/users/123")
        .with_trace_id("req-456")
        .with_errors(vec![ValidationViolation {
            message: "Email is required".to_owned(),
            field: "email".to_owned(),
            code: None,
        }]);

        assert_eq!(p.status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(p.code, "VALIDATION_ERROR");
        assert_eq!(p.instance, "/users/123");
        assert_eq!(p.trace_id, Some("req-456".to_owned()));
        assert!(p.errors.is_some());
        assert_eq!(p.errors.as_ref().unwrap().len(), 1);
    }

    #[test]
    fn problem_serializes_status_as_u16() {
        let p = Problem::new(StatusCode::NOT_FOUND, "Not Found", "Resource not found");
        let json = serde_json::to_string(&p).unwrap();
        assert!(json.contains("\"status\":404"));
    }

    #[test]
    fn problem_deserializes_status_from_u16() {
        let json = r#"{"type":"about:blank","title":"Not Found","status":404,"detail":"Resource not found","instance":"","code":"","trace_id":null,"errors":null}"#;
        let p: Problem = serde_json::from_str(json).unwrap();
        assert_eq!(p.status, StatusCode::NOT_FOUND);
    }
}
