use http::{HeaderMap, HeaderName, HeaderValue};
use crate::domain::dto::{PassthroughMode, RequestHeaderRules};

const HOP_BY_HOP_HEADERS: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// Apply passthrough filter: decide which inbound headers to forward.
/// Content-Type is always forwarded when present (needed for POST/PUT bodies).
pub fn apply_passthrough(
    inbound: &HeaderMap,
    mode: &PassthroughMode,
    allowlist: &[String],
) -> HeaderMap {
    let mut out = match mode {
        PassthroughMode::None => HeaderMap::new(),
        PassthroughMode::All => inbound.clone(),
        PassthroughMode::Allowlist => {
            let mut h = HeaderMap::new();
            for name in allowlist {
                if let Ok(n) = HeaderName::from_bytes(name.to_lowercase().as_bytes())
                    && let Some(v) = inbound.get(&n)
                {
                    h.insert(n, v.clone());
                }
            }
            h
        }
    };

    // Always forward Content-Type if present.
    if !out.contains_key(http::header::CONTENT_TYPE)
        && let Some(ct) = inbound.get(http::header::CONTENT_TYPE)
    {
        out.insert(http::header::CONTENT_TYPE, ct.clone());
    }

    out
}

/// Remove hop-by-hop headers that must not be forwarded.
pub fn strip_hop_by_hop(headers: &mut HeaderMap) {
    for name in HOP_BY_HOP_HEADERS {
        headers.remove(*name);
    }
}

/// Remove X-OAGW-* internal headers.
pub fn strip_internal_headers(headers: &mut HeaderMap) {
    let to_remove: Vec<HeaderName> = headers
        .keys()
        .filter(|k| k.as_str().starts_with("x-oagw-"))
        .cloned()
        .collect();
    for name in to_remove {
        headers.remove(&name);
    }
}

/// Apply set/add/remove header rules from upstream config.
pub fn apply_header_rules(headers: &mut HeaderMap, rules: &RequestHeaderRules) {
    // Remove first.
    for name in &rules.remove {
        if let Ok(n) = HeaderName::from_bytes(name.to_lowercase().as_bytes()) {
            headers.remove(n);
        }
    }
    // Set (overwrite).
    for (name, value) in &rules.set {
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.to_lowercase().as_bytes()),
            HeaderValue::from_str(value),
        ) {
            headers.insert(n, v);
        }
    }
    // Add (append).
    for (name, value) in &rules.add {
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name.to_lowercase().as_bytes()),
            HeaderValue::from_str(value),
        ) {
            headers.append(n, v);
        }
    }
}

