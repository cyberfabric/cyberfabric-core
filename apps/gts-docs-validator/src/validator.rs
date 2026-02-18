//! GTS ID validation logic.
//!
//! This module provides validation of GTS identifiers by delegating to the
//! authoritative `gts` crate. It does NOT re-implement GTS parsing.

use crate::normalize::NormalizedCandidate;

/// Contexts where wildcards are allowed (in documentation)
pub const WILDCARD_ALLOWED_CONTEXTS: &[&str] = &[
    "pattern",
    "filter",
    "query",
    "$filter",
    "starts_with",
    "with_pattern",
    "resource_pattern",
    "discovery",
    "match",
    "wildcard",
    "differs from",
    "get",
    "list",
];

/// Contexts that indicate "bad example" or intentionally invalid identifiers.
/// Tightened from original: removed overly generic tokens like "error", "fail", "bad".
/// These must appear on the same line as the candidate, before it (proximity constraint).
pub const SKIP_VALIDATION_CONTEXTS: &[&str] = &[
    "\u{274c}",
    "\u{2717}",
    "invalid:", // colon required to avoid matching "invalid" in prose
    "wrong:",
    "bad:",
    "// invalid", // code comment prefix
    "not allowed:",
];

/// Example vendors used in documentation that are tolerated during vendor validation.
/// These are placeholder/example vendors commonly used in docs and tutorials.
pub const EXAMPLE_VENDORS: &[&str] = &[
    "acme",     // Classic example company name
    "globex",   // Another example company name
    "example",  // Generic example
    "demo",     // Demo purposes
    "test",     // Test purposes
    "sample",   // Sample code
    "tutorial", // Tutorial examples
];

/// Check if a vendor is an example/placeholder vendor that should be tolerated
#[must_use]
pub fn is_example_vendor(vendor: &str) -> bool {
    EXAMPLE_VENDORS.contains(&vendor)
}

/// Check if the GTS identifier is in a context where wildcards are allowed.
/// Checks the text before the match on the same line.
#[must_use]
pub fn is_wildcard_context(line: &str, match_start: usize) -> bool {
    // Use get() to safely handle potential mid-codepoint byte offsets
    let before = match line.get(..match_start) {
        Some(s) => s.to_lowercase(),
        None => return false, // Invalid byte offset, assume not wildcard context
    };

    for ctx in WILDCARD_ALLOWED_CONTEXTS {
        if before.contains(ctx) {
            return true;
        }
    }

    // Also check for $filter anywhere in the line
    if line.to_lowercase().contains("$filter") {
        return true;
    }

    false
}

/// Check if the GTS identifier is in a "bad example" context.
/// Tightened: same-line only (no 3-line lookback), with proximity constraint.
/// The skip token must appear BEFORE the candidate on the same line.
#[must_use]
pub fn is_bad_example_context(line: &str, match_start: usize) -> bool {
    // Check current line only, before the match position
    let before = match line.get(..match_start) {
        Some(s) => s.to_lowercase(),
        None => return false,
    };

    for ctx in SKIP_VALIDATION_CONTEXTS {
        if before.contains(ctx) {
            return true;
        }
    }

    false
}

