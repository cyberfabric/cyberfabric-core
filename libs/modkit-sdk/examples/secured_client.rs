//! Example demonstrating the security context scoping wrapper.
//!
//! This example shows how to use `Secured<'a, C>` to bind a `SecurityContext`
//! Example demonstrating the `Secured` wrapper for security context binding

#![allow(clippy::expect_used)]

use modkit_sdk::secured::WithSecurityContext;
use modkit_security::SecurityContext;
use uuid::Uuid;

struct UserClient {
    base_url: String,
}

impl UserClient {
    fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.to_owned(),
        }
    }

    fn get_base_url(&self) -> &str {
        &self.base_url
    }
}

fn main() {
    let client = UserClient::new("https://api.example.com");

    let tenant_id =
        Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").expect("valid tenant UUID");
    let subject_id =
        Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").expect("valid subject UUID");

    let ctx = SecurityContext::builder()
        .subject_tenant_id(tenant_id)
        .subject_id(subject_id)
        .subject_type("user")
        .build()
        .expect("valid security context");

    let secured = client.security_ctx(&ctx);

    println!("Client base URL: {}", secured.client().get_base_url());
    println!("Tenant ID: {}", secured.ctx().subject_tenant_id());
    println!("Subject ID: {}", secured.ctx().subject_id());

    // Context for internal operations (still scoped to a tenant)
    let internal_ctx = SecurityContext::builder()
        .subject_tenant_id(tenant_id)
        .subject_id(subject_id)
        .build()
        .expect("valid security context");
    let secured_internal = client.security_ctx(&internal_ctx);

    println!("\nInternal context:");
    println!("Tenant ID: {}", secured_internal.ctx().subject_tenant_id());
    println!("Subject ID: {}", secured_internal.ctx().subject_id());
}
