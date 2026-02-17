//! Client implementation for the single-tenant resolver plugin.
//!
//! Implements `TenantResolverPluginClient` using single-tenant (flat) semantics.
//! In single-tenant mode:
//! - There is only one tenant (the one from the security context)
//! - It has no parent and no children
//! - Hierarchy operations return minimal results

use async_trait::async_trait;
use modkit_security::SecurityContext;
use tenant_resolver_sdk::{
    GetAncestorsOptions, GetAncestorsResponse, GetDescendantsOptions, GetDescendantsResponse,
    GetTenantsOptions, IsAncestorOptions, TenantId, TenantInfo, TenantRef, TenantResolverError,
    TenantResolverPluginClient, TenantStatus, matches_status,
};

use super::service::Service;

// Tenant name for single-tenant mode.
const TENANT_NAME: &str = "Default";

/// Build tenant info for the single-tenant mode.
fn build_tenant_info(id: TenantId) -> TenantInfo {
    TenantInfo {
        id,
        name: TENANT_NAME.to_owned(),
        status: TenantStatus::Active,
        tenant_type: None,
        parent_id: None,     // Root tenant (no parent)
        self_managed: false, // Not a barrier
    }
}

/// Build tenant ref for hierarchy operations in single-tenant mode.
fn build_tenant_ref(id: TenantId) -> TenantRef {
    TenantRef {
        id,
        status: TenantStatus::Active,
        tenant_type: None,
        parent_id: None,     // Root tenant (no parent)
        self_managed: false, // Not a barrier
    }
}

#[async_trait]
impl TenantResolverPluginClient for Service {
    async fn get_tenant(
        &self,
        ctx: &SecurityContext,
        id: TenantId,
    ) -> Result<TenantInfo, TenantResolverError> {
        // Reject nil UUID (anonymous context)
        if ctx.subject_tenant_id().is_nil() {
            return Err(TenantResolverError::TenantNotFound { tenant_id: id });
        }
        // Only return tenant info if ID matches security context
        if id == ctx.subject_tenant_id() {
            Ok(build_tenant_info(id))
        } else {
            Err(TenantResolverError::TenantNotFound { tenant_id: id })
        }
    }

    async fn get_tenants(
        &self,
        ctx: &SecurityContext,
        ids: &[TenantId],
        options: &GetTenantsOptions,
    ) -> Result<Vec<TenantInfo>, TenantResolverError> {
        // Nil UUID context means no tenant exists
        if ctx.subject_tenant_id().is_nil() {
            return Ok(vec![]);
        }

        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for id in ids {
            if !seen.insert(id) {
                continue; // Skip duplicate IDs
            }
            // Only the context tenant exists
            if *id == ctx.subject_tenant_id() {
                let tenant = build_tenant_info(*id);
                if matches_status(&tenant, &options.status) {
                    result.push(tenant);
                }
            }
            // Other IDs are silently skipped (they don't exist)
        }

        Ok(result)
    }

    async fn get_ancestors(
        &self,
        ctx: &SecurityContext,
        id: TenantId,
        _options: &GetAncestorsOptions,
    ) -> Result<GetAncestorsResponse, TenantResolverError> {
        // Reject nil UUID (anonymous context)
        if ctx.subject_tenant_id().is_nil() {
            return Err(TenantResolverError::TenantNotFound { tenant_id: id });
        }
        // Only the context tenant exists
        if id != ctx.subject_tenant_id() {
            return Err(TenantResolverError::TenantNotFound { tenant_id: id });
        }

        // In single-tenant mode, the tenant is the root (no ancestors)
        Ok(GetAncestorsResponse {
            tenant: build_tenant_ref(id),
            ancestors: vec![], // No ancestors in flat model
        })
    }

    async fn get_descendants(
        &self,
        ctx: &SecurityContext,
        id: TenantId,
        _options: &GetDescendantsOptions,
    ) -> Result<GetDescendantsResponse, TenantResolverError> {
        // Reject nil UUID (anonymous context)
        if ctx.subject_tenant_id().is_nil() {
            return Err(TenantResolverError::TenantNotFound { tenant_id: id });
        }
        // Only the context tenant exists
        if id != ctx.subject_tenant_id() {
            return Err(TenantResolverError::TenantNotFound { tenant_id: id });
        }

        // In single-tenant mode, there are no descendants
        Ok(GetDescendantsResponse {
            tenant: build_tenant_ref(id),
            descendants: vec![],
        })
    }

