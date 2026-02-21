use authz_resolver_sdk::pep::ResourceType;

pub(crate) mod headers;
pub(crate) mod request_builder;
pub(crate) mod service;

pub(crate) use service::DataPlaneServiceImpl;

pub(crate) mod resources {
    use super::ResourceType;
    use modkit_security::pep_properties;

    /// Resource type identifying a proxied upstream target.
    pub const PROXY: ResourceType = ResourceType {
        name: "gts.x.core.oagw.proxy.v1~",
        supported_properties: &[pep_properties::OWNER_TENANT_ID],
    };
}

pub(crate) mod actions {
    /// Action name for invoking (proxying a request to) an upstream.
    pub const INVOKE: &str = "invoke";
}
