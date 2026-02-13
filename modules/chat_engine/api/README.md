# Chat Engine API Protocol Specifications

This directory contains protocol specification files for the Chat Engine API, defining the HTTP REST API, WebSocket streaming API, and Webhook protocols.

## Overview

Protocol specification files complement the domain model schemas in `../schemas/` by defining:

- **API operations and flows**: How clients interact with the server
- **Event sequences**: Order and structure of events in request/response cycles
- **Protocol-level constraints**: Timeouts, error handling, streaming patterns
- **Connection configuration**: Authentication, transport details

The Chat Engine API uses **HTTP with chunked streaming**:
- **HTTP REST API**: For CRUD operations, queries, and control operations
- **HTTP Chunked Streaming**: For real-time streaming responses (newline-delimited JSON)
- **Stateless Architecture**: No persistent connections, simpler scaling and deployment

## Files

### http-protocol.json

**Format**: OpenAPI 3.0.3

Complete HTTP REST API specification defining the RESTful endpoints for Chat Engine client operations.

**Contents**:
- **15 REST endpoints** across 3 categories:
  - **Session Management (10)**:
    - `POST /sessions` - Create session
    - `GET /sessions/{id}` - Get session
    - `DELETE /sessions/{id}` - Delete session
    - `PATCH /sessions/{id}/type` - Switch type
    - `POST /sessions/{id}/export` - Export session
    - `POST /sessions/{id}/share` - Share session
    - `GET /share/{token}` - Access shared
    - `GET /sessions/{id}/search` - Search in session
    - `GET /search` - Search all sessions
    - `POST /sessions/{id}/summarize` - Generate summary (streaming)

  - **Message Operations (5)**:
    - `POST /messages/send` - Send message (streaming)
    - `POST /messages/{id}/recreate` - Recreate message (streaming)
    - `GET /sessions/{id}/messages` - List messages
    - `GET /messages/{id}` - Get message
    - `GET /messages/{id}/variants` - Get variants
    - `POST /messages/{id}/reaction` - React to message

  - **Search Operations (2)**: Included above

**HTTP Configuration**:
- Base URL: `https://chat-engine/api/v1`
- Authentication: JWT Bearer token in Authorization header
- Content-Type: `application/json`
- Standard HTTP status codes (200, 201, 400, 401, 404, 500, etc.)

**Use Cases**:
- Session lifecycle management (create, read, update, delete)
- Message retrieval and navigation
- Search across conversations
- Export and sharing operations
- Real-time streaming responses (send message, recreate, summarize)
- Cancellation via connection close (stateless)

### HTTP Chunked Streaming

**Format**: Newline-Delimited JSON (NDJSON) over HTTP chunked transfer encoding

**Streaming Endpoints**:
- `POST /messages/send` - Send message with streaming response
- `POST /messages/{id}/recreate` - Recreate message with streaming
- `POST /sessions/{id}/summarize` - Generate summary with streaming

**Streaming Events**:
- `start` - Streaming begins, includes message_id
- `chunk` - Content chunk (text, code, image, etc.)
- `complete` - Streaming finished successfully
- `error` - Error occurred during streaming

**HTTP Configuration**:
- Content-Type: `application/x-ndjson`
- Transfer-Encoding: chunked
- Authentication: JWT Bearer token in Authorization header
- Cancellation: Close HTTP connection

**Use Cases**:
- Real-time message streaming from AI backends
- Recreating responses with variants
- Session summarization with streaming
- Stateless scaling (no persistent connections)

### webhook-protocol.json

**Format**: GTS JSON Schema (custom format)

**GTS ID**: `gts://gts.x.chat_engine.api.webhook_protocol.v1~`

Complete Webhook API specification defining HTTP POST calls from Chat Engine to backend services.

**Contents**:
- **7 Webhook operations**:
  - `session.created` - Session creation notification
  - `message.new` - New user message processing
  - `message.recreate` - Message regeneration request
  - `message.aborted` - Streaming cancellation notification
  - `session.deleted` - Session deletion notification
  - `session.summary` - Session summarization request
  - `session_type.health_check` - Backend health check

**HTTP Configuration**:
  - Method: POST
  - Content-Type: application/json
  - Accept: application/json, text/event-stream

**Streaming Protocol**:
  - HTTP chunked streaming (NDJSON) format
  - Event types: chunk, complete, error
  - Content chunk structure

**Resilience Patterns**:
  - Retry policy (exponential backoff)
  - Circuit breaker (failure threshold, timeout)
  - Timeout handling (abort and notify)

## Protocol Architecture

