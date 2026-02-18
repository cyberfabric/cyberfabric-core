//! JSON file scanner for GTS identifiers.
//!
//! Uses tree-walking to scan string values (not keys by default).

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::error::DocValidationError;
use crate::normalize::normalize_candidate;
use crate::validator::validate_candidate;

/// Scan a JSON file for GTS identifiers.
pub fn scan_json_file(
    path: &Path,
    vendor: Option<&str>,
    verbose: bool,
    max_file_size: u64,
    scan_keys: bool,
) -> Vec<DocValidationError> {
    // Check file size
    if let Ok(metadata) = fs::metadata(path)
        && metadata.len() > max_file_size
    {
        if verbose {
            eprintln!(
                "  Skipping {} (size {} exceeds limit {})",
                path.display(),
                metadata.len(),
                max_file_size
            );
        }
        return vec![];
    }

    // Read as UTF-8; skip file with warning on encoding error
    let content = match fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => {
            if verbose {
                eprintln!("  Skipping {} (read error): {}", path.display(), e);
            }
            return vec![];
        }
    };

    let value: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            if verbose {
                eprintln!("  Skipping {} (invalid JSON): {e}", path.display());
            }
            return vec![];
        }
    };

    let mut errors = Vec::new();
    walk_json_value(&value, path, vendor, &mut errors, "$", scan_keys);
    errors
}

