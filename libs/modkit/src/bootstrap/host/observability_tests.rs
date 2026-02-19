use super::*;

fn valid_config() -> TokioConsoleConfig {
    TokioConsoleConfig {
        server_addr: "127.0.0.1:6669".to_owned(),
    }
}

fn invalid_config() -> TokioConsoleConfig {
    TokioConsoleConfig {
        server_addr: "not-a-socket-addr".to_owned(),
    }
}

// ----- tests for the `tokio-console` feature enabled branch -----

#[cfg(feature = "tokio-console")]
mod with_feature {
    use super::*;

    #[test]
    fn none_config_returns_none() {
        assert!(build_console_layer(None).is_none());
    }

    #[test]
    fn valid_addr_returns_some() {
        let cfg = valid_config();
        assert!(build_console_layer(Some(&cfg)).is_some());
    }

    #[test]
    fn invalid_addr_returns_none() {
        let cfg = invalid_config();
        assert!(build_console_layer(Some(&cfg)).is_none());
    }
}

// ----- tests for the no-op fallback (feature disabled) -----

#[cfg(not(feature = "tokio-console"))]
mod without_feature {
    use super::*;

    #[test]
    fn none_config_returns_none() {
        assert!(build_console_layer(None).is_none());
    }

    #[test]
    fn valid_addr_returns_none_when_feature_disabled() {
        let cfg = valid_config();
        assert!(build_console_layer(Some(&cfg)).is_none());
    }

    #[test]
    fn invalid_addr_returns_none_when_feature_disabled() {
        let cfg = invalid_config();
        assert!(build_console_layer(Some(&cfg)).is_none());
    }
}