    async fn is_ancestor(
        &self,
        ctx: &SecurityContext,
        ancestor_id: TenantId,
        descendant_id: TenantId,
        _options: &IsAncestorOptions,
    ) -> Result<bool, TenantResolverError> {
        // Reject nil UUID (anonymous context)
        if ctx.subject_tenant_id().is_nil() {
            return Err(TenantResolverError::TenantNotFound {
                tenant_id: ancestor_id,
            });
        }

        let self_id = ctx.subject_tenant_id();

        // Both must be the context tenant (only one tenant exists)
        if ancestor_id != self_id {
            return Err(TenantResolverError::TenantNotFound {
                tenant_id: ancestor_id,
            });
        }
        if descendant_id != self_id {
            return Err(TenantResolverError::TenantNotFound {
                tenant_id: descendant_id,
            });
        }

        // Self is NOT an ancestor of self
        Ok(false)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use tenant_resolver_sdk::TenantStatus;
    use uuid::Uuid;

    fn ctx_for_tenant(tenant_id: Uuid) -> SecurityContext {
        SecurityContext::builder()
            .subject_id(Uuid::new_v4())
            .subject_tenant_id(tenant_id)
            .build()
            .unwrap()
    }

    const TENANT_A: &str = "11111111-1111-1111-1111-111111111111";
    const TENANT_B: &str = "22222222-2222-2222-2222-222222222222";

    // ==================== get_tenant tests ====================

    #[tokio::test]
    async fn get_tenant_returns_info_for_matching_id() {
        let service = Service;
        let tenant_id = Uuid::parse_str(TENANT_A).unwrap();
        let ctx = ctx_for_tenant(tenant_id);

        let result = service.get_tenant(&ctx, tenant_id).await;

        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.id, tenant_id);
        assert_eq!(info.name, TENANT_NAME);
        assert_eq!(info.status, TenantStatus::Active);
        assert!(info.tenant_type.is_none());
        assert!(info.parent_id.is_none());
        assert!(!info.self_managed);
    }

