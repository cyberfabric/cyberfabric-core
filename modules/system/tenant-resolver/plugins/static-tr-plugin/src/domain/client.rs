//! Client implementation for the static tenant resolver plugin.
//!
//! Implements `TenantResolverPluginClient` using the domain service.

use async_trait::async_trait;
use modkit_security::SecurityContext;
use tenant_resolver_sdk::{
    GetAncestorsOptions, GetAncestorsResponse, GetDescendantsOptions, GetDescendantsResponse,
    GetTenantsOptions, IsAncestorOptions, TenantId, TenantInfo, TenantResolverError,
    TenantResolverPluginClient, matches_status,
};

use super::service::Service;

#[async_trait]
impl TenantResolverPluginClient for Service {
    async fn get_tenant(
        &self,
        _ctx: &SecurityContext,
        id: TenantId,
    ) -> Result<TenantInfo, TenantResolverError> {
        self.tenants
            .get(&id)
            .cloned()
            .ok_or(TenantResolverError::TenantNotFound { tenant_id: id })
    }

    async fn get_tenants(
        &self,
        _ctx: &SecurityContext,
        ids: &[TenantId],
        options: &GetTenantsOptions,
    ) -> Result<Vec<TenantInfo>, TenantResolverError> {
        let mut result = Vec::new();
        let mut seen = std::collections::HashSet::new();

        for id in ids {
            if !seen.insert(id) {
                continue; // Skip duplicate IDs
            }
            if let Some(tenant) = self.tenants.get(id)
                && matches_status(tenant, &options.status)
            {
                result.push(tenant.clone());
            }
            // Missing IDs are silently skipped
        }

        Ok(result)
    }

    async fn get_ancestors(
        &self,
        _ctx: &SecurityContext,
        id: TenantId,
        options: &GetAncestorsOptions,
    ) -> Result<GetAncestorsResponse, TenantResolverError> {
        // Get the tenant first
        let tenant = self
            .tenants
            .get(&id)
            .ok_or(TenantResolverError::TenantNotFound { tenant_id: id })?;

        // Collect ancestors
        let ancestors = self.collect_ancestors(id, options.barrier_mode);

        Ok(GetAncestorsResponse {
            tenant: tenant.into(),
            ancestors,
        })
    }

    async fn get_descendants(
        &self,
        _ctx: &SecurityContext,
        id: TenantId,
        options: &GetDescendantsOptions,
    ) -> Result<GetDescendantsResponse, TenantResolverError> {
        // Get the tenant first (filter does NOT apply to the starting tenant)
        let tenant = self
            .tenants
            .get(&id)
            .ok_or(TenantResolverError::TenantNotFound { tenant_id: id })?;

        // Collect descendants with filter applied during traversal:
        // - Results are in pre-order (parent before children)
        // - Nodes that don't pass filter are excluded along with their subtrees
        let descendants =
            self.collect_descendants(id, &options.status, options.barrier_mode, options.max_depth);

        Ok(GetDescendantsResponse {
            tenant: tenant.into(),
            descendants,
        })
    }

