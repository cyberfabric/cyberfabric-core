# ADR: gRPC Support

**ID**: `cpt-cf-oagw-adr-grpc-support`

- **Status**: Proposed
- **Date**: 2026-02-03
- **Deciders**: OAGW Team

## Context and Problem Statement

OAGW currently supports HTTP/1.1 requests. Modern APIs increasingly use gRPC for efficient service-to-service communication. OAGW needs to proxy gRPC requests to upstream services
while maintaining the same routing, authentication, and policy enforcement as HTTP.

**Key challenges**:

- gRPC uses HTTP/2 exclusively
- gRPC requires specific headers (`content-type: application/grpc`)
- gRPC uses bidirectional streaming
- Protocol detection needed when HTTP and gRPC share same port

## Decision Drivers

- Minimize infrastructure complexity (avoid multiple ports)
- Transparent proxying without breaking gRPC semantics
- Support all gRPC patterns (unary, client streaming, server streaming, bidirectional)
- Reuse existing auth/routing/rate-limiting infrastructure
- Performance (minimal overhead)
- Protocol detection reliability

## Considered Options

### Option 1: Separate Port (gRPC-only)

Dedicate separate port (e.g., 50051) exclusively for gRPC traffic.

```
Client → :443 (HTTP/REST)
Client → :50051 (gRPC only)
         ↓
      OAGW → Upstream
```

**Configuration**:

```json
{
  "server": {
    "http_port": 443,
    "grpc_port": 50051
  }
}
```

**Pros**:

- Simple implementation (no protocol detection)
- Clear separation of concerns
- No ambiguity about protocol
- Easy to configure separate TLS settings

**Cons**:

- Extra port management (firewall rules, load balancer config)
- Clients must know which port to use
- Doesn't work well with API gateways that expect single endpoint
- Incompatible with many cloud environments (single ingress port)

### Option 2: Connection Hijacking (Port 443, Protocol Detection)

Single port handles both HTTP/1.1 and gRPC (HTTP/2). Detect protocol during TLS handshake via ALPN or first request bytes.

```
Client → :443
         ↓
   TLS Handshake (ALPN: h2)
         ↓
   Protocol Detection
         ├─ h2 + content-type: application/grpc → gRPC handler
         └─ h2/http/1.1 → HTTP handler
         ↓
      OAGW → Upstream
```

**Implementation**:

```rust
async fn handle_connection(stream: TcpStream, tls_acceptor: TlsAcceptor) {
    let tls_stream = tls_acceptor.accept(stream).await?;
    
    match tls_stream.negotiated_alpn_protocol() {
        Some(b"h2") => {
            // Read first frame to check content-type
            let first_bytes = peek_first_request_header(&tls_stream).await?;
            
            if is_grpc_content_type(&first_bytes) {
                handle_grpc_request(tls_stream).await
            } else {
                handle_http2_request(tls_stream).await
            }
        }
        Some(b"http/1.1") | None => {
            handle_http1_request(tls_stream).await
        }
        _ => return Err("Unsupported protocol")
    }
}

fn is_grpc_content_type(headers: &[u8]) -> bool {
    // Parse HPACK-encoded headers, check content-type: application/grpc
    parse_http2_headers(headers)
        .get("content-type")
        .map(|v| v.starts_with("application/grpc"))
        .unwrap_or(false)
}
```

**Pros**:

- Single port (443) for all traffic
- Works with standard cloud load balancers
- Transparent to clients (just use gRPC client libraries)
- Industry standard (Envoy, Linkerd, Istio all do this)
- Better for Kubernetes ingress

**Cons**:

- Complex protocol detection logic
- Small overhead for first request analysis
- Requires HTTP/2 support in OAGW core
- Edge cases with protocol misdetection

### Option 3: HTTP/2 with gRPC Multiplexing (Recommended)

Single port, native HTTP/2 server with automatic gRPC detection via `content-type` header.

```rust
// Use hyper with HTTP/2 support
use hyper::server::conn::http2;

async fn handle_request(req: Request<Body>) -> Response<Body> {
    let is_grpc = req.headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.starts_with("application/grpc"))
        .unwrap_or(false);
    
    if is_grpc {
        route_grpc_request(req).await
    } else {
        route_http_request(req).await
    }
}
```

**Configuration** (unified):

```json
{
  "server": {
    "port": 443,
    "protocols": ["http/1.1", "h2", "h2c"],
    "grpc_enabled": true
  },
  "upstream": {
    "protocol": "gts.x.core.oagw.protocol.v1~x.core.oagw.grpc.v1",
    "server": {
      "endpoints": [
        {"scheme": "grpc", "host": "grpc-service.example.com", "port": 50051}
      ]
    }
  }
}
```

**Pros**:

- Single port, single TLS config
- Native HTTP/2 (gRPC is just HTTP/2 with specific headers)
- ALPN negotiation handled by TLS library
- Simple routing: check content-type header
- Works with all gRPC patterns (streaming included)
- Standard approach (matches Envoy, Istio)

**Cons**:

- Must implement HTTP/2 server (but hyper supports this)
- All OAGW nodes must support HTTP/2

## Comparison Matrix

| Criteria                  | Option 1 (Separate Port) | Option 2 (Hijacking) | Option 3 (Multiplexing) |
|---------------------------|:------------------------:|:--------------------:|:-----------------------:|
| Single ingress point      |            No            |         Yes          |           Yes           |
| Implementation complexity |           Low            |         High         |         Medium          |
| Protocol detection        |        Not needed        |    ALPN + peeking    |    Content-type hdr     |
| Works with cloud LB       |         Partial          |         Yes          |           Yes           |
| HTTP/2 requirement        |         Optional         |      Mandatory       |        Mandatory        |
| Performance overhead      |         Minimal          |  Small (detection)   |         Minimal         |
| Kubernetes-friendly       |            No            |         Yes          |           Yes           |
| Streaming support         |           Full           |         Full         |          Full           |