/// Walk a JSON value tree and validate GTS identifiers in string values.
/// This is shared by both JSON and YAML scanners.
pub fn walk_json_value(
    value: &Value,
    path: &Path,
    vendor: Option<&str>,
    errors: &mut Vec<DocValidationError>,
    json_path: &str,
    scan_keys: bool,
) {
    match value {
        Value::String(s) => {
            let candidate_str = s.as_str();
            let is_xgts_ref = json_path.ends_with(".x-gts-ref");

            // PRE-FILTER: x-gts-ref special values that are NOT GTS identifiers.
            // These must be checked BEFORE normalization to avoid misleading errors.
            // Spec section 9.6 defines allowed x-gts-ref values:
            //   - GTS identifier (gts.vendor.pkg...)
            //   - Wildcard pattern (gts.*)
            //   - Bare wildcard (*)
            //   - Relative JSON pointer (/$id, /properties/id, etc.)
            if is_xgts_ref && (candidate_str.starts_with('/') || candidate_str == "*") {
                return; // valid x-gts-ref value, not a GTS ID to validate
            }

            // Only consider strings that look like GTS identifiers
            // Skip filenames that contain GTS IDs (e.g., "gts.x.core.type.v1~.schema.json")
            // A string is likely a filename if it contains a tilde followed by a dot and extension
            let looks_like_filename = candidate_str.contains("~.")
                && candidate_str
                    .rfind('.')
                    .is_some_and(|pos| pos > candidate_str.rfind('~').unwrap_or(0));

            if (candidate_str.starts_with("gts://gts.") || candidate_str.starts_with("gts."))
                && !looks_like_filename
            {
                match normalize_candidate(candidate_str) {
                    Ok(candidate) => {
                        let allow_wildcards = is_xgts_ref;
                        let validation_errors =
                            validate_candidate(&candidate, vendor, allow_wildcards);
                        for err in validation_errors {
                            errors.push(DocValidationError {
                                file: path.to_owned(),
                                line: 0,
                                column: 0,
                                json_path: json_path.to_owned(),
                                raw_value: candidate.original.clone(),
                                normalized_id: candidate.gts_id.clone(),
                                error: err,
                                context: json_path.to_owned(),
                            });
                        }
                    }
                    Err(e) => {
                        errors.push(DocValidationError {
                            file: path.to_owned(),
                            line: 0,
                            column: 0,
                            json_path: json_path.to_owned(),
                            raw_value: candidate_str.to_owned(),
                            normalized_id: String::new(),
                            error: e,
                            context: json_path.to_owned(),
                        });
                    }
                }
            }
        }
        Value::Object(map) => {
            for (key, val) in map {
                // Optionally scan keys
                if scan_keys && (key.starts_with("gts://") || key.starts_with("gts.")) {
                    match normalize_candidate(key) {
                        Ok(candidate) => {
                            let validation_errors = validate_candidate(&candidate, vendor, false);
                            for err in validation_errors {
                                errors.push(DocValidationError {
                                    file: path.to_owned(),
                                    line: 0,
                                    column: 0,
                                    json_path: format!("{json_path}.{key}"),
                                    raw_value: candidate.original.clone(),
                                    normalized_id: candidate.gts_id.clone(),
                                    error: err,
                                    context: format!("key: {key}"),
                                });
                            }
                        }
                        Err(e) => {
                            errors.push(DocValidationError {
                                file: path.to_owned(),
                                line: 0,
                                column: 0,
                                json_path: format!("{json_path}.{key}"),
                                raw_value: key.clone(),
                                normalized_id: String::new(),
                                error: e,
                                context: format!("key: {key}"),
                            });
                        }
                    }
                }
                walk_json_value(
                    val,
                    path,
                    vendor,
                    errors,
                    &format!("{json_path}.{key}"),
                    scan_keys,
                );
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                walk_json_value(
                    val,
                    path,
                    vendor,
                    errors,
                    &format!("{json_path}[{i}]"),
                    scan_keys,
                );
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_json(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_scan_json_valid_id() {
        let content = r#"{"$id": "gts://gts.x.core.events.type.v1~"}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(errors.is_empty(), "Unexpected errors: {errors:?}");
    }

    #[test]
    fn test_scan_json_invalid_id() {
        let content = r#"{"$id": "gts.invalid"}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_scan_json_xgts_ref_wildcard() {
        let content = r#"{"x-gts-ref": "gts.x.core.*"}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Wildcards in x-gts-ref should be allowed"
        );
    }

    #[test]
    fn test_scan_json_xgts_ref_bare_wildcard() {
        let content = r#"{"x-gts-ref": "*"}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Bare wildcard in x-gts-ref should be skipped"
        );
    }

    #[test]
    fn test_scan_json_xgts_ref_relative_pointer() {
        let content = r#"{"x-gts-ref": "/$id"}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Relative pointers in x-gts-ref should be skipped"
        );
    }

    #[test]
    fn test_scan_json_nested_values() {
        let content = r#"{
            "properties": {
                "type": {
                    "x-gts-ref": "gts.x.core.events.type.v1~"
                }
            }
        }"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Nested values should be found and validated"
        );
    }

    #[test]
    fn test_scan_json_array_values() {
        let content = r#"{
            "capabilities": [
                "gts.x.core.events.type.v1~",
                "gts.x.core.events.topic.v1~"
            ]
        }"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Array values should be found and validated"
        );
    }

    #[test]
    fn test_scan_json_invalid_json() {
        let content = r#"{"invalid": json}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Invalid JSON should be skipped with warning"
        );
    }

    #[test]
    fn test_scan_json_error_includes_json_path() {
        let content = r#"{"properties": {"type": {"x-gts-ref": "gts.invalid"}}}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(!errors.is_empty());
        assert!(errors[0].json_path.contains("properties.type.x-gts-ref"));
    }

    #[test]
    fn test_scan_json_vendor_mismatch() {
        let content = r#"{"$id": "gts://gts.hx.core.events.type.v1~"}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), Some("x"), false, 10_485_760, false);
        assert!(!errors.is_empty());
        assert!(errors[0].error.contains("Vendor mismatch"));
    }

    #[test]
    fn test_scan_json_keys_not_scanned_by_default() {
        let content = r#"{"gts.x.core.type.v1~": "value"}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, false);
        assert!(errors.is_empty(), "Keys should not be scanned by default");
    }

    #[test]
    fn test_scan_json_keys_scanned_when_enabled() {
        let content = r#"{"gts.x.core.events.type.v1~": "value"}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, true);
        assert!(
            errors.is_empty(),
            "Valid GTS ID keys should pass validation"
        );
    }

    #[test]
    fn test_scan_json_invalid_key_when_scanning_enabled() {
        let content = r#"{"gts.invalid": "value"}"#;
        let file = create_temp_json(content);
        let errors = scan_json_file(file.path(), None, false, 10_485_760, true);
        assert!(
            !errors.is_empty(),
            "Invalid GTS ID keys should be caught when key scanning is enabled"
        );
    }
}
