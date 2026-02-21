# Web Search Gateway — PRD

**Unified, Provider-Agnostic Search API with Plugin-Based Extensibility**

---

## 1. Overview

**Purpose**: The Web Search Gateway is HyperSpot's centralized web search service that provides a unified API for web search operations, routing requests to configurable provider plugins while enforcing governance, normalization, and observability.

Web search integration must be consistent across all modules without embedding provider-specific logic in each consumer. The Web Search Gateway achieves this by providing a common search API and delegating provider calls to configurable plugins via the Outbound API Gateway (OAGW).

Phase 1 focuses on tenant-scoped web search with a single provider (Tavily), result normalization, and core governance (authN/Z, quotas, rate limits). It does not implement LLM-powered features (query refinement, answer extraction), additional search types (news, images), or advanced features (semantic caching, ML re-ranking); those are planned for Phase 2 and beyond.

**Target Users**: Platform engineers integrating search into AI agents and RAG pipelines. Tenant admins configuring provider selection, quotas, and policies.

**Key Problems Solved**:

- **LLM hallucinations**: Make chatbots more accurate by providing real-time web context.
- **Provider fragmentation**: Unified API abstracts differences in provider APIs, enabling seamless provider switching without consumer code changes.
- **Duplicated integration code**: A single gateway eliminates redundant provider integrations across modules.
- **Centralized governance**: Unified quotas, rate limits, and audit logging across all search consumers.

**Success Criteria**:

- All search requests resolve using tenant scope from SecurityContext; requests can specify provider or use tenant's default.
- Consumers can query enabled providers for their tenant via gateway endpoint.
- Provider plugins are discovered via types-registry and can be added without gateway code changes.

**Capabilities**:

- Tenant-scoped web search with per-request provider selection from tenant-enabled providers
- Normalized `SearchResponse` schema regardless of underlying provider
- Response caching with tenant isolation and configurable TTL
- Plugin-based provider integration via types-registry (GTS)
- Provider failover with circuit breaker pattern
- Governance: authentication, authorization, quotas, rate limiting
- Observability: metrics, structured logging, distributed tracing, audit events

---

## 2. Actors

| Actor | Description |
|-------|-------------|
| **Platform Engineer** | Integrates search into AI agents and RAG systems via the Web Search Gateway |
| **Tenant Admin** | Configures provider selection, monitors quota usage, and manages cost controls for their tenant |
| **Plugin Developer** | Implements new search provider plugins using the plugin contract, and registration process |

---

## 3. Primary Use Cases

### UC1: Basic Web Search

**Actor**: Platform Engineer (via service)  
**Preconditions**: Tenant is onboarded; at least one provider is enabled for tenant  
**Flow**:
1. Service sends `POST /web-search/v1/search` with query, parameters, and optional `provider_id`
2. Gateway authenticates request via bearer token
3. Gateway authorizes against tenant's search permissions
4. Gateway checks rate limit and quota
5. Gateway checks cache; if hit, returns cached response with `metadata.from_cache: true`
6. If cache miss:
   - If `provider_id` specified: verify provider is enabled for tenant; if not, return 400
   - If `provider_id` not specified: use tenant's configured default provider
7. Gateway invokes provider plugin, receives raw results
8. Gateway normalizes results into `SearchResponse` schema
9. Gateway caches response (if cacheable)
10. Gateway returns response with latency headers

**Postconditions**: Response logged; metrics emitted; quota decremented

### UC2: Tenant Provider Configuration

**Actor**: Tenant Admin  
**Flow**:
1. Admin queries enabled providers for their tenant via `GET /web-search/v1/providers`
2. Admin updates tenant configuration in Settings Service to:
   - Enable specific providers for the tenant (e.g., `[{"provider_id":"tavily","priority":10},{"provider_id":"serper","priority":20}]`)
   - Set a default provider (e.g., `"tavily"`)
   - Configure auto-failover behavior (enabled/disabled)
3. Subsequent requests from that tenant can specify any enabled provider via `provider_id`
4. Requests without `provider_id` use the tenant's default provider

### UC2a: Per-Request Provider Selection

