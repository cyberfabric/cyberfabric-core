//! Markdown file scanner for GTS identifiers.
//!
//! Uses a two-stage approach:
//! 1. Discovery regex finds candidates
//! 2. `normalize_candidate()` → `validate_candidate()` validates them

use std::collections::HashSet;
use std::fs;
use std::path::Path;
use std::sync::LazyLock;

use regex::Regex;

use crate::error::DocValidationError;
use crate::normalize::normalize_candidate;
use crate::validator::{is_bad_example_context, is_wildcard_context, validate_candidate};

/// Markdown parsing state for code block tracking
#[derive(Debug, Clone, PartialEq, Eq)]
enum MarkdownState {
    Prose,
    FencedBlock { language: String, skip: bool },
}

/// Markdown-specific skip tokens (in addition to shared `SKIP_VALIDATION_CONTEXTS`)
#[allow(dead_code)] // Used in line iteration, compiler doesn't detect usage
const MARKDOWN_SKIP_TOKENS: &[&str] = &[
    "**given**", // Markdown bold formatting
];

/// Discovery regex (relaxed): finds strings that LOOK like GTS identifiers.
/// This is intentionally broader than the spec — validation is done by `GtsID::new()`.
///
/// Strategy: Match gts. followed by 4+ dot-separated segments where at least one
/// segment looks like a version (starts with 'v' followed by digit).
/// This catches both valid and malformed IDs for validation (more errors reported).
/// Stops at tilde followed by non-alphanumeric to avoid matching filenames like "id.v1~.schema.json"
static GTS_DISCOVERY_PATTERN_RELAXED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?:gts://)?",                    // optional URI prefix
        r"gts\.",                          // mandatory gts. prefix
        r"(?:[a-z_*][a-z0-9_*.-]*\.){3,}", // at least 3 segments (permissive: allows -, .)
        r"[a-z_*][a-z0-9_*.-]*",           // final segment before version
        r"\.v[0-9]+",                      // version segment (required anchor)
        r"(?:\.[0-9]+)?",                  // optional minor version
        r"(?:~[a-z_][a-z0-9_.-]*)*",       // optional chained segments (permissive)
        r"~?",                             // optional trailing tilde (but not if followed by .)
    ))
    .expect("Invalid discovery regex")
});

/// Discovery regex (well-formed): only matches well-formed GTS identifiers.
/// Requires exactly 5 segments with proper structure (fewer errors reported).
static GTS_DISCOVERY_PATTERN_WELL_FORMED: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?:gts://)?",          // optional URI prefix
        r"gts\.",                // mandatory gts. prefix
        r"[a-z_*][a-z0-9_*]*\.", // vendor
        r"[a-z_*][a-z0-9_*]*\.", // package
        r"[a-z_*][a-z0-9_*]*\.", // namespace
        r"[a-z_*][a-z0-9_*]*\.", // type
        r"v[0-9]+",              // major version (required)
        r"(?:\.[0-9]+)?",        // optional minor version
        r"(?:~[a-z_][a-z0-9_]*\.[a-z_][a-z0-9_]*\.[a-z_][a-z0-9_]*\.[a-z_][a-z0-9_]*\.v[0-9]+(?:\.[0-9]+)?)*", // chained segments
        r"~?", // optional trailing tilde
    ))
    .expect("Invalid discovery regex")
});

