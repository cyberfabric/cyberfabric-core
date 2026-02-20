# GTS System Errors Catalog

All types below show the full chained GTS type identifier used in the Problem `type` field (`gts://` prefix stripped for brevity). All identifiers end with `~` since they are schemas.

---

## 1. Transport Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.system.transport.connection_refused.v1~` | 502 | Connection Refused |
| `gts.cf.core.errors.err.v1~cf.system.transport.connection_reset.v1~` | 502 | Connection Reset |
| `gts.cf.core.errors.err.v1~cf.system.transport.connection_timeout.v1~` | 504 | Connection Timeout |
| `gts.cf.core.errors.err.v1~cf.system.transport.dns_failed.v1~` | 502 | DNS Resolution Failed |
| `gts.cf.core.errors.err.v1~cf.system.transport.tls_handshake_failed.v1~` | 502 | TLS Handshake Failed |
| `gts.cf.core.errors.err.v1~cf.system.transport.tls_certificate_invalid.v1~` | 502 | Invalid Certificate |
| `gts.cf.core.errors.err.v1~cf.system.transport.network_unreachable.v1~` | 502 | Network Unreachable |

## 2. Runtime Errors

| Full GTS Type | Status | Title | Metadata Fields |
|---------------|--------|-------|-----------------|
| `gts.cf.core.errors.err.v1~cf.system.runtime.panic.v1~` | 500 | Service Panic | |
| `gts.cf.core.errors.err.v1~cf.system.runtime.oom.v1~` | 503 | Out of Memory | |
| `gts.cf.core.errors.err.v1~cf.system.runtime.timeout.v1~` | 504 | Request Timeout | |
| `gts.cf.core.errors.err.v1~cf.system.runtime.rate_limited.v1~` | 429 | Too Many Requests | `retry_after` |
| `gts.cf.core.errors.err.v1~cf.system.runtime.circuit_open.v1~` | 503 | Circuit Breaker Open | |
| `gts.cf.core.errors.err.v1~cf.system.runtime.resource_exhausted.v1~` | 503 | Resource Exhausted | |
| `gts.cf.core.errors.err.v1~cf.system.runtime.internal.v1~` | 500 | Internal Server Error | |
| `gts.cf.core.errors.err.v1~cf.system.runtime.unhandled.v1~` | 500 | Unhandled Error | |
| `gts.cf.core.errors.err.v1~cf.system.runtime.unavailable.v1~` | 503 | Service Unavailable | |

## 3. HTTP Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.system.http.bad_request.v1~` | 400 | Bad Request |
| `gts.cf.core.errors.err.v1~cf.system.http.unauthorized.v1~` | 401 | Unauthorized |
| `gts.cf.core.errors.err.v1~cf.system.http.forbidden.v1~` | 403 | Forbidden |
| `gts.cf.core.errors.err.v1~cf.system.http.not_found.v1~` | 404 | Not Found |
| `gts.cf.core.errors.err.v1~cf.system.http.method_not_allowed.v1~` | 405 | Method Not Allowed |
| `gts.cf.core.errors.err.v1~cf.system.http.not_acceptable.v1~` | 406 | Not Acceptable |
| `gts.cf.core.errors.err.v1~cf.system.http.conflict.v1~` | 409 | Conflict |
| `gts.cf.core.errors.err.v1~cf.system.http.gone.v1~` | 410 | Gone |
| `gts.cf.core.errors.err.v1~cf.system.http.payload_too_large.v1~` | 413 | Payload Too Large |
| `gts.cf.core.errors.err.v1~cf.system.http.unsupported_media_type.v1~` | 415 | Unsupported Media Type |
| `gts.cf.core.errors.err.v1~cf.system.http.unprocessable_entity.v1~` | 422 | Unprocessable Entity |
| `gts.cf.core.errors.err.v1~cf.system.http.upstream_error.v1~` | 502 | Bad Gateway |
| `gts.cf.core.errors.err.v1~cf.system.http.upstream_timeout.v1~` | 504 | Gateway Timeout |

## 4. gRPC Errors

