# Feature: Streaming & Header Transformation

- [ ] `p2` - **ID**: `cpt-cf-oagw-featstatus-streaming-headers`

- [ ] `p2` - `cpt-cf-oagw-feature-streaming-headers`

## 1. Feature Context

### 1.1 Overview

Header transformation pipeline (set/add/remove, hop-by-hop stripping, passthrough control) and streaming support for HTTP request/response, Server-Sent Events (SSE), WebSocket, and WebTransport session flows with connection lifecycle management.

### 1.2 Purpose

Many external APIs (especially AI providers like OpenAI) use streaming responses. OAGW must transparently proxy these without buffering. Header transformation enables adapting requests/responses to upstream API requirements and enforcing security policies on headers.

Addresses PRD requirements: `cpt-cf-oagw-fr-header-transform`, `cpt-cf-oagw-fr-streaming`.

### 1.3 Actors

| Actor | Role in Feature |
|-------|-----------------|
| `cpt-cf-oagw-actor-platform-operator` | Configures header transformation rules on upstreams |
| `cpt-cf-oagw-actor-tenant-admin` | Configures tenant-specific header rules |
| `cpt-cf-oagw-actor-app-developer` | Sends requests that may require streaming responses (SSE, WebSocket, WebTransport) |
| `cpt-cf-oagw-actor-upstream-service` | External service that produces streaming responses |

### 1.4 References

- **PRD**: [PRD.md](../PRD.md)
- **Design**: [DESIGN.md](../DESIGN.md)
- **Requirements**: `cpt-cf-oagw-fr-header-transform`, `cpt-cf-oagw-fr-streaming`
- **Design elements**: `cpt-cf-oagw-design-oagw`, `cpt-cf-oagw-adr-request-routing`
- **Dependencies**: `cpt-cf-oagw-feature-proxy-auth` (header transformation and streaming operate within the proxy pipeline)

## 2. Actor Flows (CDSL)

### SSE Streaming Proxy Flow

- [ ] `p2` - **ID**: `cpt-cf-oagw-flow-sse-proxy`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Success Scenarios**:
- SSE events from upstream are forwarded to client as received without buffering
- Connection lifecycle (open, data, error, close) is managed correctly

**Error Scenarios**:
- Upstream connection drops: close event propagated to client
- Client disconnects: upstream connection is closed, resources released
- Upstream timeout during SSE: 504 returned

**Steps**:
1. [ ] - `p2` - Application sends request to proxy endpoint (upstream responds with Content-Type: text/event-stream) - `inst-sse-1`
2. [ ] - `p2` - Proxy pipeline resolves upstream, matches route, applies auth and plugins - `inst-sse-2`
3. [ ] - `p2` - Establish connection to upstream service - `inst-sse-3`
4. [ ] - `p2` - Detect SSE response (Content-Type: text/event-stream) - `inst-sse-4`
5. [ ] - `p2` - Begin streaming: forward SSE events to client as received - `inst-sse-5`
6. [ ] - `p2` - **IF** upstream sends close event or connection drops - `inst-sse-6`
   1. [ ] - `p2` - Propagate close to client - `inst-sse-6a`
   2. [ ] - `p2` - Release resources (rate limit permits, in-flight counters) - `inst-sse-6b`
7. [ ] - `p2` - **IF** client disconnects - `inst-sse-7`
   1. [ ] - `p2` - Abort upstream request - `inst-sse-7a`
   2. [ ] - `p2` - Release resources - `inst-sse-7b`

### WebSocket Proxy Flow

- [ ] `p2` - **ID**: `cpt-cf-oagw-flow-websocket-proxy`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Success Scenarios**:
- WebSocket upgrade is proxied to upstream
- Bi-directional messages are forwarded in both directions
- Connection close is propagated cleanly

**Error Scenarios**:
- Upstream rejects WebSocket upgrade
- Idle timeout exceeded: connection closed
- Client or upstream abruptly disconnects

**Steps**:
1. [ ] - `p2` - Application sends WebSocket upgrade request to proxy endpoint - `inst-ws-1`
2. [ ] - `p2` - Proxy pipeline resolves upstream, matches route, applies auth on handshake - `inst-ws-2`
3. [ ] - `p2` - Forward upgrade request to upstream - `inst-ws-3`
4. [ ] - `p2` - **IF** upstream accepts upgrade - `inst-ws-4`
   1. [ ] - `p2` - Establish bi-directional WebSocket relay - `inst-ws-4a`
   2. [ ] - `p2` - Forward messages in both directions - `inst-ws-4b`
