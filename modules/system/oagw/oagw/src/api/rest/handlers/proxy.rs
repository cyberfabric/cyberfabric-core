use axum::body::Body;
use axum::extract::{Extension, Request};
use axum::response::Response;
use modkit_security::SecurityContext;
use crate::domain::error::DomainError;
use crate::domain::dto::{ErrorSource, ProxyContext};

use crate::api::rest::error::error_response;
use crate::module::AppState;

const MAX_BODY_SIZE: usize = 100 * 1024 * 1024; // 100 MB

/// Proxy handler for `/oagw/v1/proxy/{alias}/{path:.*}`.
///
/// Parses the alias and path suffix from the URL, validates the request,
/// builds a `ProxyContext`, and delegates to the Data Plane service.
pub async fn proxy_handler(
    Extension(state): Extension<AppState>,
    Extension(ctx): Extension<SecurityContext>,
    req: Request,
) -> Result<Response, Response> {
    let (parts, body) = req.into_parts();

    // Parse alias and path suffix from the URI.
    let path = parts.uri.path();
    let prefix = "/oagw/v1/proxy/";
    let remaining = path.strip_prefix(prefix).ok_or_else(|| {
        error_response(DomainError::Validation {
            detail: "invalid proxy path".into(),
            instance: path.to_string(),
        })
    })?;

    // Split alias from path suffix at the first '/'.
    let (alias, path_suffix) = match remaining.find('/') {
        Some(pos) => (&remaining[..pos], &remaining[pos..]),
        None => (remaining, ""),
    };

    if alias.is_empty() {
        return Err(error_response(DomainError::Validation {
            detail: "missing alias in proxy path".into(),
            instance: path.to_string(),
        }));
    }

    // Parse query parameters.
    let query_params: Vec<(String, String)> = parts
        .uri
        .query()
        .map(|q| {
            q.split('&')
                .filter(|s| !s.is_empty())
                .map(|pair| {
                    let mut parts = pair.splitn(2, '=');
                    let key = parts.next().unwrap_or("").to_string();
                    let value = parts.next().unwrap_or("").to_string();
                    (key, value)
                })
                .collect()
        })
        .unwrap_or_default();

    // Validate Content-Length if present.
    if let Some(cl) = parts.headers.get(http::header::CONTENT_LENGTH) {
        let cl_str = cl.to_str().map_err(|_| {
            error_response(DomainError::Validation {
                detail: "invalid Content-Length header".into(),
                instance: path.to_string(),
            })
        })?;
        let cl_val: usize = cl_str.parse().map_err(|_| {
            error_response(DomainError::Validation {
                detail: format!("Content-Length is not a valid integer: '{cl_str}'"),
                instance: path.to_string(),
            })
        })?;
        if cl_val > MAX_BODY_SIZE {
            return Err(error_response(DomainError::PayloadTooLarge {
                detail: format!(
                    "request body of {cl_val} bytes exceeds maximum of {MAX_BODY_SIZE} bytes"
                ),
                instance: path.to_string(),
            }));
        }
    }

    // Read body bytes (limited to MAX_BODY_SIZE).
    let body_bytes = axum::body::to_bytes(body, MAX_BODY_SIZE)
        .await
        .map_err(|_| {
            error_response(DomainError::PayloadTooLarge {
                detail: format!("request body exceeds maximum of {MAX_BODY_SIZE} bytes"),
                instance: path.to_string(),
            })
        })?;

    let instance_uri = path.to_string();

    // Build ProxyContext.
    let proxy_ctx = ProxyContext {
        ctx,
        method: parts.method,
        alias: alias.to_string(),
        path_suffix: path_suffix.to_string(),
        query_params,
        headers: parts.headers,
        body: body_bytes,
        instance_uri,
    };

    // Execute proxy pipeline.
    let proxy_response = state.dp.proxy_request(proxy_ctx).await.map_err(error_response)?;

    // Build axum response from ProxyResponse.
    let mut builder = Response::builder().status(proxy_response.status);

    // Copy upstream response headers.
    for (name, value) in proxy_response.headers.iter() {
        builder = builder.header(name, value);
    }

    // Add error source header.
    let source = match proxy_response.error_source {
        ErrorSource::Gateway => "gateway",
        ErrorSource::Upstream => "upstream",
    };
    builder = builder.header("x-oagw-error-source", source);

    // Stream the response body.
    let body = Body::from_stream(proxy_response.body);

    builder.body(body).map_err(|e| {
        error_response(DomainError::DownstreamError {
            detail: format!("failed to build response: {e}"),
            instance: String::new(),
        })
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn parse_alias_and_suffix() {
        let path = "/oagw/v1/proxy/api.openai.com/v1/chat/completions";
        let prefix = "/oagw/v1/proxy/";
        let remaining = path.strip_prefix(prefix).unwrap();
        let (alias, suffix) = match remaining.find('/') {
            Some(pos) => (&remaining[..pos], &remaining[pos..]),
            None => (remaining, ""),
        };
        assert_eq!(alias, "api.openai.com");
        assert_eq!(suffix, "/v1/chat/completions");
    }

    #[test]
    fn parse_alias_with_port() {
        let path = "/oagw/v1/proxy/host:8443/path";
        let prefix = "/oagw/v1/proxy/";
        let remaining = path.strip_prefix(prefix).unwrap();
        let (alias, suffix) = match remaining.find('/') {
            Some(pos) => (&remaining[..pos], &remaining[pos..]),
            None => (remaining, ""),
        };
        assert_eq!(alias, "host:8443");
        assert_eq!(suffix, "/path");
    }

    #[test]
    fn parse_alias_no_suffix() {
        let path = "/oagw/v1/proxy/api.openai.com";
        let prefix = "/oagw/v1/proxy/";
        let remaining = path.strip_prefix(prefix).unwrap();
        let (alias, suffix) = match remaining.find('/') {
            Some(pos) => (&remaining[..pos], &remaining[pos..]),
            None => (remaining, ""),
        };
        assert_eq!(alias, "api.openai.com");
        assert_eq!(suffix, "");
    }

    #[test]
    fn parse_query_params() {
        let query = "version=2&model=gpt-4";
        let params: Vec<(String, String)> = query
            .split('&')
            .filter(|s| !s.is_empty())
            .map(|pair| {
                let mut parts = pair.splitn(2, '=');
                let key = parts.next().unwrap_or("").to_string();
                let value = parts.next().unwrap_or("").to_string();
                (key, value)
            })
            .collect();
        assert_eq!(params.len(), 2);
        assert_eq!(params[0], ("version".into(), "2".into()));
        assert_eq!(params[1], ("model".into(), "gpt-4".into()));
    }
}
