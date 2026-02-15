// REST DTOs for the OAGW API.
//
// These types own serde annotations and JSON schema concerns. They convert
// to/from internal domain types via `From` impls for the service layer boundary.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::domain::dto as domain;

// ---------------------------------------------------------------------------
// Shared enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SharingMode {
    #[default]
    Private,
    Inherit,
    Enforce,
}

// ---------------------------------------------------------------------------
// Endpoint / Server
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Scheme {
    Http,
    #[default]
    Https,
    Wss,
    Wt,
    Grpc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Endpoint {
    #[serde(default)]
    pub scheme: Scheme,
    pub host: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

fn default_port() -> u16 {
    443
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Server {
    pub endpoints: Vec<Endpoint>,
}

// ---------------------------------------------------------------------------
// AuthConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AuthConfig {
    #[serde(rename = "type")]
    pub plugin_type: String,
    #[serde(default)]
    pub sharing: SharingMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub config: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// HeadersConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct HeadersConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<RequestHeaderRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub response: Option<ResponseHeaderRules>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct RequestHeaderRules {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub set: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub add: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
    #[serde(default)]
    pub passthrough: PassthroughMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub passthrough_allowlist: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ResponseHeaderRules {
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub set: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub add: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub remove: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PassthroughMode {
    #[default]
    None,
    Allowlist,
    All,
}

// ---------------------------------------------------------------------------
// RateLimitConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RateLimitConfig {
    #[serde(default)]
    pub sharing: SharingMode,
    #[serde(default)]
    pub algorithm: RateLimitAlgorithm,
    pub sustained: SustainedRate,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burst: Option<BurstConfig>,
    #[serde(default)]
    pub scope: RateLimitScope,
    #[serde(default)]
    pub strategy: RateLimitStrategy,
    #[serde(default = "default_cost")]
    pub cost: u32,
}

fn default_cost() -> u32 {
    1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum RateLimitAlgorithm {
    #[default]
    TokenBucket,
    SlidingWindow,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SustainedRate {
    pub rate: u32,
    #[serde(default)]
    pub window: Window,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Window {
    #[default]
    Second,
    Minute,
    Hour,
    Day,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BurstConfig {
    pub capacity: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitScope {
    Global,
    #[default]
    Tenant,
    User,
    Ip,
    Route,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum RateLimitStrategy {
    #[default]
    Reject,
    Queue,
    Degrade,
}

// ---------------------------------------------------------------------------
// PluginsConfig
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct PluginsConfig {
    #[serde(default)]
    pub sharing: SharingMode,
    #[serde(default)]
    pub items: Vec<String>,
}

// ---------------------------------------------------------------------------
// Route matching
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PathSuffixMode {
    Disabled,
    #[default]
    Append,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HttpMatch {
    pub methods: Vec<HttpMethod>,
    pub path: String,
    #[serde(default)]
    pub query_allowlist: Vec<String>,
    #[serde(default)]
    pub path_suffix_mode: PathSuffixMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrpcMatch {
    pub service: String,
    pub method: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MatchRules {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub http: Option<HttpMatch>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grpc: Option<GrpcMatch>,
}

// ---------------------------------------------------------------------------
// Upstream request DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateUpstreamRequest {
    pub server: Server,
    pub protocol: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HeadersConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct UpdateUpstreamRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<Server>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alias: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HeadersConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

// ---------------------------------------------------------------------------
// Route request DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateRouteRequest {
    pub upstream_id: Uuid,
    #[serde(rename = "match")]
    pub match_rules: MatchRules,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub priority: i32,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct UpdateRouteRequest {
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "match")]
    pub match_rules: Option<MatchRules>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tags: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
}

// ---------------------------------------------------------------------------
// Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpstreamResponse {
    pub id: String,
    pub tenant_id: Uuid,
    pub alias: String,
    pub server: Server,
    pub protocol: String,
    pub enabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<HeadersConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResponse {
    pub id: String,
    pub tenant_id: Uuid,
    pub upstream_id: Uuid,
    #[serde(rename = "match")]
    pub match_rules: MatchRules,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugins: Option<PluginsConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rate_limit: Option<RateLimitConfig>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    pub priority: i32,
    pub enabled: bool,
}

// ---------------------------------------------------------------------------
// From conversions: REST value types → domain value types
// ---------------------------------------------------------------------------

impl From<SharingMode> for domain::SharingMode {
    fn from(v: SharingMode) -> Self {
        match v {
            SharingMode::Private => Self::Private,
            SharingMode::Inherit => Self::Inherit,
            SharingMode::Enforce => Self::Enforce,
        }
    }
}

impl From<Scheme> for domain::Scheme {
    fn from(v: Scheme) -> Self {
        match v {
            Scheme::Http => Self::Http,
            Scheme::Https => Self::Https,
            Scheme::Wss => Self::Wss,
            Scheme::Wt => Self::Wt,
            Scheme::Grpc => Self::Grpc,
        }
    }
}

impl From<Endpoint> for domain::Endpoint {
    fn from(v: Endpoint) -> Self {
        Self {
            scheme: v.scheme.into(),
            host: v.host,
            port: v.port,
        }
    }
}

impl From<Server> for domain::Server {
    fn from(v: Server) -> Self {
        Self {
            endpoints: v.endpoints.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<AuthConfig> for domain::AuthConfig {
    fn from(v: AuthConfig) -> Self {
        Self {
            plugin_type: v.plugin_type,
            sharing: v.sharing.into(),
            config: v.config,
        }
    }
}

impl From<PassthroughMode> for domain::PassthroughMode {
    fn from(v: PassthroughMode) -> Self {
        match v {
            PassthroughMode::None => Self::None,
            PassthroughMode::Allowlist => Self::Allowlist,
            PassthroughMode::All => Self::All,
        }
    }
}

impl From<RequestHeaderRules> for domain::RequestHeaderRules {
    fn from(v: RequestHeaderRules) -> Self {
        Self {
            set: v.set,
            add: v.add,
            remove: v.remove,
            passthrough: v.passthrough.into(),
            passthrough_allowlist: v.passthrough_allowlist,
        }
    }
}

impl From<ResponseHeaderRules> for domain::ResponseHeaderRules {
    fn from(v: ResponseHeaderRules) -> Self {
        Self {
            set: v.set,
            add: v.add,
            remove: v.remove,
        }
    }
}

impl From<HeadersConfig> for domain::HeadersConfig {
    fn from(v: HeadersConfig) -> Self {
        Self {
            request: v.request.map(Into::into),
            response: v.response.map(Into::into),
        }
    }
}

impl From<RateLimitAlgorithm> for domain::RateLimitAlgorithm {
    fn from(v: RateLimitAlgorithm) -> Self {
        match v {
            RateLimitAlgorithm::TokenBucket => Self::TokenBucket,
            RateLimitAlgorithm::SlidingWindow => Self::SlidingWindow,
        }
    }
}

impl From<Window> for domain::Window {
    fn from(v: Window) -> Self {
        match v {
            Window::Second => Self::Second,
            Window::Minute => Self::Minute,
            Window::Hour => Self::Hour,
            Window::Day => Self::Day,
        }
    }
}

impl From<SustainedRate> for domain::SustainedRate {
    fn from(v: SustainedRate) -> Self {
        Self {
            rate: v.rate,
            window: v.window.into(),
        }
    }
}

impl From<BurstConfig> for domain::BurstConfig {
    fn from(v: BurstConfig) -> Self {
        Self {
            capacity: v.capacity,
        }
    }
}

impl From<RateLimitScope> for domain::RateLimitScope {
    fn from(v: RateLimitScope) -> Self {
        match v {
            RateLimitScope::Global => Self::Global,
            RateLimitScope::Tenant => Self::Tenant,
            RateLimitScope::User => Self::User,
            RateLimitScope::Ip => Self::Ip,
            RateLimitScope::Route => Self::Route,
        }
    }
}

impl From<RateLimitStrategy> for domain::RateLimitStrategy {
    fn from(v: RateLimitStrategy) -> Self {
        match v {
            RateLimitStrategy::Reject => Self::Reject,
            RateLimitStrategy::Queue => Self::Queue,
            RateLimitStrategy::Degrade => Self::Degrade,
        }
    }
}

impl From<RateLimitConfig> for domain::RateLimitConfig {
    fn from(v: RateLimitConfig) -> Self {
        Self {
            sharing: v.sharing.into(),
            algorithm: v.algorithm.into(),
            sustained: v.sustained.into(),
            burst: v.burst.map(Into::into),
            scope: v.scope.into(),
            strategy: v.strategy.into(),
            cost: v.cost,
        }
    }
}

impl From<PluginsConfig> for domain::PluginsConfig {
    fn from(v: PluginsConfig) -> Self {
        Self {
            sharing: v.sharing.into(),
            items: v.items,
        }
    }
}

impl From<HttpMethod> for domain::HttpMethod {
    fn from(v: HttpMethod) -> Self {
        match v {
            HttpMethod::Get => Self::Get,
            HttpMethod::Post => Self::Post,
            HttpMethod::Put => Self::Put,
            HttpMethod::Delete => Self::Delete,
            HttpMethod::Patch => Self::Patch,
        }
    }
}

impl From<PathSuffixMode> for domain::PathSuffixMode {
    fn from(v: PathSuffixMode) -> Self {
        match v {
            PathSuffixMode::Disabled => Self::Disabled,
            PathSuffixMode::Append => Self::Append,
        }
    }
}

impl From<HttpMatch> for domain::HttpMatch {
    fn from(v: HttpMatch) -> Self {
        Self {
            methods: v.methods.into_iter().map(Into::into).collect(),
            path: v.path,
            query_allowlist: v.query_allowlist,
            path_suffix_mode: v.path_suffix_mode.into(),
        }
    }
}

impl From<GrpcMatch> for domain::GrpcMatch {
    fn from(v: GrpcMatch) -> Self {
        Self {
            service: v.service,
            method: v.method,
        }
    }
}

impl From<MatchRules> for domain::MatchRules {
    fn from(v: MatchRules) -> Self {
        Self {
            http: v.http.map(Into::into),
            grpc: v.grpc.map(Into::into),
        }
    }
}

// ---------------------------------------------------------------------------
// From conversions: domain value types → REST value types
// ---------------------------------------------------------------------------

impl From<domain::SharingMode> for SharingMode {
    fn from(v: domain::SharingMode) -> Self {
        match v {
            domain::SharingMode::Private => Self::Private,
            domain::SharingMode::Inherit => Self::Inherit,
            domain::SharingMode::Enforce => Self::Enforce,
        }
    }
}

impl From<domain::Scheme> for Scheme {
    fn from(v: domain::Scheme) -> Self {
        match v {
            domain::Scheme::Http => Self::Http,
            domain::Scheme::Https => Self::Https,
            domain::Scheme::Wss => Self::Wss,
            domain::Scheme::Wt => Self::Wt,
            domain::Scheme::Grpc => Self::Grpc,
        }
    }
}

impl From<domain::Endpoint> for Endpoint {
    fn from(v: domain::Endpoint) -> Self {
        Self {
            scheme: v.scheme.into(),
            host: v.host,
            port: v.port,
        }
    }
}

impl From<domain::Server> for Server {
    fn from(v: domain::Server) -> Self {
        Self {
            endpoints: v.endpoints.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<domain::AuthConfig> for AuthConfig {
    fn from(v: domain::AuthConfig) -> Self {
        Self {
            plugin_type: v.plugin_type,
            sharing: v.sharing.into(),
            config: v.config,
        }
    }
}

impl From<domain::PassthroughMode> for PassthroughMode {
    fn from(v: domain::PassthroughMode) -> Self {
        match v {
            domain::PassthroughMode::None => Self::None,
            domain::PassthroughMode::Allowlist => Self::Allowlist,
            domain::PassthroughMode::All => Self::All,
        }
    }
}

impl From<domain::RequestHeaderRules> for RequestHeaderRules {
    fn from(v: domain::RequestHeaderRules) -> Self {
        Self {
            set: v.set,
            add: v.add,
            remove: v.remove,
            passthrough: v.passthrough.into(),
            passthrough_allowlist: v.passthrough_allowlist,
        }
    }
}

impl From<domain::ResponseHeaderRules> for ResponseHeaderRules {
    fn from(v: domain::ResponseHeaderRules) -> Self {
        Self {
            set: v.set,
            add: v.add,
            remove: v.remove,
        }
    }
}

impl From<domain::HeadersConfig> for HeadersConfig {
    fn from(v: domain::HeadersConfig) -> Self {
        Self {
            request: v.request.map(Into::into),
            response: v.response.map(Into::into),
        }
    }
}

impl From<domain::RateLimitAlgorithm> for RateLimitAlgorithm {
    fn from(v: domain::RateLimitAlgorithm) -> Self {
        match v {
            domain::RateLimitAlgorithm::TokenBucket => Self::TokenBucket,
            domain::RateLimitAlgorithm::SlidingWindow => Self::SlidingWindow,
        }
    }
}

impl From<domain::Window> for Window {
    fn from(v: domain::Window) -> Self {
        match v {
            domain::Window::Second => Self::Second,
            domain::Window::Minute => Self::Minute,
            domain::Window::Hour => Self::Hour,
            domain::Window::Day => Self::Day,
        }
    }
}

impl From<domain::SustainedRate> for SustainedRate {
    fn from(v: domain::SustainedRate) -> Self {
        Self {
            rate: v.rate,
            window: v.window.into(),
        }
    }
}

impl From<domain::BurstConfig> for BurstConfig {
    fn from(v: domain::BurstConfig) -> Self {
        Self {
            capacity: v.capacity,
        }
    }
}

impl From<domain::RateLimitScope> for RateLimitScope {
    fn from(v: domain::RateLimitScope) -> Self {
        match v {
            domain::RateLimitScope::Global => Self::Global,
            domain::RateLimitScope::Tenant => Self::Tenant,
            domain::RateLimitScope::User => Self::User,
            domain::RateLimitScope::Ip => Self::Ip,
            domain::RateLimitScope::Route => Self::Route,
        }
    }
}

impl From<domain::RateLimitStrategy> for RateLimitStrategy {
    fn from(v: domain::RateLimitStrategy) -> Self {
        match v {
            domain::RateLimitStrategy::Reject => Self::Reject,
            domain::RateLimitStrategy::Queue => Self::Queue,
            domain::RateLimitStrategy::Degrade => Self::Degrade,
        }
    }
}

impl From<domain::RateLimitConfig> for RateLimitConfig {
    fn from(v: domain::RateLimitConfig) -> Self {
        Self {
            sharing: v.sharing.into(),
            algorithm: v.algorithm.into(),
            sustained: v.sustained.into(),
            burst: v.burst.map(Into::into),
            scope: v.scope.into(),
            strategy: v.strategy.into(),
            cost: v.cost,
        }
    }
}

impl From<domain::PluginsConfig> for PluginsConfig {
    fn from(v: domain::PluginsConfig) -> Self {
        Self {
            sharing: v.sharing.into(),
            items: v.items,
        }
    }
}

impl From<domain::HttpMethod> for HttpMethod {
    fn from(v: domain::HttpMethod) -> Self {
        match v {
            domain::HttpMethod::Get => Self::Get,
            domain::HttpMethod::Post => Self::Post,
            domain::HttpMethod::Put => Self::Put,
            domain::HttpMethod::Delete => Self::Delete,
            domain::HttpMethod::Patch => Self::Patch,
        }
    }
}

impl From<domain::PathSuffixMode> for PathSuffixMode {
    fn from(v: domain::PathSuffixMode) -> Self {
        match v {
            domain::PathSuffixMode::Disabled => Self::Disabled,
            domain::PathSuffixMode::Append => Self::Append,
        }
    }
}

impl From<domain::HttpMatch> for HttpMatch {
    fn from(v: domain::HttpMatch) -> Self {
        Self {
            methods: v.methods.into_iter().map(Into::into).collect(),
            path: v.path,
            query_allowlist: v.query_allowlist,
            path_suffix_mode: v.path_suffix_mode.into(),
        }
    }
}

impl From<domain::GrpcMatch> for GrpcMatch {
    fn from(v: domain::GrpcMatch) -> Self {
        Self {
            service: v.service,
            method: v.method,
        }
    }
}

impl From<domain::MatchRules> for MatchRules {
    fn from(v: domain::MatchRules) -> Self {
        Self {
            http: v.http.map(Into::into),
            grpc: v.grpc.map(Into::into),
        }
    }
}

// ---------------------------------------------------------------------------
// From conversions: REST request DTOs → domain request types
// ---------------------------------------------------------------------------

impl From<CreateUpstreamRequest> for domain::CreateUpstreamRequest {
    fn from(r: CreateUpstreamRequest) -> Self {
        Self {
            server: r.server.into(),
            protocol: r.protocol,
            alias: r.alias,
            auth: r.auth.map(Into::into),
            headers: r.headers.map(Into::into),
            plugins: r.plugins.map(Into::into),
            rate_limit: r.rate_limit.map(Into::into),
            tags: r.tags,
            enabled: r.enabled,
        }
    }
}

impl From<UpdateUpstreamRequest> for domain::UpdateUpstreamRequest {
    fn from(r: UpdateUpstreamRequest) -> Self {
        Self {
            server: r.server.map(Into::into),
            protocol: r.protocol,
            alias: r.alias,
            auth: r.auth.map(Into::into),
            headers: r.headers.map(Into::into),
            plugins: r.plugins.map(Into::into),
            rate_limit: r.rate_limit.map(Into::into),
            tags: r.tags,
            enabled: r.enabled,
        }
    }
}

impl From<CreateRouteRequest> for domain::CreateRouteRequest {
    fn from(r: CreateRouteRequest) -> Self {
        Self {
            upstream_id: r.upstream_id,
            match_rules: r.match_rules.into(),
            plugins: r.plugins.map(Into::into),
            rate_limit: r.rate_limit.map(Into::into),
            tags: r.tags,
            priority: r.priority,
            enabled: r.enabled,
        }
    }
}

impl From<UpdateRouteRequest> for domain::UpdateRouteRequest {
    fn from(r: UpdateRouteRequest) -> Self {
        Self {
            match_rules: r.match_rules.map(Into::into),
            plugins: r.plugins.map(Into::into),
            rate_limit: r.rate_limit.map(Into::into),
            tags: r.tags,
            priority: r.priority,
            enabled: r.enabled,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}