5. [ ] - `p2` - **IF** upstream rejects upgrade - `inst-ws-5`
   1. [ ] - `p2` - **RETURN** upstream rejection response to client - `inst-ws-5a`
6. [ ] - `p2` - **IF** idle timeout exceeded - `inst-ws-6`
   1. [ ] - `p2` - Send close frame to both sides - `inst-ws-6a`
   2. [ ] - `p2` - Release resources - `inst-ws-6b`
7. [ ] - `p2` - **IF** either side disconnects - `inst-ws-7`
   1. [ ] - `p2` - Propagate close to the other side - `inst-ws-7a`
   2. [ ] - `p2` - Release resources - `inst-ws-7b`

### WebTransport Session Flow

- [ ] `p2` - **ID**: `cpt-cf-oagw-flow-webtransport-proxy`

**Actor**: `cpt-cf-oagw-actor-app-developer`

**Success Scenarios**:
- WebTransport session is established and forwarded to upstream
- Session lifecycle is managed with bounded idle semantics

**Error Scenarios**:
- Upstream does not support WebTransport
- Session idle timeout exceeded

**Steps**:
1. [ ] - `p2` - Application initiates WebTransport session to proxy endpoint - `inst-wt-1`
2. [ ] - `p2` - Proxy pipeline resolves upstream, applies auth at session establishment - `inst-wt-2`
3. [ ] - `p2` - Forward session establishment to upstream - `inst-wt-3`
4. [ ] - `p2` - **IF** upstream accepts session - `inst-wt-4`
   1. [ ] - `p2` - Relay streams and datagrams bi-directionally - `inst-wt-4a`
5. [ ] - `p2` - **IF** idle timeout exceeded or either side closes - `inst-wt-5`
   1. [ ] - `p2` - Terminate session, release resources - `inst-wt-5a`

### Configure Header Transformation Flow

- [ ] `p2` - **ID**: `cpt-cf-oagw-flow-configure-headers`

**Actor**: `cpt-cf-oagw-actor-platform-operator`

**Success Scenarios**:
- Header transformation rules are saved on upstream configuration
- Rules are applied to all subsequent proxy requests for that upstream

**Error Scenarios**:
- Invalid header name or value format (400)

**Steps**:
1. [ ] - `p2` - Operator includes `headers` configuration in upstream create/update request - `inst-ch-1`
2. [ ] - `p2` - Validate header transformation rules (valid header names, no CR/LF in values) - `inst-ch-2`
3. [ ] - `p2` - DB: persist headers configuration in oagw_upstream.headers JSONB column - `inst-ch-3`
4. [ ] - `p2` - **RETURN** updated upstream with headers configuration - `inst-ch-4`

## 3. Processes / Business Logic (CDSL)

### Header Transformation Algorithm

- [ ] `p2` - **ID**: `cpt-cf-oagw-algo-header-transform`

**Input**: Inbound request headers, upstream.headers configuration, route configuration

**Output**: Transformed outbound headers

**Steps**:
1. [ ] - `p2` - Start with inbound request headers - `inst-ht-1`
2. [ ] - `p2` - Strip routing headers: remove X-OAGW-Target-Host (consumed by routing) - `inst-ht-2`
3. [ ] - `p2` - Strip hop-by-hop headers: Connection, Keep-Alive, Proxy-Authenticate, Proxy-Authorization, TE, Trailer, Transfer-Encoding, Upgrade - `inst-ht-3`
4. [ ] - `p2` - Replace Host header with upstream endpoint host (HTTP/1.1) or :authority pseudo-header (HTTP/2) - `inst-ht-4`
5. [ ] - `p2` - Apply upstream.headers configured transforms in order - `inst-ht-5`
   1. [ ] - `p2` - **FOR EACH** rule in upstream.headers - `inst-ht-5a`
      1. [ ] - `p2` - **IF** action = set: overwrite header value - `inst-ht-5a1`
      2. [ ] - `p2` - **IF** action = add: append header value (multi-value) - `inst-ht-5a2`
      3. [ ] - `p2` - **IF** action = remove: delete header - `inst-ht-5a3`