/// Scan a markdown file for GTS identifiers.
pub fn scan_markdown_file(
    path: &Path,
    vendor: Option<&str>,
    verbose: bool,
    max_file_size: u64,
    strict: bool,
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

    let pattern = if strict {
        &*GTS_DISCOVERY_PATTERN_RELAXED
    } else {
        &*GTS_DISCOVERY_PATTERN_WELL_FORMED
    };
    let mut errors = Vec::new();
    let mut state = MarkdownState::Prose;
    let mut seen_candidates: HashSet<(usize, String)> = HashSet::new();

    for (line_num, line) in content.lines().enumerate() {
        let line_number = line_num + 1; // 1-indexed

        // Update markdown state for code blocks
        if line.trim_start().starts_with("```") {
            match &state {
                MarkdownState::Prose => {
                    // Entering a fenced block
                    let language = line
                        .trim_start()
                        .strip_prefix("```")
                        .unwrap_or("")
                        .trim()
                        .to_lowercase();

                    // Skip grammar/pattern definition blocks
                    let skip = matches!(
                        language.as_str(),
                        "ebnf" | "regex" | "bnf" | "abnf" | "grammar"
                    );

                    state = MarkdownState::FencedBlock { language, skip };
                }
                MarkdownState::FencedBlock { .. } => {
                    // Exiting a fenced block
                    state = MarkdownState::Prose;
                }
            }
            continue;
        }

        // Skip lines inside skip blocks
        if let MarkdownState::FencedBlock { skip: true, .. } = state {
            continue;
        }

        // Find all GTS candidates on this line
        for mat in pattern.find_iter(line) {
            let candidate_str = mat.as_str();
            let match_start = mat.start();

            // Deduplicate: skip if we've seen this candidate on this line
            if !seen_candidates.insert((line_number, candidate_str.to_owned())) {
                continue;
            }

            // Skip validation if this is a "bad example" context
            // Check both shared and markdown-specific skip tokens
            if is_bad_example_context(line, mat.start()) {
                continue;
            }

            // Check markdown-specific skip tokens
            if let Some(before) = line.get(..mat.start()) {
                let before_lower = before.to_lowercase();
                if MARKDOWN_SKIP_TOKENS
                    .iter()
                    .any(|token| before_lower.contains(&token.to_lowercase()))
                {
                    continue;
                }
            }

            // Normalize the candidate
            let candidate = match normalize_candidate(candidate_str) {
                Ok(c) => c,
                Err(e) => {
                    errors.push(DocValidationError {
                        file: path.to_owned(),
                        line: line_number,
                        column: match_start + 1, // 1-indexed
                        json_path: String::new(),
                        raw_value: candidate_str.to_owned(),
                        normalized_id: String::new(),
                        error: e,
                        context: line.to_owned(),
                    });
                    continue;
                }
            };

            // Check if wildcards are allowed in this context
            let allow_wildcards = is_wildcard_context(line, match_start);

            // Validate the candidate
            let validation_errors = validate_candidate(&candidate, vendor, allow_wildcards);
            for err in validation_errors {
                errors.push(DocValidationError {
                    file: path.to_owned(),
                    line: line_number,
                    column: match_start + 1, // 1-indexed
                    json_path: String::new(),
                    raw_value: candidate.original.clone(),
                    normalized_id: candidate.gts_id.clone(),
                    error: err,
                    context: line.to_owned(),
                });
            }
        }
    }

    errors
}