**Actor**: Platform Engineer (via service)  
**Preconditions**: Tenant has multiple providers enabled  
**Flow**:
1. Service queries enabled providers via `GET /web-search/v1/providers`
2. Service sends search request with `provider_id: "serper"` for a specific use case
3. Gateway verifies "serper" is enabled for the tenant
4. Gateway routes request to Serper plugin
5. Response includes `metadata.provider_used: "serper"`

### UC3: Provider Failover

**Actor**: System (automatic)  
**Trigger**: Requested provider returns 5xx, 429, or times out  
**Preconditions**: Tenant has auto-failover enabled; tenant has other providers enabled  
**Flow**:
1. Gateway detects failure condition on requested provider
2. Circuit breaker opens for failed provider
3. Gateway selects another enabled provider from tenant's list based on plugin priority
4. Gateway retries request with fallback provider
5. Response includes `metadata.provider_used` (fallback)
6. Metrics record failover event

**Alternative Flow** (auto-failover disabled):
1. Gateway detects failure condition
2. Gateway returns error response with Problem Details
3. Consumer can retry with different `provider_id` if desired

### UC4: Quota Exhaustion

**Actor**: Platform Engineer (via service)  
**Trigger**: Tenant quota exceeded  
**Flow**:
1. Request arrives; gateway checks quota
2. Quota check fails; request rejected with `429 Too Many Requests`
3. Response includes `Retry-After` header and Problem Details body
4. Metrics record quota denial

### UC5: Adding a New Provider Plugin

**Actor**: Plugin Developer  
**Flow**:
1. Plugin developer implements `WebSearchPluginClient` trait
2. Plugin developer registers plugin instance in types-registry with capability declaration
3. Gateway discovers new plugin on next startup or config reload
4. Tenant Admin enables new provider for specific tenants
5. Gateway routes matching requests to new provider

### UC6: LLM-Enhanced Search with Answer Extraction

**Actor**: AI Chatbot (via service)  
**Preconditions**: LLM Gateway available; tenant has LLM features enabled  
**Flow**:
1. Chatbot sends search request with `options.extract_answer: true`
2. Gateway executes web search via provider
3. Gateway invokes LLM Gateway to extract direct answer from top results
4. Response includes `answer` object with concise text, source URLs, and confidence
5. Chatbot uses extracted answer to ground its response, reducing hallucination risk

**Postconditions**: Answer extraction logged; LLM cost tracked

### UC7: Query Refinement for Ambiguous Searches

**Actor**: Platform Engineer (via service)  
**Flow**:
1. Service sends search with `options.refine_query: true`
2. Gateway invokes LLM to refine query (spelling correction, expansion, intent detection)
3. Gateway searches with refined query
4. Response includes original and refined query in metadata
5. Improved search results returned

---

## 4. Scope

### 4.1 In Scope

| Category | Items |
|----------|-------|
| **Search modalities** | Web search (MVP); news, image, video, academic as extensions |
| **Provider plugins** | Plugin architecture with stable contract; initial plugins: Tavily |
| **Multi-tenancy** | Per-tenant provider config, quotas, rate limits, cache policies |
| **API** | REST API with OpenAPI documentation; gRPC MAY be added later |
| **Normalization** | Unified `SearchResponse` schema for all providers |
| **Caching** | Response caching with configurable TTL, tenant isolation |
| **Governance** | AuthN (bearer tokens, service-to-service), AuthZ, quotas, rate limits |
| **Observability** | Metrics, structured logs, distributed tracing, audit events |
| **Error handling** | RFC 9457 Problem Details for all error responses |
| **Admin operations** | Provider health status, cache invalidation, quota management |
| **LLM integration** | Query refinement, answer extraction, result summarization (opt-in via LLM Gateway) |
| **Safe search** | Configurable content filtering (off, moderate, strict) with tenant-level enforcement |

### 4.2 Out of Scope

| Item | Rationale |
|------|-----------|
| Web crawling / scraping | Gateway is for Search API execution, not for deep scraping or crawling |
| Search result storage | Gateway is stateless for results; caching is ephemeral |
| Billing integration | Cost metrics are emitted; billing is a separate platform concern |
| BYO-Key Management | Users do not supply their own API keys; keys are managed centrally by the platform  |

---

## 5. Functional Requirements