### HTTP with Chunked Streaming

**HTTP REST API with Streaming** provides:
- ✅ Simple CRUD operations (no persistent connection overhead)
- ✅ Queries and search (standard HTTP caching, CDN-friendly)
- ✅ Standard tooling (curl, Postman, HTTP clients)
- ✅ Easy testing and debugging
- ✅ RESTful patterns and conventions
- ✅ Streaming responses (real-time incremental delivery via chunked transfer)
- ✅ Stateless scaling (no sticky sessions required)
- ✅ Simple cancellation (close connection)
- ✅ Standard load balancing and proxy support

This approach follows modern patterns used by:
- OpenAI API (HTTP streaming)
- Anthropic API (HTTP streaming)
- Modern serverless architectures

### Protocol Decision Matrix

| Operation Type | Protocol | Reason |
|---------------|----------|--------|
| Create session | HTTP POST | Simple request/response, no streaming needed |
| Get session | HTTP GET | Standard retrieval, cacheable |
| Delete session | HTTP DELETE | Simple command, idempotent |
| Send message | **HTTP POST (streaming)** | Streaming response via chunked transfer |
| List messages | HTTP GET | Standard query, pagination support |
| Stop streaming | **Close connection** | Stateless cancellation |
| Recreate message | **HTTP POST (streaming)** | Streaming response via chunked transfer |
| Search messages | HTTP GET | Query operation, standard REST patterns |
| Summarize session | **HTTP POST (streaming)** | Streaming response via chunked transfer |

## Relationship to Domain Schemas

Protocol specifications **reference** domain schemas from `../schemas/` using JSON Schema `$ref` or by sharing common types:

```json
{
  "request": {
    "schema": "../schemas/session/SessionCreateRequest.json"
  },
  "response": {
    "schema": "../schemas/session/SessionCreateResponse.json"
  }
}
```

**Domain schemas** (`../schemas/`) define:
- Message structures (requests, responses, events)
- Entity types (Session, Message, SessionType)
- Enums and common types

**Protocol specs** (`./`) define:
- How and when to use those message structures
- Operation flows and sequences
- Protocol-level behavior (timeouts, errors, streaming)

## Usage Examples

### HTTP REST API Examples

**TypeScript Client**:
```typescript
// Create session
const response = await fetch('https://chat-engine/api/v1/sessions', {
  method: 'POST',
  headers: {
    'Authorization': `Bearer ${jwt}`,
    'Content-Type': 'application/json'
  },
  body: JSON.stringify({
    session_type_id: 'uuid-123',
    client_id: 'user-456'
  })
});
const { session_id, available_capabilities } = await response.json();

// Get session
const session = await fetch(`https://chat-engine/api/v1/sessions/${session_id}`, {
  headers: { 'Authorization': `Bearer ${jwt}` }
}).then(r => r.json());

// Search in session
const results = await fetch(
  `https://chat-engine/api/v1/sessions/${session_id}/search?query=hello&limit=20`,
  { headers: { 'Authorization': `Bearer ${jwt}` }}
).then(r => r.json());
```

**Python Client**:
```python
import requests

# Authentication
headers = {'Authorization': f'Bearer {jwt}'}

# Create session
response = requests.post(
    'https://chat-engine/api/v1/sessions',
    json={'session_type_id': 'uuid-123', 'client_id': 'user-456'},
    headers=headers
)
session_id = response.json()['session_id']

# Delete session
requests.delete(f'https://chat-engine/api/v1/sessions/{session_id}', headers=headers)
```

### HTTP Streaming API Examples

**TypeScript Client**:
```typescript
// Send message with streaming response
async function sendMessage(sessionId: string, content: string) {
  const response = await fetch('https://chat-engine/api/v1/messages/send', {
    method: 'POST',
    headers: {
      'Authorization': `Bearer ${jwt}`,
      'Content-Type': 'application/json'
    },
    body: JSON.stringify({
      session_id: sessionId,
      content: content,
      enabled_capabilities: ['web_search']
    })
  });

  const reader = response.body!.getReader();
  const decoder = new TextDecoder();

  while (true) {
    const {done, value} = await reader.read();
    if (done) break;

    const chunk = decoder.decode(value);
    const lines = chunk.split('\n').filter(line => line.trim());

    for (const line of lines) {
      const event = JSON.parse(line);

      switch (event.type) {
        case 'start':
          console.log('Streaming started:', event.message_id);
          break;
        case 'chunk':
          console.log('Chunk:', event.chunk);
          displayChunk(event.chunk);
          break;
        case 'complete':
          console.log('Complete:', event.metadata);
          break;
        case 'error':
          console.error('Error:', event.message);
          break;
      }
    }
  }
}
```

**Python Client**:
```python
import requests
import json

