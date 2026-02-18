//! YAML file scanner for GTS identifiers.
//!
//! Uses tree-walking to scan string values (not keys by default).

use std::fs;
use std::path::Path;

use serde_json::Value;

use crate::error::DocValidationError;
use crate::scan_json::walk_json_value;

/// Scan a YAML file for GTS identifiers.
pub fn scan_yaml_file(
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

    // Parse YAML to serde_json::Value via serde
    let value: Value = match serde_saphyr::from_str(&content) {
        Ok(v) => v,
        Err(e) => {
            if verbose {
                eprintln!("  Skipping {} (invalid YAML): {e}", path.display());
            }
            return vec![];
        }
    };

    let mut errors = Vec::new();
    // Reuse the JSON walker since both operate on serde_json::Value
    walk_json_value(&value, path, vendor, &mut errors, "$", scan_keys);
    errors
}

#[cfg(test)]
#[allow(unknown_lints)]
#[allow(de0901_gts_string_pattern)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_yaml(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_scan_yaml_valid_id() {
        let content = r"
$id: gts://gts.x.core.events.type.v1~
";
        let file = create_temp_yaml(content);
        let errors = scan_yaml_file(file.path(), None, false, 10_485_760, false);
        assert!(errors.is_empty(), "Unexpected errors: {errors:?}");
    }

    #[test]
    fn test_scan_yaml_invalid_id() {
        let content = r"
$id: gts.invalid
";
        let file = create_temp_yaml(content);
        let errors = scan_yaml_file(file.path(), None, false, 10_485_760, false);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_scan_yaml_xgts_ref_wildcard() {
        let content = r"
x-gts-ref: gts.x.core.*
";
        let file = create_temp_yaml(content);
        let errors = scan_yaml_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Wildcards in x-gts-ref should be allowed"
        );
    }

    #[test]
    fn test_scan_yaml_xgts_ref_bare_wildcard() {
        let content = r#"
x-gts-ref: "*"
"#;
        let file = create_temp_yaml(content);
        let errors = scan_yaml_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Bare wildcard in x-gts-ref should be skipped"
        );
    }

    #[test]
    fn test_scan_yaml_nested_values() {
        let content = r"
properties:
  type:
    x-gts-ref: gts.x.core.events.type.v1~
";
        let file = create_temp_yaml(content);
        let errors = scan_yaml_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Nested values should be found and validated"
        );
    }

    #[test]
    fn test_scan_yaml_array_values() {
        let content = r"
capabilities:
  - gts.x.core.events.type.v1~
  - gts.x.core.events.topic.v1~
";
        let file = create_temp_yaml(content);
        let errors = scan_yaml_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Array values should be found and validated"
        );
    }

    #[test]
    fn test_scan_yaml_invalid_yaml() {
        let content = r"
invalid: yaml: syntax:
";
        let file = create_temp_yaml(content);
        let errors = scan_yaml_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Invalid YAML should be skipped with warning"
        );
    }
}