6. [ ] - `p2` - Validate well-known headers: reject invalid Content-Length, multiple Host headers, CR/LF in values - `inst-ht-6`
7. [ ] - `p2` - **IF** validation fails - `inst-ht-7`
   1. [ ] - `p2` - **RETURN** 400 Bad Request - `inst-ht-7a`
8. [ ] - `p2` - **RETURN** transformed headers - `inst-ht-8`

### Streaming Response Detection Algorithm

- [ ] `p2` - **ID**: `cpt-cf-oagw-algo-stream-detection`

**Input**: Upstream response headers

**Output**: Stream type (none, sse, websocket, webtransport) and streaming mode

**Steps**:
1. [ ] - `p2` - **IF** Content-Type = text/event-stream - `inst-sd-1`
   1. [ ] - `p2` - **RETURN** stream type = sse - `inst-sd-1a`
2. [ ] - `p2` - **IF** response is WebSocket upgrade (101 Switching Protocols with Upgrade: websocket) - `inst-sd-2`
   1. [ ] - `p2` - **RETURN** stream type = websocket - `inst-sd-2a`
3. [ ] - `p2` - **IF** response is WebTransport session establishment - `inst-sd-3`
   1. [ ] - `p2` - **RETURN** stream type = webtransport - `inst-sd-3a`
4. [ ] - `p2` - **RETURN** stream type = none (standard HTTP response, still streamed without buffering) - `inst-sd-4`

### Stream Lifecycle Management Algorithm

- [ ] `p2` - **ID**: `cpt-cf-oagw-algo-stream-lifecycle`

**Input**: Active stream connection, stream type, idle timeout configuration

**Output**: Clean stream termination with resource cleanup

**Steps**:
1. [ ] - `p2` - Track stream start time and last activity timestamp - `inst-sl-1`
2. [ ] - `p2` - **IF** stream type = sse - `inst-sl-2`
   1. [ ] - `p2` - Forward events as received; update last_activity on each event - `inst-sl-2a`
3. [ ] - `p2` - **IF** stream type = websocket - `inst-sl-3`
   1. [ ] - `p2` - Relay messages bi-directionally; update last_activity on each message - `inst-sl-3a`
4. [ ] - `p2` - **IF** stream type = webtransport - `inst-sl-4`
   1. [ ] - `p2` - Relay streams and datagrams; update last_activity - `inst-sl-4a`
5. [ ] - `p2` - **IF** now - last_activity > idle_timeout - `inst-sl-5`
   1. [ ] - `p2` - Initiate graceful close on both sides - `inst-sl-5a`
6. [ ] - `p2` - On termination (any cause): release rate limit permits, decrement in-flight counters, close connections - `inst-sl-6`
7. [ ] - `p2` - **IF** stream aborted unexpectedly - `inst-sl-7`
   1. [ ] - `p2` - Map to StreamAborted error with X-OAGW-Error-Source: gateway or upstream as appropriate - `inst-sl-7a`

## 4. States (CDSL)

### Stream Connection State Machine

- [ ] `p2` - **ID**: `cpt-cf-oagw-state-stream-connection`

**States**: establishing, active, draining, closed

**Initial State**: establishing

**Transitions**:
1. [ ] - `p2` - **FROM** establishing **TO** active **WHEN** upstream accepts connection/upgrade/session - `inst-sc-1`
2. [ ] - `p2` - **FROM** establishing **TO** closed **WHEN** upstream rejects or connection timeout - `inst-sc-2`
3. [ ] - `p2` - **FROM** active **TO** draining **WHEN** either side initiates close or idle timeout exceeded - `inst-sc-3`
4. [ ] - `p2` - **FROM** active **TO** closed **WHEN** abrupt disconnect (no graceful close) - `inst-sc-4`
5. [ ] - `p2` - **FROM** draining **TO** closed **WHEN** both sides acknowledge close - `inst-sc-5`

**Effects**:
- establishing: auth and plugin chain executing; no data forwarded yet
- active: data flowing bi-directionally; idle timeout monitored
- draining: close propagating; in-flight data completing; no new data accepted
- closed: all resources released; metrics updated

## 5. Definitions of Done

### Implement Header Transformation

- [ ] `p2` - **ID**: `cpt-cf-oagw-dod-header-transform`

