//! Security context scoping for clients
//!
//! This module provides a lightweight, zero-allocation wrapper that binds a `SecurityContext`
//! to any client type, enabling security-aware API calls without cloning or Arc overhead.
//!
//! # Example
//!
//! ```rust,ignore
//! use modkit_sdk::secured::{Secured, WithSecurityContext};
//! use modkit_security::SecurityContext;
//!
//! let client = MyClient::new();
//! let ctx = SecurityContext::builder()
//!     .subject_id(TEST_SUBJECT_ID)
//!     .subject_tenant_id(TEST_TENANT_ID)
//!     .build()?;
//!
//! // Bind the security context to the client
//! let secured = client.security_ctx(&ctx);
//!
//! // Access the client and context
//! let client_ref = secured.client();
//! let ctx_ref = secured.ctx();
//! ```

use modkit_security::SecurityContext;

/// A wrapper that binds a `SecurityContext` to a client reference.
///
/// This struct provides a zero-cost abstraction for carrying both a client
/// and its associated security context together, without any allocation or cloning.
///
/// # Type Parameters
///
/// * `'a` - The lifetime of both the client and security context references
/// * `C` - The client type being wrapped
#[derive(Debug)]
pub struct Secured<'a, C> {
    client: &'a C,
    ctx: &'a SecurityContext,
}

impl<'a, C> Secured<'a, C> {
    /// Creates a new `Secured` wrapper binding a client and security context.
    ///
    /// # Arguments
    ///
    /// * `client` - Reference to the client
    /// * `ctx` - Reference to the security context
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let secured = Secured::new(&client, &ctx);
    /// ```
    #[must_use]
    pub fn new(client: &'a C, ctx: &'a SecurityContext) -> Self {
        Self { client, ctx }
    }

    /// Returns a reference to the wrapped client.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let client_ref = secured.client();
    /// ```
    #[must_use]
    pub fn client(&self) -> &'a C {
        self.client
    }

    /// Returns a reference to the security context.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let ctx_ref = secured.ctx();
    /// let tenant_id = ctx_ref.subject_tenant_id();
    /// ```
    #[must_use]
    pub fn ctx(&self) -> &'a SecurityContext {
        self.ctx
    }

    /// Create a new query builder for the given schema.
    ///
    /// This provides an ergonomic entrypoint for building queries from a secured client.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// use modkit_sdk::odata::items_stream;
    ///
    /// let items = items_stream(
    ///     client.security_ctx(&ctx)
    ///         .query::<UserSchema>()
    ///         .filter(user::email().contains("@example.com")),
    ///     |query| async move { client.list_users(query).await },
    /// );
    /// ```
    #[must_use]
    pub fn query<S: crate::odata::Schema>(&self) -> crate::odata::QueryBuilder<S> {
        crate::odata::QueryBuilder::new()
    }
}

impl<C> Clone for Secured<'_, C> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<C> Copy for Secured<'_, C> {}

/// Extension trait that adds the `security_ctx` method to any type.
///
/// This trait enables any client to be wrapped with a security context
/// using a fluent API: `client.security_ctx(&ctx)`.
///
/// # Example
///
/// ```rust,ignore
/// use modkit_sdk::secured::WithSecurityContext;
///
/// let secured = my_client.security_ctx(&security_context);
/// ```
pub trait WithSecurityContext {
    /// Binds a security context to this client, returning a `Secured` wrapper.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Reference to the security context to bind
    ///
    /// # Returns
    ///
    /// A `Secured` wrapper containing references to both the client and context.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let secured = client.security_ctx(&ctx);
    /// assert_eq!(secured.ctx().subject_tenant_id(), ctx.subject_tenant_id());
    /// ```
    fn security_ctx<'a>(&'a self, ctx: &'a SecurityContext) -> Secured<'a, Self>
    where
        Self: Sized;
}

impl<T> WithSecurityContext for T {
    fn security_ctx<'a>(&'a self, ctx: &'a SecurityContext) -> Secured<'a, Self>
    where
        Self: Sized,
    {
        Secured::new(self, ctx)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use uuid::{Uuid, uuid};

    /// Test tenant ID for unit tests.
    const TEST_TENANT_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000001");
    /// Test subject ID for unit tests.
    const TEST_SUBJECT_ID: Uuid = uuid!("00000000-0000-0000-0000-000000000002");

    fn test_ctx() -> SecurityContext {
        SecurityContext::builder()
            .subject_id(TEST_SUBJECT_ID)
            .subject_tenant_id(TEST_TENANT_ID)
            .build()
            .unwrap()
    }

    struct MockClient {
        name: String,
    }

    impl MockClient {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_owned(),
            }
        }