/// Validate a GTS identifier candidate.
///
/// This function delegates all validation to `gts::GtsID::new()` and `gts::GtsWildcard::new()`.
/// It does NOT re-implement GTS parsing.
///
/// # Arguments
///
/// * `candidate` - A normalized candidate (after stripping gts://, quotes, etc.)
/// * `expected_vendor` - Optional vendor to check against (with `EXAMPLE_VENDORS` tolerance)
/// * `allow_wildcards` - Whether wildcard patterns are allowed in this context
///
/// # Returns
///
/// A vector of error messages. Empty if valid.
pub fn validate_candidate(
    candidate: &NormalizedCandidate,
    expected_vendor: Option<&str>,
    allow_wildcards: bool,
) -> Vec<String> {
    let mut errors = Vec::new();
    let gts_id = &candidate.gts_id;

    // Handle wildcards
    if gts_id.contains('*') {
        if !allow_wildcards {
            return vec![format!(
                "Wildcards not allowed outside pattern contexts: '{}'",
                candidate.original
            )];
        }
        // GtsWildcard::new() delegates to GtsID::new() internally,
        // so all spec rules are enforced
        if let Err(e) = gts::GtsWildcard::new(gts_id) {
            errors.push(format!("{e}"));
        }
        // Vendor check for wildcards (if vendor is not wildcarded)
        if let Some(expected) = expected_vendor {
            let rest = &gts_id[4..]; // Remove 'gts.' prefix
            if let Some(first_seg) = rest.split('~').next()
                && let Some(vendor) = first_seg.split('.').next()
                && !vendor.contains('*')
                && vendor != expected
                && !is_example_vendor(vendor)
            {
                errors.push(format!(
                    "Vendor mismatch: expected '{expected}', found '{vendor}'"
                ));
            }
        }
    } else {
        // Delegate to gts crate — the single source of truth
        match gts::GtsID::new(gts_id) {
            Ok(parsed) => {
                // Vendor check
                if let Some(expected) = expected_vendor
                    && let Some(first_seg) = parsed.gts_id_segments.first()
                    && first_seg.vendor != expected
                    && !is_example_vendor(&first_seg.vendor)
                {
                    errors.push(format!(
                        "Vendor mismatch: expected '{expected}', found '{}'",
                        first_seg.vendor
                    ));
                }
            }
            Err(e) => {
                errors.push(format!("{e}"));
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
    use crate::normalize::normalize_candidate;

    #[test]
    fn test_validate_candidate_valid_type() {
        let candidate = normalize_candidate("gts.x.idp.users.user.v1.0~").unwrap();
        let errors = validate_candidate(&candidate, None, false);
        assert!(errors.is_empty(), "Unexpected errors: {errors:?}");
    }

    #[test]
    fn test_validate_candidate_valid_chained() {
        let candidate =
            normalize_candidate("gts.x.core.events.type.v1~ven.app._.custom_event.v1~").unwrap();
        let errors = validate_candidate(&candidate, None, false);
        assert!(errors.is_empty(), "Unexpected errors: {errors:?}");
    }

    #[test]
    fn test_validate_candidate_vendor_match() {
        let candidate = normalize_candidate("gts.x.core.modkit.plugin.v1~").unwrap();
        let errors = validate_candidate(&candidate, Some("x"), false);
        assert!(errors.is_empty(), "Unexpected errors: {errors:?}");
    }

    #[test]
    fn test_validate_candidate_vendor_mismatch() {
        let candidate = normalize_candidate("gts.hx.core.modkit.plugin.v1~").unwrap();
        let errors = validate_candidate(&candidate, Some("x"), false);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("Vendor mismatch"));
    }

    #[test]
    fn test_validate_candidate_example_vendor_tolerated() {
        let candidate = normalize_candidate("gts.acme.core.events.user_created.v1~").unwrap();
        let errors = validate_candidate(&candidate, Some("x"), false);
        assert!(
            errors.is_empty(),
            "Example vendor 'acme' should be tolerated: {errors:?}"
        );

        let candidate = normalize_candidate("gts.globex.core.events.order.v1~").unwrap();
        let errors = validate_candidate(&candidate, Some("x"), false);
        assert!(
            errors.is_empty(),
            "Example vendor 'globex' should be tolerated: {errors:?}"
        );
    }

    #[test]
    fn test_validate_candidate_invalid_hyphen() {
        let candidate = normalize_candidate("gts.my-vendor.core.events.type.v1~").unwrap();
        let errors = validate_candidate(&candidate, None, false);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validate_candidate_invalid_uppercase() {
        let candidate = normalize_candidate("gts.X.core.events.type.v1~").unwrap();
        let errors = validate_candidate(&candidate, None, false);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validate_candidate_invalid_digit_start() {
        let candidate = normalize_candidate("gts.1vendor.core.events.type.v1~").unwrap();
        let errors = validate_candidate(&candidate, None, false);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validate_candidate_wildcard_allowed() {
        let candidate = normalize_candidate("gts.x.*").unwrap();
        let errors = validate_candidate(&candidate, None, true);
        assert!(errors.is_empty(), "Unexpected errors: {errors:?}");
    }

    #[test]
    fn test_validate_candidate_wildcard_not_allowed() {
        let candidate = normalize_candidate("gts.x.*").unwrap();
        let errors = validate_candidate(&candidate, None, false);
        assert!(!errors.is_empty());
        assert!(errors[0].contains("Wildcards"));
    }

    #[test]
    fn test_is_example_vendor() {
        assert!(is_example_vendor("acme"));
        assert!(is_example_vendor("globex"));
        assert!(is_example_vendor("example"));
        assert!(is_example_vendor("demo"));
        assert!(is_example_vendor("test"));
        assert!(!is_example_vendor("x"));
        assert!(!is_example_vendor("hx"));
        assert!(!is_example_vendor("cf"));
    }

    #[test]
    fn test_is_wildcard_context() {
        assert!(is_wildcard_context(
            "$filter=type_id eq 'gts.x.*'",
            "$filter=type_id eq '".len()
        ));
        assert!(is_wildcard_context(
            "Use this pattern: gts.x.core.*",
            "Use this pattern: ".len()
        ));
        assert!(!is_wildcard_context(
            "The type gts.x.core.type.v1~",
            "The type ".len()
        ));
    }

    #[test]
    fn test_is_bad_example_context_same_line_only() {
        // Skip token before the match
        assert!(is_bad_example_context(
            "invalid: gts.bad.id",
            "invalid: ".len()
        ));
        assert!(is_bad_example_context(
            "\u{274c} gts.x.y.z.a.v1~",
            "\u{274c} ".len()
        ));

        // Skip token after the match should NOT skip
        assert!(!is_bad_example_context("gts.x.core.type.v1~ is invalid", 0));

        // Generic "error" in unrelated context should NOT skip (removed from list)
        assert!(!is_bad_example_context(
            "The error handling uses gts.x.core.type.v1~",
            "The error handling uses ".len()
        ));
    }
}