/// Set the Host header to match the upstream endpoint.
pub fn set_host_header(headers: &mut HeaderMap, host: &str, port: u16) {
    let host_value = if port == 443 || port == 80 {
        host.to_string()
    } else {
        format!("{host}:{port}")
    };
    if let Ok(v) = HeaderValue::from_str(&host_value) {
        headers.insert(http::header::HOST, v);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    #[test]
    fn hop_by_hop_stripped() {
        let mut headers = HeaderMap::new();
        headers.insert("connection", "keep-alive".parse().unwrap());
        headers.insert("transfer-encoding", "chunked".parse().unwrap());
        headers.insert("x-custom", "keep-me".parse().unwrap());

        strip_hop_by_hop(&mut headers);

        assert!(headers.get("connection").is_none());
        assert!(headers.get("transfer-encoding").is_none());
        assert_eq!(headers.get("x-custom").unwrap(), "keep-me");
    }

    #[test]
    fn host_replaced() {
        let mut headers = HeaderMap::new();
        headers.insert(http::header::HOST, "original.com".parse().unwrap());

        set_host_header(&mut headers, "api.openai.com", 443);

        assert_eq!(headers.get(http::header::HOST).unwrap(), "api.openai.com");
    }

    #[test]
    fn host_nonstandard_port() {
        let mut headers = HeaderMap::new();
        set_host_header(&mut headers, "api.openai.com", 8443);

        assert_eq!(
            headers.get(http::header::HOST).unwrap(),
            "api.openai.com:8443"
        );
    }

    #[test]
    fn internal_headers_stripped() {
        let mut headers = HeaderMap::new();
        headers.insert("x-oagw-target-host", "evil.com".parse().unwrap());
        headers.insert("x-oagw-trace-id", "abc".parse().unwrap());
        headers.insert("x-custom", "keep".parse().unwrap());

        strip_internal_headers(&mut headers);

        assert!(headers.get("x-oagw-target-host").is_none());
        assert!(headers.get("x-oagw-trace-id").is_none());
        assert_eq!(headers.get("x-custom").unwrap(), "keep");
    }

    #[test]
    fn set_overwrites_existing() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-version", "v1".parse().unwrap());

        let rules = RequestHeaderRules {
            set: {
                let mut m = HashMap::new();
                m.insert("x-api-version".into(), "v2".into());
                m
            },
            add: HashMap::new(),
            remove: vec![],
            passthrough: PassthroughMode::None,
            passthrough_allowlist: vec![],
        };

        apply_header_rules(&mut headers, &rules);
        assert_eq!(headers.get("x-api-version").unwrap(), "v2");
    }

    #[test]
    fn add_appends() {
        let mut headers = HeaderMap::new();
        headers.insert("x-tag", "a".parse().unwrap());

        let rules = RequestHeaderRules {
            set: HashMap::new(),
            add: {
                let mut m = HashMap::new();
                m.insert("x-tag".into(), "b".into());
                m
            },
            remove: vec![],
            passthrough: PassthroughMode::None,
            passthrough_allowlist: vec![],
        };

        apply_header_rules(&mut headers, &rules);
        let values: Vec<&str> = headers
            .get_all("x-tag")
            .iter()
            .map(|v| v.to_str().unwrap())
            .collect();
        assert!(values.contains(&"a"));
        assert!(values.contains(&"b"));
    }

    #[test]
    fn remove_deletes() {
        let mut headers = HeaderMap::new();
        headers.insert("x-remove-me", "gone".parse().unwrap());
        headers.insert("x-keep-me", "stay".parse().unwrap());

        let rules = RequestHeaderRules {
            set: HashMap::new(),
            add: HashMap::new(),
            remove: vec!["x-remove-me".into()],
            passthrough: PassthroughMode::None,
            passthrough_allowlist: vec![],
        };

        apply_header_rules(&mut headers, &rules);
        assert!(headers.get("x-remove-me").is_none());
        assert_eq!(headers.get("x-keep-me").unwrap(), "stay");
    }

    #[test]
    fn passthrough_none_starts_empty_but_keeps_content_type() {
        let mut inbound = HeaderMap::new();
        inbound.insert("x-custom", "val".parse().unwrap());
        inbound.insert(
            http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );

        let out = apply_passthrough(&inbound, &PassthroughMode::None, &[]);

        assert!(out.get("x-custom").is_none());
        assert_eq!(
            out.get(http::header::CONTENT_TYPE).unwrap(),
            "application/json"
        );
    }

    #[test]
    fn passthrough_all_copies_everything() {
        let mut inbound = HeaderMap::new();
        inbound.insert("x-custom", "val".parse().unwrap());
        inbound.insert("x-other", "val2".parse().unwrap());

        let out = apply_passthrough(&inbound, &PassthroughMode::All, &[]);
        assert_eq!(out.len(), 2);
    }

    #[test]
    fn passthrough_allowlist_filters() {
        let mut inbound = HeaderMap::new();
        inbound.insert("x-allowed", "yes".parse().unwrap());
        inbound.insert("x-blocked", "no".parse().unwrap());

        let out = apply_passthrough(&inbound, &PassthroughMode::Allowlist, &["x-allowed".into()]);

        assert_eq!(out.get("x-allowed").unwrap(), "yes");
        assert!(out.get("x-blocked").is_none());
    }
}
