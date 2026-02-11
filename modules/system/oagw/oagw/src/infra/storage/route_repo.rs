use crate::domain::dto::{HttpMethod, ListQuery, Route};
use crate::domain::repo::{RepositoryError, RouteRepository};
use dashmap::DashMap;
use modkit_macros::domain_model;
use uuid::Uuid;

/// In-memory route repository backed by `DashMap`.
#[domain_model]
pub struct InMemoryRouteRepo {
    /// Primary store: route_id -> Route.
    store: DashMap<Uuid, Route>,
    /// Upstream index: upstream_id -> vec of route_ids.
    upstream_index: DashMap<Uuid, Vec<Uuid>>,
}

impl InMemoryRouteRepo {
    #[must_use]
    pub fn new() -> Self {
        Self {
            store: DashMap::new(),
            upstream_index: DashMap::new(),
        }
    }
}

impl Default for InMemoryRouteRepo {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl RouteRepository for InMemoryRouteRepo {
    async fn create(&self, route: Route) -> Result<Route, RepositoryError> {
        let route_id = route.id;
        let upstream_id = route.upstream_id;

        self.store.insert(route_id, route.clone());

        // Update upstream index.
        self.upstream_index
            .entry(upstream_id)
            .or_default()
            .push(route_id);

        Ok(route)
    }

    async fn get_by_id(&self, tenant_id: Uuid, id: Uuid) -> Result<Route, RepositoryError> {
        self.store
            .get(&id)
            .filter(|r| r.tenant_id == tenant_id)
            .map(|r| r.clone())
            .ok_or(RepositoryError::NotFound)
    }

    async fn list_by_upstream(
        &self,
        tenant_id: Uuid,
        upstream_id: Uuid,
        query: &ListQuery,
    ) -> Result<Vec<Route>, RepositoryError> {
        let route_ids: Vec<Uuid> = self
            .upstream_index
            .get(&upstream_id)
            .map(|ids| ids.clone())
            .unwrap_or_default();

        let routes: Vec<Route> = route_ids
            .iter()
            .filter_map(|id| {
                self.store
                    .get(id)
                    .filter(|r| r.tenant_id == tenant_id)
                    .map(|r| r.clone())
            })
            .collect();

        let skip = query.skip as usize;
        let top = query.top as usize;
        Ok(routes.into_iter().skip(skip).take(top).collect())
    }

    async fn find_matching(
        &self,
        tenant_id: Uuid,
        upstream_id: Uuid,
        method: &str,
        path: &str,
    ) -> Result<Route, RepositoryError> {
        let route_ids: Vec<Uuid> = self
            .upstream_index
            .get(&upstream_id)
            .map(|ids| ids.clone())
            .unwrap_or_default();

        let request_method = parse_method(method);

        let mut best: Option<Route> = None;
        let mut best_path_len = 0;
        let mut best_priority = i32::MIN;

        for id in &route_ids {
            let Some(route_ref) = self.store.get(id) else {
                continue;
            };
            let route = route_ref.value();

            // Must match tenant.
            if route.tenant_id != tenant_id {
                continue;
            }
            // Must be enabled.
            if !route.enabled {
                continue;
            }
            // Must have HTTP match rules.
            let Some(http_match) = &route.match_rules.http else {
                continue;
            };
            // Method must match.
            if let Some(req_method) = &request_method
                && !http_match.methods.contains(req_method)
            {
                continue;
            }
            // Path must be a prefix match.
            if !path.starts_with(&http_match.path) {
                continue;
            }

            let path_len = http_match.path.len();
            let priority = route.priority;

            // Select by longest path prefix, then highest priority.
            if path_len > best_path_len || (path_len == best_path_len && priority > best_priority) {
                best_path_len = path_len;
                best_priority = priority;
                best = Some(route.clone());
            }
        }

        best.ok_or(RepositoryError::NotFound)
    }

    async fn update(&self, route: Route) -> Result<Route, RepositoryError> {
        if !self.store.contains_key(&route.id) {
            return Err(RepositoryError::NotFound);
        }
        self.store.insert(route.id, route.clone());
        Ok(route)
    }

    async fn delete(&self, tenant_id: Uuid, id: Uuid) -> Result<(), RepositoryError> {
        let removed = self
            .store
            .remove(&id)
            .filter(|(_, r)| r.tenant_id == tenant_id);

        match removed {
            Some((_, route)) => {
                // Remove from upstream index.
                if let Some(mut ids) = self.upstream_index.get_mut(&route.upstream_id) {
                    ids.retain(|rid| *rid != id);
                }
                Ok(())
            }
            None => Err(RepositoryError::NotFound),
        }
    }

