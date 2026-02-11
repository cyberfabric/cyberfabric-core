use crate::domain::plugin::{AuthContext, AuthPlugin, PluginError};

/// GTS identifier for the noop auth plugin.
pub const NOOP_AUTH_PLUGIN_ID: &str = "gts.x.core.oagw.auth_plugin.v1~x.core.oagw.noop.v1";

/// Auth plugin that does nothing — used for upstreams requiring no authentication.
pub struct NoopAuthPlugin;

#[async_trait::async_trait]
impl AuthPlugin for NoopAuthPlugin {
    async fn authenticate(&self, _ctx: &mut AuthContext) -> Result<(), PluginError> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;

    use super::*;

    #[tokio::test]
    async fn noop_leaves_headers_unchanged() {
        let plugin = NoopAuthPlugin;
        let mut headers = HeaderMap::new();
        headers.insert("x-existing", "value".parse().unwrap());

        let mut ctx = AuthContext {
            headers: headers.clone(),
            config: serde_json::Value::Null,
        };

        plugin.authenticate(&mut ctx).await.unwrap();

        // Headers should be identical.
        assert_eq!(ctx.headers.len(), 1);
        assert_eq!(
            ctx.headers.get("x-existing").unwrap().to_str().unwrap(),
            "value"
        );
    }
}