> **Note:** gRPC uses its own status codes, mapped to HTTP for consistency in the unified system.

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.system.grpc.cancelled.v1~` | 499 | Request Cancelled |
| `gts.cf.core.errors.err.v1~cf.system.grpc.invalid_argument.v1~` | 400 | Invalid Argument |
| `gts.cf.core.errors.err.v1~cf.system.grpc.deadline_exceeded.v1~` | 504 | Deadline Exceeded |
| `gts.cf.core.errors.err.v1~cf.system.grpc.not_found.v1~` | 404 | Not Found |
| `gts.cf.core.errors.err.v1~cf.system.grpc.already_exists.v1~` | 409 | Already Exists |
| `gts.cf.core.errors.err.v1~cf.system.grpc.permission_denied.v1~` | 403 | Permission Denied |
| `gts.cf.core.errors.err.v1~cf.system.grpc.resource_exhausted.v1~` | 429 | Resource Exhausted |
| `gts.cf.core.errors.err.v1~cf.system.grpc.failed_precondition.v1~` | 400 | Failed Precondition |
| `gts.cf.core.errors.err.v1~cf.system.grpc.aborted.v1~` | 409 | Aborted |
| `gts.cf.core.errors.err.v1~cf.system.grpc.unimplemented.v1~` | 501 | Unimplemented |
| `gts.cf.core.errors.err.v1~cf.system.grpc.internal.v1~` | 500 | Internal Error |
| `gts.cf.core.errors.err.v1~cf.system.grpc.unavailable.v1~` | 503 | Unavailable |
| `gts.cf.core.errors.err.v1~cf.system.grpc.data_loss.v1~` | 500 | Data Loss |
| `gts.cf.core.errors.err.v1~cf.system.grpc.unauthenticated.v1~` | 401 | Unauthenticated |

## 5. Logical Errors

| Full GTS Type | Status | Title | Metadata Fields |
|---------------|--------|-------|-----------------|
| `gts.cf.core.errors.err.v1~cf.system.logical.validation_failed.v1~` | 422 | Validation Failed | `errors` |
| `gts.cf.core.errors.err.v1~cf.system.logical.not_found.v1~` | 404 | Not Found | |
| `gts.cf.core.errors.err.v1~cf.system.logical.already_exists.v1~` | 409 | Already Exists | |
| `gts.cf.core.errors.err.v1~cf.system.logical.precondition_failed.v1~` | 412 | Precondition Failed | |
| `gts.cf.core.errors.err.v1~cf.system.logical.state_conflict.v1~` | 409 | State Conflict | |
| `gts.cf.core.errors.err.v1~cf.system.logical.operation_failed.v1~` | 500 | Operation Failed | |

## 6. Auth Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.auth.token.expired.v1~` | 401 | Token Expired |
| `gts.cf.core.errors.err.v1~cf.auth.token.invalid.v1~` | 401 | Invalid Token |
| `gts.cf.core.errors.err.v1~cf.auth.token.missing.v1~` | 401 | Missing Token |
| `gts.cf.core.errors.err.v1~cf.auth.issuer.mismatch.v1~` | 401 | Issuer Mismatch |
| `gts.cf.core.errors.err.v1~cf.auth.audience.mismatch.v1~` | 401 | Audience Mismatch |
| `gts.cf.core.errors.err.v1~cf.auth.jwks.fetch_failed.v1~` | 500 | JWKS Fetch Failed |
| `gts.cf.core.errors.err.v1~cf.auth.scope.insufficient.v1~` | 403 | Insufficient Scope |
| `gts.cf.core.errors.err.v1~cf.auth.tenant.mismatch.v1~` | 403 | Tenant Mismatch |
| `gts.cf.core.errors.err.v1~cf.auth.system.internal.v1~` | 500 | Internal Error |

## 7. Types Registry Errors

