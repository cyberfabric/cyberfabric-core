//! OAGW-specific GTS identifier helpers.
//!
//! Thin wrappers around the external `gts` crate for formatting and parsing
//! resource GTS identifiers of the form `gts.x.core.oagw.<type>.v1~<uuid>`.

use crate::domain::error::DomainError;
use uuid::Uuid;

pub(crate) const UPSTREAM_SCHEMA: &str = "gts.x.core.oagw.upstream.v1~";
pub(crate) const ROUTE_SCHEMA: &str = "gts.x.core.oagw.route.v1~";

/// Format an upstream resource as a GTS identifier.
#[must_use]
pub fn format_upstream_gts(id: Uuid) -> String {
    format!("{UPSTREAM_SCHEMA}{}", id.simple())
}

/// Format a route resource as a GTS identifier.
#[must_use]
pub fn format_route_gts(id: Uuid) -> String {
    format!("{ROUTE_SCHEMA}{}", id.simple())
}

/// Parse a resource GTS identifier, extracting the schema and UUID instance.
///
/// Validates the schema portion using the `gts` crate and parses the instance
/// portion as a UUID. OAGW resource identifiers use bare UUIDs as instances
/// (e.g. `gts.x.core.oagw.upstream.v1~<hex-uuid>`), which the `gts` crate does
/// not accept as a full identifier, so we validate schema and instance separately.
pub fn parse_resource_gts(s: &str) -> Result<(String, Uuid), DomainError> {
    // Split at '~' to separate schema from instance.
    let tilde_pos = s.rfind('~').ok_or_else(|| DomainError::Validation {
        detail: "missing '~' separator in GTS identifier".into(),
        instance: s.to_string(),
    })?;

    let schema_with_tilde = &s[..=tilde_pos]; // e.g. "gts.x.core.oagw.upstream.v1~"
    let instance = &s[tilde_pos + 1..];

    // Validate schema portion as a GTS type (ends with ~).
    gts::GtsID::new(schema_with_tilde).map_err(|e| DomainError::Validation {
        detail: format!("invalid GTS schema: {e}"),
        instance: s.to_string(),
    })?;

    // Parse the instance portion as a UUID.
    let uuid = Uuid::parse_str(instance).map_err(|e| DomainError::Validation {
        detail: format!("invalid UUID in GTS instance: {e}"),
        instance: s.to_string(),
    })?;

    Ok((s[..tilde_pos].to_string(), uuid))
}