    async fn delete_by_upstream(
        &self,
        tenant_id: Uuid,
        upstream_id: Uuid,
    ) -> Result<u64, RepositoryError> {
        let route_ids: Vec<Uuid> = self
            .upstream_index
            .remove(&upstream_id)
            .map(|(_, ids)| ids)
            .unwrap_or_default();

        let mut deleted = 0u64;
        for id in route_ids {
            if let Some((_, route)) = self.store.remove(&id) {
                if route.tenant_id == tenant_id {
                    deleted += 1;
                } else {
                    // Put it back — wrong tenant.
                    self.store.insert(id, route);
                }
            }
        }
        Ok(deleted)
    }
}

fn parse_method(s: &str) -> Option<HttpMethod> {
    match s.to_uppercase().as_str() {
        "GET" => Some(HttpMethod::Get),
        "POST" => Some(HttpMethod::Post),
        "PUT" => Some(HttpMethod::Put),
        "DELETE" => Some(HttpMethod::Delete),
        "PATCH" => Some(HttpMethod::Patch),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::dto::{HttpMatch, MatchRules, PathSuffixMode};

    use super::*;

    fn make_route(
        tenant_id: Uuid,
        upstream_id: Uuid,
        methods: Vec<HttpMethod>,
        path: &str,
        priority: i32,
    ) -> Route {
        Route {
            id: Uuid::new_v4(),
            tenant_id,
            upstream_id,
            match_rules: MatchRules {
                http: Some(HttpMatch {
                    methods,
                    path: path.into(),
                    query_allowlist: vec![],
                    path_suffix_mode: PathSuffixMode::Append,
                }),
                grpc: None,
            },
            plugins: None,
            rate_limit: None,
            tags: vec![],
            priority,
            enabled: true,
        }
    }

    #[tokio::test]
    async fn find_matching_longest_prefix_wins() {
        let repo = InMemoryRouteRepo::new();
        let tenant = Uuid::new_v4();
        let upstream = Uuid::new_v4();

        let short = make_route(tenant, upstream, vec![HttpMethod::Post], "/v1", 0);
        let long = make_route(
            tenant,
            upstream,
            vec![HttpMethod::Post],
            "/v1/chat/completions",
            0,
        );
        repo.create(short).await.unwrap();
        repo.create(long.clone()).await.unwrap();

        let matched = repo
            .find_matching(tenant, upstream, "POST", "/v1/chat/completions")
            .await
            .unwrap();
        assert_eq!(matched.id, long.id);
    }

    #[tokio::test]
    async fn find_matching_priority_tiebreak() {
        let repo = InMemoryRouteRepo::new();
        let tenant = Uuid::new_v4();
        let upstream = Uuid::new_v4();

        let low = make_route(tenant, upstream, vec![HttpMethod::Post], "/v1/chat", 0);
        let high = make_route(tenant, upstream, vec![HttpMethod::Post], "/v1/chat", 10);
        repo.create(low).await.unwrap();
        repo.create(high.clone()).await.unwrap();

        let matched = repo
            .find_matching(tenant, upstream, "POST", "/v1/chat/completions")
            .await
            .unwrap();
        assert_eq!(matched.id, high.id);
    }

    #[tokio::test]
    async fn find_matching_method_mismatch_excluded() {
        let repo = InMemoryRouteRepo::new();
        let tenant = Uuid::new_v4();
        let upstream = Uuid::new_v4();

        let post_only = make_route(
            tenant,
            upstream,
            vec![HttpMethod::Post],
            "/v1/chat/completions",
            0,
        );
        repo.create(post_only).await.unwrap();

        let result = repo
            .find_matching(tenant, upstream, "GET", "/v1/chat/completions")
            .await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn find_matching_disabled_excluded() {
        let repo = InMemoryRouteRepo::new();
        let tenant = Uuid::new_v4();
        let upstream = Uuid::new_v4();

        let mut route = make_route(tenant, upstream, vec![HttpMethod::Post], "/v1/chat", 0);
        route.enabled = false;
        repo.create(route).await.unwrap();

        let result = repo
            .find_matching(tenant, upstream, "POST", "/v1/chat/completions")
            .await;
        assert!(matches!(result, Err(RepositoryError::NotFound)));
    }

    #[tokio::test]
    async fn list_by_upstream_returns_correct_set() {
        let repo = InMemoryRouteRepo::new();
        let tenant = Uuid::new_v4();
        let u1 = Uuid::new_v4();
        let u2 = Uuid::new_v4();

        repo.create(make_route(tenant, u1, vec![HttpMethod::Post], "/a", 0))
            .await
            .unwrap();
        repo.create(make_route(tenant, u1, vec![HttpMethod::Get], "/b", 0))
            .await
            .unwrap();
        repo.create(make_route(tenant, u2, vec![HttpMethod::Post], "/c", 0))
            .await
            .unwrap();

        let routes = repo
            .list_by_upstream(tenant, u1, &ListQuery { top: 50, skip: 0 })
            .await
            .unwrap();
        assert_eq!(routes.len(), 2);
    }

    #[tokio::test]
    async fn delete_by_upstream_cascade() {
        let repo = InMemoryRouteRepo::new();
        let tenant = Uuid::new_v4();
        let upstream = Uuid::new_v4();

        let r1 = make_route(tenant, upstream, vec![HttpMethod::Post], "/a", 0);
        let r2 = make_route(tenant, upstream, vec![HttpMethod::Get], "/b", 0);
        repo.create(r1.clone()).await.unwrap();
        repo.create(r2.clone()).await.unwrap();

        let deleted = repo.delete_by_upstream(tenant, upstream).await.unwrap();
        assert_eq!(deleted, 2);

        assert!(repo.get_by_id(tenant, r1.id).await.is_err());
        assert!(repo.get_by_id(tenant, r2.id).await.is_err());
    }
}