| Full GTS Type | Status | Title | Metadata Fields |
|---------------|--------|-------|------------------|
| `gts.cf.core.errors.err.v1~cf.types_registry.entity.not_found.v1~` | 404 | Entity Not Found | `gts_id` |
| `gts.cf.core.errors.err.v1~cf.types_registry.entity.already_exists.v1~` | 409 | Entity Already Exists | `gts_id` |
| `gts.cf.core.errors.err.v1~cf.types_registry.validation.invalid_gts_id.v1~` | 400 | Invalid GTS ID | `message` |
| `gts.cf.core.errors.err.v1~cf.types_registry.validation.failed.v1~` | 422 | Validation Failed | `message`, `errors` |
| `gts.cf.core.errors.err.v1~cf.types_registry.operational.not_ready.v1~` | 503 | Service Not Ready | |
| `gts.cf.core.errors.err.v1~cf.types_registry.operational.activation_failed.v1~` | 500 | Activation Failed | `error_count`, `summary` |
| `gts.cf.core.errors.err.v1~cf.types_registry.system.internal.v1~` | 500 | Internal Error | |

## 8. File Parser Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.file_parser.file.not_found.v1~` | 404 | File Not Found |
| `gts.cf.core.errors.err.v1~cf.file_parser.validation.invalid_format.v1~` | 422 | Invalid File Format |
| `gts.cf.core.errors.err.v1~cf.file_parser.validation.unsupported_type.v1~` | 400 | Unsupported File Type |
| `gts.cf.core.errors.err.v1~cf.file_parser.validation.no_parser.v1~` | 415 | No Parser Available |
| `gts.cf.core.errors.err.v1~cf.file_parser.parsing.failed.v1~` | 422 | Parse Error |
| `gts.cf.core.errors.err.v1~cf.file_parser.validation.invalid_url.v1~` | 400 | Invalid URL |
| `gts.cf.core.errors.err.v1~cf.file_parser.validation.invalid_request.v1~` | 400 | Invalid Request |
| `gts.cf.core.errors.err.v1~cf.file_parser.transport.download_failed.v1~` | 502 | Download Failed |
| `gts.cf.core.errors.err.v1~cf.file_parser.system.io_error.v1~` | 500 | IO Error |
| `gts.cf.core.errors.err.v1~cf.file_parser.system.internal.v1~` | 500 | Internal Error |

## 9. Database Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.db.connection.failed.v1~` | 503 | Connection Failed |
| `gts.cf.core.errors.err.v1~cf.db.connection.timeout.v1~` | 504 | Connection Timeout |
| `gts.cf.core.errors.err.v1~cf.db.query.failed.v1~` | 500 | Query Failed |
| `gts.cf.core.errors.err.v1~cf.db.transaction.failed.v1~` | 500 | Transaction Failed |
| `gts.cf.core.errors.err.v1~cf.db.constraint.violation.v1~` | 409 | Constraint Violation |
| `gts.cf.core.errors.err.v1~cf.db.entity.not_found.v1~` | 404 | Entity Not Found |

## 10. Nodes Registry Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.nodes_registry.node.not_found.v1~` | 404 | Node Not Found |
| `gts.cf.core.errors.err.v1~cf.nodes_registry.sysinfo.collection_failed.v1~` | 500 | System Info Collection Failed |
| `gts.cf.core.errors.err.v1~cf.nodes_registry.syscap.collection_failed.v1~` | 500 | System Capabilities Collection Failed |
| `gts.cf.core.errors.err.v1~cf.nodes_registry.validation.invalid_input.v1~` | 400 | Invalid Input |
| `gts.cf.core.errors.err.v1~cf.nodes_registry.system.internal.v1~` | 500 | Internal Error |

## 11. OData Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.odata.query.invalid_filter.v1~` | 422 | Invalid Filter |
| `gts.cf.core.errors.err.v1~cf.odata.query.invalid_orderby.v1~` | 422 | Invalid OrderBy |
| `gts.cf.core.errors.err.v1~cf.odata.query.invalid_cursor.v1~` | 422 | Invalid Cursor |
| `gts.cf.core.errors.err.v1~cf.odata.system.internal.v1~` | 500 | Internal OData Error |

