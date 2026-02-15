use std::collections::HashMap;
use std::pin::Pin;

use bytes::Bytes;
use futures_util::Stream;
use http::{HeaderMap, Method, StatusCode};
use modkit_security::SecurityContext;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Body / Error aliases
// ---------------------------------------------------------------------------

pub type BoxError = Box<dyn std::error::Error + Send + Sync>;
pub type BodyStream = Pin<Box<dyn Stream<Item = Result<Bytes, BoxError>> + Send>>;

// ---------------------------------------------------------------------------
// Shared enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SharingMode {
    #[default]
    Private,
    Inherit,
    Enforce,
}

// ---------------------------------------------------------------------------
// Endpoint / Server
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Scheme {
    Http,
    #[default]
    Https,
    Wss,
    Wt,
    Grpc,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub scheme: Scheme,
    pub host: String,
    pub port: u16,
}

impl Endpoint {
    #[must_use]
    pub fn alias_contribution(&self) -> String {
        if self.port == 443 || self.port == 80 {
            self.host.clone()
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Server {
    pub endpoints: Vec<Endpoint>,
}

// ---------------------------------------------------------------------------
// AuthConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct AuthConfig {
    pub plugin_type: String,
    pub sharing: SharingMode,
    pub config: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// HeadersConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct HeadersConfig {
    pub request: Option<RequestHeaderRules>,
    pub response: Option<ResponseHeaderRules>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct RequestHeaderRules {
    pub set: HashMap<String, String>,
    pub add: HashMap<String, String>,
    pub remove: Vec<String>,
    pub passthrough: PassthroughMode,
    pub passthrough_allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResponseHeaderRules {
    pub set: HashMap<String, String>,
    pub add: HashMap<String, String>,
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PassthroughMode {
    #[default]
    None,
    Allowlist,
    All,
}

// ---------------------------------------------------------------------------
// RateLimitConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct RateLimitConfig {
    pub sharing: SharingMode,
    pub algorithm: RateLimitAlgorithm,
    pub sustained: SustainedRate,
    pub burst: Option<BurstConfig>,
    pub scope: RateLimitScope,
    pub strategy: RateLimitStrategy,
    pub cost: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RateLimitAlgorithm {
    #[default]
    TokenBucket,
    SlidingWindow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SustainedRate {
    pub rate: u32,
    pub window: Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Window {
    #[default]
    Second,
    Minute,
    Hour,
    Day,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BurstConfig {
    pub capacity: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RateLimitScope {
    Global,
    #[default]
    Tenant,
    User,
    Ip,
    Route,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RateLimitStrategy {
    #[default]
    Reject,
    Queue,
    Degrade,
}

// ---------------------------------------------------------------------------
// PluginsConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Default)]
pub struct PluginsConfig {
    pub sharing: SharingMode,
    pub items: Vec<String>,
}

// ---------------------------------------------------------------------------
// Route matching
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PathSuffixMode {
    Disabled,
    #[default]
    Append,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HttpMatch {
    pub methods: Vec<HttpMethod>,
    pub path: String,
    pub query_allowlist: Vec<String>,
    pub path_suffix_mode: PathSuffixMode,
}

#[derive(Debug, Clone, PartialEq)]
pub struct GrpcMatch {
    pub service: String,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchRules {
    pub http: Option<HttpMatch>,
    pub grpc: Option<GrpcMatch>,
}

// ---------------------------------------------------------------------------
// Domain entities
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Route {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub upstream_id: Uuid,
    pub match_rules: MatchRules,
    pub plugins: Option<PluginsConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub tags: Vec<String>,
    pub priority: i32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Upstream {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub alias: String,
    pub server: Server,
    pub protocol: String,
    pub enabled: bool,
    pub auth: Option<AuthConfig>,
    pub headers: Option<HeadersConfig>,
    pub plugins: Option<PluginsConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub tags: Vec<String>,
}

// ---------------------------------------------------------------------------
// Pagination
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListQuery {
    pub top: u32,
    pub skip: u32,
}

impl Default for ListQuery {
    fn default() -> Self {
        Self { top: 50, skip: 0 }
    }
}

// ---------------------------------------------------------------------------
// Request types (public fields, no builder)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct CreateUpstreamRequest {
    pub server: Server,
    pub protocol: String,
    pub alias: Option<String>,
    pub auth: Option<AuthConfig>,
    pub headers: Option<HeadersConfig>,
    pub plugins: Option<PluginsConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub tags: Vec<String>,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UpdateUpstreamRequest {
    pub server: Option<Server>,
    pub protocol: Option<String>,
    pub alias: Option<String>,
    pub auth: Option<AuthConfig>,
    pub headers: Option<HeadersConfig>,
    pub plugins: Option<PluginsConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub tags: Option<Vec<String>>,
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CreateRouteRequest {
    pub upstream_id: Uuid,
    pub match_rules: MatchRules,
    pub plugins: Option<PluginsConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub tags: Vec<String>,
    pub priority: i32,
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct UpdateRouteRequest {
    pub match_rules: Option<MatchRules>,
    pub plugins: Option<PluginsConfig>,
    pub rate_limit: Option<RateLimitConfig>,
    pub tags: Option<Vec<String>>,
    pub priority: Option<i32>,
    pub enabled: Option<bool>,
}

// ---------------------------------------------------------------------------
// Proxy types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSource {
    Gateway,
    Upstream,
}

pub struct ProxyContext {
    pub ctx: SecurityContext,
    pub method: Method,
    pub alias: String,
    pub path_suffix: String,
    pub query_params: Vec<(String, String)>,
    pub headers: HeaderMap,
    pub body: Bytes,
    pub instance_uri: String,
}

pub struct ProxyResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: BodyStream,
    pub error_source: ErrorSource,
}