## Decision Outcome

**Chosen**: Option 3 (HTTP/2 with gRPC Multiplexing)

**Rationale**:

1. **Standard approach**: Envoy, Istio, Linkerd all use HTTP/2 multiplexing. Battle-tested pattern.
2. **Simple detection**: `content-type: application/grpc*` header check is reliable and lightweight.
3. **Single endpoint**: Simplifies deployment, firewall rules, TLS management.
4. **Native HTTP/2**: gRPC is HTTP/2. Native support avoids protocol translation hacks.
5. **Cloud native**: Works seamlessly with Kubernetes ingress, cloud load balancers.

**Trade-offs accepted**:

- All OAGW nodes must support HTTP/2 (modern Rust stacks do)
- Slightly more complex than HTTP/1.1-only (but hyper handles this)

Options 1 and 2 rejected:

- Option 1: Multiple ports add operational complexity, poor cloud integration
- Option 2: Hijacking adds unnecessary complexity for same outcome as Option 3

### Consequences

**Positive**:

- Single ingress point for all traffic (HTTP and gRPC)
- Simple and reliable protocol detection via content-type header
- Cloud-native deployment (works with Kubernetes, cloud LBs)
- Full streaming support for all gRPC patterns

**Negative**:

- All OAGW nodes must support HTTP/2
- Slightly more complex than HTTP/1.1-only

### Confirmation

Confirmation will be achieved through prototype validation:

- gRPC health check (`grpc.health.v1.Health/Check`) works end-to-end through OAGW
- HTTP/1.1 REST requests coexist on the same port without interference
- Bidirectional streaming works without timeouts
- Rate limiting applies to gRPC requests

## Pros and Cons of the Options

### Option 1: Separate Port

- **Good**: Simple implementation, no protocol detection, clear separation
- **Bad**: Extra port management, incompatible with many cloud environments, clients must know which port

### Option 2: Connection Hijacking

- **Good**: Single port, works with cloud LBs, transparent to clients
- **Bad**: Complex protocol detection, small overhead, edge cases with misdetection

### Option 3: HTTP/2 with gRPC Multiplexing

- **Good**: Single port, native HTTP/2, simple content-type detection, standard approach
- **Bad**: Requires HTTP/2 support on all nodes

## Implementation Notes

### Route Matching for gRPC

gRPC routes use service/method instead of HTTP path:

```json
{
  "match": {
    "grpc": {
      "service": "example.v1.UserService",
      "method": "GetUser"
    }
  }
}
```

Internally maps to HTTP/2 path: `/example.v1.UserService/GetUser`

### gRPC-specific Headers

Preserve gRPC headers during proxying:

| Header          | Required | Action      |
|-----------------|----------|-------------|
| `content-type`  | Yes      | Validate    |
| `grpc-encoding` | No       | Passthrough |
| `grpc-timeout`  | No       | Enforce     |
| `grpc-status`   | Response | Passthrough |
| `grpc-message`  | Response | Passthrough |

### Error Mapping

gRPC status codes map to OAGW errors:

| gRPC Status        | Code | OAGW Error           |
|--------------------|------|----------------------|
| OK                 | 0    | Success              |
| UNAUTHENTICATED    | 16   | AuthenticationFailed |
| PERMISSION_DENIED  | 7    | Forbidden            |
| RESOURCE_EXHAUSTED | 8    | RateLimitExceeded    |
| UNAVAILABLE        | 14   | LinkUnavailable      |
| DEADLINE_EXCEEDED  | 4    | RequestTimeout       |

### Streaming Support

All gRPC streaming patterns supported:

```
Unary:          Client ──request──> Server ──response──> Client
Server stream:  Client ──request──> Server ──stream───> Client
Client stream:  Client ──stream──> Server ──response──> Client
Bidirectional:  Client <=stream==> Server
```

OAGW acts as transparent proxy, does not buffer streams.

### Performance Considerations

- **Connection pooling**: Reuse HTTP/2 connections to upstreams (multiplexing)
- **Frame forwarding**: Forward gRPC frames directly without parsing Protobuf
- **Backpressure**: Respect HTTP/2 flow control windows
- **Keep-alive**: gRPC requires HTTP/2 pings, ensure enabled

## Prototype Requirements

Before implementation, prototype must validate:

1. **ALPN negotiation**: Verify h2 ALPN works with target Rust TLS stack (rustls/openssl)
2. **Content-type detection**: Reliable gRPC detection from first HTTP/2 headers
3. **Streaming**: Bidirectional streaming without buffering (memory bounded)
4. **Performance**: <5% overhead vs direct gRPC proxy
5. **Error handling**: gRPC status code preservation during proxying
6. **HTTP/1.1 coexistence**: Both protocols work on same port without interference

**Acceptance criteria**:

- gRPC health check (`grpc.health.v1.Health/Check`) works end-to-end
- HTTP/1.1 REST request to same port succeeds
- gRPC streaming (server/client/bidi) works without timeouts
- Rate limiting applies to gRPC requests
- Auth plugin can inspect gRPC metadata

## Links

- [gRPC over HTTP/2](https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md)
- [ALPN Protocol Negotiation](https://datatracker.ietf.org/doc/html/rfc7301)
- [Envoy gRPC Proxying](https://www.envoyproxy.io/docs/envoy/latest/intro/arch_overview/other_protocols/grpc)
- [hyper HTTP/2 Support](https://docs.rs/hyper/latest/hyper/server/conn/http2/index.html)
- [gRPC Status Codes](https://grpc.io/docs/guides/status-codes/)