## 12. Tenant Resolver Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.tenant_resolver.registry.unavailable.v1~` | 503 | Types Registry Unavailable |
| `gts.cf.core.errors.err.v1~cf.tenant_resolver.plugin.not_found.v1~` | 404 | Plugin Not Found |
| `gts.cf.core.errors.err.v1~cf.tenant_resolver.plugin.invalid_instance.v1~` | 500 | Invalid Plugin Instance |
| `gts.cf.core.errors.err.v1~cf.tenant_resolver.plugin.client_not_found.v1~` | 500 | Plugin Client Not Found |
| `gts.cf.core.errors.err.v1~cf.tenant_resolver.tenant.not_found.v1~` | 404 | Tenant Not Found |
| `gts.cf.core.errors.err.v1~cf.tenant_resolver.tenant.access_denied.v1~` | 403 | Access Denied |
| `gts.cf.core.errors.err.v1~cf.tenant_resolver.auth.unauthorized.v1~` | 401 | Unauthorized |
| `gts.cf.core.errors.err.v1~cf.tenant_resolver.system.internal.v1~` | 500 | Internal Error |

## 13. API Gateway Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.api_gateway.request.bad_request.v1~` | 400 | Bad Request |
| `gts.cf.core.errors.err.v1~cf.api_gateway.auth.unauthorized.v1~` | 401 | Unauthorized |
| `gts.cf.core.errors.err.v1~cf.api_gateway.auth.forbidden.v1~` | 403 | Forbidden |
| `gts.cf.core.errors.err.v1~cf.api_gateway.routing.not_found.v1~` | 404 | Not Found |
| `gts.cf.core.errors.err.v1~cf.api_gateway.state.conflict.v1~` | 409 | Conflict |
| `gts.cf.core.errors.err.v1~cf.api_gateway.rate.limited.v1~` | 429 | Too Many Requests |
| `gts.cf.core.errors.err.v1~cf.api_gateway.system.internal.v1~` | 500 | Internal Error |

## 14. Simple User Settings Errors

| Full GTS Type | Status | Title |
|---------------|--------|-------|
| `gts.cf.core.errors.err.v1~cf.simple_user_settings.settings.not_found.v1~` | 404 | Settings Not Found |
| `gts.cf.core.errors.err.v1~cf.simple_user_settings.settings.validation.v1~` | 422 | Validation Error |
| `gts.cf.core.errors.err.v1~cf.simple_user_settings.system.internal_database.v1~` | 500 | Internal Database Error |

---

## Error Code Quick Reference

| Status | Transport | Runtime | HTTP | gRPC | Logical |
|--------|-----------|---------|------|------|---------|
| **400** | - | - | `http.bad_request` | `grpc.invalid_argument`, `grpc.failed_precondition` | - |
| **401** | - | - | `http.unauthorized` | `grpc.unauthenticated` | - |
| **403** | - | - | `http.forbidden` | `grpc.permission_denied` | - |
| **404** | - | - | `http.not_found` | `grpc.not_found` | `logical.not_found` |
| **405** | - | - | `http.method_not_allowed` | - | - |
| **406** | - | - | `http.not_acceptable` | - | - |
| **409** | - | - | `http.conflict` | `grpc.already_exists`, `grpc.aborted` | `logical.already_exists`, `logical.state_conflict` |
| **410** | - | - | `http.gone` | - | - |
| **412** | - | - | - | - | `logical.precondition_failed` |
| **413** | - | - | `http.payload_too_large` | - | - |
| **415** | - | - | `http.unsupported_media_type` | - | - |
| **422** | - | - | `http.unprocessable_entity` | - | `logical.validation_failed` |
| **429** | - | `runtime.rate_limited` | - | `grpc.resource_exhausted` | - |
| **499** | - | - | - | `grpc.cancelled` | - |
| **500** | - | `runtime.internal`, `runtime.panic`, `runtime.unhandled` | - | `grpc.internal`, `grpc.data_loss` | `logical.operation_failed` |
| **501** | - | - | - | `grpc.unimplemented` | - |
| **502** | `transport.connection_refused`, `transport.connection_reset`, `transport.dns_failed`, `transport.tls_*`, `transport.network_unreachable` | - | `http.upstream_error` | - | - |
| **503** | - | `runtime.oom`, `runtime.circuit_open`, `runtime.resource_exhausted`, `runtime.unavailable` | - | `grpc.unavailable` | - |
| **504** | `transport.connection_timeout` | `runtime.timeout` | `http.upstream_timeout` | `grpc.deadline_exceeded` | - |
