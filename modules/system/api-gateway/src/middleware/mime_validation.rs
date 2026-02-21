//! MIME type validation middleware for enforcing per-operation allowed Content-Type headers
use axum::extract::Request;
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use dashmap::DashMap;
use http::Method;
use std::sync::Arc;

use modkit::api::{OperationSpec, Problem};

/// Map from (method, path) to allowed content types
pub type MimeValidationMap = Arc<DashMap<(Method, String), Vec<&'static str>>>;

/// Build MIME validation map from operation specs
#[must_use]
pub fn build_mime_validation_map(specs: &[OperationSpec]) -> MimeValidationMap {
    let map = DashMap::new();

    for spec in specs {
        if let Some(ref allowed) = spec.allowed_request_content_types {
            let key = (spec.method.clone(), spec.path.clone());

            map.insert(key, allowed.clone());
        }
    }

    Arc::new(map)
}

/// Extract and normalize the Content-Type header value.
///
/// Strips parameters like charset from "application/json; charset=utf-8"
/// to just "application/json".
fn extract_content_type(req: &Request) -> Option<String> {
    let ct_header = req.headers().get(http::header::CONTENT_TYPE)?;
    let ct_str = ct_header.to_str().ok()?;
    let ct_main = ct_str.split(';').next().map_or(ct_str, str::trim);
    Some(ct_main.to_owned())
}

/// Create an Unsupported Media Type error response.
fn create_unsupported_media_type_error(detail: String) -> Response {
    Problem::new(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "Unsupported Media Type",
        detail,
    )
    .into_response()
}

/// Validate that the content type is in the allowed list.
///
/// Returns Ok(()) if allowed, Err(Response) with error details if not.
fn validate_content_type(
    content_type: &str,
    allowed_types: &[&str],
    method: &Method,
    path: &str,
) -> Result<(), Box<Response>> {
    if allowed_types.contains(&content_type) {
        return Ok(());
    }

    tracing::warn!(
        method = %method,
        path = %path,
        content_type = content_type,
        allowed_types = ?allowed_types,
        "MIME type not allowed for this endpoint"
    );

    let detail = format!(
        "Content-Type '{}' is not allowed for this endpoint. Allowed types: {}",
        content_type,
        allowed_types.join(", ")
    );

    Err(Box::new(create_unsupported_media_type_error(detail)))
}

/// MIME validation middleware
///
/// Checks the Content-Type header against the allowed types configured
/// for the operation. Returns 415 Unsupported Media Type if the content
/// type is not allowed.
pub async fn mime_validation_middleware(
    validation_map: MimeValidationMap,
    req: Request,
    next: Next,
) -> Response {
    let method = req.method().clone();
    // Use MatchedPath extension (set by Axum router) for accurate route matching
    let path = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map_or_else(|| req.uri().path().to_owned(), |p| p.as_str().to_owned());

    let nestedpath = req.extensions().get::<axum::extract::NestedPath>();

    let path = nestedpath
        .and_then(|np| path.strip_prefix(np.as_str()))
        .unwrap_or(&path)
        .to_owned();

    // Check if this operation has MIME validation configured
    let Some(allowed_types) = validation_map.get(&(method.clone(), path.clone())) else {
        // No validation configured - proceed
        return next.run(req).await;
    };

    // Extract and validate Content-Type header
    let Some(content_type) = extract_content_type(&req) else {
        tracing::warn!(
            method = %method,
            path = %path,
            allowed_types = ?allowed_types.value(),
            "Missing Content-Type header for endpoint with MIME validation"
        );

        let detail = format!(
            "Missing Content-Type header. Allowed types: {}",
            allowed_types.join(", ")
        );
        return create_unsupported_media_type_error(detail);
    };

    // Validate the content type
    if let Err(error_response) =
        validate_content_type(&content_type, &allowed_types, &method, &path)
    {
        return *error_response;
    }

    // Validation passed - proceed
    next.run(req).await
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use modkit::api::operation_builder::VendorExtensions;

    #[test]
    fn test_build_mime_validation_map() {
        use modkit::api::operation_builder::{RequestBodySchema, RequestBodySpec};

        let specs = vec![OperationSpec {
            method: Method::POST,
            path: "/files/v1/upload".to_owned(),
            operation_id: None,
            summary: None,
            description: None,
            tags: vec![],
            params: vec![],
            request_body: Some(RequestBodySpec {
                content_type: "multipart/form-data",
                description: None,
                schema: RequestBodySchema::MultipartFile {
                    field_name: "file".to_owned(),
                },
                required: true,
            }),
            responses: vec![],
            handler_id: "test".to_owned(),
            authenticated: false,
            is_public: false,
            license_requirement: None,
            rate_limit: None,
            allowed_request_content_types: Some(vec!["multipart/form-data", "application/pdf"]),
            vendor_extensions: VendorExtensions::default(),
        }];

        let map = build_mime_validation_map(&specs);

        assert!(map.contains_key(&(Method::POST, "/files/v1/upload".to_owned())));
        let allowed = map
            .get(&(Method::POST, "/files/v1/upload".to_owned()))
            .unwrap();
        assert_eq!(allowed.len(), 2);
        assert!(allowed.contains(&"multipart/form-data"));
        assert!(allowed.contains(&"application/pdf"));
    }

    #[test]
    fn test_content_type_parameter_stripping() {
        // Test the logic for stripping parameters from Content-Type
        let ct_with_charset = "application/json; charset=utf-8";
        let ct_main = ct_with_charset
            .split(';')
            .next()
            .map_or(ct_with_charset, str::trim);

        assert_eq!(ct_main, "application/json");

        // Test with multiple parameters
        let ct_complex = "multipart/form-data; boundary=----WebKitFormBoundary7MA4YWxkTrZu0gW";
        let ct_main2 = ct_complex.split(';').next().map_or(ct_complex, str::trim);

        assert_eq!(ct_main2, "multipart/form-data");

        // Test without parameters
        let ct_simple = "application/pdf";
        let ct_main3 = ct_simple.split(';').next().map_or(ct_simple, str::trim);

        assert_eq!(ct_main3, "application/pdf");
    }
}
