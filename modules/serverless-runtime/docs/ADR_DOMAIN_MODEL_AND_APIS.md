# ADR — Serverless Runtime Domain Model and APIs

## Status

Proposed

## Context

The Serverless Runtime module needs a stable, implementation-agnostic domain model and API contract that:

- supports runtime creation, registration, and invocation of functions and workflows
- provides tenant-safe isolation with security context propagation
- enables observability, governance, and lifecycle management
- can be satisfied by different implementation technologies (Temporal, Starlark, cloud-native FaaS)

This ADR defines:

- core domain entities and their relationships
- GTS-based identifier conventions
- API contracts for definition management, invocation, scheduling, and observability
- error taxonomy and status model
- governance and quota structures

This document is intentionally implementation-agnostic per the PRD requirements and serves as the unified contract
between API consumers and runtime implementations.

## Goals

- Provide a stable domain model for functions and workflows (unified as entrypoints).
- Satisfy PRD business requirements for governance, durability, security context, and observability.
- Keep runtime implementation agnostic (Temporal, Starlark, cloud FaaS, etc.).
- Provide a consistent API surface with explicit validation, lifecycle control, and auditability.

## Non-Goals

- Selecting a specific runtime technology.
- Defining workflow DSL syntax in detail (implementation-specific).
- Defining UI/UX for authoring or debugging workflows.

---

## Domain Model

This section defines the core domain entities following
the [GTS specification](https://github.com/globaltypesystem/gts-spec).
All type definitions use GTS identifiers and JSON Schema for validation.

### Entrypoints

Functions and workflows are unified as **entrypoints** — registered definitions that can be invoked via the runtime API.
They are distinguished via GTS type inheritance:

| Entrypoint Type                                                                              | Description          |
|----------------------------------------------------------------------------------------------|----------------------|
| gts.x.core.serverless.entrypoint.v1~                                                         | Base entrypoint type |
| gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~                           | Function type        |
| gts.x.core.serverless.entrypoint.v1~x.core.serverless.workflow.v1~                           | Workflow type        |
| gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~vendor.app.my_func.v1~     | Custom function      |
| gts.x.core.serverless.entrypoint.v1~x.core.serverless.workflow.v1~vendor.app.my_workflow.v1~ | Custom workflow      |

#### Invocation modes

- **sync** — caller waits for completion and receives the result in the response. Best for short runs.
- **async** — caller receives an `invocation_id` immediately and polls for status/results later.

### Supporting Types

The following GTS types are referenced by entrypoint definitions. Each is a standalone schema that can be
validated independently and referenced via `gts://` URIs.

| GTS Type                                         | Description                                       |
|--------------------------------------------------|---------------------------------------------------|
| `gts.x.core.serverless.owner_ref.v1~`            | Ownership reference with visibility semantics     |
| `gts.x.core.serverless.io_schema.v1~`            | Input/output contract (GTS ref, schema, void)     |
| `gts.x.core.serverless.limits.v1~`               | Base limits (adapters derive specific types)      |
| `gts.x.core.serverless.rate_limit.v1~`           | Rate limiting configuration (plugin-based)        |
| `gts.x.core.serverless.retry_policy.v1~`         | Retry behavior configuration                      |
| `gts.x.core.serverless.implementation.v1~`       | Code, spec, or adapter reference                  |
| `gts.x.core.serverless.workflow_traits.v1~`      | Workflow-specific execution traits                |
| `gts.x.core.serverless.compensation_context.v1~` | Input envelope passed to compensation entrypoints |
| `gts.x.core.serverless.status.v1~`               | Invocation status (derived types per state)       |
| `gts.x.core.serverless.err.v1~`                  | Error types (derived types per error kind)        |
| `gts.x.core.serverless.timeline_event.v1~`       | Invocation timeline event for execution history   |

#### OwnerRef

**GTS ID:** `gts.x.core.serverless.owner_ref.v1~`

Defines ownership and default visibility for an entrypoint. Per PRD BR-002, the `owner_type` determines
the default access scope:

- `user` — private to the owning user by default
- `tenant` — visible to authorized users within the tenant
- `system` — platform-provided, managed by the system

Extended sharing beyond default visibility is managed through access control integration (PRD BR-123).

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.owner_ref.v1~",
  "title": "Owner Reference",
  "description": "Ownership reference. owner_type determines default visibility: user=private, tenant=tenant-visible, system=platform-provided.",
  "type": "object",
  "properties": {
    "owner_type": {
      "type": "string",
      "enum": [
        "user",
        "tenant",
        "system"
      ]
    },
    "id": {
      "type": "string"
    },
    "tenant_id": {
      "type": "string"
    }
  },
  "required": [
    "owner_type",
    "id",
    "tenant_id"
  ]
}
```

#### IOSchema

**GTS ID:** `gts.x.core.serverless.io_schema.v1~`

Defines the input/output contract for an entrypoint. Per PRD BR-032 and BR-037, inputs and outputs
are validated before invocation.

Each of `params` and `returns` accepts:

- **Inline JSON Schema** — any valid JSON Schema object
- **GTS reference** — `{ "$ref": "gts://gts.x.some.type.v1~" }` for reusable types
- **Void** — `null` or absent indicates no input/output

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.io_schema.v1~",
  "title": "IO Schema",
  "description": "Input/output contract. params/returns accept JSON Schema, GTS $ref, or null for void.",
  "type": "object",
  "properties": {
    "params": {
      "description": "Input schema. Use $ref with gts:// URI for GTS types. Null or absent for void.",
      "oneOf": [
        {
          "type": "object"
        },
        {
          "type": "null"
        }
      ]
    },
    "returns": {
      "description": "Output schema. Use $ref with gts:// URI for GTS types. Null or absent for void.",
      "oneOf": [
        {
          "type": "object"
        },
        {
          "type": "null"
        }
      ]
    },
    "errors": {
      "type": "array",
      "items": {
        "type": "string",
        "x-gts-ref": "gts.*"
      },
      "description": "GTS error type IDs.",
      "default": []
    }
  }
}
```

#### Limits

**GTS ID:** `gts.x.core.serverless.limits.v1~`

Base resource limits schema. Per PRD BR-005, BR-012, and BR-028, the runtime enforces limits to
prevent resource exhaustion and runaway executions.

The base schema defines only universal limits. Adapters register derived GTS types
with adapter-specific fields. The runtime validates limits against the adapter's schema based
on the `implementation.adapter` field.

##### Base fields

- `timeout_seconds` — maximum execution duration before termination (BR-028)
- `max_concurrent` — maximum concurrent invocations of this entrypoint (BR-012)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.limits.v1~",
  "title": "Entrypoint Limits (Base)",
  "description": "Base limits schema. Adapters derive type-specific schemas via GTS inheritance.",
  "type": "object",
  "properties": {
    "timeout_seconds": {
      "type": "integer",
      "minimum": 1,
      "default": 30,
      "description": "Max execution duration in seconds."
    },
    "max_concurrent": {
      "type": "integer",
      "minimum": 1,
      "default": 100,
      "description": "Max concurrent invocations."
    }
  },
  "additionalProperties": true
}
```

##### Adapter-Derived Limits (Examples)

Adapters register their own GTS types extending the base:

| GTS Type                                                                        | Adapter    | Additional Fields                       |
|---------------------------------------------------------------------------------|------------|-----------------------------------------|
| `gts.x.core.serverless.limits.v1~x.core.serverless.adapter.starlark.limits.v1~` | Starlark   | `memory_mb`, `cpu`                      |
| `gts.x.core.serverless.limits.v1~x.core.serverless.adapter.lambda.limits.v1~`   | AWS Lambda | `memory_mb`, `ephemeral_storage_mb`     |
| `gts.x.core.serverless.limits.v1~x.core.serverless.adapter.temporal.limits.v1~` | Temporal   | (worker-based, no per-execution limits) |

##### Example: Starlark Adapter Limits

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.limits.v1~x.core.serverless.adapter.starlark.limits.v1~",
  "title": "Starlark Adapter Limits",
  "description": "Limits for Starlark embedded runtime.",
  "allOf": [
    {
      "$ref": "gts://gts.x.core.serverless.limits.v1~"
    },
    {
      "type": "object",
      "properties": {
        "memory_mb": {
          "type": "integer",
          "minimum": 1,
          "maximum": 512,
          "default": 128,
          "description": "Memory allocation in MB."
        },
        "cpu": {
          "type": "number",
          "minimum": 0.1,
          "maximum": 1.0,
          "default": 0.2,
          "description": "CPU allocation in fractional cores."
        }
      }
    }
  ]
}
```

##### Example: Lambda Adapter Limits

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.limits.v1~x.core.serverless.adapter.lambda.limits.v1~",
  "title": "Lambda Adapter Limits",
  "description": "Limits for AWS Lambda adapter. CPU is derived from memory tier.",
  "allOf": [
    {
      "$ref": "gts://gts.x.core.serverless.limits.v1~"
    },
    {
      "type": "object",
      "properties": {
        "memory_mb": {
          "type": "integer",
          "minimum": 128,
          "maximum": 10240,
          "default": 128,
          "description": "Memory allocation in MB (CPU scales with memory)."
        },
        "ephemeral_storage_mb": {
          "type": "integer",
          "minimum": 512,
          "maximum": 10240,
          "default": 512,
          "description": "Ephemeral storage in MB."
        }
      }
    }
  ]
}
```

#### RetryPolicy

**GTS ID:** `gts.x.core.serverless.retry_policy.v1~`

Configures retry behavior for failed invocations. Per PRD BR-019, supports exponential backoff
with configurable limits:

- `max_attempts` — total attempts including the initial invocation (0 = no retries)
- `initial_delay_ms` — delay before the first retry
- `max_delay_ms` — maximum delay between retries
- `backoff_multiplier` — multiplier applied to delay after each retry
- `non_retryable_errors` — GTS error type IDs that must never be retried, regardless of their
  `RuntimeErrorCategory`

##### Retry Precedence

The runtime evaluates whether a failed invocation should be retried by combining two inputs:
the error's `category` field (from `RuntimeErrorCategory` in the `RuntimeErrorPayload` struct)
and the `non_retryable_errors` list in this `RetryPolicy` schema.

An invocation is retried **only when all** of the following conditions hold:

1. `max_attempts` has not been exhausted.
2. The error's `RuntimeErrorCategory` is `Retryable`.
3. The error's GTS type ID is **not** present in `RetryPolicy.non_retryable_errors`.

`non_retryable_errors` takes precedence over the error category: even if an error carries
`RuntimeErrorCategory::Retryable`, listing its GTS type ID in `non_retryable_errors` opts it
out of retries. This allows entrypoint authors to suppress retries for specific error types
(e.g., a retryable upstream timeout that is known to be unrecoverable in a particular context)
without changing the error's category at the source.

Errors with any other `RuntimeErrorCategory` (`NonRetryable`, `ResourceLimit`, `Timeout`,
`Canceled`) are never retried, irrespective of their presence in `non_retryable_errors`.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.retry_policy.v1~",
  "title": "Retry Policy",
  "description": "Retry configuration for failed invocations.",
  "type": "object",
  "properties": {
    "max_attempts": {
      "type": "integer",
      "minimum": 0,
      "default": 3
    },
    "initial_delay_ms": {
      "type": "integer",
      "minimum": 0,
      "default": 200
    },
    "max_delay_ms": {
      "type": "integer",
      "minimum": 0,
      "default": 10000
    },
    "backoff_multiplier": {
      "type": "number",
      "minimum": 1.0,
      "default": 2.0
    },
    "non_retryable_errors": {
      "type": "array",
      "items": {
        "type": "string",
        "x-gts-ref": "gts.*"
      }
    }
  },
  "required": [
    "max_attempts"
  ]
}
```

#### RateLimit

**GTS ID:** `gts.x.core.serverless.rate_limit.v1~`

Configures rate limiting for an entrypoint. Rate limiting is implemented as a **plugin** — the
platform provides a default rate limiter, but operators can register custom rate limiter
implementations via the plugin system with their own configuration schemas.

##### Scope and Isolation

Rate limits are enforced **per-entrypoint per-owner**:

- **Isolated across tenants** — tenant A's traffic never counts toward tenant B's limits.
- **Applies to both sync and async invocation modes** — the limit is checked at invocation
  acceptance time, before the request is queued or dispatched.

##### Base Schema

The base rate limit type is an empty marker — it carries no fields. It exists solely as the GTS
root for the rate-limiting type family. Each rate limiter plugin registers a derived GTS type that
defines the strategy-specific configuration schema.

The entrypoint's `rate_limit` field uses a `strategy` + `config` structure to reference a rate
limiter: `strategy` is the GTS type ID of the plugin, `config` is an opaque object validated by
that plugin. This structure is defined inline in the entrypoint schema, not in the base type.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.rate_limit.v1~",
  "title": "Rate Limit (Base)",
  "description": "Base rate limiting type. Empty marker — strategy-specific configuration is defined by derived types.",
  "type": "object",
  "additionalProperties": true
}
```

##### Plugin-Derived Config Schemas

Each rate limiter plugin registers a derived GTS type that defines the schema for the `config`
object in the entrypoint's `rate_limit` field:

| `strategy` GTS Type                | Strategy                      | `config` Fields                                                    |
|------------------------------------|-------------------------------|--------------------------------------------------------------------|
| `...rate_limit.token_bucket.v1~`   | Token bucket (system default) | `max_requests_per_second`, `max_requests_per_minute`, `burst_size` |
| `...rate_limit.sliding_window.v1~` | Sliding window (example)      | `window_size_seconds`, `max_requests_per_window`                   |

##### Default: Token Bucket Rate Limiter

The system-provided rate limiter uses the **token bucket** algorithm. Both per-second and per-minute
limits are enforced independently — an invocation must pass both limits to be accepted.

- `max_requests_per_second` — sustained per-second rate. `0` means no per-second limit.
- `max_requests_per_minute` — sustained per-minute rate. `0` means no per-minute limit.
- `burst_size` — maximum instantaneous burst allowed by the per-second bucket. Permits short
  traffic spikes up to `burst_size` requests before the per-second rate takes effect. Does not
  apply to the per-minute limit.

If both `max_requests_per_second` and `max_requests_per_minute` are `0`, rate limiting is disabled
for this entrypoint.

###### Config Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.rate_limit.v1~x.core.serverless.rate_limit.token_bucket.v1~",
  "title": "Token Bucket Rate Limit Config",
  "description": "Config schema for the system-default token bucket rate limiter. Per-second and per-minute limits enforced independently.",
  "type": "object",
  "properties": {
    "max_requests_per_second": {
      "type": "number",
      "minimum": 0,
      "default": 0,
      "description": "Maximum sustained invocations per second. 0 means no per-second limit."
    },
    "max_requests_per_minute": {
      "type": "integer",
      "minimum": 0,
      "default": 0,
      "description": "Maximum sustained invocations per minute. 0 means no per-minute limit."
    },
    "burst_size": {
      "type": "integer",
      "minimum": 1,
      "default": 10,
      "description": "Maximum instantaneous burst for the per-second bucket. Permits short traffic spikes before the per-second rate takes effect."
    }
  }
}
```