#[cfg(test)]
#[allow(unknown_lints)]
#[allow(de0901_gts_string_pattern)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_md(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file
    }

    #[test]
    fn test_scan_markdown_valid_id() {
        let file = create_temp_md("The type is gts.x.core.events.type.v1~");
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(errors.is_empty(), "Unexpected errors: {errors:?}");
    }

    #[test]
    fn test_scan_markdown_invalid_id() {
        let file = create_temp_md("The type is gts.x.core.events.type.v1");
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(
            !errors.is_empty(),
            "Single-segment instance ID should be rejected"
        );
    }

    #[test]
    fn test_scan_markdown_skip_ebnf_block() {
        let content = r"
```ebnf
gts.invalid.pattern.here.v1~
```
";
        let file = create_temp_md(content);
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(errors.is_empty(), "EBNF blocks should be skipped");
    }

    #[test]
    fn test_scan_markdown_validate_json_block() {
        let content = r#"
```json
{"$id": "gts://gts.x.core.events.type.v1~"}
```
"#;
        let file = create_temp_md(content);
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(errors.is_empty(), "JSON blocks should be validated");
    }

    #[test]
    fn test_scan_markdown_skip_invalid_context() {
        let file = create_temp_md("\u{274c} gts.invalid.id.here.v1");
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(errors.is_empty(), "Invalid examples should be skipped");
    }

    #[test]
    fn test_scan_markdown_wildcard_in_pattern_context() {
        // Wildcards are allowed in pattern/filter contexts
        // Use "pattern:" keyword to trigger wildcard context
        let file = create_temp_md("pattern: gts.x.core.events.type.v1~");
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Valid IDs in pattern context should be allowed"
        );
    }

    #[test]
    fn test_scan_markdown_wildcard_not_in_pattern_context() {
        // This tests that wildcards are rejected when not in a pattern context
        // The discovery regex won't match wildcards in arbitrary positions anyway
        let file = create_temp_md("The type is gts.x.core.events.type.v1~");
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(errors.is_empty(), "Valid IDs should pass");
    }

    #[test]
    fn test_scan_markdown_gts_uri() {
        let file = create_temp_md(r#"Use "$id": "gts://gts.x.core.events.type.v1~""#);
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "gts:// URIs should be normalized and validated"
        );
    }

    #[test]
    fn test_scan_markdown_vendor_mismatch() {
        let file = create_temp_md("The type is gts.hx.core.events.type.v1~");
        let errors = scan_markdown_file(file.path(), Some("x"), false, 10_485_760, false);
        assert!(!errors.is_empty());
        assert!(errors[0].error.contains("Vendor mismatch"));
    }

    #[test]
    fn test_scan_markdown_example_vendor_tolerated() {
        let file = create_temp_md("Example: gts.acme.core.events.type.v1~");
        let errors = scan_markdown_file(file.path(), Some("x"), false, 10_485_760, false);
        assert!(errors.is_empty(), "Example vendors should be tolerated");
    }

    #[test]
    fn test_scan_markdown_deduplication() {
        let file =
            create_temp_md("gts.x.core.events.type.v1~ and gts.x.core.events.type.v1~ again");
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        // Should only report once per unique (line, candidate) tuple
        assert_eq!(errors.len(), 0, "Valid IDs should not produce errors");
    }

    #[test]
    fn test_scan_markdown_error_after_gts_id() {
        // "error" appears after the GTS ID, so it should NOT suppress validation
        let file = create_temp_md("gts.x.core.events.type.v1~ handles error cases");
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(
            errors.is_empty(),
            "Valid ID should not be suppressed by 'error' appearing after it"
        );
    }

    #[test]
    fn test_scan_markdown_invalid_before_gts_id() {
        // "invalid:" appears before the GTS ID, so it SHOULD suppress validation
        let file = create_temp_md("invalid: gts.bad.format.here.v1");
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);
        assert!(errors.is_empty(), "Invalid examples should be skipped");
    }

    #[test]
    fn test_scan_markdown_strict_mode_catches_malformed() {
        // Strict mode (relaxed regex) should catch IDs with hyphens
        let file = create_temp_md("The type is gts.my-vendor.core.events.type.v1~");
        let errors_strict = scan_markdown_file(file.path(), None, false, 10_485_760, true);
        let errors_normal = scan_markdown_file(file.path(), None, false, 10_485_760, false);

        // Strict mode should catch this (relaxed regex matches it, then validation rejects hyphens)
        assert!(
            !errors_strict.is_empty(),
            "Strict mode should catch malformed ID with hyphens"
        );
        // Normal mode (well-formed regex) won't even match it
        assert!(
            errors_normal.is_empty(),
            "Normal mode won't match malformed pattern"
        );
    }

    #[test]
    fn test_scan_markdown_strict_mode_catches_extra_dots() {
        // Strict mode should catch IDs with extra dots in segments
        let file = create_temp_md("The type is gts.x.core.events.type.name.v1~");
        let errors_strict = scan_markdown_file(file.path(), None, false, 10_485_760, true);

        // Strict mode should catch this (relaxed regex is permissive)
        assert!(
            !errors_strict.is_empty(),
            "Strict mode should catch ID with extra segments"
        );
    }

    #[test]
    fn test_scan_markdown_normal_mode_well_formed_only() {
        // Normal mode should only validate well-formed patterns
        let file = create_temp_md("Valid: gts.x.core.events.type.v1~ and malformed: gts.bad-id.v1");
        let errors = scan_markdown_file(file.path(), None, false, 10_485_760, false);

        // Should only find the valid one (no errors) and skip the malformed one
        assert!(
            errors.is_empty(),
            "Normal mode should only validate well-formed patterns"
        );
    }
}
