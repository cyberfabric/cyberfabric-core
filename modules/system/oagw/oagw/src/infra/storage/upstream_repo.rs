use crate::domain::dto::{ListQuery, Upstream};
use crate::domain::repo::{RepositoryError, UpstreamRepository};
use dashmap::DashMap;
use modkit_macros::domain_model;
use uuid::Uuid;

/// In-memory upstream repository backed by `DashMap`.
#[domain_model]
pub struct InMemoryUpstreamRepo {
    /// Primary store: id -> Upstream.
    store: DashMap<Uuid, Upstream>,
    /// Alias index: (tenant_id, alias) -> upstream_id.
    alias_index: DashMap<(Uuid, String), Uuid>,
}

impl InMemoryUpstreamRepo {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: DashMap::new(),
            alias_index: DashMap::new(),
        }
    }
}

impl Default for InMemoryUpstreamRepo {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl UpstreamRepository for InMemoryUpstreamRepo {
    async fn create(&self, upstream: Upstream) -> Result<Upstream, RepositoryError> {
        let alias_key = (upstream.tenant_id, upstream.alias.clone());

        // Check alias uniqueness for this tenant.
        if self.alias_index.contains_key(&alias_key) {
            return Err(RepositoryError::Conflict(format!(
                "alias '{}' already exists for tenant",
                upstream.alias
            )));
        }

        self.alias_index.insert(alias_key, upstream.id);
        self.store.insert(upstream.id, upstream.clone());
        Ok(upstream)
    }

    async fn get_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Upstream, RepositoryError> {
        self.store
            .get(&id)
            .filter(|u| u.tenant_id == tenant_id)
            .map(|u| u.clone())
            .ok_or(RepositoryError::NotFound)
    }

    async fn get_by_alias(
        &self,
        tenant_id: Uuid,
        alias: &str,
    ) -> Result<Upstream, RepositoryError> {
        let id = self
            .alias_index
            .get(&(tenant_id, alias.to_string()))
            .map(|r| *r.value())
            .ok_or(RepositoryError::NotFound)?;
        self.get_by_id(tenant_id, id).await
    }

    async fn list(
        &self,
        tenant_id: Uuid,
        query: &ListQuery,
    ) -> Result<Vec<Upstream>, RepositoryError> {
        let all: Vec<Upstream> = self
            .store
            .iter()
            .filter(|e| e.value().tenant_id == tenant_id)
            .map(|e| e.value().clone())
            .collect();

        let skip = query.skip as usize;
        let top = query.top as usize;
        Ok(all.into_iter().skip(skip).take(top).collect())
    }

    async fn update(&self, upstream: Upstream) -> Result<Upstream, RepositoryError> {
        let id = upstream.id;
        let tenant_id = upstream.tenant_id;

        // Get the old upstream to remove old alias if changed.
        let old = self
            .store
            .get(&id)
            .filter(|u| u.tenant_id == tenant_id)
            .map(|u| u.clone())
            .ok_or(RepositoryError::NotFound)?;

        // If alias changed, update the alias index.
        if old.alias != upstream.alias {
            // Check new alias uniqueness.
            let new_alias_key = (tenant_id, upstream.alias.clone());
            if self.alias_index.contains_key(&new_alias_key) {
                return Err(RepositoryError::Conflict(format!(
                    "alias '{}' already exists for tenant",
                    upstream.alias
                )));
            }
            // Remove old alias.
            self.alias_index.remove(&(tenant_id, old.alias.clone()));
            // Add new alias.
            self.alias_index.insert(new_alias_key, id);
        }

        self.store.insert(id, upstream.clone());
        Ok(upstream)
    }