##### Instance Example (Token Bucket)

```json
{
  "strategy": "gts.x.core.serverless.rate_limit.v1~x.core.serverless.rate_limit.token_bucket.v1~",
  "config": {
    "max_requests_per_second": 50,
    "max_requests_per_minute": 1000,
    "burst_size": 20
  }
}
```

##### Plugin Model

The rate limiter is registered as a plugin implementing the `RateLimiter` trait. Each plugin
handles exactly one strategy GTS type (1:1 mapping). The runtime resolves the plugin based on
`rate_limit.strategy`.

- The **default** system-provided plugin handles `token_bucket.v1~` and uses an in-process token
  bucket.
- Custom plugins may implement distributed rate limiting (e.g., Redis-backed), sliding window
  algorithms, or tenant-aware adaptive throttling — each with its own derived GTS configuration
  schema.
- The plugin receives `rate_limit.config` as opaque JSON (`serde_json::Value`) and is responsible
  for deserializing it into its own config type.

##### Validation

When registering an entrypoint with a `rate_limit` configuration, the runtime:

1. Reads `rate_limit.strategy` to identify the rate limiter plugin.
2. Looks up the registered `RateLimiter` plugin that handles that strategy GTS type.
3. Validates `rate_limit.config` against the plugin's config schema.
4. Rejects registration if no plugin handles the strategy or config validation fails.

##### Error Behavior

When an invocation is rejected due to rate limiting:

- The HTTP API returns **`429 Too Many Requests`** with a `Retry-After` header indicating when the
  caller may retry (in seconds).
- The response body is an RFC 9457 Problem Details JSON with error type
  `gts.x.core.serverless.err.v1~x.core.serverless.err.rate_limited.v1~`.
- The invocation is **not** created — no `InvocationRecord` is persisted and no retries are
  attempted by the runtime. The caller is responsible for respecting `Retry-After` and retrying.

##### Example: 429 Error Response

```http
HTTP/1.1 429 Too Many Requests
Content-Type: application/problem+json
Retry-After: 2
```

```json
{
  "type": "gts://gts.x.core.serverless.err.v1~x.core.serverless.err.rate_limited.v1~",
  "title": "Rate Limit Exceeded",
  "status": 429,
  "detail": "Entrypoint rate limit exceeded for tenant t_123. Retry after 2 seconds.",
  "instance": "/api/serverless-runtime/v1/invocations",
  "retry_after_seconds": 2
}
```

#### Implementation

**GTS ID:** `gts.x.core.serverless.implementation.v1~`

Defines how an entrypoint is implemented. The `adapter` field explicitly identifies the runtime
adapter, enabling validation of adapter-specific limits and traits.

##### Fields

- `adapter` — GTS type ID of the adapter (e.g., `gts.x.core.serverless.adapter.starlark.v1~`). Required for limits
  validation.
- `kind` — implementation kind: `code`, `workflow_spec`, or `adapter_ref`
- Kind-specific payload with implementation details

##### Kinds

- `code` — inline source code for embedded runtimes (Starlark, WASM, etc.)
- `workflow_spec` — declarative workflow definition (Serverless Workflow DSL, Temporal, etc.)
- `adapter_ref` — reference to an adapter-provided definition for hot-plug registration (PRD BR-035)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.implementation.v1~",
  "title": "Entrypoint Implementation",
  "description": "Implementation definition with explicit adapter for limits validation.",
  "type": "object",
  "properties": {
    "adapter": {
      "type": "string",
      "x-gts-ref": "gts.x.core.serverless.adapter.*",
      "description": "GTS type ID of the adapter (e.g., gts.x.core.serverless.adapter.starlark.v1~)."
    }
  },
  "required": [
    "adapter"
  ],
  "oneOf": [
    {
      "properties": {
        "kind": {
          "const": "code"
        },
        "code": {
          "type": "object",
          "properties": {
            "language": {
              "type": "string",
              "description": "Source language (e.g., starlark, wasm)."
            },
            "source": {
              "type": "string",
              "description": "Inline source code."
            }
          },
          "required": [
            "language",
            "source"
          ]
        }
      },
      "required": [
        "kind",
        "code"
      ]
    },
    {
      "properties": {
        "kind": {
          "const": "workflow_spec"
        },
        "workflow_spec": {
          "type": "object",
          "properties": {
            "format": {
              "type": "string",
              "description": "Workflow format (e.g., serverless-workflow)."
            },
            "spec": {
              "type": "object",
              "description": "Workflow specification object."
            }
          },
          "required": [
            "format",
            "spec"
          ]
        }
      },
      "required": [
        "kind",
        "workflow_spec"
      ]
    },
    {
      "properties": {
        "kind": {
          "const": "adapter_ref"
        },
        "adapter_ref": {
          "type": "object",
          "properties": {
            "definition_id": {
              "type": "string",
              "description": "Adapter-specific definition identifier."
            }
          },
          "required": [
            "definition_id"
          ]
        }
      },
      "required": [
        "kind",
        "adapter_ref"
      ]
    }
  ]
}
```

##### Validation Flow

1. Parse `implementation.adapter` to get the adapter GTS type (e.g., `gts.x.core.serverless.adapter.starlark.v1~`)
2. Derive the adapter's limits schema: `gts.x.core.serverless.limits.v1~x.core.serverless.adapter.starlark.limits.v1~`
3. Validate `traits.limits` against the derived limits schema
4. Reject entrypoint if limits contain fields not supported by the adapter

#### WorkflowTraits

**GTS ID:** `gts.x.core.serverless.workflow_traits.v1~`

Workflow-specific execution traits required for durable orchestrations. Includes:

- `compensation` — saga pattern support (PRD BR-133): entrypoint references for compensation on failure/cancel
- `checkpointing` — durability strategy: `automatic`, `manual`, or `disabled` (PRD BR-009)
- `max_suspension_days` — maximum time a workflow can remain suspended waiting for events (PRD BR-009)

##### Compensation Design

Since all possible runtimes cannot generically implement compensation logic (e.g., "compensate all completed steps")
compensation handlers are explicit entrypoint references. The workflow author defines a separate function or workflow
that implements the compensation logic:

- `on_failure` — GTS ID of entrypoint to invoke when workflow fails, or `null` for no compensation
- `on_cancel` — GTS ID of entrypoint to invoke when workflow is canceled, or `null` for no compensation

The referenced compensation entrypoint is invoked as a standard entrypoint with a single JSON body
conforming to the `CompensationContext` schema (`gts.x.core.serverless.compensation_context.v1~`).
This context carries the original invocation identity, failure details, and a workflow state snapshot
so the handler has everything it needs to perform rollback operations. See the
[CompensationContext](#compensationcontext) section below for the full schema, field descriptions,
and usage examples.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.workflow_traits.v1~",
  "title": "Workflow Traits",
  "description": "Workflow-specific execution traits: compensation, checkpointing, suspension.",
  "type": "object",
  "properties": {
    "compensation": {
      "type": "object",
      "description": "Compensation handlers for saga pattern. Each handler is an entrypoint reference or null. Referenced entrypoints receive a CompensationContext (gts.x.core.serverless.compensation_context.v1~) as their input.",
      "properties": {
        "on_failure": {
          "oneOf": [
            {
              "type": "string",
              "x-gts-ref": "gts.x.core.serverless.entrypoint.*",
              "description": "GTS ID of entrypoint to invoke on workflow failure. Receives CompensationContext as input."
            },
            {
              "type": "null"
            }
          ],
          "default": null,
          "description": "Entrypoint to invoke for compensation on failure, or null for no compensation. Invoked with CompensationContext as the single JSON body."
        },
        "on_cancel": {
          "oneOf": [
            {
              "type": "string",
              "x-gts-ref": "gts.x.core.serverless.entrypoint.*",
              "description": "GTS ID of entrypoint to invoke on workflow cancellation. Receives CompensationContext as input."
            },
            {
              "type": "null"
            }
          ],
          "default": null,
          "description": "Entrypoint to invoke for compensation on cancel, or null for no compensation. Invoked with CompensationContext as the single JSON body."
        }
      }
    },
    "checkpointing": {
      "type": "object",
      "properties": {
        "strategy": {
          "enum": [
            "automatic",
            "manual",
            "disabled"
          ],
          "default": "automatic"
        }
      }
    },
    "max_suspension_days": {
      "type": "integer",
      "minimum": 1,
      "default": 30
    }
  },
  "required": [
    "compensation",
    "checkpointing",
    "max_suspension_days"
  ]
}
```

#### CompensationContext

**GTS ID:** `gts.x.core.serverless.compensation_context.v1~`

Defines the input envelope passed to compensation entrypoints referenced by
`traits.workflow.compensation.on_failure` and `traits.workflow.compensation.on_cancel`.

When the runtime transitions an invocation to the `compensating` status, it constructs a
`CompensationContext` and invokes the referenced compensation entrypoint through the standard
invocation flow, passing the context as the **single JSON body** (i.e., the `params` field of
the invocation request). The compensation entrypoint is a regular entrypoint — no special
runtime path is needed.

##### Design

The platform owns the envelope structure and guarantees the required fields are always present.
The `workflow_state_snapshot` is populated by the adapter from its own checkpoint format and is
opaque to the platform. This split keeps the contract explicit for handler authors while
allowing adapters full flexibility in their state representation.

##### Required Fields

| Field                             | Type   | Required | Description                                                                                                                                                                                                                 |
|-----------------------------------|--------|----------|-----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| `trigger`                         | string | Yes      | What caused compensation: `"failure"` or `"cancellation"`. Maps to `on_failure` or `on_cancel`.                                                                                                                             |
| `original_workflow_invocation_id` | string | Yes      | Invocation ID of the workflow run being compensated. Primary correlation key.                                                                                                                                               |
| `failed_step_id`                  | string | Yes      | Identifier of the step that failed or was active at cancellation time. Adapter-specific granularity. Set to `"unknown"` when the adapter does not track step-level state.                                                   |
| `failed_step_error`               | object | No       | Error details for the failed step. Present when `trigger` is `"failure"`.                                                                                                                                                   |
| `workflow_state_snapshot`         | object | Yes      | Last checkpointed workflow state. Empty object `{}` if failure occurred before the first checkpoint. |
| `timestamp`                       | string | Yes      | ISO 8601 timestamp of when compensation was triggered.                                                                                                                                                                      |
| `invocation_metadata`             | object | Yes      | Metadata from the original invocation: entrypoint ID, original input, tenant, observability IDs.                                                                                                                            |

##### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.compensation_context.v1~",
  "title": "Compensation Context",
  "description": "Input envelope passed to compensation entrypoints. Delivered as the single JSON body (params) when the runtime invokes an on_failure or on_cancel handler.",
  "type": "object",
  "required": [
    "trigger",
    "original_workflow_invocation_id",
    "failed_step_id",
    "workflow_state_snapshot",
    "timestamp",
    "invocation_metadata"
  ],
  "properties": {
    "trigger": {
      "type": "string",
      "enum": [
        "failure",
        "cancellation"
      ],
      "description": "What caused compensation to start. 'failure' maps to on_failure, 'cancellation' maps to on_cancel."
    },
    "original_workflow_invocation_id": {
      "type": "string",
      "description": "Invocation ID of the failed/cancelled workflow run. Use this to correlate compensation actions with the original execution."
    },
    "failed_step_id": {
      "type": "string",
      "description": "Identifier of the step that failed or was active at cancellation. Adapter-specific granularity. Set to 'unknown' when the adapter does not track step-level state."
    },
    "failed_step_error": {
      "type": "object",
      "description": "Error details for the failed step. Present when trigger is 'failure', absent for 'cancellation'.",
      "properties": {
        "error_type": {
          "type": "string",
          "description": "Categorized error type (e.g., 'timeout', 'runtime_error', 'resource_exhausted')."
        },
        "message": {
          "type": "string",
          "description": "Human-readable error description."
        },
        "error_metadata": {
          "type": "object",
          "additionalProperties": true,
          "description": "Error-type-specific metadata. Structure is defined per error type (out of scope for this ADR; to be specified alongside the error taxonomy)."
        }
      },
      "required": [
        "error_type",
        "message"
      ]
    },
    "workflow_state_snapshot": {
      "type": "object",
      "description": "Last checkpointed workflow state. Adapter-specific and opaque to the platform. Contains accumulated step results, intermediate data, or adapter-native state. Empty object if failure occurred before the first checkpoint.",
      "additionalProperties": true
    },
    "timestamp": {
      "type": "string",
      "format": "date-time",
      "description": "ISO 8601 timestamp of when compensation was triggered."
    },
    "invocation_metadata": {
      "type": "object",
      "description": "Metadata from the original workflow invocation.",
      "required": [
        "entrypoint_id",
        "original_input",
        "tenant_id"
      ],
      "properties": {
        "entrypoint_id": {
          "type": "string",
          "x-gts-ref": "gts.x.core.serverless.entrypoint.*",
          "description": "GTS ID of the workflow entrypoint that failed."
        },
        "original_input": {
          "type": "object",
          "description": "The input parameters (params) the workflow was originally invoked with."
        },
        "tenant_id": {
          "type": "string",
          "description": "Tenant that owns the workflow invocation."
        },
        "correlation_id": {
          "type": "string",
          "description": "Correlation ID from the original invocation's observability context."
        },
        "started_at": {
          "type": "string",
          "format": "date-time",
          "description": "When the original workflow invocation started."
        }
      }
    }
  }
}
```

##### Example Payload

An order-processing workflow fails on step `create_shipping_label` after successfully completing
`reserve_inventory` and `charge_payment`. The runtime constructs the following `CompensationContext`
and invokes the `on_failure` entrypoint:

```json
{
  "trigger": "failure",
  "original_workflow_invocation_id": "inv_a1b2c3d4",
  "failed_step_id": "create_shipping_label",
  "failed_step_error": {
    "error_type": "runtime_error",
    "message": "Shipping provider returned 503: service unavailable",
    "error_metadata": { "retries_exhausted": true, "last_attempt": 5 }
  },
  "workflow_state_snapshot": {
    "reservation_id": "RSV-7712",
    "payment_transaction_id": "TXN-33401",
    "shipping_label": null,
    "completed_steps": [
      "reserve_inventory",
      "charge_payment"
    ]
  },
  "timestamp": "2026-02-08T10:00:47Z",
  "invocation_metadata": {
    "entrypoint_id": "gts.x.core.serverless.entrypoint.v1~x.core.serverless.workflow.v1~vendor.app.orders.process_order.v1~",
    "original_input": {
      "order_id": "ORD-9182",
      "items": [
        {
          "sku": "WIDGET-01",
          "qty": 3
        },
        {
          "sku": "GADGET-05",
          "qty": 1
        }
      ],
      "payment": {
        "method": "card",
        "token": "tok_abc123"
      }
    },
    "tenant_id": "t_123",
    "correlation_id": "corr_789",
    "started_at": "2026-02-08T10:00:00Z"
  }
}
```

##### How Compensation Handlers Use the Context

The compensation entrypoint receives the `CompensationContext` as its `params` input. Handler
authors should:

1. **Read `original_workflow_invocation_id`** to correlate compensation actions with the
   original workflow run. This is essential for idempotent rollback — the handler can check
   whether compensation for this invocation has already been performed.

2. **Read `failed_step_id`** to determine how far the workflow progressed. The handler
   iterates backward from the failed step through the `workflow_state_snapshot` to decide which
   forward actions need reversal. For example, if `failed_step_id` is `"create_shipping_label"`,
   the handler knows `reserve_inventory` and `charge_payment` completed and need rollback.

3. **Read `workflow_state_snapshot`** to obtain the forward-step outputs required for
   reversal (e.g., `reservation_id` to release inventory, `payment_transaction_id` to issue a
   refund).

4. **Inspect `failed_step_error`** (when `trigger` is `"failure"`) to adjust compensation
   strategy — e.g., a timeout error may warrant different handling than a validation error.

5. **Use `invocation_metadata.original_input`** when the original request parameters are
   needed for rollback (e.g., re-reading the order details to construct a cancellation notice).

##### Registration Validation

When registering or updating a workflow with `traits.workflow.compensation.on_failure` or `on_cancel`:

1. The referenced entrypoint **must** exist and be in `active` status.
2. The referenced entrypoint's `schema.params` **must** accept `CompensationContext`
   (`$ref: "gts://gts.x.core.serverless.compensation_context.v1~"` or a compatible superset).
3. The platform rejects registration if either condition is not met.

#### InvocationStatus

**GTS ID:** `gts.x.core.serverless.status.v1~`

Invocation lifecycle states. Each state is a derived GTS type extending the base status type.
Per PRD BR-015 and BR-014, invocations transition through these states during their lifecycle.

##### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.status.v1~",
  "title": "Invocation Status",
  "description": "Base type for invocation status. Concrete statuses are derived types.",
  "type": "string",
  "enum": [
    "queued",
    "running",
    "suspended",
    "succeeded",
    "failed",
    "canceled",
    "compensating",
    "compensated",
    "dead_lettered"
  ]
}
```

##### Derived Status Types

| GTS Type                                                                     | Description                                         |
|------------------------------------------------------------------------------|-----------------------------------------------------|
| `gts.x.core.serverless.status.v1~x.core.serverless.status.queued.v1~`        | Waiting to be scheduled                             |
| `gts.x.core.serverless.status.v1~x.core.serverless.status.running.v1~`       | Currently executing                                 |
| `gts.x.core.serverless.status.v1~x.core.serverless.status.suspended.v1~`     | Paused, waiting for event or signal                 |
| `gts.x.core.serverless.status.v1~x.core.serverless.status.succeeded.v1~`     | Completed successfully                              |
| `gts.x.core.serverless.status.v1~x.core.serverless.status.failed.v1~`        | Failed after retries exhausted                      |
| `gts.x.core.serverless.status.v1~x.core.serverless.status.canceled.v1~`      | Canceled by user or system                          |
| `gts.x.core.serverless.status.v1~x.core.serverless.status.compensating.v1~`  | Running compensation logic                          |
| `gts.x.core.serverless.status.v1~x.core.serverless.status.compensated.v1~`   | Compensation completed successfully (rollback done) |
| `gts.x.core.serverless.status.v1~x.core.serverless.status.dead_lettered.v1~` | Moved to dead letter queue (BR-027)                 |

##### Invocation Status State Machine

```mermaid
stateDiagram-v2
    [*] --> queued : start invocation

    queued --> running : scheduled
    queued --> canceled : cancel

    running --> succeeded : completed
    running --> failed : error (retries exhausted)
    running --> suspended : await event/signal or manual suspend (workflow)
    running --> canceled : cancel

    suspended --> running : resume / signal received
    suspended --> canceled : cancel
    suspended --> failed : suspension timeout

    failed --> compensating : compensation configured
    failed --> dead_lettered : no compensation
    failed --> queued : retry

    canceled --> compensating : compensation configured
    canceled --> [*] : no compensation

    compensating --> compensated : compensation succeeded
    compensating --> dead_lettered : compensation failed

    succeeded --> [*]
    compensated --> [*]
    dead_lettered --> [*]
```

**Note on `replay`:** The `replay` control action creates a **new** invocation (new `invocation_id`, starts at
`queued`) using the same parameters as the original. It does not transition the original invocation's state.
Replay is valid from `succeeded` or `failed` terminal states.

##### Allowed Transitions

| From         | To            | Trigger                                                                       |
|--------------|---------------|-------------------------------------------------------------------------------|
| (start)      | queued        | `start_invocation` API call                                                   |
| queued       | running       | Scheduler picks up invocation                                                 |
| queued       | canceled      | `control_invocation(Cancel)` before start                                     |
| running      | succeeded     | Execution completes successfully                                              |
| running      | failed        | Execution fails after retry exhaustion                                        |
| running      | suspended     | Workflow awaits event/signal or `control_invocation(Suspend)` (workflow only) |
| running      | canceled      | `control_invocation(Cancel)` during run                                       |
| suspended    | running       | `control_invocation(Resume)` or signal                                        |
| suspended    | canceled      | `control_invocation(Cancel)` while suspended                                  |
| suspended    | failed        | Suspension timeout exceeded                                                   |
| failed       | queued        | `control_invocation(Retry)` — re-queues with same params                      |
| failed       | compensating  | Compensation handler configured                                               |
| failed       | dead_lettered | No compensation, moved to DLQ                                                 |
| canceled     | compensating  | Compensation handler configured                                               |
| compensating | compensated   | Compensation completed successfully                                           |
| compensating | dead_lettered | Compensation failed, moved to DLQ                                             |

#### Error

**GTS ID:** `gts.x.core.serverless.err.v1~`

Standardized error types for invocation failures. Per PRD BR-129, errors include a stable identifier,
human-readable message, and structured details.

##### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.err.v1~",
  "title": "Serverless Error",
  "description": "Base error type. Concrete errors are derived types.",
  "type": "object",
  "properties": {
    "message": {
      "type": "string",
      "description": "Human-readable error message."
    },
    "category": {
      "type": "string",
      "enum": [
        "retryable",
        "non_retryable",
        "resource_limit",
        "timeout",
        "canceled"
      ],
      "description": "Error category for retry decisions."
    },
    "details": {
      "type": "object",
      "description": "Error-specific structured payload."
    }
  },
  "required": [
    "message",
    "category"
  ]
}
```

##### Derived Error Types

| GTS Type                                                                          | HTTP | Description                                  |
|-----------------------------------------------------------------------------------|------|----------------------------------------------|
| `gts.x.core.serverless.err.v1~x.core.serverless.err.validation.v1~`              | 422  | Input or definition validation failure       |
| `gts.x.core.serverless.err.v1~x.core.serverless.err.rate_limited.v1~`            | 429  | Per-entrypoint rate limit exceeded           |
| `gts.x.core.serverless.err.v1~x.core.serverless.err.not_found.v1~`               | 404  | Referenced entity does not exist             |
| `gts.x.core.serverless.err.v1~x.core.serverless.err.not_active.v1~`              | 409  | Entrypoint exists but is not in active state |
| `gts.x.core.serverless.err.v1~x.core.serverless.err.quota_exceeded.v1~`          | 429  | Tenant quota capacity reached                |

#### ValidationError

**GTS ID:** `gts.x.core.serverless.err.v1~x.core.serverless.err.validation.v1~`

Validation error for definition and input validation failures. Per PRD BR-011, validation errors
include the location in the definition and suggested corrections. Returned only when validation fails;
success returns the validated definition. A single validation error can contain multiple issues,
each with its own error type and location.

##### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.err.v1~x.core.serverless.err.validation.v1~",
  "title": "Validation Error",
  "description": "Validation error with multiple issues, each with error type and location.",
  "allOf": [
    {
      "$ref": "gts://gts.x.core.serverless.err.v1~"
    },
    {
      "type": "object",
      "properties": {
        "issues": {
          "type": "array",
          "description": "List of validation issues.",
          "minItems": 1,
          "items": {
            "type": "object",
            "description": "A single validation issue with type, location, and message.",
            "properties": {
              "error_type": {
                "type": "string",
                "description": "Specific validation error type (e.g., 'schema_mismatch', 'missing_field', 'invalid_format')."
              },
              "location": {
                "type": "object",
                "description": "Location of the issue in the definition or input.",
                "properties": {
                  "path": {
                    "type": "string",
                    "description": "JSON path to the error location (e.g., '$.traits.limits.timeout_seconds')."
                  },
                  "line": {
                    "type": [
                      "integer",
                      "null"
                    ],
                    "description": "Line number in source code (for code implementations)."
                  },
                  "column": {
                    "type": [
                      "integer",
                      "null"
                    ],
                    "description": "Column number in source code (for code implementations)."
                  }
                },
                "required": [
                  "path"
                ]
              },
              "message": {
                "type": "string",
                "description": "Human-readable description of the issue."
              },
              "suggestion": {
                "type": [
                  "string",
                  "null"
                ],
                "description": "Suggested correction or fix for the issue."
              }
            },
            "required": [
              "error_type",
              "location",
              "message"
            ]
          }
        }
      },
      "required": [
        "issues"
      ]
    }
  ]
}
```

#### InvocationTimelineEvent

**GTS ID:** `gts.x.core.serverless.timeline_event.v1~`

Represents a single event in the invocation execution timeline. Used for debugging, auditing,
and execution history visualization per PRD BR-015 and BR-130.

##### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.timeline_event.v1~",
  "title": "Invocation Timeline Event",
  "description": "A single event in the execution timeline.",
  "type": "object",
  "properties": {
    "at": {
      "type": "string",
      "format": "date-time",
      "description": "Timestamp when the event occurred."
    },
    "event_type": {
      "type": "string",
      "enum": [
        "started",
        "step_started",
        "step_completed",
        "step_failed",
        "step_retried",
        "suspended",
        "resumed",
        "signal_received",
        "checkpoint_created",
        "compensation_started",
        "compensation_completed",
        "compensation_failed",
        "succeeded",
        "failed",
        "canceled",
        "dead_lettered"
      ],
      "description": "Type of timeline event."
    },
    "status": {
      "$ref": "gts://gts.x.core.serverless.status.v1~",
      "description": "Invocation status after this event (short enum value, e.g. 'running')."
    },
    "step_name": {
      "type": [
        "string",
        "null"
      ],
      "description": "Name of the step (for step-related events)."
    },
    "duration_ms": {
      "type": [
        "integer",
        "null"
      ],
      "minimum": 0,
      "description": "Duration of the step or action in milliseconds."
    },
    "message": {
      "type": [
        "string",
        "null"
      ],
      "description": "Human-readable description of the event."
    },
    "details": {
      "type": "object",
      "description": "Event-specific structured data.",
      "default": {}
    }
  },
  "required": [
    "at",
    "event_type",
    "status"
  ]
}
```

#### Entrypoint (Base Type)

**GTS ID:** `gts.x.core.serverless.entrypoint.v1~`

The base entrypoint schema defines common fields for all functions and workflows.

##### Entrypoint Status State Machine

```mermaid
stateDiagram-v2
    [*] --> draft : register

    draft --> active : activate
    draft --> [*] : delete (hard delete)

    active --> deprecated : deprecate
    active --> disabled : disable

    deprecated --> disabled : disable
    deprecated --> archived : archive

    disabled --> active : enable
    disabled --> archived : archive

    archived --> [*]
