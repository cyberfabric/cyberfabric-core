use crate::domain::dto::{Endpoint, Scheme};

/// Build the full upstream URL from endpoint, route path, path suffix, and query params.
pub fn build_upstream_url(
    endpoint: &Endpoint,
    route_path: &str,
    path_suffix: &str,
    query_params: &[(String, String)],
) -> String {
    let scheme = match endpoint.scheme {
        Scheme::Http => "http",
        Scheme::Https | Scheme::Grpc => "https",
        Scheme::Wss => "wss",
        Scheme::Wt => "https",
    };

    let host_port = if is_default_port(scheme, endpoint.port) {
        endpoint.host.clone()
    } else {
        format!("{}:{}", endpoint.host, endpoint.port)
    };

    // Combine route path + path suffix, avoiding double slashes.
    let path = if path_suffix.is_empty() {
        route_path.to_string()
    } else if route_path.ends_with('/') && path_suffix.starts_with('/') {
        format!("{}{}", route_path, &path_suffix[1..])
    } else if !route_path.ends_with('/') && !path_suffix.starts_with('/') {
        format!("{route_path}/{path_suffix}")
    } else {
        format!("{route_path}{path_suffix}")
    };

    let mut url = format!("{scheme}://{host_port}{path}");

    if !query_params.is_empty() {
        url.push('?');
        let qs: Vec<String> = query_params
            .iter()
            .map(|(k, v)| {
                if v.is_empty() {
                    k.clone()
                } else {
                    format!("{k}={v}")
                }
            })
            .collect();
        url.push_str(&qs.join("&"));
    }

    url
}

fn is_default_port(scheme: &str, port: u16) -> bool {
    matches!((scheme, port), ("https" | "wss", 443) | ("http" | "ws", 80))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn endpoint(host: &str, port: u16) -> Endpoint {
        Endpoint {
            scheme: Scheme::Https,
            host: host.into(),
            port,
        }
    }

    #[test]
    fn standard_url() {
        let url = build_upstream_url(
            &endpoint("api.openai.com", 443),
            "/v1/chat",
            "/completions",
            &[],
        );
        assert_eq!(url, "https://api.openai.com/v1/chat/completions");
    }

    #[test]
    fn with_query_params() {
        let url = build_upstream_url(
            &endpoint("api.openai.com", 443),
            "/v1/chat",
            "/models/gpt-4",
            &[("version".into(), "2".into())],
        );
        assert_eq!(url, "https://api.openai.com/v1/chat/models/gpt-4?version=2");
    }

    #[test]
    fn nonstandard_port() {
        let url = build_upstream_url(&endpoint("localhost", 8080), "/api", "", &[]);
        assert_eq!(url, "https://localhost:8080/api");
    }

    #[test]
    fn empty_suffix() {
        let url = build_upstream_url(&endpoint("api.openai.com", 443), "/v1/models", "", &[]);
        assert_eq!(url, "https://api.openai.com/v1/models");
    }

    #[test]
    fn avoids_double_slash() {
        let url = build_upstream_url(&endpoint("api.openai.com", 443), "/v1/", "/chat", &[]);
        assert_eq!(url, "https://api.openai.com/v1/chat");
    }

    #[test]
    fn multiple_query_params() {
        let url = build_upstream_url(
            &endpoint("example.com", 443),
            "/api",
            "/data",
            &[("key".into(), "val".into()), ("foo".into(), "bar".into())],
        );
        assert_eq!(url, "https://example.com/api/data?key=val&foo=bar");
    }

    #[test]
    fn http_scheme() {
        let ep = Endpoint {
            scheme: Scheme::Http,
            host: "127.0.0.1".into(),
            port: 3000,
        };
        let url = build_upstream_url(&ep, "/v1/test", "", &[]);
        assert_eq!(url, "http://127.0.0.1:3000/v1/test");
    }

    #[test]
    fn http_default_port() {
        let ep = Endpoint {
            scheme: Scheme::Http,
            host: "example.com".into(),
            port: 80,
        };
        let url = build_upstream_url(&ep, "/api", "", &[]);
        assert_eq!(url, "http://example.com/api");
    }
}
