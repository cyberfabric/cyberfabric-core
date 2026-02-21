use crate::config::ApiGatewayConfig;
use anyhow::{Context, Result, anyhow};
use axum::http::{HeaderValue, Method, StatusCode, header};
use axum::{
    extract::Request,
    middleware::Next,
    response::{IntoResponse, Response},
};
use governor::clock::Clock;
use governor::middleware::StateInformationMiddleware;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use std::collections::HashMap;
use std::num::NonZeroU32;
use std::sync::Arc;
use tokio::sync::Semaphore;

type RateLimitKey = (Method, String);
type BucketMap = Arc<HashMap<RateLimitKey, Arc<BucketMapEntry>>>;
type InflightMap = Arc<HashMap<RateLimitKey, Arc<Semaphore>>>;

#[derive(Default, Clone)]
pub struct RateLimiterMap {
    buckets: BucketMap,
    inflight: InflightMap,
}

struct BucketMapEntry {
    bucket: DefaultDirectRateLimiter<StateInformationMiddleware>,
    policy: HeaderValue,
    burst: HeaderValue,
}

impl BucketMapEntry {
    pub fn new(rps: u32, burst: u32) -> Result<Self> {
        let bucket = RateLimiter::direct(
            Quota::per_second(NonZeroU32::new(rps).with_context(|| anyhow!("rps is zero"))?)
                .allow_burst(NonZeroU32::new(burst).with_context(|| anyhow!("burst is zero"))?),
        )
        .with_middleware::<StateInformationMiddleware>();
        let policy = HeaderValue::from_str(&format!("\"burst\";q={burst};w={rps}"))
            .context("Failed to create rate limit policy")?;
        Ok(Self {
            bucket,
            policy,
            burst: burst.into(),
        })
    }
}

impl RateLimiterMap {
    /// # Errors
    /// Returns an error if any rate limit spec is 0.
    pub fn from_specs(
        specs: &Vec<modkit::api::OperationSpec>,
        cfg: &ApiGatewayConfig,
    ) -> Result<Self> {
        let mut buckets = HashMap::new();
        let mut inflight = HashMap::new();
        // TODO: Add support for per-route rate limiting
        for spec in specs {
            let (rps, burst, max_in_flight) = spec.rate_limit.as_ref().map_or(
                (
                    cfg.defaults.rate_limit.rps,
                    cfg.defaults.rate_limit.burst,
                    cfg.defaults.rate_limit.in_flight,
                ),
                |r| (r.rps, r.burst, r.in_flight),
            );

            let key = (spec.method.clone(), spec.path.clone());
            buckets.insert(
                key.clone(),
                Arc::new(
                    BucketMapEntry::new(rps, burst)
                        .with_context(|| anyhow!("RateLimit spec invalid {spec:?} invalid"))?,
                ),
            );
            inflight.insert(key, Arc::new(Semaphore::new(max_in_flight as usize)));
        }
        Ok(Self {
            buckets: Arc::new(buckets),
            inflight: Arc::new(inflight),
        })
    }
}

// TODO: Use tower-governor instead of own implementation (upd: https://github.com/benwis/tower-governor/issues/59 )
pub async fn rate_limit_middleware(map: RateLimiterMap, mut req: Request, next: Next) -> Response {
    let method = req.method().clone();
    // Use MatchedPath extension (set by Axum router) for accurate route matching
    let path = req
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map_or_else(|| req.uri().path().to_owned(), |p| p.as_str().to_owned());

    let nestedpath = req.extensions().get::<axum::extract::NestedPath>();

    let path = nestedpath
        .and_then(|np| path.strip_prefix(np.as_str()))
        .unwrap_or(&path)
        .to_owned();

    let key = (method, path);

    if let Some(bucker_map_entry) = map.buckets.get(&key) {
        let headers = req.headers_mut();
        headers.insert("RateLimit-Policy", bucker_map_entry.policy.clone());
        match bucker_map_entry.bucket.check() {
            Ok(state) => {
                headers.insert("RateLimit-Limit", bucker_map_entry.burst.clone());
                headers.insert(
                    "RateLimit-Limit-Remaining",
                    state.remaining_burst_capacity().into(),
                );
                headers.insert("X-RateLimit-Limit", bucker_map_entry.burst.clone());
                headers.insert(
                    "X-RateLimit-Remaining",
                    state.remaining_burst_capacity().into(),
                );
            }
            Err(not_until) => {
                let wait = not_until.wait_time_from(bucker_map_entry.bucket.clock().now());
                headers.insert(header::RETRY_AFTER, wait.as_secs().into());
                return StatusCode::TOO_MANY_REQUESTS.into_response();
            }
        }
    }

    if let Some(sem) = map.inflight.get(&key) {
        match sem.clone().try_acquire_owned() {
            Ok(_permit) => {
                // Allow request; permit is dropped when response future completes
                return next.run(req).await;
            }
            Err(_) => {
                return StatusCode::SERVICE_UNAVAILABLE.into_response();
            }
        }
    }

    next.run(req).await
}