    async fn is_ancestor(
        &self,
        _ctx: &SecurityContext,
        ancestor_id: TenantId,
        descendant_id: TenantId,
        options: &IsAncestorOptions,
    ) -> Result<bool, TenantResolverError> {
        self.is_ancestor_of(ancestor_id, descendant_id, options.barrier_mode)
    }
}

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use super::*;
    use crate::config::{StaticTrPluginConfig, TenantConfig};
    use tenant_resolver_sdk::{BarrierMode, TenantStatus};
    use uuid::Uuid;

    // Helper to create a test tenant config
    fn tenant(id: &str, name: &str, status: TenantStatus) -> TenantConfig {
        TenantConfig {
            id: Uuid::parse_str(id).unwrap(),
            name: name.to_owned(),
            status,
            tenant_type: None,
            parent_id: None,
            self_managed: false,
        }
    }

    fn tenant_with_parent(id: &str, name: &str, parent: &str) -> TenantConfig {
        TenantConfig {
            id: Uuid::parse_str(id).unwrap(),
            name: name.to_owned(),
            status: TenantStatus::Active,
            tenant_type: None,
            parent_id: Some(Uuid::parse_str(parent).unwrap()),
            self_managed: false,
        }
    }

    fn tenant_barrier(id: &str, name: &str, parent: &str) -> TenantConfig {
        TenantConfig {
            id: Uuid::parse_str(id).unwrap(),
            name: name.to_owned(),
            status: TenantStatus::Active,
            tenant_type: None,
            parent_id: Some(Uuid::parse_str(parent).unwrap()),
            self_managed: true,
        }
    }

    // Helper to create a security context for a tenant
    fn ctx_for_tenant(tenant_id: &str) -> SecurityContext {
        SecurityContext::builder()
            .subject_id(Uuid::new_v4())
            .subject_tenant_id(Uuid::parse_str(tenant_id).unwrap())
            .build()
            .unwrap()
    }

    // Test UUIDs
    const TENANT_A: &str = "11111111-1111-1111-1111-111111111111";
    const TENANT_B: &str = "22222222-2222-2222-2222-222222222222";
    const TENANT_C: &str = "33333333-3333-3333-3333-333333333333";
    const TENANT_D: &str = "44444444-4444-4444-4444-444444444444";
    const NONEXISTENT: &str = "99999999-9999-9999-9999-999999999999";

    // ==================== get_tenant tests ====================

    #[tokio::test]
    async fn get_tenant_existing() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![tenant(TENANT_A, "Tenant A", TenantStatus::Active)],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let result = service
            .get_tenant(&ctx, Uuid::parse_str(TENANT_A).unwrap())
            .await;

        assert!(result.is_ok());
        let info = result.unwrap();
        assert_eq!(info.name, "Tenant A");
        assert_eq!(info.status, TenantStatus::Active);
    }

    #[tokio::test]
    async fn get_tenant_nonexistent() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![tenant(TENANT_A, "Tenant A", TenantStatus::Active)],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);
        let nonexistent_id = Uuid::parse_str(NONEXISTENT).unwrap();

        let result = service.get_tenant(&ctx, nonexistent_id).await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TenantResolverError::TenantNotFound { tenant_id } => {
                assert_eq!(tenant_id, nonexistent_id);
            }
            other => panic!("Expected TenantNotFound, got: {other:?}"),
        }
    }

    // ==================== get_tenants tests ====================

    #[tokio::test]
    async fn get_tenants_all_found() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "A", TenantStatus::Active),
                tenant(TENANT_B, "B", TenantStatus::Active),
                tenant(TENANT_C, "C", TenantStatus::Suspended),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let ids = vec![
            Uuid::parse_str(TENANT_A).unwrap(),
            Uuid::parse_str(TENANT_B).unwrap(),
        ];

        let result = service
            .get_tenants(&ctx, &ids, &GetTenantsOptions::default())
            .await;
        assert!(result.is_ok());
        let tenants = result.unwrap();
        assert_eq!(tenants.len(), 2);
    }

    #[tokio::test]
    async fn get_tenants_some_missing() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![tenant(TENANT_A, "A", TenantStatus::Active)],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let ids = vec![
            Uuid::parse_str(TENANT_A).unwrap(),
            Uuid::parse_str(NONEXISTENT).unwrap(), // This one doesn't exist
        ];

        let result = service
            .get_tenants(&ctx, &ids, &GetTenantsOptions::default())
            .await;
        assert!(result.is_ok());
        let tenants = result.unwrap();
        // Only found tenant is returned, missing is silently skipped
        assert_eq!(tenants.len(), 1);
        assert_eq!(tenants[0].id, Uuid::parse_str(TENANT_A).unwrap());
    }

    #[tokio::test]
    async fn get_tenants_with_filter() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "A", TenantStatus::Active),
                tenant(TENANT_B, "B", TenantStatus::Suspended),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let ids = vec![
            Uuid::parse_str(TENANT_A).unwrap(),
            Uuid::parse_str(TENANT_B).unwrap(),
        ];

        let opts = GetTenantsOptions {
            status: vec![TenantStatus::Active],
        };
        let result = service.get_tenants(&ctx, &ids, &opts).await;
        assert!(result.is_ok());
        let tenants = result.unwrap();
        // Only active tenant is returned
        assert_eq!(tenants.len(), 1);
        assert_eq!(tenants[0].id, Uuid::parse_str(TENANT_A).unwrap());
    }

    // ==================== get_ancestors tests ====================

    #[tokio::test]
    async fn get_ancestors_root_tenant() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![tenant(TENANT_A, "Root", TenantStatus::Active)],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let result = service
            .get_ancestors(
                &ctx,
                Uuid::parse_str(TENANT_A).unwrap(),
                &GetAncestorsOptions::default(),
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.tenant.id, Uuid::parse_str(TENANT_A).unwrap());
        assert!(response.ancestors.is_empty());
    }

    #[tokio::test]
    async fn get_ancestors_with_hierarchy() {
        // A -> B -> C
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                tenant_with_parent(TENANT_B, "Child", TENANT_A),
                tenant_with_parent(TENANT_C, "Grandchild", TENANT_B),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_C);

        let result = service
            .get_ancestors(
                &ctx,
                Uuid::parse_str(TENANT_C).unwrap(),
                &GetAncestorsOptions::default(),
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.tenant.id, Uuid::parse_str(TENANT_C).unwrap());
        assert_eq!(response.ancestors.len(), 2);
        assert_eq!(response.ancestors[0].id, Uuid::parse_str(TENANT_B).unwrap());
        assert_eq!(response.ancestors[1].id, Uuid::parse_str(TENANT_A).unwrap());
    }

    #[tokio::test]
    async fn get_ancestors_with_barrier() {
        // A -> B (barrier) -> C
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                tenant_barrier(TENANT_B, "Barrier", TENANT_A),
                tenant_with_parent(TENANT_C, "Grandchild", TENANT_B),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_C);

        // Default (BarrierMode::Respect) - stops at barrier
        let result = service
            .get_ancestors(
                &ctx,
                Uuid::parse_str(TENANT_C).unwrap(),
                &GetAncestorsOptions::default(),
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.ancestors.len(), 1);
        assert_eq!(response.ancestors[0].id, Uuid::parse_str(TENANT_B).unwrap());

        // BarrierMode::Ignore - traverses through
        let req = GetAncestorsOptions {
            barrier_mode: BarrierMode::Ignore,
        };
        let result = service
            .get_ancestors(&ctx, Uuid::parse_str(TENANT_C).unwrap(), &req)
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.ancestors.len(), 2);
    }

    #[tokio::test]
    async fn get_ancestors_nonexistent() {
        let cfg = StaticTrPluginConfig::default();
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let result = service
            .get_ancestors(
                &ctx,
                Uuid::parse_str(NONEXISTENT).unwrap(),
                &GetAncestorsOptions::default(),
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TenantResolverError::TenantNotFound { .. } => {}
            other => panic!("Expected TenantNotFound, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_ancestors_starting_tenant_is_barrier() {
        // A -> B (barrier)
        // get_ancestors(B) should return empty because B is a barrier
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                tenant_barrier(TENANT_B, "Barrier", TENANT_A),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_B);

        // Default (BarrierMode::Respect) - B cannot see its parent chain
        let result = service
            .get_ancestors(
                &ctx,
                Uuid::parse_str(TENANT_B).unwrap(),
                &GetAncestorsOptions::default(),
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.tenant.id, Uuid::parse_str(TENANT_B).unwrap());
        assert!(response.ancestors.is_empty());

        // BarrierMode::Ignore - B can see A
        let req = GetAncestorsOptions {
            barrier_mode: BarrierMode::Ignore,
        };
        let result = service
            .get_ancestors(&ctx, Uuid::parse_str(TENANT_B).unwrap(), &req)
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.ancestors.len(), 1);
        assert_eq!(response.ancestors[0].id, Uuid::parse_str(TENANT_A).unwrap());
    }

    // ==================== get_descendants tests ====================

    #[tokio::test]
    async fn get_descendants_no_children() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![tenant(TENANT_A, "Root", TenantStatus::Active)],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let result = service
            .get_descendants(
                &ctx,
                Uuid::parse_str(TENANT_A).unwrap(),
                &GetDescendantsOptions::default(),
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.tenant.id, Uuid::parse_str(TENANT_A).unwrap());
        assert!(response.descendants.is_empty());
    }

    #[tokio::test]
    async fn get_descendants_with_hierarchy() {
        // A -> B -> C
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                tenant_with_parent(TENANT_B, "Child", TENANT_A),
                tenant_with_parent(TENANT_C, "Grandchild", TENANT_B),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        // Unlimited depth
        let result = service
            .get_descendants(
                &ctx,
                Uuid::parse_str(TENANT_A).unwrap(),
                &GetDescendantsOptions::default(),
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.tenant.id, Uuid::parse_str(TENANT_A).unwrap());
        assert_eq!(response.descendants.len(), 2);

        // Depth 1 only
        let req = GetDescendantsOptions {
            max_depth: Some(1),
            ..Default::default()
        };
        let result = service
            .get_descendants(&ctx, Uuid::parse_str(TENANT_A).unwrap(), &req)
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.descendants.len(), 1);
        assert_eq!(
            response.descendants[0].id,
            Uuid::parse_str(TENANT_B).unwrap()
        );
    }

    #[tokio::test]
    async fn get_descendants_with_barrier() {
        // A -> B (barrier) -> C
        //   -> D
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                tenant_barrier(TENANT_B, "Barrier", TENANT_A),
                tenant_with_parent(TENANT_C, "Under Barrier", TENANT_B),
                tenant_with_parent(TENANT_D, "Normal Child", TENANT_A),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        // Default (BarrierMode::Respect) - only D is visible
        let result = service
            .get_descendants(
                &ctx,
                Uuid::parse_str(TENANT_A).unwrap(),
                &GetDescendantsOptions::default(),
            )
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.descendants.len(), 1);
        assert_eq!(
            response.descendants[0].id,
            Uuid::parse_str(TENANT_D).unwrap()
        );

        // BarrierMode::Ignore - all descendants visible
        let req = GetDescendantsOptions {
            barrier_mode: BarrierMode::Ignore,
            ..Default::default()
        };
        let result = service
            .get_descendants(&ctx, Uuid::parse_str(TENANT_A).unwrap(), &req)
            .await;

        assert!(result.is_ok());
        let response = result.unwrap();
        assert_eq!(response.descendants.len(), 3);
    }

    #[tokio::test]
    async fn get_descendants_nonexistent() {
        let cfg = StaticTrPluginConfig::default();
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let result = service
            .get_descendants(
                &ctx,
                Uuid::parse_str(NONEXISTENT).unwrap(),
                &GetDescendantsOptions::default(),
            )
            .await;

        assert!(result.is_err());
        match result.unwrap_err() {
            TenantResolverError::TenantNotFound { .. } => {}
            other => panic!("Expected TenantNotFound, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn get_descendants_filter_stops_traversal() {
        // A (active) -> B (suspended) -> C (active)
        //           -> D (active)
        // Filter for active-only should return D only, NOT C
        // (because B doesn't pass filter, so its subtree is excluded)
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                {
                    let mut t = tenant_with_parent(TENANT_B, "Suspended", TENANT_A);
                    t.status = TenantStatus::Suspended;
                    t
                },
                tenant_with_parent(TENANT_C, "Child of Suspended", TENANT_B),
                tenant_with_parent(TENANT_D, "Active Child", TENANT_A),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        // Without filter: all 3 descendants (pre-order: B, C, D)
        let result = service
            .get_descendants(
                &ctx,
                Uuid::parse_str(TENANT_A).unwrap(),
                &GetDescendantsOptions::default(),
            )
            .await
            .unwrap();
        assert_eq!(result.descendants.len(), 3);

        // With active-only filter: only D (B filtered out, so C is unreachable)
        let req = GetDescendantsOptions {
            status: vec![TenantStatus::Active],
            ..Default::default()
        };
        let result = service
            .get_descendants(&ctx, Uuid::parse_str(TENANT_A).unwrap(), &req)
            .await
            .unwrap();

        assert_eq!(result.descendants.len(), 1);
        assert_eq!(result.descendants[0].id, Uuid::parse_str(TENANT_D).unwrap());
    }

    #[tokio::test]
    async fn get_descendants_pre_order() {
        // Verify pre-order traversal: parent before children
        // A -> B -> C
        // Pre-order from A: B first, then C (B must come before its child C)
        // Note: Sibling order is not guaranteed (HashMap), so we test a linear chain
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                tenant_with_parent(TENANT_B, "Child B", TENANT_A),
                tenant_with_parent(TENANT_C, "Grandchild C", TENANT_B),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let result = service
            .get_descendants(
                &ctx,
                Uuid::parse_str(TENANT_A).unwrap(),
                &GetDescendantsOptions::default(),
            )
            .await
            .unwrap();

        assert_eq!(result.descendants.len(), 2);
        // Pre-order guarantee: B comes before C (parent before child)
        assert_eq!(result.descendants[0].id, Uuid::parse_str(TENANT_B).unwrap());
        assert_eq!(result.descendants[1].id, Uuid::parse_str(TENANT_C).unwrap());
    }

    // ==================== is_ancestor tests ====================

    #[tokio::test]
    async fn is_ancestor_self_returns_false() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![tenant(TENANT_A, "Root", TenantStatus::Active)],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);
        let a_id = Uuid::parse_str(TENANT_A).unwrap();

        let result = service
            .is_ancestor(&ctx, a_id, a_id, &IsAncestorOptions::default())
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn is_ancestor_direct_parent() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                tenant_with_parent(TENANT_B, "Child", TENANT_A),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let a_id = Uuid::parse_str(TENANT_A).unwrap();
        let b_id = Uuid::parse_str(TENANT_B).unwrap();

        // A is ancestor of B
        let result = service
            .is_ancestor(&ctx, a_id, b_id, &IsAncestorOptions::default())
            .await;
        assert!(result.is_ok());
        assert!(result.unwrap());

        // B is NOT ancestor of A
        let result = service
            .is_ancestor(&ctx, b_id, a_id, &IsAncestorOptions::default())
            .await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }

    #[tokio::test]
    async fn is_ancestor_with_barrier() {
        // A -> B (barrier) -> C
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                tenant_barrier(TENANT_B, "Barrier", TENANT_A),
                tenant_with_parent(TENANT_C, "Grandchild", TENANT_B),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let a_id = Uuid::parse_str(TENANT_A).unwrap();
        let b_id = Uuid::parse_str(TENANT_B).unwrap();
        let c_id = Uuid::parse_str(TENANT_C).unwrap();

        // B is direct parent of C - allowed
        let result = service
            .is_ancestor(&ctx, b_id, c_id, &IsAncestorOptions::default())
            .await;
        assert!(result.unwrap());

        // A blocked by barrier B
        let result = service
            .is_ancestor(&ctx, a_id, c_id, &IsAncestorOptions::default())
            .await;
        assert!(!result.unwrap());

        // With BarrierMode::Ignore - A is ancestor of C
        let req = IsAncestorOptions {
            barrier_mode: BarrierMode::Ignore,
        };
        let result = service.is_ancestor(&ctx, a_id, c_id, &req).await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn is_ancestor_direct_barrier_child() {
        // A -> B (barrier)
        // is_ancestor(A, B) should return false because B is a barrier
        let cfg = StaticTrPluginConfig {
            tenants: vec![
                tenant(TENANT_A, "Root", TenantStatus::Active),
                tenant_barrier(TENANT_B, "Barrier", TENANT_A),
            ],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let a_id = Uuid::parse_str(TENANT_A).unwrap();
        let b_id = Uuid::parse_str(TENANT_B).unwrap();

        // A is NOT ancestor of B when B is a barrier (default BarrierMode::Respect)
        let result = service
            .is_ancestor(&ctx, a_id, b_id, &IsAncestorOptions::default())
            .await;
        assert!(!result.unwrap());

        // With BarrierMode::Ignore, A IS ancestor of B
        let req = IsAncestorOptions {
            barrier_mode: BarrierMode::Ignore,
        };
        let result = service.is_ancestor(&ctx, a_id, b_id, &req).await;
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn is_ancestor_nonexistent() {
        let cfg = StaticTrPluginConfig {
            tenants: vec![tenant(TENANT_A, "Root", TenantStatus::Active)],
            ..Default::default()
        };
        let service = Service::from_config(&cfg);
        let ctx = ctx_for_tenant(TENANT_A);

        let a_id = Uuid::parse_str(TENANT_A).unwrap();
        let nonexistent = Uuid::parse_str(NONEXISTENT).unwrap();

        // Nonexistent ancestor
        let result = service
            .is_ancestor(&ctx, nonexistent, a_id, &IsAncestorOptions::default())
            .await;
        assert!(result.is_err());

        // Nonexistent descendant
        let result = service
            .is_ancestor(&ctx, a_id, nonexistent, &IsAncestorOptions::default())
            .await;
        assert!(result.is_err());
    }
}