    #[tokio::test]
    async fn get_tenant_returns_error_for_different_id() {
        let service = Service;
        let ctx_tenant = Uuid::parse_str(TENANT_A).unwrap();
        let query_tenant = Uuid::parse_str(TENANT_B).unwrap();
        let ctx = ctx_for_tenant(ctx_tenant);

        let result = service.get_tenant(&ctx, query_tenant).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TenantResolverError::TenantNotFound { tenant_id } => {
                assert_eq!(tenant_id, query_tenant);
            }
            other => panic!("Expected TenantNotFound, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_tenant_rejects_nil_uuid() {
        let service = Service;
        let nil_id = Uuid::nil();
        let ctx = ctx_for_tenant(nil_id);

        // Even if id matches ctx.subject_tenant_id().unwrap_or_default(), nil UUID is rejected
        let result = service.get_tenant(&ctx, nil_id).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TenantResolverError::TenantNotFound { tenant_id } => {
                assert_eq!(tenant_id, nil_id);
            }
            other => panic!("Expected TenantNotFound, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_tenants_rejects_nil_uuid() {
        let service = Service;
        let nil_id = Uuid::nil();
        let ctx = ctx_for_tenant(nil_id);

        let result = service
            .get_tenants(&ctx, &[nil_id], &GetTenantsOptions::default())
            .await;

        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn get_ancestors_rejects_nil_uuid() {
        let service = Service;
        let nil_id = Uuid::nil();
        let ctx = ctx_for_tenant(nil_id);

        let result = service
            .get_ancestors(&ctx, nil_id, &GetAncestorsOptions::default())
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TenantResolverError::TenantNotFound { .. }
        ));
    }

    #[tokio::test]
    async fn get_descendants_rejects_nil_uuid() {
        let service = Service;
        let nil_id = Uuid::nil();
        let ctx = ctx_for_tenant(nil_id);

        let result = service
            .get_descendants(&ctx, nil_id, &GetDescendantsOptions::default())
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TenantResolverError::TenantNotFound { .. }
        ));
    }

    #[tokio::test]
    async fn is_ancestor_rejects_nil_uuid() {
        let service = Service;
        let nil_id = Uuid::nil();
        let ctx = ctx_for_tenant(nil_id);

        let result = service
            .is_ancestor(&ctx, nil_id, nil_id, &IsAncestorOptions::default())
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TenantResolverError::TenantNotFound { .. }
        ));
    }

    // ==================== get_tenants tests ====================

    #[tokio::test]
    async fn get_tenants_returns_self() {
        let service = Service;
        let tenant_id = Uuid::parse_str(TENANT_A).unwrap();
        let ctx = ctx_for_tenant(tenant_id);

        let result = service
            .get_tenants(&ctx, &[tenant_id], &GetTenantsOptions::default())
            .await;

        assert!(result.is_ok());
        let tenants = result.unwrap();
        assert_eq!(tenants.len(), 1);
        assert_eq!(tenants[0].id, tenant_id);
    }

    #[tokio::test]
    async fn get_tenants_skips_nonexistent() {
        let service = Service;
        let ctx_tenant = Uuid::parse_str(TENANT_A).unwrap();
        let other_tenant = Uuid::parse_str(TENANT_B).unwrap();
        let ctx = ctx_for_tenant(ctx_tenant);

        // Request both the context tenant and a nonexistent one
        let result = service
            .get_tenants(
                &ctx,
                &[ctx_tenant, other_tenant],
                &GetTenantsOptions::default(),
            )
            .await;

        assert!(result.is_ok());
        let tenants = result.unwrap();
        // Only the context tenant is returned
        assert_eq!(tenants.len(), 1);
        assert_eq!(tenants[0].id, ctx_tenant);
    }

    #[tokio::test]
    async fn get_tenants_with_filter() {
        let service = Service;
        let tenant_id = Uuid::parse_str(TENANT_A).unwrap();
        let ctx = ctx_for_tenant(tenant_id);

        // Filter for suspended status (our tenant is Active)
        let opts = GetTenantsOptions {
            status: vec![TenantStatus::Suspended],
        };
        let result = service.get_tenants(&ctx, &[tenant_id], &opts).await;

        assert!(result.is_ok());
        // Filtered out because status doesn't match
        assert!(result.unwrap().is_empty());
    }

    // ==================== get_ancestors tests ====================

    #[tokio::test]
    async fn get_ancestors_returns_empty_for_self() {
        let service = Service;
        let tenant_id = Uuid::parse_str(TENANT_A).unwrap();
        let ctx = ctx_for_tenant(tenant_id);

        let result = service
            .get_ancestors(&ctx, tenant_id, &GetAncestorsOptions::default())
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.tenant.id, tenant_id);
        assert!(response.ancestors.is_empty());
    }

    #[tokio::test]
    async fn get_ancestors_error_for_different_id() {
        let service = Service;
        let ctx_tenant = Uuid::parse_str(TENANT_A).unwrap();
        let other_tenant = Uuid::parse_str(TENANT_B).unwrap();
        let ctx = ctx_for_tenant(ctx_tenant);

        let result = service
            .get_ancestors(&ctx, other_tenant, &GetAncestorsOptions::default())
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TenantResolverError::TenantNotFound { tenant_id } => {
                assert_eq!(tenant_id, other_tenant);
            }
            other => panic!("Expected TenantNotFound, got: {other:?}"),
        }
    }

    // ==================== get_descendants tests ====================

    #[tokio::test]
    async fn get_descendants_returns_empty_for_self() {
        let service = Service;
        let tenant_id = Uuid::parse_str(TENANT_A).unwrap();
        let ctx = ctx_for_tenant(tenant_id);

        let result = service
            .get_descendants(&ctx, tenant_id, &GetDescendantsOptions::default())
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.tenant.id, tenant_id);
        assert!(response.descendants.is_empty());
    }

    #[tokio::test]
    async fn get_descendants_error_for_different_id() {
        let service = Service;
        let ctx_tenant = Uuid::parse_str(TENANT_A).unwrap();
        let other_tenant = Uuid::parse_str(TENANT_B).unwrap();
        let ctx = ctx_for_tenant(ctx_tenant);

        let result = service
            .get_descendants(&ctx, other_tenant, &GetDescendantsOptions::default())
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TenantResolverError::TenantNotFound { tenant_id } => {
                assert_eq!(tenant_id, other_tenant);
            }
            other => panic!("Expected TenantNotFound, got: {other:?}"),
        }
    }

    // ==================== is_ancestor tests ====================

    #[tokio::test]
    async fn is_ancestor_self_returns_false() {
        let service = Service;
        let tenant_id = Uuid::parse_str(TENANT_A).unwrap();
        let ctx = ctx_for_tenant(tenant_id);

        let result = service
            .is_ancestor(&ctx, tenant_id, tenant_id, &IsAncestorOptions::default())
            .await;

        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn is_ancestor_error_for_different_ancestor() {
        let service = Service;
        let ctx_tenant = Uuid::parse_str(TENANT_A).unwrap();
        let other_tenant = Uuid::parse_str(TENANT_B).unwrap();
        let ctx = ctx_for_tenant(ctx_tenant);

        let result = service
            .is_ancestor(
                &ctx,
                other_tenant,
                ctx_tenant,
                &IsAncestorOptions::default(),
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TenantResolverError::TenantNotFound { tenant_id } => {
                assert_eq!(tenant_id, other_tenant);
            }
            other => panic!("Expected TenantNotFound, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn is_ancestor_error_for_different_descendant() {
        let service = Service;
        let ctx_tenant = Uuid::parse_str(TENANT_A).unwrap();
        let other_tenant = Uuid::parse_str(TENANT_B).unwrap();
        let ctx = ctx_for_tenant(ctx_tenant);

        let result = service
            .is_ancestor(
                &ctx,
                ctx_tenant,
                other_tenant,
                &IsAncestorOptions::default(),
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TenantResolverError::TenantNotFound { tenant_id } => {
                assert_eq!(tenant_id, other_tenant);
            }
            other => panic!("Expected TenantNotFound, got: {other:?}"),
        }
    }
}