```

##### Allowed Transitions

| From       | To         | Action    | Description                                      |
|------------|------------|-----------|--------------------------------------------------|
| (start)    | draft      | register  | New entrypoint registered                        |
| draft      | active     | activate  | Entrypoint ready for invocation                  |
| draft      | (deleted)  | delete    | Hard delete (only in draft status)               |
| active     | deprecated | deprecate | Mark as deprecated (still callable, discouraged) |
| active     | disabled   | disable   | Disable invocation (not callable)                |
| deprecated | disabled   | disable   | Disable deprecated entrypoint                    |
| deprecated | archived   | archive   | Archive for historical reference                 |
| disabled   | active     | enable    | Re-enable for invocation                         |
| disabled   | archived   | archive   | Archive disabled entrypoint                      |

##### Status Behavior

| Status     | Callable | Editable | Visible in Registry | Notes                              |
|------------|----------|----------|---------------------|------------------------------------|
| draft      | No       | Yes      | Yes                 | Work in progress                   |
| active     | Yes      | No       | Yes                 | Production-ready, immutable        |
| deprecated | Yes      | No       | Yes                 | Callable but discouraged           |
| disabled   | No       | No       | Yes                 | Temporarily unavailable            |
| archived   | No       | No       | Optional            | Historical reference, soft-deleted |

##### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.entrypoint.v1~",
  "title": "Serverless Entrypoint",
  "description": "Base schema for serverless entrypoints (functions and workflows). Identity is the GTS instance address.",
  "type": "object",
  "properties": {
    "version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$"
    },
    "tenant_id": {
      "type": "string"
    },
    "owner": {
      "$ref": "gts://gts.x.core.serverless.owner_ref.v1~"
    },
    "status": {
      "type": "string",
      "enum": [
        "draft",
        "active",
        "deprecated",
        "disabled",
        "archived"
      ],
      "default": "draft"
    },
    "tags": {
      "type": "array",
      "items": {
        "type": "string"
      },
      "default": []
    },
    "title": {
      "type": "string"
    },
    "description": {
      "type": "string"
    },
    "schema": {
      "$ref": "gts://gts.x.core.serverless.io_schema.v1~"
    },
    "traits": {
      "type": "object",
      "properties": {
        "invocation": {
          "type": "object",
          "properties": {
            "supported": {
              "type": "array",
              "items": {
                "enum": [
                  "sync",
                  "async"
                ]
              }
            },
            "default": {
              "enum": [
                "sync",
                "async"
              ]
            }
          },
          "required": [
            "supported",
            "default"
          ]
        },
        "is_idempotent": {
          "type": "boolean",
          "default": false
        },
        "caching": {
          "type": "object",
          "description": "Response caching policy. Caching is only active when the caller provides an `Idempotency-Key` header AND `max_age_seconds > 0`",
          "properties": {
            "max_age_seconds": {
              "type": "integer",
              "minimum": 0,
              "default": 0,
              "description": "Time-to-live in seconds for cached successful results. `0` disables response caching even when an idempotency key is present."
            }
          }
        },
        "rate_limit": {
          "description": "Optional rate limiting. Null or absent means no rate limiting.",
          "oneOf": [
            {
              "type": "object",
              "required": ["strategy", "config"],
              "properties": {
                "strategy": {
                  "type": "string",
                  "description": "GTS type ID of the rate limiter plugin (derived from gts.x.core.serverless.rate_limit.v1~)."
                },
                "config": {
                  "type": "object",
                  "description": "Strategy-specific configuration. Validated by the resolved plugin against its derived schema.",
                  "additionalProperties": true
                }
              },
              "additionalProperties": false
            },
            { "type": "null" }
          ],
          "default": null
        },
        "limits": {
          "$ref": "gts://gts.x.core.serverless.limits.v1~"
        },
        "retry": {
          "$ref": "gts://gts.x.core.serverless.retry_policy.v1~"
        }
      },
      "required": [
        "invocation",
        "limits",
        "retry"
      ]
    },
    "implementation": {
      "$ref": "gts://gts.x.core.serverless.implementation.v1~"
    },
    "created_at": {
      "type": "string",
      "format": "date-time"
    },
    "updated_at": {
      "type": "string",
      "format": "date-time"
    }
  },
  "required": [
    "version",
    "tenant_id",
    "owner",
    "status",
    "title",
    "schema",
    "traits",
    "implementation"
  ],
  "additionalProperties": true
}
```

#### Function

**GTS ID:** `gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~`

Functions are stateless, short-lived entrypoints designed for request/response invocation:

- Stateless with respect to the runtime (durable state lives externally)
- Typically short-lived and bounded by platform timeout limits
- Commonly used as building blocks for APIs, event handlers, and single-step jobs
- Authors SHOULD design for idempotency when side effects are possible

##### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~",
  "title": "Serverless Function",
  "description": "Stateless, short-lived entrypoint for request/response invocation.",
  "allOf": [
    {
      "$ref": "gts://gts.x.core.serverless.entrypoint.v1~"
    }
  ]
}
```

##### Instance Example

GTS Address: `gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~vendor.app.billing.calculate_tax.v1~`

```json
{
  "version": "1.0.0",
  "tenant_id": "t_123",
  "owner": {
    "owner_type": "user",
    "id": "u_456",
    "tenant_id": "t_123"
  },
  "status": "active",
  "tags": [
    "billing",
    "tax"
  ],
  "title": "Calculate Tax",
  "description": "Calculate tax for invoice.",
  "schema": {
    "params": {
      "type": "object",
      "properties": {
        "invoice_id": {
          "type": "string"
        },
        "amount": {
          "type": "number"
        }
      },
      "required": [
        "invoice_id",
        "amount"
      ]
    },
    "returns": {
      "type": "object",
      "properties": {
        "tax": {
          "type": "number"
        },
        "total": {
          "type": "number"
        }
      }
    },
    "errors": [
      "gts.x.core.serverless.err.v1~x.core.serverless.err.validation.v1~"
    ]
  },
  "traits": {
    "invocation": {
      "supported": [
        "sync",
        "async"
      ],
      "default": "async"
    },
    "is_idempotent": true,
    "caching": {
      "max_age_seconds": 0
    },
    "rate_limit": {
      "strategy": "gts.x.core.serverless.rate_limit.v1~x.core.serverless.rate_limit.token_bucket.v1~",
      "config": {
        "max_requests_per_second": 50,
        "max_requests_per_minute": 1000,
        "burst_size": 20
      }
    },
    "limits": {
      "timeout_seconds": 30,
      "memory_mb": 128,
      "cpu": 0.2,
      "max_concurrent": 100
    },
    "retry": {
      "max_attempts": 3,
      "initial_delay_ms": 200,
      "max_delay_ms": 10000,
      "backoff_multiplier": 2.0
    }
  },
  "implementation": {
    "adapter": "gts.x.core.serverless.adapter.starlark.v1~",
    "kind": "code",
    "code": {
      "language": "starlark",
      "source": "def main(ctx, input):\n  return {\"tax\": input.amount * 0.1, \"total\": input.amount * 1.1}\n"
    }
  },
  "created_at": "2026-01-01T00:00:00.000Z",
  "updated_at": "2026-01-01T00:00:00.000Z"
}
```

#### Workflow

**GTS ID:** `gts.x.core.serverless.entrypoint.v1~x.core.serverless.workflow.v1~`

Workflows are durable, multi-step orchestrations that coordinate actions over time:

- Persisted invocation state (durable progress across restarts)
- Supports long-running behavior (timers, waiting on external events, human-in-the-loop)
- Encodes orchestration logic (fan-out/fan-in, branching, retries, compensation)
- Steps are typically function calls but may also invoke other workflows (sub-orchestration)

The runtime is responsible for:

- Step identification and retry scheduling
- Compensation orchestration
- Checkpointing and suspend/resume
- Event subscription and event-driven continuation

##### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.entrypoint.v1~x.core.serverless.workflow.v1~",
  "title": "Serverless Workflow",
  "description": "Durable, multi-step orchestration with state persistence.",
  "allOf": [
    {
      "$ref": "gts://gts.x.core.serverless.entrypoint.v1~"
    },
    {
      "type": "object",
      "properties": {
        "traits": {
          "type": "object",
          "properties": {
            "workflow": {
              "$ref": "gts://gts.x.core.serverless.workflow_traits.v1~"
            }
          },
          "required": [
            "workflow"
          ]
        }
      },
      "required": [
        "traits"
      ]
    }
  ]
}
```

##### Instance Example

GTS Address: `gts.x.core.serverless.entrypoint.v1~x.core.serverless.workflow.v1~vendor.app.orders.process_order.v1~`

```json
{
  "version": "1.0.0",
  "tenant_id": "t_123",
  "owner": {
    "owner_type": "tenant",
    "id": "t_123",
    "tenant_id": "t_123"
  },
  "status": "active",
  "tags": [
    "orders",
    "processing"
  ],
  "title": "Process Order Workflow",
  "description": "Multi-step order processing with payment and fulfillment.",
  "schema": {
    "params": {
      "type": "object",
      "properties": {
        "order_id": {
          "type": "string"
        },
        "customer_id": {
          "type": "string"
        }
      },
      "required": [
        "order_id",
        "customer_id"
      ]
    },
    "returns": {
      "type": "object",
      "properties": {
        "status": {
          "type": "string"
        },
        "tracking_id": {
          "type": "string"
        }
      }
    },
    "errors": []
  },
  "traits": {
    "invocation": {
      "supported": [
        "async"
      ],
      "default": "async"
    },
    "is_idempotent": false,
    "caching": {
      "max_age_seconds": 0
    },
    "limits": {
      "timeout_seconds": 86400,
      "memory_mb": 256,
      "cpu": 0.5,
      "max_concurrent": 50
    },
    "retry": {
      "max_attempts": 5,
      "initial_delay_ms": 1000,
      "max_delay_ms": 60000,
      "backoff_multiplier": 2.0
    },
    "workflow": {
      "compensation": {
        "on_failure": "gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~vendor.app.orders.rollback_order.v1~",
        "on_cancel": null
      },
      "checkpointing": {
        "strategy": "automatic"
      },
      "max_suspension_days": 30
    }
  },
  "implementation": {
    "adapter": "gts.x.core.serverless.adapter.starlark.v1~",
    "kind": "code",
    "code": {
      "language": "starlark",
      "source": "def main(ctx, input):\n  # workflow steps...\n  return {\"status\": \"completed\"}\n"
    }
  },
  "created_at": "2026-01-01T00:00:00.000Z",
  "updated_at": "2026-01-01T00:00:00.000Z"
}
```

### InvocationRecord

**GTS ID:** `gts.x.core.serverless.invocation.v1~`

An invocation record tracks the lifecycle of a single entrypoint execution, including status, parameters,
results, timing, and observability data. Per PRD BR-015, BR-021, and BR-034, invocations are queryable
with tenant and correlation identifiers for traceability.

#### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.invocation.v1~",
  "title": "Invocation Record",
  "description": "Tracks lifecycle of a single entrypoint execution.",
  "type": "object",
  "properties": {
    "invocation_id": {
      "type": "string",
      "description": "Opaque unique identifier for this invocation."
    },
    "entrypoint_id": {
      "type": "string",
      "x-gts-ref": "gts.x.core.serverless.entrypoint.*",
      "description": "GTS ID of the invoked entrypoint."
    },
    "entrypoint_version": {
      "type": "string",
      "pattern": "^\\d+\\.\\d+\\.\\d+$"
    },
    "tenant_id": {
      "type": "string"
    },
    "status": {
      "$ref": "gts://gts.x.core.serverless.status.v1~",
      "description": "Invocation status (short enum value, e.g. 'running')."
    },
    "mode": {
      "type": "string",
      "enum": [
        "sync",
        "async"
      ]
    },
    "params": {
      "type": "object",
      "description": "Input parameters passed to the entrypoint."
    },
    "result": {
      "description": "Execution result (null if not completed or failed).",
      "oneOf": [
        {
          "type": "object"
        },
        {
          "type": "null"
        }
      ]
    },
    "error": {
      "description": "Error details (null if succeeded or still running).",
      "oneOf": [
        {
          "type": "object",
          "properties": {
            "error_type_id": {
              "type": "string",
              "x-gts-ref": "gts.*"
            },
            "message": {
              "type": "string"
            },
            "category": {
              "type": "string",
              "enum": [
                "retryable",
                "non_retryable",
                "resource_limit",
                "timeout",
                "canceled"
              ],
              "description": "Error category for retry decisions."
              "description": "Error category for retry decisions."
            },
            "details": {
              "type": "object"
            }
          },
          "required": [
            "error_type_id",
            "message",
            "category"
          ]
        },
        {
          "type": "null"
        }
      ]
    },
    "timestamps": {
      "type": "object",
      "properties": {
        "created_at": {
          "type": "string",
          "format": "date-time"
        },
        "started_at": {
          "type": [
            "string",
            "null"
          ],
          "format": "date-time"
        },
        "suspended_at": {
          "type": [
            "string",
            "null"
          ],
          "format": "date-time"
        },
        "finished_at": {
          "type": [
            "string",
            "null"
          ],
          "format": "date-time"
        }
      },
      "required": [
        "created_at"
      ]
    },
    "observability": {
      "type": "object",
      "properties": {
        "correlation_id": {
          "type": "string"
        },
        "trace_id": {
          "type": "string"
        },
        "span_id": {
          "type": "string"
        },
        "metrics": {
          "type": "object",
          "properties": {
            "duration_ms": {
              "type": [
                "integer",
                "null"
              ]
            },
            "billed_duration_ms": {
              "type": [
                "integer",
                "null"
              ]
            },
            "cpu_time_ms": {
              "type": [
                "integer",
                "null"
              ]
            },
            "memory_limit_mb": {
              "type": [
                "integer",
                "null"
              ]
            },
            "max_memory_used_mb": {
              "type": [
                "integer",
                "null"
              ]
            },
            "step_count": {
              "type": [
                "integer",
                "null"
              ]
            }
          }
        }
      },
      "required": [
        "correlation_id"
      ]
    }
  },
  "required": [
    "invocation_id",
    "entrypoint_id",
    "entrypoint_version",
    "tenant_id",
    "status",
    "mode",
    "timestamps",
    "observability"
  ]
}
```

#### Instance Example

```json
{
  "invocation_id": "inv_abc",
  "entrypoint_id": "gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~vendor.app.namespace.calculate_tax.v1~",
  "entrypoint_version": "1.0.0",
  "tenant_id": "t_123",
  "status": "running",
  "mode": "async",
  "params": {
    "invoice_id": "inv_001",
    "amount": 100.0
  },
  "result": null,
  "error": null,
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": "2026-01-01T00:00:00.010Z",
    "suspended_at": null,
    "finished_at": null
  },
  "observability": {
    "correlation_id": "corr_789",
    "trace_id": "trace_123",
    "span_id": "span_456",
    "metrics": {
      "duration_ms": null,
      "billed_duration_ms": null,
      "cpu_time_ms": null,
      "memory_limit_mb": 128,
      "max_memory_used_mb": null,
      "step_count": null
    }
  }
}
```

### Schedule

**GTS ID:** `gts.x.core.serverless.schedule.v1~`

A schedule defines a recurring trigger for an entrypoint based on cron expressions or intervals.
Per PRD BR-007 and BR-022, schedules support lifecycle management and configurable missed schedule policies.

#### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.schedule.v1~",
  "title": "Schedule",
  "description": "Recurring trigger for an entrypoint.",
  "type": "object",
  "properties": {
    "schedule_id": {
      "type": "string",
      "description": "Opaque unique identifier for this schedule."
    },
    "tenant_id": {
      "type": "string"
    },
    "entrypoint_id": {
      "type": "string",
      "x-gts-ref": "gts.x.core.serverless.entrypoint.*",
      "description": "GTS ID of the entrypoint to invoke."
    },
    "name": {
      "type": "string",
      "description": "Human-readable schedule name."
    },
    "timezone": {
      "type": "string",
      "default": "UTC",
      "description": "IANA timezone for schedule evaluation."
    },
    "expression": {
      "type": "object",
      "oneOf": [
        {
          "properties": {
            "kind": {
              "const": "cron"
            },
            "value": {
              "type": "string",
              "description": "Cron expression."
            }
          },
          "required": [
            "kind",
            "value"
          ]
        },
        {
          "properties": {
            "kind": {
              "const": "interval"
            },
            "value": {
              "type": "string",
              "description": "ISO 8601 duration (e.g., PT1H)."
            }
          },
          "required": [
            "kind",
            "value"
          ]
        }
      ]
    },
    "input_overrides": {
      "type": "object",
      "description": "Parameters merged with entrypoint defaults for each scheduled run.",
      "default": {}
    },
    "missed_policy": {
      "type": "string",
      "enum": [
        "skip",
        "catch_up",
        "backfill"
      ],
      "default": "skip",
      "description": "Policy for missed schedules: skip (ignore), catch_up (execute once), backfill (execute each)."
    },
    "status": {
      "type": "string",
      "enum": [
        "active",
        "paused",
        "disabled"
      ],
      "default": "active"
    },
    "next_run_at": {
      "type": [
        "string",
        "null"
      ],
      "format": "date-time"
    },
    "last_run_at": {
      "type": [
        "string",
        "null"
      ],
      "format": "date-time"
    },
    "created_at": {
      "type": "string",
      "format": "date-time"
    },
    "updated_at": {
      "type": "string",
      "format": "date-time"
    }
  },
  "required": [
    "schedule_id",
    "tenant_id",
    "entrypoint_id",
    "name",
    "expression",
    "status"
  ]
}
```

