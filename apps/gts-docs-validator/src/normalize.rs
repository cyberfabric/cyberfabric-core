//! Normalization of GTS identifier candidates.
//!
//! This module provides a single normalization function that ALL scanners must call
//! before passing candidates to the validator. It handles:
//! - Trimming whitespace
//! - Stripping surrounding quotes
//! - Stripping `gts://` URI prefix
//! - Rejecting URI fragments (#) and query strings (?)
//! - Verifying the `gts.` prefix

/// Result of normalizing a raw candidate string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCandidate {
    /// The canonical GTS identifier string (ready for `GtsID::new()`)
    pub gts_id: String,
    /// Whether the original had a gts:// prefix
    pub had_uri_prefix: bool,
    /// The original raw string (for error reporting)
    pub original: String,
}

/// Normalize a raw candidate string into a form suitable for `GtsID::new()`.
///
/// Steps:
/// 1. Trim whitespace
/// 2. Strip surrounding quotes (" or ')
/// 3. Strip `gts://` prefix if present
/// 4. Reject if URI fragment (#) or query (?) is present after gts://
/// 5. Verify starts with `gts.`
///
/// # Errors
///
/// Returns an error if:
/// - The string contains URI fragments (#) or query strings (?) after `gts://`
/// - The string does not start with `gts.` after normalization
///
/// # Examples
///
/// ```
/// use gts_docs_validator::normalize::normalize_candidate;
///
/// // Strip gts:// prefix
/// let result = normalize_candidate("gts://gts.x.core.type.v1~").unwrap();
/// assert_eq!(result.gts_id, "gts.x.core.type.v1~");
/// assert!(result.had_uri_prefix);
///
/// // Plain GTS ID passthrough
/// let result = normalize_candidate("gts.x.core.type.v1~").unwrap();
/// assert_eq!(result.gts_id, "gts.x.core.type.v1~");
/// assert!(!result.had_uri_prefix);
///
/// // Reject fragments
/// assert!(normalize_candidate("gts://gts.x.core.type.v1~#foo").is_err());
///
/// // Reject query strings
/// assert!(normalize_candidate("gts://gts.x.core.type.v1~?bar=1").is_err());
/// ```
pub fn normalize_candidate(raw: &str) -> Result<NormalizedCandidate, String> {
    let trimmed = raw.trim().trim_matches(|c: char| c == '"' || c == '\'');

    let (gts_id, had_uri_prefix) = if let Some(stripped) = trimmed.strip_prefix("gts://") {
        // Reject URI fragments and query strings — spec section 9.1 says
        // remainder must be a plain GTS identifier
        if stripped.contains('#') || stripped.contains('?') {
            return Err(format!(
                "gts:// URI must not contain fragments (#) or query strings (?): '{raw}'"
            ));
        }
        (stripped.to_owned(), true)
    } else {
        (trimmed.to_owned(), false)
    };

    if !gts_id.starts_with("gts.") {
        return Err(format!("Does not start with 'gts.': '{raw}'"));
    }

    Ok(NormalizedCandidate {
        gts_id,
        had_uri_prefix,
        original: raw.to_owned(),
    })
}

#[cfg(test)]
#[allow(unknown_lints)]
#[allow(de0901_gts_string_pattern)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_gts_uri() {
        let result = normalize_candidate("gts://gts.x.core.type.v1~").unwrap();
        assert_eq!(result.gts_id, "gts.x.core.type.v1~");
        assert!(result.had_uri_prefix);
        assert_eq!(result.original, "gts://gts.x.core.type.v1~");
    }

    #[test]
    fn test_normalize_plain_gts_id() {
        let result = normalize_candidate("gts.x.core.type.v1~").unwrap();
        assert_eq!(result.gts_id, "gts.x.core.type.v1~");
        assert!(!result.had_uri_prefix);
        assert_eq!(result.original, "gts.x.core.type.v1~");
    }

    #[test]
    fn test_normalize_with_whitespace() {
        let result = normalize_candidate("  gts.x.core.type.v1~  ").unwrap();
        assert_eq!(result.gts_id, "gts.x.core.type.v1~");
        assert!(!result.had_uri_prefix);
    }

    #[test]
    fn test_normalize_with_quotes() {
        let result = normalize_candidate("\"gts.x.core.type.v1~\"").unwrap();
        assert_eq!(result.gts_id, "gts.x.core.type.v1~");
        assert!(!result.had_uri_prefix);

        let result = normalize_candidate("'gts.x.core.type.v1~'").unwrap();
        assert_eq!(result.gts_id, "gts.x.core.type.v1~");
        assert!(!result.had_uri_prefix);
    }

    #[test]
    fn test_reject_fragment() {
        let result = normalize_candidate("gts://gts.x.core.type.v1~#foo");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("fragments (#)"));
    }

    #[test]
    fn test_reject_query_string() {
        let result = normalize_candidate("gts://gts.x.core.type.v1~?bar=1");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("query strings (?)"));
    }

    #[test]
    fn test_reject_no_gts_prefix() {
        let result = normalize_candidate("x.core.type.v1~");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Does not start with 'gts.'"));
    }

    #[test]
    fn test_normalize_chained_id() {
        let result =
            normalize_candidate("gts.x.core.events.type.v1~ven.app._.custom_event.v1~").unwrap();
        assert_eq!(
            result.gts_id,
            "gts.x.core.events.type.v1~ven.app._.custom_event.v1~"
        );
        assert!(!result.had_uri_prefix);
    }

    #[test]
    fn test_normalize_with_wildcard() {
        let result = normalize_candidate("gts.x.core.*").unwrap();
        assert_eq!(result.gts_id, "gts.x.core.*");
        assert!(!result.had_uri_prefix);
    }
}
