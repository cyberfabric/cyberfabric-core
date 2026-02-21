/// Normalize a prefix path and prepend it to a spec path.
///
/// Ensures the prefix has a leading `/` (unless empty) and no trailing `/`,
/// then concatenates it with the given spec path.
#[must_use]
pub fn prefixed_path(prefix_path: &str, spec_path: &str) -> String {
    let raw_prefix = prefix_path.trim_end_matches('/');
    let prefix = if raw_prefix.is_empty() || raw_prefix.starts_with('/') {
        raw_prefix.to_owned()
    } else {
        format!("/{raw_prefix}")
    };

    if prefix.is_empty() {
        spec_path.to_owned()
    } else {
        format!("{prefix}{spec_path}")
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;

    #[test]
    fn empty_prefix_returns_spec_path() {
        assert_eq!(prefixed_path("", "/v1/users"), "/v1/users");
    }

    #[test]
    fn prefix_with_leading_slash() {
        assert_eq!(prefixed_path("/cf", "/v1/users"), "/cf/v1/users");
    }

    #[test]
    fn prefix_without_leading_slash() {
        assert_eq!(prefixed_path("cf", "/v1/users"), "/cf/v1/users");
    }

    #[test]
    fn prefix_with_trailing_slash() {
        assert_eq!(prefixed_path("/cf/", "/v1/users"), "/cf/v1/users");
    }
}