#### Instance Example

```json
{
  "schedule_id": "sch_001",
  "tenant_id": "t_123",
  "entrypoint_id": "gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~vendor.app.billing.calculate_tax.v1~",
  "name": "Daily Tax Calculation",
  "timezone": "UTC",
  "expression": {
    "kind": "cron",
    "value": "0 * * * *"
  },
  "input_overrides": {
    "region": "EU"
  },
  "missed_policy": "skip",
  "status": "active",
  "next_run_at": "2026-01-01T01:00:00.000Z",
  "last_run_at": "2026-01-01T00:00:00.000Z",
  "created_at": "2026-01-01T00:00:00.000Z",
  "updated_at": "2026-01-01T00:00:00.000Z"
}
```

### Trigger

**GTS ID:** `gts.x.core.serverless.trigger.v1~`

A trigger binds an event type to an entrypoint, enabling event-driven invocation.
Per PRD BR-007, triggers are one of three supported trigger mechanisms (schedule, API, event).

#### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.trigger.v1~",
  "title": "Trigger",
  "description": "Binds an event type to an entrypoint for event-driven invocation.",
  "type": "object",
  "properties": {
    "trigger_id": {
      "type": "string",
      "description": "Opaque unique identifier for this trigger."
    },
    "tenant_id": {
      "type": "string"
    },
    "event_type_id": {
      "type": "string",
      "x-gts-ref": "gts.x.core.events.*",
      "description": "GTS ID of the event type to listen for."
    },
    "event_filter_query": {
      "type": "string",
      "description": "Optional filter expression to match specific events. Syntax TBD during EventBroker implementation."
    },
    "dead_letter_queue": {
      "type": "object",
      "description": "Dead letter queue configuration for failed event processing. DLQ management API is out of scope and will be defined during EventBroker implementation.",
      "properties": {
        "enabled": {
          "type": "boolean",
          "default": true,
          "description": "Whether failed events should be moved to DLQ after retry exhaustion."
        },
        "retry_policy": {
          "$ref": "gts://gts.x.core.serverless.retry_policy.v1~",
          "description": "Retry policy before moving to DLQ. Uses exponential backoff with configurable attempts."
        },
        "dlq_topic": {
          "oneOf": [
            {
              "type": "string",
              "x-gts-ref": "gts.x.core.*",
              "description": "GTS type ID of the topic to publish dead-lettered events to."
            },
            {
              "type": "null"
            }
          ],
          "default": null,
          "description": "Optional topic for routing dead-lettered events, or null for the platform-default DLQ topic. Topic type and management are defined by the EventBroker."
        }
      }
    },
    "entrypoint_id": {
      "type": "string",
      "x-gts-ref": "gts.x.core.serverless.entrypoint.*",
      "description": "GTS ID of the entrypoint to invoke."
    },
    "status": {
      "type": "string",
      "enum": [
        "active",
        "paused",
        "disabled"
      ],
      "default": "active"
    },
    "created_at": {
      "type": "string",
      "format": "date-time"
    },
    "updated_at": {
      "type": "string",
      "format": "date-time"
    }
  },
  "required": [
    "trigger_id",
    "tenant_id",
    "event_type_id",
    "entrypoint_id",
    "status"
  ]
}
```

#### Instance Example

```json
{
  "trigger_id": "trg_001",
  "tenant_id": "t_123",
  "event_type_id": "gts.x.core.events.event.v1~vendor.app.orders.approved.v1~",
  "event_filter_query": "payload.order_id != null",
  "entrypoint_id": "gts.x.core.serverless.entrypoint.v1~x.core.serverless.workflow.v1~vendor.app.orders.process_approval.v1~",
  "dead_letter_queue": {
    "enabled": true,
    "retry_policy": {
      "max_attempts": 3,
      "initial_delay_ms": 1000,
      "max_delay_ms": 30000,
      "backoff_multiplier": 2.0
    },
    "dlq_topic": null
  },
  "status": "active",
  "created_at": "2026-01-01T00:00:00.000Z",
  "updated_at": "2026-01-01T00:00:00.000Z"
}
```

### TenantRuntimePolicy

**GTS ID:** `gts.x.core.serverless.tenant_policy.v1~`

Tenant-level governance settings including quotas, retention policies, and defaults.
Per PRD BR-020, BR-106, and BR-107, tenants are provisioned with isolation and governance settings.

#### GTS Schema

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.serverless.tenant_policy.v1~",
  "title": "Tenant Runtime Policy",
  "description": "Tenant-level governance settings for the serverless runtime.",
  "type": "object",
  "properties": {
    "tenant_id": {
      "type": "string",
      "description": "Tenant identifier (also serves as the policy identity)."
    },
    "enabled": {
      "type": "boolean",
      "default": true,
      "description": "Whether the serverless runtime is enabled for this tenant."
    },
    "quotas": {
      "type": "object",
      "description": "Resource quotas for the tenant.",
      "properties": {
        "max_concurrent_executions": {
          "type": "integer",
          "minimum": 1
        },
        "max_definitions": {
          "type": "integer",
          "minimum": 1
        },
        "max_schedules": {
          "type": "integer",
          "minimum": 0
        },
        "max_triggers": {
          "type": "integer",
          "minimum": 0
        },
        "max_execution_history_mb": {
          "type": "integer",
          "minimum": 1
        },
        "max_memory_per_execution_mb": {
          "type": "integer",
          "minimum": 1
        },
        "max_cpu_per_execution": {
          "type": "number",
          "minimum": 0
        },
        "max_execution_duration_seconds": {
          "type": "integer",
          "minimum": 1
        }
      }
    },
    "retention": {
      "type": "object",
      "description": "Retention policies for execution history and audit logs.",
      "properties": {
        "execution_history_days": {
          "type": "integer",
          "minimum": 1,
          "default": 7
        },
        "audit_log_days": {
          "type": "integer",
          "minimum": 1,
          "default": 90
        }
      }
    },
    "policies": {
      "type": "object",
      "description": "Governance policies.",
      "properties": {
        "allowed_runtimes": {
          "type": "array",
          "items": {
            "type": "string",
            "x-gts-ref": "gts.x.core.serverless.adapter.*"
          },
          "description": "List of allowed adapter GTS type IDs (e.g., gts.x.core.serverless.adapter.starlark.v1~). Validated against implementation.adapter at registration time."
        }
      }
    },
    "idempotency": {
      "type": "object",
      "description": "Idempotency configuration for invocations.",
      "properties": {
        "deduplication_window_seconds": {
          "type": "integer",
          "minimum": 60,
          "maximum": 2628000,
          "default": 86400,
          "description": "Duration in seconds to retain idempotency keys for deduplication."
        }
      }
    },
    "defaults": {
      "type": "object",
      "description": "Default limits applied to new entrypoints.",
      "properties": {
        "timeout_seconds": {
          "type": "integer",
          "minimum": 1,
          "default": 30
        },
        "memory_mb": {
          "type": "integer",
          "minimum": 1,
          "default": 128
        },
        "cpu": {
          "type": "number",
          "minimum": 0,
          "default": 0.2
        }
      }
    },
    "created_at": {
      "type": "string",
      "format": "date-time"
    },
    "updated_at": {
      "type": "string",
      "format": "date-time"
    }
  },
  "required": [
    "tenant_id",
    "enabled",
    "quotas",
    "retention"
  ]
}
```

#### Instance Example

```json
{
  "tenant_id": "t_123",
  "enabled": true,
  "quotas": {
    "max_concurrent_executions": 200,
    "max_definitions": 500,
    "max_schedules": 50,
    "max_triggers": 100,
    "max_execution_history_mb": 10240,
    "max_memory_per_execution_mb": 512,
    "max_cpu_per_execution": 2.0,
    "max_execution_duration_seconds": 86400
  },
  "retention": {
    "execution_history_days": 7,
    "audit_log_days": 90
  },
  "policies": {
    "allowed_runtimes": [
      "gts.x.core.serverless.adapter.starlark.v1~",
      "gts.x.core.serverless.adapter.temporal.v1~"
    ]
  },
  "idempotency": {
    "deduplication_window_seconds": 86400
  },
  "defaults": {
    "timeout_seconds": 30,
    "memory_mb": 128,
    "cpu": 0.2
  },
  "created_at": "2026-01-01T00:00:00.000Z",
  "updated_at": "2026-01-01T00:00:00.000Z"
}
```

---

## APIs