    async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<(), RepositoryError> {
        let removed = self
            .store
            .remove(&id)
            .filter(|(_, u)| u.tenant_id == tenant_id);
        match removed {
            Some((_, upstream)) => {
                self.alias_index
                    .remove(&(tenant_id, upstream.alias.clone()));
                Ok(())
            }
            None => Err(RepositoryError::NotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::dto::{Endpoint, Scheme, Server};

    use super::*;

    fn make_upstream(tenant_id: Uuid, alias: &str) -> Upstream {
        Upstream {
            id: Uuid::new_v4(),
            tenant_id,
            alias: alias.into(),
            server: Server {
                endpoints: vec![Endpoint {
                    scheme: Scheme::Https,
                    host: "api.openai.com".into(),
                    port: 443,
                }],
            },
            protocol: "gts.x.core.oagw.protocol.v1~x.core.oagw.http.v1".into(),
            enabled: true,
            auth: None,
            headers: None,
            plugins: None,
            rate_limit: None,
            tags: vec![],
        }
    }

    #[tokio::test]
    async fn create_and_get_round_trip() {
        let repo = InMemoryUpstreamRepo::new();
        let tenant = Uuid::new_v4();
        let u = make_upstream(tenant, "openai");
        let id = u.id;

        let created = repo.create(u).await.unwrap();
        assert_eq!(created.alias, "openai");

        let fetched = repo.get_by_id(tenant, id).await.unwrap();
        assert_eq!(fetched.id, id);
        assert_eq!(fetched.alias, "openai");
    }

    #[tokio::test]
    async fn get_by_alias() {
        let repo = InMemoryUpstreamRepo::new();
        let tenant = Uuid::new_v4();
        let u = make_upstream(tenant, "openai");
        repo.create(u.clone()).await.unwrap();

        let fetched = repo.get_by_alias(tenant, "openai").await.unwrap();
        assert_eq!(fetched.id, u.id);
    }

    #[tokio::test]
    async fn alias_uniqueness_same_tenant() {
        let repo = InMemoryUpstreamRepo::new();
        let tenant = Uuid::new_v4();

        repo.create(make_upstream(tenant, "openai")).await.unwrap();
        let err = repo.create(make_upstream(tenant, "openai")).await;
        assert!(matches!(err, Err(RepositoryError::Conflict(_))));
    }

    #[tokio::test]
    async fn alias_allowed_different_tenant() {
        let repo = InMemoryUpstreamRepo::new();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();

        repo.create(make_upstream(t1, "openai")).await.unwrap();
        repo.create(make_upstream(t2, "openai")).await.unwrap();
    }

    #[tokio::test]
    async fn update_preserves_id() {
        let repo = InMemoryUpstreamRepo::new();
        let tenant = Uuid::new_v4();
        let mut u = make_upstream(tenant, "openai");
        let id = u.id;
        repo.create(u.clone()).await.unwrap();

        u.alias = "openai-v2".into();
        let updated = repo.update(u).await.unwrap();
        assert_eq!(updated.id, id);
        assert_eq!(updated.alias, "openai-v2");

        // Old alias should not resolve.
        assert!(repo.get_by_alias(tenant, "openai").await.is_err());
        // New alias should resolve.
        assert!(repo.get_by_alias(tenant, "openai-v2").await.is_ok());
    }

    #[tokio::test]
    async fn delete_removes_alias_index() {
        let repo = InMemoryUpstreamRepo::new();
        let tenant = Uuid::new_v4();
        let u = make_upstream(tenant, "openai");
        let id = u.id;
        repo.create(u).await.unwrap();

        repo.delete(tenant, id).await.unwrap();
        assert!(repo.get_by_id(tenant, id).await.is_err());
        assert!(repo.get_by_alias(tenant, "openai").await.is_err());
    }

    #[tokio::test]
    async fn list_with_pagination() {
        let repo = InMemoryUpstreamRepo::new();
        let tenant = Uuid::new_v4();

        for i in 0..5 {
            repo.create(make_upstream(tenant, &format!("svc-{i}")))
                .await
                .unwrap();
        }

        let all = repo
            .list(tenant, &ListQuery { top: 50, skip: 0 })
            .await
            .unwrap();
        assert_eq!(all.len(), 5);

        let page = repo
            .list(tenant, &ListQuery { top: 2, skip: 1 })
            .await
            .unwrap();
        assert_eq!(page.len(), 2);
    }

    #[tokio::test]
    async fn cross_tenant_isolation() {
        let repo = InMemoryUpstreamRepo::new();
        let t1 = Uuid::new_v4();
        let t2 = Uuid::new_v4();

        let u = make_upstream(t1, "openai");
        let id = u.id;
        repo.create(u).await.unwrap();

        // Different tenant cannot see it.
        assert!(repo.get_by_id(t2, id).await.is_err());
    }
}
