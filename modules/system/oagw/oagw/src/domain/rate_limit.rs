use std::time::Instant;

use dashmap::DashMap;
use crate::domain::dto::{RateLimitConfig, Window};
use crate::domain::error::DomainError;

pub struct RateLimiter {
    buckets: DashMap<String, TokenBucket>,
}

struct TokenBucket {
    capacity: f64,
    tokens: f64,
    refill_rate: f64, // tokens per second
    last_refill: Instant,
}

impl TokenBucket {
    fn new(config: &RateLimitConfig) -> Self {
        let capacity = config
            .burst
            .as_ref()
            .map_or(config.sustained.rate as f64, |b| b.capacity as f64);
        let window_secs = window_to_secs(&config.sustained.window);
        let refill_rate = config.sustained.rate as f64 / window_secs;
        Self {
            capacity,
            tokens: capacity,
            refill_rate,
            last_refill: Instant::now(),
        }
    }

    fn refill(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_refill).as_secs_f64();
        self.tokens = (self.tokens + elapsed * self.refill_rate).min(self.capacity);
        self.last_refill = now;
    }

    fn try_consume(&mut self, cost: f64) -> bool {
        self.refill();
        if self.tokens >= cost {
            self.tokens -= cost;
            true
        } else {
            false
        }
    }

    fn retry_after_secs(&self, cost: f64) -> u64 {
        if self.refill_rate <= 0.0 {
            return 60;
        }
        let needed = cost - self.tokens;
        if needed <= 0.0 {
            return 0;
        }
        (needed / self.refill_rate).ceil() as u64
    }
}

fn window_to_secs(window: &Window) -> f64 {
    match window {
        Window::Second => 1.0,
        Window::Minute => 60.0,
        Window::Hour => 3600.0,
        Window::Day => 86400.0,
    }
}

impl RateLimiter {
    #[must_use]
    pub fn new() -> Self {
        Self {
            buckets: DashMap::new(),
        }
    }

    /// Try to consume tokens for the given key.
    ///
    /// # Errors
    /// Returns `DomainError::RateLimitExceeded` with Retry-After seconds when exhausted.
    pub fn try_consume(
        &self,
        key: &str,
        config: &RateLimitConfig,
        instance_uri: &str,
    ) -> Result<(), DomainError> {
        let cost = config.cost as f64;
        let mut bucket = self
            .buckets
            .entry(key.to_string())
            .or_insert_with(|| TokenBucket::new(config));

        if bucket.try_consume(cost) {
            Ok(())
        } else {
            let retry_after = bucket.retry_after_secs(cost);
            Err(DomainError::RateLimitExceeded {
                detail: format!("rate limit exceeded for key: {key}"),
                instance: instance_uri.to_string(),
                retry_after_secs: Some(retry_after),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::dto::{
        BurstConfig, RateLimitAlgorithm, RateLimitScope, RateLimitStrategy, SustainedRate,
    };

    use super::*;

    fn make_config(rate: u32, window: Window, burst_capacity: Option<u32>) -> RateLimitConfig {
        RateLimitConfig {
            sharing: Default::default(),
            algorithm: RateLimitAlgorithm::TokenBucket,
            sustained: SustainedRate { rate, window },
            burst: burst_capacity.map(|c| BurstConfig { capacity: c }),
            scope: RateLimitScope::Tenant,
            strategy: RateLimitStrategy::Reject,
            cost: 1,
        }
    }

    #[test]
    fn allows_within_capacity() {
        let limiter = RateLimiter::new();
        let config = make_config(10, Window::Second, None);
        for _ in 0..10 {
            assert!(limiter.try_consume("test", &config, "/test").is_ok());
        }
    }

    #[test]
    fn denies_when_exhausted() {
        let limiter = RateLimiter::new();
        let config = make_config(2, Window::Second, None);
        assert!(limiter.try_consume("test", &config, "/test").is_ok());
        assert!(limiter.try_consume("test", &config, "/test").is_ok());
        let err = limiter.try_consume("test", &config, "/test").unwrap_err();
        assert!(matches!(err, DomainError::RateLimitExceeded { .. }));
    }

    #[test]
    fn retry_after_is_calculated() {
        let limiter = RateLimiter::new();
        let config = make_config(1, Window::Minute, None);
        assert!(limiter.try_consume("test", &config, "/test").is_ok());
        match limiter.try_consume("test", &config, "/test") {
            Err(DomainError::RateLimitExceeded {
                retry_after_secs, ..
            }) => {
                // ~60 seconds (1 token per minute).
                assert!(retry_after_secs.unwrap() > 0);
                assert!(retry_after_secs.unwrap() <= 60);
            }
            other => panic!("expected RateLimitExceeded, got {other:?}"),
        }
    }

    #[test]
    fn burst_capacity_used() {
        let limiter = RateLimiter::new();
        let config = make_config(1, Window::Second, Some(5));
        for _ in 0..5 {
            assert!(limiter.try_consume("test", &config, "/test").is_ok());
        }
        assert!(limiter.try_consume("test", &config, "/test").is_err());
    }

    #[test]
    fn separate_keys_independent() {
        let limiter = RateLimiter::new();
        let config = make_config(1, Window::Second, None);
        assert!(limiter.try_consume("key-a", &config, "/test").is_ok());
        assert!(limiter.try_consume("key-b", &config, "/test").is_ok());
        assert!(limiter.try_consume("key-a", &config, "/test").is_err());
        assert!(limiter.try_consume("key-b", &config, "/test").is_err());
    }
}