Follows [DNA (Development Norms & Architecture)](https://github.com/hypernetix/DNA) guidelines.

### Single Resource Response

```json
{
  "id": "sch_001",
  "name": "Daily Tax Calculation",
  "status": "active",
  "created_at": "2026-01-20T10:30:15.123Z"
}
```

**List Response** (cursor-based pagination)

```json
{
  "items": [
    {
      "id": "inv_001",
      "status": "running"
    },
    {
      "id": "inv_002",
      "status": "succeeded"
    }
  ],
  "page_info": {
    "next_cursor": "eyJpZCI6Imludl8wMDIifQ",
    "prev_cursor": null,
    "has_more": true
  }
}
```

**Error Response** (RFC 9457 Problem Details)

Content-Type: `application/problem+json`

```json
{
  "type": "gts://gts.x.core.serverless.err.v1~x.core.serverless.err.validation.v1~",
  "title": "Validation Error",
  "status": 422,
  "detail": "Input validation failed for field 'params.amount'",
  "instance": "/api/serverless-runtime/v1/invocations",
  "code": "gts.x.core.serverless.err.v1~x.core.serverless.err.validation.v1~",
  "trace_id": "abc123",
  "errors": [
    {
      "path": "$.params.amount",
      "message": "Must be a positive number"
    }
  ]
}
```

### Pagination Parameters

| Parameter | Default | Max | Description                    |
|-----------|---------|-----|--------------------------------|
| `limit`   | 25      | 200 | Items per page                 |
| `cursor`  | —       | —   | Opaque cursor from `page_info` |

**Filtering** (OData-style `$filter`)

```text
GET /api/serverless-runtime/v1/invocations?$filter=status eq 'running' and created_at ge 2026-01-01T00:00:00.000Z
```

Operators: `eq`, `ne`, `lt`, `le`, `gt`, `ge`, `in`, `and`, `or`, `not`

**Sorting** (OData-style `$orderby`)

```text
GET /api/serverless-runtime/v1/invocations?$orderby=created_at desc,status asc
```

---

### Entrypoint Registry API

#### Path Parameter `{id}` Format

The `{id}` path parameter is a URL-safe opaque identifier assigned by the system at registration time (e.g.,
`ep_a1b2c3d4`).
It is **not** the full GTS address. The GTS address is returned in the response body and can be used in `entrypoint_id`
fields when starting invocations. To look up an entrypoint by GTS address, use the list endpoint with a filter.

#### Versioning

Entrypoint versioning is inherent in the GTS address (e.g., `...my_func.v1~`, `...my_func.v2~`). Each version is a
separate entrypoint registration. To create a new version, register a new entrypoint with an incremented version
in the GTS address. The `PUT` endpoint only allows updates to entrypoints in `draft` status; once an entrypoint
is `active`, it is immutable and a new version must be registered instead.

| Method   | Endpoint                                             | Description                                          |
|----------|------------------------------------------------------|------------------------------------------------------|
| `POST`   | `/api/serverless-runtime/v1/entrypoints`             | Register new entrypoint (or new version)             |
| `POST`   | `/api/serverless-runtime/v1/entrypoints:validate`    | Validate without saving                              |
| `GET`    | `/api/serverless-runtime/v1/entrypoints`             | List entrypoints (filter by GTS prefix for versions) |
| `GET`    | `/api/serverless-runtime/v1/entrypoints/{id}`        | Get by ID                                            |
| `PUT`    | `/api/serverless-runtime/v1/entrypoints/{id}`        | Update entrypoint (draft status only)                |
| `POST`   | `/api/serverless-runtime/v1/entrypoints/{id}:status` | Update status (activate, deprecate, disable, enable) |
| `DELETE` | `/api/serverless-runtime/v1/entrypoints/{id}`        | Hard delete (draft only) or archive                  |

#### Entrypoint Status Actions

The `:status` endpoint accepts a JSON body with `action` field:

```json
{
  "action": "deprecate"
}
```

Valid actions and state transitions:

| Action      | Description                      | Transition                           |
|-------------|----------------------------------|--------------------------------------|
| `activate`  | Activate a draft entrypoint      | `draft` → `active`                   |
| `deprecate` | Mark as deprecated (still works) | `active` → `deprecated`              |
| `disable`   | Disable (not callable)           | `active`/`deprecated` → `disabled`   |
| `enable`    | Re-enable a disabled entrypoint  | `disabled` → `active`                |
| `archive`   | Archive for historical reference | `deprecated`/`disabled` → `archived` |

---

### Invocation API

| Method | Endpoint                                                         | Description                  |
|--------|------------------------------------------------------------------|------------------------------|
| `POST` | `/api/serverless-runtime/v1/invocations`                         | Start invocation             |
| `GET`  | `/api/serverless-runtime/v1/invocations`                         | List invocations             |
| `GET`  | `/api/serverless-runtime/v1/invocations/{invocation_id}`         | Get status                   |
| `POST` | `/api/serverless-runtime/v1/invocations/{invocation_id}:control` | Control invocation lifecycle |

#### Invocation Control Actions

The `:control` endpoint accepts a JSON body with `action` field:

```json
{
  "action": "cancel"
}
```

Valid actions and state requirements:

| Action    | Description                                       | Valid From States         |
|-----------|---------------------------------------------------|---------------------------|
| `cancel`  | Cancel a running or queued invocation             | `queued`, `running`       |
| `suspend` | Suspend a running workflow                        | `running` (workflow only) |
| `resume`  | Resume a suspended invocation                     | `suspended`               |
| `retry`   | Retry a failed invocation with same parameters    | `failed`                  |
| `replay`  | Create new invocation from completed one's params | `succeeded`, `failed`     |

#### Start Invocation Request

```json
{
  "entrypoint_id": "gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~...",
  "mode": "async",
  "params": {
    "invoice_id": "inv_001",
    "amount": 100.0
  },
  "dry_run": false
}
```

- `dry_run`: When `true`, validates invocation readiness without executing. See [Dry-Run Behavior](#dry-run-behavior)
  below.
- `Idempotency-Key` header prevents duplicate starts. Retention is configurable per tenant via
  `TenantRuntimePolicy.idempotency.deduplication_window_seconds` (default: 24 hours, per BR-134).
- When the `Idempotency-Key` header is present and the entrypoint enables response caching
  (`traits.is_idempotent: true` and `traits.caching.max_age_seconds > 0`), the runtime may return
  a cached successful result instead of re-executing. See [Response Caching](#response-caching).

#### Dry-Run Behavior

When `dry_run: true`, the `POST /invocations` endpoint performs **validation only** and returns
a synthetic `InvocationResult` without producing any durable state or side effects (BR-103).

##### Validations Performed

The following checks run in order; the first failure short-circuits and returns an
RFC 9457 Problem Details response (`application/problem+json`):

1. **Entrypoint exists** — resolve `entrypoint_id` to an `EntrypointDefinition`. Return
   `404 Not Found` with error type
   `gts.x.core.serverless.err.v1~x.core.serverless.err.not_found.v1~` if missing.
2. **Entrypoint is callable** — verify `status` is `active` or `deprecated`. Return `409 Conflict`
   with error type `gts.x.core.serverless.err.v1~x.core.serverless.err.not_active.v1~` if the
   entrypoint is in `draft`, `disabled`, or `archived` state.
3. **Input params match schema** — validate `params` against `entrypoint.schema.params` JSON
   Schema. Return `422 Unprocessable Entity` with error type
   `gts.x.core.serverless.err.v1~x.core.serverless.err.validation.v1~` and per-field `errors`
   array on mismatch.
4. **Tenant quota** — verify the tenant has not exhausted `max_concurrent_executions` from
   `TenantQuotas`. Return `429 Too Many Requests` with error type
   `gts.x.core.serverless.err.v1~x.core.serverless.err.quota_exceeded.v1~` if at capacity.

##### What Dry-Run Does NOT Do

- Does **not** create an `InvocationRecord` in the persistence layer.
- Does **not** execute any user code (function body or workflow steps).
- Does **not** consume quota — the check is read-only.
- Does **not** count against per-entrypoint rate limits.
- Does **not** generate observability traces or billing events.
- Does **not** evaluate or enforce the `Idempotency-Key` header.

##### Successful Response

On validation success the endpoint returns `200 OK` (not `201 Created`) with the same
`InvocationResult` structure. The embedded `InvocationRecord` is **synthetic**:

| Field                | Value                                                                              |
|----------------------|------------------------------------------------------------------------------------|
| `invocation_id`      | Synthetic, prefixed `dryrun_` (e.g. `dryrun_a1b2c3d4-...`). Not queryable via GET. |
| `entrypoint_id`      | Echoed from request.                                                               |
| `entrypoint_version` | Current version of the resolved entrypoint.                                        |
| `tenant_id`          | Caller's tenant from `SecurityContext`.                                            |
| `status`             | `queued` — indicates validation passed and the invocation *would* be queued.       |
| `mode`               | Echoed from request.                                                               |
| `params`             | Echoed from request.                                                               |
| `result`             | `null`                                                                             |
| `error`              | `null`                                                                             |
| `timestamps`         | `created_at` set to current time; all others `null`.                               |
| `observability`      | `correlation_id` generated; `trace_id`, `span_id`, metrics all `null`/zero.        |

The `InvocationResult.dry_run` flag is set to `true` so callers can programmatically distinguish
synthetic results from real invocations.

##### Error Response

Validation failures return an RFC 9457 Problem Details body (`application/problem+json`) with the
`type` set to the GTS error URI, an appropriate HTTP status code (see table above), and a
human-readable `detail` message. For schema validation failures (`422`), the `errors` array
contains per-field violations. The error shape is identical to a normal invocation error — no
special dry-run error format exists.

##### Example: Dry-Run Success Response (200 OK)

```json
{
  "record": {
    "invocation_id": "dryrun_a1b2c3d4-e5f6-7890-abcd-ef1234567890",
    "entrypoint_id": "gts.x.core.serverless.entrypoint.v1~x.core.serverless.function.v1~vendor.app.namespace.calculate_tax.v1~",
    "entrypoint_version": "1.0.0",
    "tenant_id": "t_123",
    "status": "queued",
    "mode": "async",
    "params": {
      "invoice_id": "inv_001",
      "amount": 100.0
    },
    "result": null,
    "error": null,
    "timestamps": {
      "created_at": "2026-01-21T10:00:00.000Z",
      "started_at": null,
      "suspended_at": null,
      "finished_at": null
    },
    "observability": {
      "correlation_id": "corr_dry_xyz",
      "trace_id": null,
      "span_id": null,
      "metrics": {
        "duration_ms": null,
        "billed_duration_ms": null,
        "cpu_time_ms": null,
        "memory_limit_mb": 256,
        "max_memory_used_mb": null,
        "step_count": null
      }
    }
  },
  "dry_run": true,
  "cached": false
}
```

#### Response Caching

Response caching allows the runtime to return a previously computed successful result for an
entrypoint invocation without re-executing the entrypoint (BR-118, BR-132). This reduces
redundant processing for idempotent operations and improves latency for repeated calls.

##### Activation Conditions

Response caching is active for a given invocation **only** when **all** of the following
conditions are met:

1. The caller provides an `Idempotency-Key` header in the invocation request.
2. The entrypoint's `traits.caching.max_age_seconds` is greater than `0`.
3. The entrypoint's `traits.is_idempotent` is `true`.

If any condition is not met, the invocation always executes normally — no cache lookup or
storage occurs.

##### Cache Key

The cache key depends on the entrypoint's **owner type** (from `owner.owner_type`):

| Owner Type          | Cache Key Tuple                                                    |
|---------------------|--------------------------------------------------------------------|
| `user`              | `(subject_id, entrypoint_id, entrypoint_version, idempotency_key)` |
| `tenant` / `system` | `(tenant_id, entrypoint_id, entrypoint_version, idempotency_key)`  |

- **`subject_id`** — the authenticated user's identity from `SecurityContext`. Used for
  user-owned entrypoints so that each user's cached results are private and isolated.
- **`tenant_id`** — the caller's tenant from `SecurityContext`. Used for tenant-owned and
  system-owned entrypoints where the cache is shared among all authorized callers within the
  same tenant.
- **`entrypoint_id`** — the full GTS type ID of the invoked entrypoint.
- **`entrypoint_version`** — the semantic version of the entrypoint definition at invocation
  time. A new version produces a different cache key, so cached results from a previous version
  are never served for a new version.
- **`idempotency_key`** — the value of the `Idempotency-Key` header provided by the caller.

##### Cache Scope and Tenant Isolation

Cache scope is **per entrypoint owner** and **never shared across tenants**:

- **User-owned entrypoints** (`owner_type: user`) — cache is scoped to the individual
  `subject_id`. Different users invoking the same user-owned entrypoint with the same
  idempotency key get independent cache entries.
- **Tenant-owned entrypoints** (`owner_type: tenant`) — cache is scoped to the `tenant_id`.
  All authorized callers within the same tenant share cache entries for the same entrypoint,
  version, and idempotency key.
- **System-owned entrypoints** (`owner_type: system`) — cache is scoped to the `tenant_id` of
  the caller. Even though the entrypoint definition is platform-provided, cached results are
  tenant-isolated.

Cached results are **never** shared across tenants regardless of owner type.

##### Cacheable Results

Only invocations that complete with a `succeeded` status are eligible for caching. Invocations
that fail, are canceled, or produce any non-success terminal status are **not** cached and do
**not** invalidate existing cache entries for the same key.

##### TTL and Expiration

Cached results expire after the number of seconds specified by
`traits.caching.max_age_seconds`. After expiration, the next matching invocation executes
normally and, if successful, repopulates the cache.

##### Cache Hit Behavior

When a cache hit occurs:

- The runtime returns the previously stored `InvocationResult` with `cached: true`.
- The embedded `InvocationRecord` is the **original** record from the execution that produced
  the cached result (including original `invocation_id`, `timestamps`, and `observability`).
- **No** new `InvocationRecord` is created or persisted.
- **No** user code is executed.
- **No** quota is consumed and no rate-limit counters are incremented.
- **No** new observability traces or billing events are generated.

##### Cache Invalidation

Cache entries are implicitly invalidated when:

- The TTL (`max_age_seconds`) expires.
- The entrypoint version changes (the version is part of the cache key).

There is no explicit cache purge API. Authors who need to force re-execution should use a
different `Idempotency-Key` value or wait for TTL expiration.

##### Interaction with Other Features

| Feature        | Interaction                                                                                                                                                                                                                      |
|----------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| Dry-run        | Caching does **not** apply. Dry-run never reads from or writes to the cache.                                                                                                                                                     |
| Idempotency    | Idempotency deduplication (BR-134) and response caching are complementary. Deduplication prevents duplicate *starts* within the deduplication window; caching returns stored *results* for completed invocations within the TTL. |
| Rate limiting  | Cache hits bypass rate limiting — the entrypoint is not re-executed.                                                                                                                                                             |
| Retry / Replay | `retry` and `replay` control actions always execute fresh and do **not** consult the cache.                                                                                                                                      |

---

### Schedule API

| Method   | Endpoint                                            | Description       |
|----------|-----------------------------------------------------|-------------------|
| `POST`   | `/api/serverless-runtime/v1/schedules`              | Create schedule   |
| `GET`    | `/api/serverless-runtime/v1/schedules`              | List schedules    |
| `GET`    | `/api/serverless-runtime/v1/schedules/{id}`         | Get by ID         |
| `PUT`    | `/api/serverless-runtime/v1/schedules/{id}`         | Update            |
| `POST`   | `/api/serverless-runtime/v1/schedules/{id}:pause`   | Pause             |
| `POST`   | `/api/serverless-runtime/v1/schedules/{id}:resume`  | Resume            |
| `DELETE` | `/api/serverless-runtime/v1/schedules/{id}`         | Delete            |
| `GET`    | `/api/serverless-runtime/v1/schedules/{id}/history` | Execution history |

---

### Trigger API (Event-Driven)

| Method   | Endpoint                                   | Description    |
|----------|--------------------------------------------|----------------|
| `POST`   | `/api/serverless-runtime/v1/triggers`      | Create trigger |
| `GET`    | `/api/serverless-runtime/v1/triggers`      | List triggers  |
| `GET`    | `/api/serverless-runtime/v1/triggers/{id}` | Get by ID      |
| `PUT`    | `/api/serverless-runtime/v1/triggers/{id}` | Update         |
| `DELETE` | `/api/serverless-runtime/v1/triggers/{id}` | Delete         |

---

### Tenant Runtime Policy API

| Method | Endpoint                                                        | Description   |
|--------|-----------------------------------------------------------------|---------------|
| `GET`  | `/api/serverless-runtime/v1/tenants/{tenant_id}/runtime-policy` | Get policy    |
| `PUT`  | `/api/serverless-runtime/v1/tenants/{tenant_id}/runtime-policy` | Update policy |

---

### Quota Usage API

| Method | Endpoint                                                       | Description                  |
|--------|----------------------------------------------------------------|------------------------------|
| `GET`  | `/api/serverless-runtime/v1/tenants/{tenant_id}/usage`         | Get current usage vs. quotas |
| `GET`  | `/api/serverless-runtime/v1/tenants/{tenant_id}/usage/history` | Get usage history over time  |

#### Usage Response

```json
{
  "tenant_id": "t_123",
  "timestamp": "2026-01-21T12:00:00.000Z",
  "current": {
    "concurrent_executions": 45,
    "total_definitions": 120,
    "total_schedules": 15,
    "total_triggers": 8,
    "execution_history_mb": 2048
  },
  "quotas": {
    "max_concurrent_executions": 200,
    "max_definitions": 500,
    "max_schedules": 50,
    "max_triggers": 100,
    "max_execution_history_mb": 10240
  },
  "utilization_percent": {
    "concurrent_executions": 22.5,
    "definitions": 24.0,
    "schedules": 30.0,
    "triggers": 8.0,
    "execution_history": 20.0
  }
}
```

---

## Rust Domain Types and Runtime Traits

This section provides a Rust-oriented representation of the domain model and an abstract
`ServerlessRuntime` interface that can be implemented by adapters (Temporal, Starlark, cloud FaaS).
These types are transport-agnostic and intended for SDK or core runtime crates.

### Core Types (Rust)

```rust
use chrono::{DateTime, Utc};
use serde_json::Value as JsonValue;

pub type GtsId = String;
pub type EntrypointId = String;
pub type InvocationId = String;
pub type ScheduleId = String;
pub type TriggerId = String;
pub type TenantId = String;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DefinitionStatus {
    Draft,
    Active,
    Deprecated,
    Disabled,
    Archived,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvocationMode {
    Sync,
    Async,
}

/// Invocation lifecycle status. Matches the short enum values in the
/// `gts.x.core.serverless.status.v1~` GTS schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvocationStatus {
    Queued,
    Running,
    Suspended,
    Succeeded,
    Failed,
    Canceled,
    Compensating,
    Compensated,
    DeadLettered,
}

/// Entrypoint type derived from GTS chain (not stored, computed from entrypoint_id).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntrypointType {
    Function,
    Workflow,
}

impl EntrypointType {
    /// Determines entrypoint type by checking the GTS chain for known base types.
    pub fn from_gts_id(entrypoint_id: &str) -> Option<Self> {
        if entrypoint_id.contains("x.core.serverless.function.") {
            Some(EntrypointType::Function)
        } else if entrypoint_id.contains("x.core.serverless.workflow.") {
            Some(EntrypointType::Workflow)
        } else {
            None
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScheduleStatus {
    Active,
    Paused,
    Disabled,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MissedSchedulePolicy {
    Skip,
    CatchUp,
    Backfill,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImplementationKind {
    Code,
    WorkflowSpec,
    AdapterRef,
}

#[derive(Clone, Debug)]
pub struct EntrypointSchema {
    pub params: JsonValue,
    pub returns: JsonValue,
    pub errors: Vec<GtsId>,
}

#[derive(Clone, Debug)]
pub struct EntrypointTraits {
    pub supported_invocations: Vec<InvocationMode>,
    pub default_invocation: InvocationMode,
    pub is_idempotent: bool,
    pub caching: ResponseCachingPolicy,
    pub rate_limit: Option<RateLimit>,
    pub limits: EntrypointLimits,
    pub retry: RetryPolicy,
    pub workflow: Option<WorkflowTraits>,
}

/// Response caching policy for an entrypoint (BR-118, BR-132).
///
/// Caching is only active when **all** conditions are met:
/// 1. The caller provides an `Idempotency-Key` header.
/// 2. `max_age_seconds > 0`.
/// 3. The entrypoint's `is_idempotent` trait is `true`.
///
/// Cache key depends on entrypoint owner type:
/// - `user` owner: `(subject_id, entrypoint_id, entrypoint_version, idempotency_key)`
/// - `tenant`/`system` owner: `(tenant_id, entrypoint_id, entrypoint_version, idempotency_key)`
///
/// Cache scope is per entrypoint owner — never shared across tenants.
/// Only successful (`succeeded`) results are cached.
#[derive(Clone, Debug)]
pub struct ResponseCachingPolicy {
    /// TTL in seconds for cached successful results. `0` disables caching.
    pub max_age_seconds: u64,
}

/// Entrypoint-level rate limiting reference. Enforced per-entrypoint
/// per-tenant (isolated across tenants, aggregated across users within tenant).
/// Applies to both sync and async invocation modes.
///
/// `strategy` is the GTS type ID of the rate limiter plugin (derived from
/// `gts.x.core.serverless.rate_limit.v1~`); `config` is the strategy-specific
/// settings as an opaque JSON object validated by the resolved plugin.
#[derive(Clone, Debug)]
pub struct RateLimit {
    /// GTS type ID of the rate limiting strategy. The runtime resolves
    /// the rate limiter plugin from this value.
    pub strategy: GtsId,
    /// Strategy-specific configuration. Opaque to the platform; the resolved
    /// plugin deserializes this into its own config type.
    pub config: serde_json::Value,
}

/// System-default token bucket rate limiter configuration.
/// GTS ID: gts.x.core.serverless.rate_limit.v1~x.core.serverless.rate_limit.token_bucket.v1~
///
/// Both per-second and per-minute limits are enforced independently.
/// `burst_size` applies to the per-second bucket only.
#[derive(Clone, Debug)]
pub struct TokenBucketRateLimit {
    /// Maximum sustained invocations per second. `0` = no per-second limit.
    pub max_requests_per_second: f64,
    /// Maximum sustained invocations per minute. `0` = no per-minute limit.
    pub max_requests_per_minute: u64,
    /// Maximum instantaneous burst for the per-second bucket.
    pub burst_size: u64,
}

/// Admission decision returned by a `RateLimiter` plugin.
#[derive(Clone, Debug)]
pub enum RateLimitDecision {
    /// Request is allowed.
    Allow,
    /// Request is rejected. `retry_after_seconds` is the suggested wait time.
    Reject { retry_after_seconds: u64 },
}

/// Plugin trait for rate limiting. Each plugin handles exactly one strategy
/// GTS type. The runtime resolves the plugin based on `rate_limit.strategy`
/// and passes the opaque `config` for admission checks.
///
/// The default system implementation handles `token_bucket.v1~` using an
/// in-process token bucket. Custom plugins may implement distributed rate
/// limiting (e.g., Redis-backed), sliding window, or adaptive throttling.
#[async_trait]
pub trait RateLimiter: Send + Sync {
    /// The single GTS type ID this plugin handles.
    fn strategy_type(&self) -> &GtsId;

    /// Check whether an invocation should be admitted.
    async fn check(
        &self,
        ctx: &SecurityContext,
        entrypoint_id: &EntrypointId,
        config: &serde_json::Value,
    ) -> RateLimitDecision;
}

#[derive(Clone, Debug)]
pub struct WorkflowTraits {
    pub compensation: CompensationConfig,
    pub checkpointing: CheckpointingConfig,
    pub max_suspension_days: u64,
}

#[derive(Clone, Debug)]
pub struct CompensationConfig {
    /// GTS ID of entrypoint to invoke on workflow failure, or None for no compensation.
    pub on_failure: Option<EntrypointId>,
    /// GTS ID of entrypoint to invoke on workflow cancellation, or None for no compensation.
    pub on_cancel: Option<EntrypointId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointingStrategy {
    Automatic,
    Manual,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct CheckpointingConfig {
    pub strategy: CheckpointingStrategy,
}

/// Base limits; adapters may extend with additional fields (memory_mb, cpu, etc.)
#[derive(Clone, Debug)]
pub struct EntrypointLimits {
    pub timeout_seconds: u64,
    pub max_concurrent: u64,
    /// Adapter-specific limits (e.g., memory_mb, cpu for Starlark adapter)
    pub extra: Option<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Clone, Debug)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub initial_delay_ms: u64,
    pub max_delay_ms: u64,
    pub backoff_multiplier: f32,
    pub non_retryable_errors: Vec<GtsId>,
}

/// Implementation with explicit adapter for limits validation.
#[derive(Clone, Debug)]
pub struct EntrypointImplementation {
    /// GTS type ID of the adapter (e.g., gts.x.core.serverless.adapter.starlark.v1~)
    pub adapter: GtsId,
    pub kind: ImplementationKind,
    pub payload: ImplementationPayload,
}

#[derive(Clone, Debug)]
pub enum ImplementationPayload {
    Code { language: String, source: String },
    WorkflowSpec { format: String, spec: JsonValue },
    AdapterRef { definition_id: String },
}

/// Entrypoint definition. Identity is the GTS instance address (external).
/// Entrypoint type (function/workflow) is derived from the GTS chain.
#[derive(Clone, Debug)]
pub struct EntrypointDefinition {
    pub version: String,
    pub tenant_id: TenantId,
    /// Owner determines default visibility (per PRD BR-002):
    /// - User-scoped: private by default
    /// - Tenant-scoped: visible to tenant users by default
    /// - System: platform-provided
    pub owner: OwnerRef,
    pub status: DefinitionStatus,
    pub tags: Vec<String>,
    pub title: String,
    pub description: String,
    pub schema: EntrypointSchema,
    pub traits: EntrypointTraits,
    pub implementation: EntrypointImplementation,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OwnerType {
    /// User-scoped: private to the owning user by default.
    User,
    /// Tenant-scoped: visible to authorized users within the tenant by default.
    Tenant,
    /// System-provided: managed by the platform.
    System,
}

#[derive(Clone, Debug)]
pub struct OwnerRef {
    pub owner_type: OwnerType,
    pub id: String,
    pub tenant_id: TenantId,
}

#[derive(Clone, Debug)]
pub struct InvocationRecord {
    pub invocation_id: InvocationId,
    /// GTS type ID; entrypoint type (function/workflow) is derived from the chain.
    pub entrypoint_id: EntrypointId,
    pub entrypoint_version: String,
    pub tenant_id: TenantId,
    pub status: InvocationStatus,
    pub mode: InvocationMode,
    pub params: JsonValue,
    pub result: Option<JsonValue>,
    pub error: Option<RuntimeErrorPayload>,
    pub timestamps: InvocationTimestamps,
    pub observability: InvocationObservability,
}

#[derive(Clone, Debug)]
pub struct InvocationTimestamps {
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub suspended_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug)]
pub struct InvocationObservability {
    pub correlation_id: String,
    pub trace_id: Option<String>,
    pub span_id: Option<String>,
    pub metrics: InvocationMetrics,
}

#[derive(Clone, Debug)]
pub struct InvocationMetrics {
    pub duration_ms: Option<u64>,
    pub billed_duration_ms: Option<u64>,
    pub cpu_time_ms: Option<u64>,
    pub memory_limit_mb: u64,
    pub max_memory_used_mb: Option<u64>,
    pub step_count: Option<u64>,
}

#[derive(Clone, Debug)]
pub struct Schedule {
    pub schedule_id: ScheduleId,
    pub tenant_id: TenantId,
    pub entrypoint_id: EntrypointId,
    pub name: String,
    pub timezone: String,
    pub expression: ScheduleExpression,
    pub input_overrides: JsonValue,
    pub missed_policy: MissedSchedulePolicy,
    pub status: ScheduleStatus,
    pub next_run_at: Option<DateTime<Utc>>,
    pub last_run_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct ScheduleExpression {
    pub kind: String,
    pub value: String,
}

#[derive(Clone, Debug)]
pub struct Trigger {
    pub trigger_id: TriggerId,
    pub tenant_id: TenantId,
    /// GTS event type ID to listen for.
    pub event_type_id: GtsId,
    /// Filter expression syntax TBD during EventBroker implementation.
    pub event_filter_query: Option<String>,
    pub entrypoint_id: EntrypointId,
    pub dead_letter_queue: Option<DeadLetterQueueConfig>,
    pub status: TriggerStatus,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct DeadLetterQueueConfig {
    pub enabled: bool,
    /// Retry policy before moving to DLQ.
    pub retry_policy: RetryPolicy,
    /// GTS type ID of the topic to publish dead-lettered events to,
    /// or None for the platform-default DLQ topic. Topic type and
    /// management are defined by the EventBroker.
    pub dlq_topic: Option<GtsId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TriggerStatus {
    Active,
    Paused,
    Disabled,
}

#[derive(Clone, Debug)]
pub struct TenantRuntimePolicy {
    pub tenant_id: TenantId,
    pub enabled: bool,
    pub quotas: TenantQuotas,
    pub retention: TenantRetention,
    pub policies: TenantPolicies,
    pub idempotency: TenantIdempotency,
    pub defaults: TenantDefaults,
}

#[derive(Clone, Debug)]
pub struct TenantQuotas {
    pub max_concurrent_executions: u64,
    pub max_definitions: u64,
    pub max_schedules: u64,
    pub max_triggers: u64,
    pub max_execution_history_mb: u64,
    pub max_memory_per_execution_mb: u64,
    pub max_cpu_per_execution: f32,
    pub max_execution_duration_seconds: u64,
}

#[derive(Clone, Debug)]
pub struct TenantRetention {
    pub execution_history_days: u64,
    pub audit_log_days: u64,
}

#[derive(Clone, Debug)]
pub struct TenantPolicies {
    /// Allowed adapter GTS type IDs (e.g., gts.x.core.serverless.adapter.starlark.v1~).
    /// Validated against `implementation.adapter` at entrypoint registration time.
    pub allowed_runtimes: Vec<GtsId>,
}

#[derive(Clone, Debug)]
pub struct TenantIdempotency {
    pub deduplication_window_seconds: u64,
}

/// Default limits for new entrypoints (base limits only; adapters may add more).
#[derive(Clone, Debug)]
pub struct TenantDefaults {
    pub timeout_seconds: u64,
    pub memory_mb: u64,
    pub cpu: f32,
}
```

### Runtime Errors

```rust
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuntimeErrorCategory {
    Retryable,
    NonRetryable,
    ResourceLimit,
    Timeout,
    Canceled,
}

#[derive(Clone, Debug)]
pub struct RuntimeErrorPayload {
    /// GTS error type ID (e.g., gts.x.core.serverless.err.v1~x.core.serverless.err.validation.v1~)
    pub error_type_id: GtsId,
    pub message: String,
    pub category: RuntimeErrorCategory,
    pub details: serde_json::Value,
}
```

### Abstract Runtime Interface

```rust
use async_trait::async_trait;
use modkit_security::SecurityContext;

#[derive(Clone, Debug)]
pub struct InvocationRequest {
    pub entrypoint_id: EntrypointId,
    pub mode: InvocationMode,
    pub params: serde_json::Value,
    pub dry_run: bool,
    pub idempotency_key: Option<String>,
}

#[derive(Clone, Debug)]
pub struct InvocationResult {
    pub record: InvocationRecord,
    /// `true` when the result was produced by a dry-run invocation.
    /// The embedded record is synthetic and was not persisted.
    pub dry_run: bool,
    /// `true` when the result was served from the response cache (cache hit).
    /// The embedded record is the original record from the execution that
    /// produced the cached result. No new invocation was created.
    pub cached: bool,
}

/// Actions for entrypoint lifecycle status transitions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntrypointStatusAction {
    /// Mark entrypoint as deprecated (still callable but discouraged).
    Deprecate,
    /// Disable entrypoint (not callable, can be re-enabled).
    Disable,
    /// Re-enable a disabled entrypoint.
    Enable,
    /// Activate a draft entrypoint.
    Activate,
    /// Archive a deprecated or disabled entrypoint for historical reference.
    Archive,
}

/// Control actions for invocation lifecycle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum InvocationControlAction {
    /// Cancel a running or queued invocation.
    Cancel,
    /// Suspend a running invocation (workflow only).
    Suspend,
    /// Resume a suspended invocation.
    Resume,
    /// Retry a failed invocation with same parameters.
    Retry,
    /// Replay a completed invocation (create new invocation with same parameters).
    Replay,
}

#[async_trait]
pub trait ServerlessRuntime: Send + Sync {
    async fn register_entrypoint(
        &self,
        ctx: &SecurityContext,
        entrypoint: EntrypointDefinition,
    ) -> Result<EntrypointDefinition, RuntimeErrorPayload>;

    /// Validate entrypoint definition without saving.
    /// Returns Ok(()) on success, Err(ValidationError) on validation failure.
    async fn validate_entrypoint(
        &self,
        ctx: &SecurityContext,
        entrypoint: EntrypointDefinition,
    ) -> Result<(), RuntimeErrorPayload>;

    async fn list_entrypoints(
        &self,
        ctx: &SecurityContext,
        filter: EntrypointListFilter,
    ) -> Result<Vec<EntrypointDefinition>, RuntimeErrorPayload>;

    async fn get_entrypoint(
        &self,
        ctx: &SecurityContext,
        entrypoint_id: &EntrypointId,
    ) -> Result<EntrypointDefinition, RuntimeErrorPayload>;

    /// Replace an entrypoint definition while it is still in `Draft` status.
    /// The updated definition is re-validated before saving.
    async fn update_entrypoint(
        &self,
        ctx: &SecurityContext,
        entrypoint_id: &EntrypointId,
        entrypoint: EntrypointDefinition,
    ) -> Result<EntrypointDefinition, RuntimeErrorPayload>;

    /// Transition entrypoint status (deprecate, disable, enable, activate, archive).
    async fn update_entrypoint_status(
        &self,
        ctx: &SecurityContext,
        entrypoint_id: &EntrypointId,
        action: EntrypointStatusAction,
    ) -> Result<EntrypointDefinition, RuntimeErrorPayload>;

    /// Delete an entrypoint. Hard-deletes if in `draft` status; archives otherwise
    /// (equivalent to `update_entrypoint_status(Archive)` for non-draft entrypoints).
    async fn delete_entrypoint(
        &self,
        ctx: &SecurityContext,
        entrypoint_id: &EntrypointId,
    ) -> Result<(), RuntimeErrorPayload>;

    /// Start a new invocation of the given entrypoint.
    ///
    /// When `request.dry_run` is `true`, validates readiness (entrypoint exists
    /// and is callable, params match schema, tenant quota available) and returns a
    /// synthetic `InvocationResult` with `dry_run: true`. No record is persisted,
    /// no code is executed, and no quota or rate-limit counters are affected.
    ///
    /// When response caching is active (idempotency key present, `is_idempotent`
    /// is `true`, and `caching.max_age_seconds > 0`), the runtime checks the
    /// cache before executing. The cache key scope depends on the entrypoint
    /// owner type: `subject_id` for user-owned, `tenant_id` for tenant/system-
    /// owned — combined with `(entrypoint_id, entrypoint_version,
    /// idempotency_key)`. On cache hit the previously stored successful result
    /// is returned with `cached: true` — no new invocation is created and no
    /// code is executed.
    async fn start_invocation(
        &self,
        ctx: &SecurityContext,
        request: InvocationRequest,
    ) -> Result<InvocationResult, RuntimeErrorPayload>;

    async fn get_invocation(
        &self,
        ctx: &SecurityContext,
        invocation_id: &InvocationId,
    ) -> Result<InvocationRecord, RuntimeErrorPayload>;

    /// Control invocation lifecycle (cancel, suspend, resume, retry, replay).
    async fn control_invocation(
        &self,
        ctx: &SecurityContext,
        invocation_id: &InvocationId,
        action: InvocationControlAction,
    ) -> Result<InvocationRecord, RuntimeErrorPayload>;

    async fn list_invocations(
        &self,
        ctx: &SecurityContext,
        filter: InvocationListFilter,
    ) -> Result<Vec<InvocationRecord>, RuntimeErrorPayload>;

    async fn get_invocation_timeline(
        &self,
        ctx: &SecurityContext,
        invocation_id: &InvocationId,
    ) -> Result<Vec<InvocationTimelineEvent>, RuntimeErrorPayload>;

    async fn create_schedule(
        &self,
        ctx: &SecurityContext,
        schedule: Schedule,
    ) -> Result<Schedule, RuntimeErrorPayload>;

    async fn list_schedules(
        &self,
        ctx: &SecurityContext,
        filter: ScheduleListFilter,
    ) -> Result<Vec<Schedule>, RuntimeErrorPayload>;

    async fn get_schedule(
        &self,
        ctx: &SecurityContext,
        schedule_id: &ScheduleId,
    ) -> Result<Schedule, RuntimeErrorPayload>;

    async fn patch_schedule(
        &self,
        ctx: &SecurityContext,
        schedule_id: &ScheduleId,
        patch: SchedulePatch,
    ) -> Result<Schedule, RuntimeErrorPayload>;

    async fn pause_schedule(
        &self,
        ctx: &SecurityContext,
        schedule_id: &ScheduleId,
    ) -> Result<Schedule, RuntimeErrorPayload>;

    async fn resume_schedule(
        &self,
        ctx: &SecurityContext,
        schedule_id: &ScheduleId,
    ) -> Result<Schedule, RuntimeErrorPayload>;

    async fn delete_schedule(
        &self,
        ctx: &SecurityContext,
        schedule_id: &ScheduleId,
    ) -> Result<(), RuntimeErrorPayload>;

    async fn get_schedule_history(
        &self,
        ctx: &SecurityContext,
        schedule_id: &ScheduleId,
    ) -> Result<Vec<InvocationRecord>, RuntimeErrorPayload>;

    async fn create_trigger(
        &self,
        ctx: &SecurityContext,
        trigger: Trigger,
    ) -> Result<Trigger, RuntimeErrorPayload>;

    async fn list_triggers(
        &self,
        ctx: &SecurityContext,
        filter: TriggerListFilter,
    ) -> Result<Vec<Trigger>, RuntimeErrorPayload>;

    async fn get_trigger(
        &self,
        ctx: &SecurityContext,
        trigger_id: &TriggerId,
    ) -> Result<Trigger, RuntimeErrorPayload>;

    async fn update_trigger(
        &self,
        ctx: &SecurityContext,
        trigger_id: &TriggerId,
        patch: TriggerPatch,
    ) -> Result<Trigger, RuntimeErrorPayload>;

    async fn delete_trigger(
        &self,
        ctx: &SecurityContext,
        trigger_id: &TriggerId,
    ) -> Result<(), RuntimeErrorPayload>;

    async fn get_tenant_runtime_policy(
        &self,
        ctx: &SecurityContext,
        tenant_id: &TenantId,
    ) -> Result<TenantRuntimePolicy, RuntimeErrorPayload>;

    async fn update_tenant_runtime_policy(
        &self,
        ctx: &SecurityContext,
        tenant_id: &TenantId,
        policy: TenantRuntimePolicy,
    ) -> Result<TenantRuntimePolicy, RuntimeErrorPayload>;

    async fn get_tenant_usage(
        &self,
        ctx: &SecurityContext,
        tenant_id: &TenantId,
    ) -> Result<TenantUsage, RuntimeErrorPayload>;

    async fn get_tenant_usage_history(
        &self,
        ctx: &SecurityContext,
        tenant_id: &TenantId,
        filter: UsageHistoryFilter,
    ) -> Result<Vec<TenantUsage>, RuntimeErrorPayload>;
}

/// GTS ID: gts.x.core.serverless.err.v1~x.core.serverless.err.validation.v1~
/// Validation error extending base error, containing multiple issues.
/// Returned only when validation fails; success returns the validated definition.
#[derive(Clone, Debug)]
pub struct ValidationError {
    /// Inherited from base error type.
    pub message: String,
    /// Inherited from base error type (always NonRetryable for validation errors).
    pub category: RuntimeErrorCategory,
    /// Inherited from base error type.
    pub details: Option<serde_json::Value>,
    /// List of validation issues (at least one).
    pub issues: Vec<ValidationIssue>,
}

/// A single validation issue with error type, location, and message.
#[derive(Clone, Debug)]
pub struct ValidationIssue {
    /// Specific validation error type (e.g., "schema_mismatch", "missing_field").
    pub error_type: String,
    /// Location of the issue in the definition or input.
    pub location: ValidationLocation,
    /// Human-readable description of the issue.
    pub message: String,
    /// Suggested correction or fix for the issue.
    pub suggestion: Option<String>,
}

/// Location of a validation issue within a definition or input.
#[derive(Clone, Debug)]
pub struct ValidationLocation {
    /// JSON path to the error location (e.g., "$.traits.limits.timeout_seconds").
    pub path: String,
    /// Line number in source code (for code implementations).
    pub line: Option<u64>,
    /// Column number in source code (for code implementations).
    pub column: Option<u64>,
}

#[derive(Clone, Debug, Default)]
pub struct EntrypointListFilter {
    pub tenant_id: Option<TenantId>,
    /// GTS ID prefix for filtering (supports wildcards per GTS spec section 10).
    pub entrypoint_id_prefix: Option<String>,
    pub status: Option<DefinitionStatus>,
    /// Filter by ownership scope (user, tenant, system) per PRD BR-036.
    pub owner_type: Option<OwnerType>,
    pub runtime: Option<String>,
    pub tags: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct InvocationListFilter {
    pub tenant_id: Option<TenantId>,
    pub entrypoint_id: Option<EntrypointId>,
    pub status: Option<InvocationStatus>,
    pub mode: Option<InvocationMode>,
    pub correlation_id: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ScheduleListFilter {
    pub tenant_id: Option<TenantId>,
    pub entrypoint_id: Option<EntrypointId>,
    pub status: Option<ScheduleStatus>,
}

#[derive(Clone, Debug, Default)]
pub struct TriggerListFilter {
    pub tenant_id: Option<TenantId>,
    pub event_type_id: Option<GtsId>,
    pub entrypoint_id: Option<EntrypointId>,
}

#[derive(Clone, Debug)]
pub struct TriggerPatch {
    pub event_type_id: Option<GtsId>,
    pub event_filter_query: Option<String>,
    pub entrypoint_id: Option<EntrypointId>,
    pub dead_letter_queue: Option<DeadLetterQueueConfig>,
    pub status: Option<TriggerStatus>,
}

#[derive(Clone, Debug)]
pub struct SchedulePatch {
    pub name: Option<String>,
    pub timezone: Option<String>,
    pub expression: Option<ScheduleExpression>,
    pub input_overrides: Option<JsonValue>,
    pub missed_policy: Option<MissedSchedulePolicy>,
    pub status: Option<ScheduleStatus>,
}

#[derive(Clone, Debug)]
pub struct TenantUsage {
    pub tenant_id: TenantId,
    pub timestamp: DateTime<Utc>,
    pub current: UsageMetrics,
    pub quotas: TenantQuotas,
    pub utilization_percent: UsageUtilization,
}

#[derive(Clone, Debug)]
pub struct UsageMetrics {
    pub concurrent_executions: u64,
    pub total_definitions: u64,
    pub total_schedules: u64,
    pub total_triggers: u64,
    pub execution_history_mb: u64,
}

#[derive(Clone, Debug)]
pub struct UsageUtilization {
    pub concurrent_executions: f64,
    pub definitions: f64,
    pub schedules: f64,
    pub triggers: f64,
    pub execution_history: f64,
}

#[derive(Clone, Debug, Default)]
pub struct UsageHistoryFilter {
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub granularity: Option<UsageGranularity>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum UsageGranularity {
    Hourly,
    Daily,
    Weekly,
}

/// GTS ID: gts.x.core.serverless.timeline_event.v1~
#[derive(Clone, Debug)]
pub struct InvocationTimelineEvent {
    pub at: DateTime<Utc>,
    pub event_type: TimelineEventType,
    pub status: InvocationStatus,
    pub step_name: Option<String>,
    pub duration_ms: Option<u64>,
    pub message: Option<String>,
    pub details: JsonValue,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimelineEventType {
    Started,
    StepStarted,
    StepCompleted,
    StepFailed,
    StepRetried,
    Suspended,
    Resumed,
    SignalReceived,
    CheckpointCreated,
    CompensationStarted,
    CompensationCompleted,
    CompensationFailed,
    Succeeded,
    Failed,
    Canceled,
    DeadLettered,
}
```

---

## Implementation Considerations

- Authorization checks MUST be enforced on all operations (definition management, invocation lifecycle, schedules,
  debug, tenant policy).
- In-flight invocations MUST continue with their original definition version.
- Audit events MUST include `tenant_id`, `actor`, and `correlation_id`.
- Observability MUST include correlation and trace identifiers, and metrics segmented by tenant and entrypoint.