def send_message(session_id: str, content: str):
    response = requests.post(
        'https://chat-engine/api/v1/messages/send',
        headers={'Authorization': f'Bearer {jwt}'},
        json={
            'session_id': session_id,
            'content': content,
            'enabled_capabilities': []
        },
        stream=True
    )

    for line in response.iter_lines():
        if line:
            event = json.loads(line)

            if event['type'] == 'start':
                print(f"Streaming started: {event['message_id']}")
            elif event['type'] == 'chunk':
                print(event['chunk']['content'], end='', flush=True)
            elif event['type'] == 'complete':
                print('\n[Complete]')
            elif event['type'] == 'error':
                print(f"\nError: {event['message']}")

send_message('uuid-123', 'Hello AI')
```

### Validating Protocol Compliance

**Python**:
```python
import json
import jsonschema

# Validate HTTP REST API spec (OpenAPI 3.0)
with open('api/http-protocol.json') as f:
    openapi_spec = json.load(f)

# Validate against OpenAPI 3.0 schema
from openapi_spec_validator import validate_spec
validate_spec(openapi_spec)

# Validate WebSocket protocol (GTS format)
with open('api/websocket-protocol.json') as f:
    ws_protocol = json.load(f)

# Access operation definitions
operations = ws_protocol['client_to_server']['operations']
for op in operations['items']:
    print(f"Operation: {op['properties']['operation_id']['const']}")
    print(f"  Event Type: {op['properties']['event_type']['const']}")
```

## Protocol Versioning

Protocol specifications use semantic versioning:

**HTTP REST API** (`http-protocol.json`):
- **Current version**: `1.0.0` (in OpenAPI `info.version`)
- **URL versioning**: `/api/v1/` prefix
- **Breaking changes**: Increment major version, update URL prefix to `/api/v2/`

**WebSocket API** (`websocket-protocol.json`):
- **Current version**: `2.0` (version 2.0 - split from HTTP)
- **GTS identifier**: `v2~`
- **Breaking changes**: Increment major version (`v3~`)
- **Version negotiation**: Sent in `connection.ready` event

**Webhook API** (`webhook-protocol.json`):
- **Current version**: `1.0`
- **GTS identifier**: `v1~`
- **Breaking changes**: Increment major version, notify backends

**Version compatibility rules**:
- Clients must support protocol version from server handshake
- New operations can be added without version bump (optional features)
- Changing existing operation signatures requires version bump
- Event sequence changes require version bump

## Validation and Testing

### JSON Syntax Validation

```bash
# Validate JSON syntax
python3 -m json.tool api/http-protocol.json > /dev/null
python3 -m json.tool api/websocket-protocol.json > /dev/null
python3 -m json.tool api/webhook-protocol.json > /dev/null
```

### OpenAPI Validation

```bash
# Validate HTTP REST API spec
npx @redocly/cli lint api/http-protocol.json
```

### Protocol Completeness Check

Compare protocol specifications with DESIGN.md:

```python
import json

# Verify HTTP REST endpoints (14 operations)
expected_http_endpoints = [
    ("POST", "/sessions"),
    ("GET", "/sessions/{id}"),
    ("DELETE", "/sessions/{id}"),
    ("PATCH", "/sessions/{id}/type"),
    ("POST", "/sessions/{id}/export"),
    ("POST", "/sessions/{id}/share"),
    ("GET", "/share/{token}"),
    ("GET", "/sessions/{id}/search"),
    ("GET", "/search"),
    ("GET", "/sessions/{id}/messages"),
    ("GET", "/messages/{id}"),
    ("POST", "/messages/{id}/stop"),
    ("GET", "/messages/{id}/variants")
]

with open('api/http-protocol.json') as f:
    http_spec = json.load(f)
    paths = http_spec['paths']

    documented = []
    for path, methods in paths.items():
        for method in methods.keys():
            if method.upper() in ['GET', 'POST', 'PATCH', 'DELETE']:
                documented.append((method.upper(), path))

    missing = set(expected_http_endpoints) - set(documented)
    if missing:
        print(f"Missing HTTP endpoints: {missing}")
    else:
        print("All HTTP endpoints documented ✓")

# Verify WebSocket operations (3 streaming operations)
expected_ws_operations = ["message.send", "message.recreate", "session.summarize"]