| ID | Priority | Category | Requirement | Rationale | Acceptance Criteria |
|----|----------|----------|-------------|-----------|---------------------|
| **FR-001** | P0 | API | Gateway MUST provide a unified `Search` endpoint accepting `SearchRequest` and returning `SearchResponse` | Single entry point for all search operations | Endpoint returns 200 with valid `SearchResponse` for valid requests |
| **FR-002** | P0 | Routing | Gateway MUST route requests to provider based on: (1) `provider_id` in request if specified, (2) tenant's default provider if not specified; Gateway MUST verify requested provider is enabled for tenant | Per-request provider flexibility with tenant governance | Request with valid `provider_id` routes to that provider; request with invalid/disabled provider returns 400; request without `provider_id` uses tenant default |
| **FR-003** | P0 | Discovery | Gateway MUST expose a provider discovery endpoint returning list of providers enabled for the requesting tenant | Consumer awareness of available providers | Endpoint returns array of provider objects with `id`, `name`, `capabilities`; list filtered to tenant's enabled providers |
| **FR-004** | P0 | Normalization | Gateway MUST transform all provider responses into a unified response schema | Provider-agnostic responses for consumers | All fields populated correctly regardless of provider; provider-specific data isolated |
| **FR-005** | P0 | AuthN | Gateway MUST authenticate all requests | Security: no anonymous access | Requests without valid auth return 401; valid tokens extract tenant and user context |
| **FR-006** | P0 | AuthZ | Gateway MUST authorize requests against tenant's search permissions | Enforce access control | Unauthorized requests return 403; authorized requests proceed |
| **FR-007** | P0 | Rate Limiting | Gateway MUST enforce per-tenant rate limits | Protect providers and ensure fair usage | Rate limit hit returns 429 with retry information |
| **FR-008** | P0 | Quota | Gateway MUST track and enforce per-tenant quotas (requests per period) | Cost control and fair usage | Quota exhaustion returns error; quota resets at configured cadence |
| **FR-009** | P0 | Errors | Gateway MUST return consistent, machine-readable error responses | Uniform error handling for consumers | All error responses follow consistent structure with error code, message, and details |
| **FR-010** | P0 | Failover | Gateway MUST support configurable auto-failover per tenant; when enabled and provider fails, gateway attempts another enabled provider; when disabled, gateway returns error | Tenant control over failover behavior | Auto-failover enabled: failure triggers fallback; Auto-failover disabled: failure returns error |
| **FR-011** | P1 | Caching | Gateway SHOULD cache responses with tenant isolation and configurable TTL | Cost reduction, latency improvement | Identical requests return cached response; cache isolated per tenant; TTL configurable |
| **FR-012** | P1 | Caching | Gateway SHOULD support cache bypass on request | Allow fresh results when needed | Cache bypass returns fresh provider response |
| **FR-013** | P1 | Observability | Gateway SHOULD emit structured logs with correlation IDs for all requests | Debugging and audit | Logs contain correlation ID, tenant ID, provider used, latency; PII redacted |
| **FR-014** | P1 | Observability | Gateway SHOULD support distributed tracing | End-to-end request tracing | Trace context propagated to provider calls; spans visible in tracing backend |
| **FR-015** | P1 | Audit | Gateway SHOULD emit audit events for: config changes, quota/rate actions, provider selection | Compliance and incident investigation | Audit events contain timestamp, actor, action, resource, outcome |
| **FR-016** | P1 | Admin | Gateway SHOULD expose health and cache management endpoints for operators | Operational visibility and control | Health status per provider; cache invalidation capability |
| **FR-017** | P1 | Search Types | Gateway SHOULD support news search with freshness, category, and sort parameters | Real-time information for chatbots | News search returns results with freshness filter; category filtering works |
| **FR-018** | P1 | Search Types | Gateway SHOULD support image search with size, color, type, and license filters | Content discovery use cases | Image results include thumbnails, dimensions, license info |
| **FR-019** | P1 | Search Types | Gateway SHOULD support video search with duration and quality filters | Video research use cases | Video results include duration, quality, platform info |
| **FR-020** | P1 | Search Types | Gateway SHOULD support academic/scholarly search with publication type, year range, and citation count | Research and fact-checking | Academic results include citation count, publication type, authors |
| **FR-021** | P1 | Safe Search | Gateway SHOULD enforce safe search filtering with configurable levels (off, moderate, strict) per tenant | Content safety and compliance | Safe search level enforced; platform-wide minimum level can override tenant settings |
| **FR-022** | P1 | LLM Integration | Gateway SHOULD support optional LLM-based query refinement (spelling correction, query expansion, intent detection) | Improve search quality | Refinement opt-in per request; refined query logged in metadata; original preserved |
| **FR-023** | P1 | LLM Integration | Gateway SHOULD support LLM-based answer extraction from search results | Reduce hallucinations in AI chatbots | Answer includes text, type (fact/definition/summary), source URL, confidence score |
| **FR-024** | P1 | LLM Integration | Gateway SHOULD support LLM-based summarization of top search results | Concise context for RAG pipelines | Summary includes configurable length, source URLs |
| **FR-025** | P1 | Aggregation | Gateway SHOULD support multi-provider aggregation with deduplication and score merging | Comprehensive result coverage | Aggregation opt-in per request; results deduplicated by URL; provider source tracked per result |
| **FR-026** | P1 | Filters | Gateway SHOULD support comprehensive filtering: domain include/exclude, file type, freshness, date range | Refined search results | Filters mapped to provider syntax; unsupported filters indicated in response metadata |
| **FR-027** | P1 | Provider Health | Gateway SHOULD track provider health and availability to inform routing decisions | Intelligent provider selection | Health includes error rates, response time trends, rate limit status, circuit breaker state |
| **FR-028** | P1 | Cost Tracking | Gateway SHOULD track actual monetary costs per provider with budget alerts | Cost visibility and control | Cost per query by provider tracked; monthly totals per tenant available; alerts on thresholds |
| **FR-029** | P1 | Streaming | Gateway SHOULD support streaming search results as they become available | Improved perceived latency | SSE endpoint returns results incrementally; final response aggregates all |
| **FR-030** | P1 | Cache Warming | Gateway SHOULD support pre-populating cache with common/trending queries | Improve cache hit rates | Admin API accepts query list for warming; warming jobs logged |
| **FR-031** | P1 | Re-ranking | Gateway SHOULD support re-ranking search results based on tenant preferences, quality signals, and recency | Result quality improvement | Re-ranking configurable per tenant; ranking factors logged |
| **FR-032** | P1 | History | Gateway SHOULD support configurable retention of search history for analytics (default 30 days) | Usage analytics and quality improvement | History queryable by tenant admin; retention configurable; PII rules applied |
| **FR-033** | P1 | Export | Gateway SHOULD support exporting search results in multiple formats (JSON, CSV) | Offline analysis | Export endpoint accepts result set; formats validated |
| **FR-034** | P2 | MCP Tool | Gateway MAY expose search as an MCP tool interface for LLM function calling | LLM agent integration | MCP tool schema published; tool callable by LLM agents |
| **FR-035** | P2 | Semantic Cache | Gateway MAY support embeddings for semantic caching and result enrichment | Improve cache hit rates for paraphrased queries; enable downstream semantic operations | Similar queries return cached results; embeddings optionally returned in response |
| **FR-036** | P2 | Pagination | Gateway MAY support pagination for large result sets via `offset` parameter | Navigate through large result sets | Request accepts `offset`; response includes `has_more` indicator |

---

## 6. Non-Functional Requirements (NFRs)

### 6.1 Performance

| Metric | Target | Measurement |
|--------|--------|-------------|
| **p95 latency** | ≤800ms (excluding provider latency) | Gateway processing time only |
| **p99 latency** | ≤1200ms (excluding provider latency) | Gateway processing time only |

---
## 7. Gateway Constraints

### 7.1 Multi-Tenancy Requirements

| Constraint | Description |
|------------|-------------|
| **Tenant isolation** | Tenant A's configuration, cache, quotas MUST NOT affect Tenant B |
| **Provider enablement** | Tenant Admin enables specific providers from the platform-wide available list; requests can only use enabled providers |

### 7.2 Provider Contract Constraints

| Constraint | Description |
|------------|-------------|
| **Capability mismatch** | If request requires capability provider lacks, return 400 with `capability_not_supported` error |


---