        fn get_name(&self) -> &str {
            &self.name
        }
    }

    #[test]
    fn test_secured_new() {
        let client = MockClient::new("test-client");
        let ctx = test_ctx();

        let secured = Secured::new(&client, &ctx);

        assert_eq!(secured.client().get_name(), "test-client");
        assert_eq!(secured.ctx().subject_tenant_id(), ctx.subject_tenant_id());
    }

    #[test]
    fn test_secured_getters() {
        let client = MockClient::new("test-client");
        let ctx = test_ctx();

        let secured = Secured::new(&client, &ctx);

        let client_ref = secured.client();
        assert_eq!(client_ref.get_name(), "test-client");

        let ctx_ref = secured.ctx();
        assert_eq!(ctx_ref.subject_tenant_id(), TEST_TENANT_ID);
    }

    #[test]
    fn test_with_security_context_trait() {
        let client = MockClient::new("test-client");
        let ctx = test_ctx();

        let secured = client.security_ctx(&ctx);

        assert_eq!(secured.client().get_name(), "test-client");
        assert_eq!(secured.ctx().subject_tenant_id(), ctx.subject_tenant_id());
    }

    #[test]
    fn test_secured_clone() {
        let client = MockClient::new("test-client");
        let ctx = test_ctx();

        let secured1 = client.security_ctx(&ctx);
        let secured2 = secured1;

        assert_eq!(secured1.client().get_name(), secured2.client().get_name());
        assert_eq!(
            secured1.ctx().subject_tenant_id(),
            secured2.ctx().subject_tenant_id()
        );
    }

    #[test]
    fn test_secured_copy() {
        let client = MockClient::new("test-client");
        let ctx = test_ctx();

        let secured1 = client.security_ctx(&ctx);
        let secured2 = secured1;

        assert_eq!(secured1.client().get_name(), secured2.client().get_name());
        assert_eq!(
            secured1.ctx().subject_tenant_id(),
            secured2.ctx().subject_tenant_id()
        );
    }

    #[test]
    fn test_secured_with_anonymous_context() {
        let client = MockClient::new("test-client");
        let ctx = SecurityContext::anonymous();

        let secured = client.security_ctx(&ctx);

        assert_eq!(secured.client().get_name(), "test-client");
        assert_eq!(secured.ctx().subject_tenant_id(), Uuid::default());
        assert_eq!(secured.ctx().subject_id(), Uuid::default());
    }

    #[test]
    fn test_secured_with_custom_context() {
        let client = MockClient::new("test-client");
        let tenant_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap();
        let subject_id = Uuid::parse_str("550e8400-e29b-41d4-a716-446655440001").unwrap();

        let ctx = SecurityContext::builder()
            .subject_tenant_id(tenant_id)
            .subject_id(subject_id)
            .subject_type("user")
            .build()
            .unwrap();

        let secured = client.security_ctx(&ctx);

        assert_eq!(secured.ctx().subject_tenant_id(), tenant_id);
        assert_eq!(secured.ctx().subject_id(), subject_id);
    }

    #[test]
    fn test_secured_zero_allocation() {
        let client = MockClient::new("test-client");
        let ctx = test_ctx();

        let secured = client.security_ctx(&ctx);

        assert_eq!(
            std::mem::size_of_val(&secured),
            std::mem::size_of::<&MockClient>() + std::mem::size_of::<&SecurityContext>()
        );
    }

    #[test]
    fn test_multiple_clients_with_same_context() {
        let client1 = MockClient::new("client-1");
        let client2 = MockClient::new("client-2");
        let ctx = test_ctx();

        let secured1 = client1.security_ctx(&ctx);
        let secured2 = client2.security_ctx(&ctx);

        assert_eq!(secured1.client().get_name(), "client-1");
        assert_eq!(secured2.client().get_name(), "client-2");
        assert_eq!(
            secured1.ctx().subject_tenant_id(),
            secured2.ctx().subject_tenant_id()
        );
    }

    #[test]
    fn test_secured_preserves_lifetimes() {
        let client = MockClient::new("test-client");
        let ctx = test_ctx();

        let secured = client.security_ctx(&ctx);

        assert_eq!(secured.client().get_name(), "test-client");
        assert_eq!(secured.ctx().subject_tenant_id(), ctx.subject_tenant_id());
    }

    #[test]
    fn test_secured_query_builder() {
        use crate::odata::Schema;

        #[derive(Copy, Clone, Eq, PartialEq, Debug)]
        enum TestField {
            #[allow(dead_code)]
            Name,
        }

        struct TestSchema;

        impl Schema for TestSchema {
            type Field = TestField;

            fn field_name(field: Self::Field) -> &'static str {
                match field {
                    TestField::Name => "name",
                }
            }
        }

        let client = MockClient::new("test-client");
        let ctx = test_ctx();

        let secured = client.security_ctx(&ctx);
        let query_builder = secured.query::<TestSchema>();

        let query = query_builder.page_size(50).build();
        assert_eq!(query.limit, Some(50));
    }
}