with open('api/websocket-protocol.json') as f:
    ws_protocol = json.load(f)
    operations = ws_protocol['client_to_server']['operations']['items']
    event_types = [op['properties']['event_type']['const'] for op in operations]

    missing = set(expected_ws_operations) - set(event_types)
    if missing:
        print(f"Missing WebSocket operations: {missing}")
    else:
        print("All WebSocket operations documented ✓")
```

## Tools and Libraries

### HTTP REST API

- **OpenAPI Tools**:
  - Redoc: Interactive documentation
  - Swagger UI: API explorer
  - OpenAPI Generator: Client/server code generation

- **Testing**:
  - Postman: Manual testing and collections
  - curl: Command-line testing
  - pytest with requests: Automated testing

### WebSocket API

- **JSON Schema Validation**:
  - Python: `jsonschema` library
  - TypeScript: `ajv` library
  - Rust: `jsonschema` crate

- **WebSocket Clients**:
  - JavaScript: native WebSocket, `ws` library
  - Python: `websockets`, `asyncio`
  - Rust: `tokio-tungstenite`

### Documentation Generation

- **HTTP**: Redoc, Swagger UI (OpenAPI 3.0)
- **WebSocket**: Custom documentation from JSON Schema
- **Both**: Can convert to AsyncAPI 2.x format for unified docs

## Migration Guide

For clients migrating from WebSocket to HTTP streaming:

### All Operations Now Use HTTP

| Old (WebSocket) | New (HTTP REST/Streaming) |
|----------------|---------------------------|
| `session.create` | `POST /sessions` |
| `session.get` | `GET /sessions/{id}` |
| `session.delete` | `DELETE /sessions/{id}` |
| `session.switch_type` | `PATCH /sessions/{id}/type` |
| `session.export` | `POST /sessions/{id}/export` |
| `session.share` | `POST /sessions/{id}/share` |
| `session.access_shared` | `GET /share/{token}` |
| `message.list` | `GET /sessions/{id}/messages` |
| `message.get` | `GET /messages/{id}` |
| `message.send` | `POST /messages/send` (streaming) |
| `message.recreate` | `POST /messages/{id}/recreate` (streaming) |
| `message.stop` | Close HTTP connection |
| `message.get_variants` | `GET /messages/{id}/variants` |
| `session.search` | `GET /sessions/{id}/search` |
| `sessions.search` | `GET /search` |
| `session.summarize` | `POST /sessions/{id}/summarize` (streaming) |

### Key Changes

| Feature | Old (WebSocket) | New (HTTP Streaming) |
|---------|-----------------|----------------------|
| **Connection** | Persistent WebSocket | HTTP request per operation |
| **Streaming** | WebSocket frames | HTTP chunked transfer (NDJSON) |
| **Cancellation** | `message.stop` event | Close connection |
| **Authentication** | JWT in handshake | Bearer token per request |
| **Scaling** | Sticky sessions | Stateless (any server) |
| **Push events** | `session.updated`, `message.created` | Removed (client polls if needed) |

## See Also

- [`../schemas/README.md`](../schemas/README.md) - Domain model schema documentation
- [`../DESIGN.md`](../DESIGN.md) - Complete architecture and design (section 3.3: API Contracts)
- [`../PRD.md`](../PRD.md) - Product requirements
- [`../ADR/`](../ADR/) - Architecture decision records

## Examples

### Complete Request Flow Example

**Create session and send message**:

1. **HTTP**: Create session
   ```http
   POST /api/v1/sessions
   Authorization: Bearer <token>
   Content-Type: application/json

   {"session_type_id": "uuid", "client_id": "user-id"}
   ```

2. **HTTP Streaming**: Send message
   ```http
   POST /api/v1/messages/send
   Authorization: Bearer <token>
   Content-Type: application/json

   {"session_id": "uuid", "content": "Hello", "enabled_capabilities": []}
   ```

3. **HTTP Streaming**: Receive response (NDJSON)
   ```json
   {"type":"start","message_id":"msg-456"}
   {"type":"chunk","message_id":"msg-456","chunk":{"type":"text","content":"Hi"}}
   {"type":"chunk","message_id":"msg-456","chunk":{"type":"text","content":" there"}}
   {"type":"complete","message_id":"msg-456","metadata":{"usage":{"input_tokens":10,"output_tokens":5}}}
   ```

4. **HTTP**: Retrieve message history
   ```http
   GET /api/v1/sessions/{uuid}/messages
   Authorization: Bearer <token>
   ```

---

**Protocol Version**: HTTP REST API 1.0.0, Webhook API 1.0
**Last Updated**: 2025-02-05
**Maintainers**: Chat Engine Team