The system **MUST** support request and response header transformation: set (overwrite), add (append), and remove operations configured via upstream.headers JSONB. The system **MUST** strip hop-by-hop headers by default. The system **MUST** replace Host/`:authority` with upstream host. The system **MUST** reject invalid header names/values (CR/LF, obs-fold, multiple Host).

**Implements**:
- `cpt-cf-oagw-flow-configure-headers`
- `cpt-cf-oagw-algo-header-transform`

**Touches**:
- API: `PUT /api/oagw/v1/upstreams/{id}`
- DB: `oagw_upstream.headers`
- Entities: `DataPlaneService`

### Implement SSE Streaming Proxy

- [ ] `p2` - **ID**: `cpt-cf-oagw-dod-sse-streaming`

The system **MUST** support SSE streaming by forwarding events as received without buffering. On client disconnect, the system **MUST** abort the upstream request and release resources. On upstream connection drop, the system **MUST** propagate close to the client.

**Implements**:
- `cpt-cf-oagw-flow-sse-proxy`
- `cpt-cf-oagw-algo-stream-detection`
- `cpt-cf-oagw-algo-stream-lifecycle`

**Touches**:
- API: `{METHOD} /api/oagw/v1/proxy/{alias}/*`
- Entities: `DataPlaneService`

### Implement WebSocket Proxy

- [ ] `p2` - **ID**: `cpt-cf-oagw-dod-websocket-proxy`

The system **MUST** support WebSocket proxying: upgrade handshake, bi-directional message relay, auth on handshake, idle timeout enforcement, and close propagation.

**Implements**:
- `cpt-cf-oagw-flow-websocket-proxy`
- `cpt-cf-oagw-state-stream-connection`

**Touches**:
- API: `{METHOD} /api/oagw/v1/proxy/{alias}/*`
- Entities: `DataPlaneService`

### Implement WebTransport Session Proxy

- [ ] `p2` - **ID**: `cpt-cf-oagw-dod-webtransport-proxy`

The system **MUST** support WebTransport session forwarding with auth at session establishment and bounded idle semantics.

**Implements**:
- `cpt-cf-oagw-flow-webtransport-proxy`
- `cpt-cf-oagw-state-stream-connection`

**Touches**:
- API: `{METHOD} /api/oagw/v1/proxy/{alias}/*`
- Entities: `DataPlaneService`

### Implement Request/Response Body Streaming

- [ ] `p2` - **ID**: `cpt-cf-oagw-dod-body-streaming`

The system **MUST** stream request bodies to upstream and response bodies to client without full buffering (backpressure-safe). Large responses and SSE streams **MUST** be forwarded incrementally.

**Implements**:
- `cpt-cf-oagw-flow-sse-proxy`
- `cpt-cf-oagw-algo-stream-lifecycle`

**Touches**:
- Entities: `DataPlaneService`, HTTP client

## 6. Acceptance Criteria

- [ ] Hop-by-hop headers (Connection, Keep-Alive, Proxy-Authenticate, etc.) are stripped from outbound requests
- [ ] Host header is replaced with upstream host; :authority pseudo-header replaced in HTTP/2
- [ ] X-OAGW-Target-Host is consumed by routing and stripped (not forwarded to upstream)
- [ ] Configured header transforms (set/add/remove) are applied in order
- [ ] Invalid header names/values (CR/LF, obs-fold) are rejected with 400
- [ ] Multiple Host headers are rejected with 400
- [ ] SSE responses (text/event-stream) are forwarded event-by-event without buffering
- [ ] Client disconnect during SSE aborts upstream request and releases resources
- [ ] Upstream disconnect during SSE propagates close to client
- [ ] WebSocket upgrade is proxied to upstream; bi-directional messages are relayed
- [ ] WebSocket idle timeout closes connection on both sides
- [ ] WebTransport session is established and relayed with bounded idle semantics
- [ ] Request bodies are streamed to upstream without full buffering
- [ ] Response bodies are streamed to client without full buffering
- [ ] Stream abort maps to StreamAborted error with correct X-OAGW-Error-Source

## 7. Not Applicable Sections

**Compliance (COMPL)**: Not applicable because OAGW does not inspect or store streaming content. Content compliance is the responsibility of upstream services and calling modules.

**Privacy (COMPL)**: Not applicable because OAGW proxies streams without inspecting bodies. PII handling is the responsibility of calling modules and upstream services.

**User-Facing Architecture (UX)**: Not applicable because this is a backend proxy capability with no user-facing frontend.
