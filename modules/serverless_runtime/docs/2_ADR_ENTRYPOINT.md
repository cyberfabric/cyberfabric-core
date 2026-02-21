# ADR — Serverless Runtime Entrypoints (Function & Workflow Definitions, Invocation API)

## Status
Proposed

## Context
The Serverless Runtime module needs a stable, tenant-safe way to:
- register functions & workflows schema and code definitions at runtime
- invoke those definitions via API
- observe execution progress and outcomes

This ADR defines:
- GTS-based identifiers and JSON Schema conventions for functions & workflows definitions
- definition “traits” (semantics, limits, runtime, error types)
- invocation API (start, status, cancel, dry-run) with observability fields

## Classifications

### Function

Functions are single-unit compute entrypoints designed for request/response execution:
- Stateless with respect to the runtime (any durable state is stored externally).
- Typically short-lived (bounded by platform timeout limits).
- Commonly used as building blocks for APIs, event handlers, and single-step jobs.
- Retries are common; authors SHOULD design for idempotency when side effects are possible.

Event waiting is supported by the platform via execution suspension and later resumption.
For functions this is an advanced capability and is primarily relevant for asynchronous execution.

### Workflow

Workflows are durable, multi-step orchestrations that coordinate one or more actions over time.
A workflow is a definition that may contain multiple execution steps, suspension points, and runtime-managed continuation.
- Persisted execution state (durable progress across restarts).
- Supports long-running behavior (timers, waiting on external events, human-in-the-loop patterns).
- Encodes orchestration logic (fan-out/fan-in, branching, retries, compensation) over multiple steps.
- Individual steps are commonly implemented by calling functions or external integrations.

For workflows, the underlying code interpreter/runtime is responsible for typical workflow execution processing, including:
- step identification
- step retry scheduling and retry status
- compensation orchestration
- checkpointing and suspend/resume
- event subscription and event-driven continuation

Event waiting for workflows is implemented via suspension plus event subscription, with runtime-level examples defined in the code runtime document (e.g. Starlark).

In Hyperspot, functions and workflows use the same entrypoint definition schema and the same implementation language constructs (if/else, loops, variables, etc.).
In practice, everything is defined and invoked as a function entrypoint, and the selected runtime MAY determine that a given entrypoint should be treated as a workflow by analyzing the code during validation. For example, the Starlark runtime can validate the program and detect runtime durability primitives (e.g., checkpoints/snapshots or awaiting events/timers); if present, the runtime can apply workflow execution semantics (durable suspend/resume and continuation) rather than treating it as a regular short-lived function.
The practical difference is execution semantics:
- Workflows are typically **async** and include internal snapshot/checkpoint behavior so the runtime can persist progress and support suspend/resume.
- Functions are typically **sync**, short-lived, and stateless with respect to the runtime.

## Synchronous vs Asynchronous

Functions and workflows can be executed synchronously or asynchronously.

- **sync**
  - The client waits for completion and receives the result (or error) in the HTTP response.
  - This is constrained by HTTP and gateway timeouts, so it is best for short executions.

- **async**
  - The client receives an `invocation_id` immediately and retrieves status/results later via the status endpoint.
  - The client “polls” for completion:
    - **Short poll**: `GET` returns immediately with the latest known status.
    - **Long poll**: `GET` waits up to a timeout before returning (otherwise identical semantics); this reduces client round-trips while still using polling.

## Functions & Workflows identifier conventions

Both functions and workflows use the same schema and semantic definition following `https://github.com/GlobalTypeSystem/gts-spec` specification.

According to GTS specification, function/workflow contract definitions are represented as GTS **types** (JSON Schema documents), identified by `$id` values that end with `~`.

* Base function/workflow definition:
  - `gts.x.core.faas.func.v1~` - defines the base function/workflow definition schema, including placeholders for `params`, `returns`, `errors`, and the schema for `traits` (semantics) such as timeouts and limits.

* Specific function/workflow definitions:
  - are derived GTS types (JSON Schemas) that reference the base type (typically via `allOf`) and then pin concrete `params`, `returns`, and initialized `traits` values.

Functions/workflows identifier examples:
  - `gts.x.core.faas.func.v1~vendor_x.app.namespace.func_name.v1~`
  - `gts.x.core.faas.func.v1~vendor_y.pkg.domain.workflow_name.v1~`

The invocation API uses GTS ID to refer to the specific function/workflow definition type identifier (the `$id` value without the `gts://` prefix).

## Schemas
This section defines base schema for functions/workflows definitions.

### Base schema: Entrypoint Definition
**Schema ID (type):** `gts.x.core.faas.func.v1~`

Main schema elements are:
- `id` - base function/workflow type identifier.
- `params` - function input schema, to be refined by specific functions/workflows.
- `returns` - function output schema, to be refined by specific functions/workflows.
- `errors` - list of possible custom error type identifiers (GTS IDs).
- `traits` - schema for `traits` (semantics) such as timeouts and limits, that must be pinned by specific functions/workflows (e.g., with property-level `const`)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.func.v1~",
  "title": "FaaS Function Definition",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string",
      "description": "Well-known type identifier of this function/workflow definition.",
      "examples": ["gts.x.core.faas.func.v1~vendor.app.namespace.func_name.v1~"]
    },

    "title": { "type": "string" },
    "description": { "type": "string" },

    "params": {
      "description": "Function input schema (DTS-style naming). May be a $ref to a GTS schema, inline JSON Schema, or void.",
      "oneOf": [
        {"$ref": "https://json-schema.org/draft/2020-12/schema"},
        {"type": "object", "additionalProperties": true},
        {"type": "null", "description": "Void params"}
      ]
    },

    "returns": {
      "description": "Function output schema (DTS-style naming). May be a $ref to a GTS schema, inline JSON Schema, or void.",
      "oneOf": [
        {"$ref": "https://json-schema.org/draft/2020-12/schema"},
        {"type": "object", "additionalProperties": true},
        {"type": "null", "description": "Void returns"}
      ]
    },

    "errors": {
      "type": "array",
      "items": {"type": "string"},
      "default": [],
      "description": "List of possible customerror type identifiers (GTS IDs)."
    },

    "traits": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "entrypoint": {
          "type": "boolean",
          "default": true,
          "description": "If true, the function is externally callable via the invocation API. If false, it is internal-only and can be referenced only from other workflows/functions."
        },
        "is_idempotent": {
          "type": "boolean",
          "default": false,
          "description": "Whether repeated execution with the same effective input produces the same effect."
        },

        "invocation": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "supported": {
              "type": "array",
              "items": {"type": "string", "enum": ["sync", "async"]},
              "minItems": 1,
              "uniqueItems": true,
              "default": ["async"],
              "description": "Which invocation modes this entrypoint supports."
            },
            "default": {
              "type": "string",
              "enum": ["sync", "async"],
              "default": "async",
              "description": "Default invocation mode if the caller does not specify one."
            }
          },
          "required": ["supported"]
        },

        "caching": {
          "type": "object",
          "additionalProperties": false,
          "description": "Client caching policy for successful results. Cache reuse is ALWAYS scoped by tenant, caller token/identity, all request headers, and the full `params` payload. This trait is a hint for the caller: the function owner is responsible for providing a reasonable hint, and the runtime cannot guarantee that a cached value equals what the function would return now. This trait only defines freshness TTL.",
          "properties": {
            "max_age_seconds": {
              "type": "integer",
              "minimum": 0,
              "default": 0,
              "description": "How long a cached result is considered fresh (TTL). 0 means no caching (clients MUST NOT reuse cached results). TTL is a client-side reuse hint and does not guarantee correctness under non-determinism, data drift, or changing upstream dependencies."
            }
          },
          "required": ["max_age_seconds"]
        },
        "runtime": {
          "type": "string",
          "description": "Mandatory runtime selector (e.g., Starlark, WASM, AWS Lambda, etc.)."
        },

        "default_timeout_seconds": {
          "type": "integer",
          "minimum": 1,
          "description": "Default timeout for this function/workflow in seconds."
        },
        "default_memory_mb": {
          "type": "integer",
          "minimum": 1,
          "description": "Default memory allocation for this function/workflow in MB."
        },
        "default_cpu": {
          "type": "number",
          "minimum": 0,
          "description": "Default CPU allocation for this function/workflow."
        },

        "localization_dictionary_id": {
          "type": "string",
          "description": "Optional localization dictionary reference (GTS ID)."
        },

        "tags": {
          "type": "array",
          "items": {"type": "string"},
          "default": [],
          "description": "Optional tags for this function/workflow categorization, searchability, etc."
        }

      },
      "required": ["runtime"]
    },

    "implementation": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "code": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "language": {"type": "string"},
            "source": {
              "type": "string",
              "description": "Inline source code (optional)."
            }
          },
          "required": ["language"]
        }
      },
      "description": "Execution implementation. `code` MUST be provided.",
      "minProperties": 1,
      "required": ["code"]
    }
  },
  "required": ["id", "params", "returns", "traits", "implementation"]
}
```

### Base schema: Error
**Schema ID (type):** `gts.x.core.faas.err.v1~`

This is the base error envelope returned by the runtime (and referenced by entrypoints via `errors`). Derived error types specialize the `details` field.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~",
  "title": "FaaS Error",
  "type": "object",
  "additionalProperties": false,
  "properties": {
    "id": {
      "type": "string",
      "description": "Error code identifier (GTS ID).",
      "examples": ["gts.x.core.faas.err.v1~x.core._.code.v1~"]
    },
    "message": {
      "type": "string",
      "description": "Human-readable error message."
    },
    "details": {
      "type": "object",
      "additionalProperties": true,
      "description": "Error-class-specific structured payload."
    }
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Upstream HTTP error
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.http.v1~`

Used for failures from upstream HTTP calls (e.g., `r_http_get_v1()`). This schema specializes `details`.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.http.v1~",
  "title": "FaaS Upstream HTTP Error",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.http.v1~"},
    "details": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "status_code": {"type": "integer", "minimum": 100, "maximum": 599},
        "headers": {
          "type": "object",
          "additionalProperties": {
            "oneOf": [
              {"type": "string"},
              {"type": "array", "items": {"type": "string"}}
            ]
          }
        },
        "body": {
          "type": "object",
          "additionalProperties": true,
          "description": "Parsed JSON body when available; otherwise an empty object."
        }
      },
      "required": ["status_code", "headers", "body"]
    }
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Upstream HTTP transport error (timeout / no connection)
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.http_transport.v1~`

Used for upstream HTTP call failures where **no HTTP response** is available (e.g., timeout, connection failure). This schema specializes `details`.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.http_transport.v1~",
  "title": "FaaS Upstream HTTP Transport Error",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.http_transport.v1~"},
    "details": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "url": {"type": "string"}
      },
      "required": ["url"]
    }
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Upstream HTTP transport timeout
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.http_transport.v1~x.core._.timeout.v1~`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.http_transport.v1~x.core._.timeout.v1~",
  "title": "FaaS Upstream HTTP Transport Timeout",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~x.core._.http_transport.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.http_transport.v1~x.core._.timeout.v1~"}
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Upstream HTTP transport no connection
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.http_transport.v1~x.core._.no_connection.v1~`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.http_transport.v1~x.core._.no_connection.v1~",
  "title": "FaaS Upstream HTTP Transport No Connection",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~x.core._.http_transport.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.http_transport.v1~x.core._.no_connection.v1~"}
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Runtime resource/timeout error
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.runtime.v1~`

Used when the runtime aborts execution due to resource limits or platform constraints.

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.runtime.v1~",
  "title": "FaaS Runtime Error",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.runtime.v1~"},
    "details": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "limit": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "timeout_seconds": {"type": "integer", "minimum": 1},
            "memory_limit_mb": {"type": "integer", "minimum": 1},
            "cpu_limit": {"type": "number", "minimum": 0}
          }
        },
        "observed": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "duration_ms": {"type": "integer", "minimum": 0},
            "cpu_time_ms": {"type": "integer", "minimum": 0},
            "max_memory_used_mb": {"type": "integer", "minimum": 0}
          }
        }
      },
      "required": []
    }
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Runtime timeout
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.timeout.v1~`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.timeout.v1~",
  "title": "FaaS Runtime Timeout",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~x.core._.runtime.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.timeout.v1~"}
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Runtime memory limit
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.memory_limit.v1~`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.memory_limit.v1~",
  "title": "FaaS Runtime Memory Limit",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~x.core._.runtime.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.memory_limit.v1~"}
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Runtime CPU limit
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.cpu_limit.v1~`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.cpu_limit.v1~",
  "title": "FaaS Runtime CPU Limit",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~x.core._.runtime.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.cpu_limit.v1~"}
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Runtime canceled
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.canceled.v1~`

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.canceled.v1~",
  "title": "FaaS Runtime Canceled",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~x.core._.runtime.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.canceled.v1~"}
  },
  "required": ["id", "message", "details"]
}
```

### Derived error schema: Code execution/validation error
**Schema ID (type):** `gts.x.core.faas.err.v1~x.core._.code.v1~`

Used for code-level failures produced by the selected runtime (e.g., Starlark validation/compile/runtime errors).

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.err.v1~x.core._.code.v1~",
  "title": "FaaS Code Error",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.err.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.err.v1~x.core._.code.v1~"},
    "details": {
      "type": "object",
      "additionalProperties": false,
      "properties": {
        "runtime": {"type": "string", "description": "Language/runtime identifier (e.g., starlark)."},
        "phase": {"type": "string", "enum": ["validate", "compile", "execute"]},
        "error_kind": {"type": "string", "description": "Runtime-specific diagnostic kind/code (not an exception)."},
        "location": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "line": {"type": "integer", "minimum": 1},
            "code": {"type": "string"}
          },
          "required": ["line", "code"]
        },
        "stack": {
          "type": "object",
          "additionalProperties": false,
          "properties": {
            "frames": {
              "type": "array",
              "items": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                  "function": {"type": "string"},
                  "file": {"type": "string"},
                  "line": {"type": "integer", "minimum": 1}
                },
                "required": ["function", "file", "line"]
              }
            }
          },
          "required": ["frames"]
        }
      },
      "required": ["runtime", "phase"]
    }
  },
  "required": ["id", "message", "details"]
}
```

## Examples
### Function definition type example #1

This is a tax calculation function example written in Starlark that:
- Supports both sync and async invocation (default is async)
- Idempotent
- Includes default resource/time limits
- Enables client caching for 60 seconds (within tenant + token/identity + headers + params)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.func.v1~vendor.app.billing.calculate_tax.v1~",
  "title": "Calculate Tax",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.func.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.func.v1~vendor.app.billing.calculate_tax.v1~"},
    "params": {
      "const": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "invoice_total": {"type": "number"},
          "region": {"type": "string"}
        },
        "required": ["invoice_total", "region"]
      }
    },
    "returns": {
      "const": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "tax": {"type": "number"}
        },
        "required": ["tax"]
      }
    },
    "errors": {"const": ["gts.x.core.faas.err.v1~x.core._.code.v1~", "gts.x.core.faas.err.v1~vendor.app.billing.tax_error.v1~"]},
    "traits": {
      "type": "object",
      "properties": {
        "runtime": {"const": "starlark"},
        "is_idempotent": {"const": true},
        "invocation": {
          "type": "object",
          "properties": {
            "supported": {"const": ["sync", "async"]},
            "default": {"const": "async"}
          }
        },
        "default_timeout_seconds": {"const": 10},
        "default_memory_mb": {"const": 128},
        "caching": {
          "type": "object",
          "properties": {
            "max_age_seconds": {"const": 60}
          },
          "required": ["max_age_seconds"]
        }
      }
    },
    "implementation": {
      "type": "object",
      "properties": {
        "code": {
          "type": "object",
          "properties": {
            "language": {"const": "starlark"},
            "source": {"const": "def main(ctx, input):\n  return {\"tax\": 0}\n"}
          },
          "required": ["language"]
        }
      },
      "required": ["code"]
    }
  },
  "required": ["id", "params", "returns", "traits", "implementation"]
}
```

### Function definition type example #2

This is an example of a customer lookup function written in Starlark that:
- Supports both sync and async invocation (default is sync)
- Idempotent
- No client caching by default (`max_age_seconds = 0`)

```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.func.v1~vendor.app.crm.lookup_customer.v1~",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.func.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.func.v1~vendor.app.crm.lookup_customer.v1~"},
    "params": {
      "const": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "customer_id": {"type": "string"}
        },
        "required": ["customer_id"]
      }
    },
    "returns": {
      "const": {
        "type": "object",
        "additionalProperties": false,
        "properties": {
          "customer": {
            "type": "object",
            "additionalProperties": true
          }
        },
        "required": ["customer"]
      }
    },
    "traits": {
      "type": "object",
      "properties": {
        "runtime": {"const": "starlark"},
        "is_idempotent": {"const": true},
        "invocation": {
          "type": "object",
          "properties": {
            "supported": {"const": ["sync", "async"]},
            "default": {"const": "sync"}
          }
        },
        "default_timeout_seconds": {"const": 5},
        "caching": {
          "type": "object",
          "properties": {
            "max_age_seconds": {"const": 0}
          },
          "required": ["max_age_seconds"]
        }
      }
    },
    "implementation": {
      "type": "object",
      "properties": {
        "code": {
          "type": "object",
          "properties": {
            "language": {"const": "starlark"},
            "source": {"const": "def main(ctx, input):\\n  url = \"https://example.crm/api/customer/\" + input[\"customer_id\"]\\n  resp = r_http_get_v1(url)\\n  return {\"customer\": resp}\\n"}
          },
          "required": ["language"]
        }
      },
      "required": ["code"]
    }
  },
  "required": ["id", "params", "returns", "traits", "implementation"]
}
```

## Invocation API (Entrypoint)
### Principles
- Executed invocations produce an **invocation record** with a stable `invocation_id`.
- Invocations SHOULD support:
  - `Idempotency-Key` (to prevent duplicate starts)
  - `X-Request-Id` / correlation id
  - `traceparent` (W3C trace context)
- Invocation status is obtained via a dedicated endpoint.

Throttling and rate limiting:
- The runtime SHOULD enforce throttling to protect itself and downstream dependencies (e.g., per-tenant rate limits, per-entrypoint concurrency caps).
- When throttled, the runtime SHOULD return `429 Too Many Requests` and SHOULD include `Retry-After`.

Dry run vs execute:
- `dry_run: true` performs validation (including authorization) without creating a durable invocation record.
- This aligns with AWS Lambda `InvocationType=DryRun`.

### 1) Invoke an entrypoint (sync or async)
`POST /api/serverless-runtime/v1/invocations`

Request body:
```json
{
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.namespace.func_name.v1~",
  "params": {},
  "mode": "async",
  "dry_run": false
}
```

Response:
- `200 OK` (when `mode` is `sync` and `dry_run` is false) — **InvocationRecord (completed)**
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.namespace.func_name.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.succeeded.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": "2026-01-01T00:00:00.010Z",
    "finished_at": "2026-01-01T00:00:00.120Z"
  },
  "result": {
    "tax": 0
  },
  "error": null,
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": 110,
      "billed_duration_ms": 200,
      "cpu_time_ms": 70,
      "memory_limit_mb": 128,
      "max_memory_used_mb": 64
    }
  }
}
```

- `200 OK` (when `mode` is `sync` and `dry_run` is false) — **InvocationRecord (failed)**
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.crm.lookup_customer.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.failed.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": "2026-01-01T00:00:00.010Z",
    "finished_at": "2026-01-01T00:00:00.120Z"
  },
  "result": null,
  "error": {
    "id": "gts.x.core.faas.err.v1~x.core._.http.v1~",
    "message": "Upstream CRM returned non-success status",
    "details": {
      "status_code": 503,
      "headers": {"content-type": "application/json"},
      "body": {"error": "service_unavailable"}
    }
  },
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": 110,
      "billed_duration_ms": 200,
      "cpu_time_ms": 70,
      "memory_limit_mb": 128,
      "max_memory_used_mb": 64
    }
  }
}
```

- `202 Accepted` (when `mode` is `async` and `dry_run` is false) — **InvocationRecord (pending)**
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.namespace.func_name.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.queued.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": null,
    "finished_at": null
  },
  "result": null,
  "error": null,
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": null,
      "billed_duration_ms": null,
      "cpu_time_ms": null,
      "memory_limit_mb": 128,
      "max_memory_used_mb": null
    }
  }
}
```

- `200 OK` (when `dry_run` is true)
```json
{
  "dry_run": true,
  "result": {},
  "error": null,
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": null,
      "billed_duration_ms": null,
      "cpu_time_ms": null,
      "memory_limit_mb": null,
      "max_memory_used_mb": null
    }
  }
}
```

### 2) Get invocation status
`GET /api/serverless-runtime/v1/invocations/{invocation_id}`

Response:
- `200 OK` — **InvocationRecord (pending)**
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.namespace.func_name.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.queued.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": null,
    "finished_at": null
  },
  "result": null,
  "error": null,
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": null,
      "billed_duration_ms": null,
      "cpu_time_ms": null,
      "memory_limit_mb": 128,
      "max_memory_used_mb": null
    }
  }
}
```

- `200 OK` — **InvocationRecord (failed: code error)**
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.billing.calculate_tax.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.failed.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": "2026-01-01T00:00:00.010Z",
    "finished_at": "2026-01-01T00:00:00.020Z"
  },
  "result": null,
  "error": {
    "id": "gts.x.core.faas.err.v1~x.core._.code.v1~",
    "message": "Starlark execution error: division by zero",
    "details": {
      "runtime": "starlark",
      "phase": "execute",
      "error_kind": "division_by_zero",
      "location": {"line": 12, "code": "x = 1 / 0"},
      "stack": {
        "frames": [
          {"function": "main", "file": "inline", "line": 1},
          {"function": "calculate", "file": "inline", "line": 12}
        ]
      }
    }
  },
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": 10,
      "billed_duration_ms": 100,
      "cpu_time_ms": 8,
      "memory_limit_mb": 128,
      "max_memory_used_mb": 24
    }
  }
}
```

- `200 OK` — **InvocationRecord (failed: runtime timeout)**
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.namespace.func_name.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.failed.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": "2026-01-01T00:00:00.010Z",
    "finished_at": "2026-01-01T00:00:05.010Z"
  },
  "result": null,
  "error": {
    "id": "gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.timeout.v1~",
    "message": "Execution timed out",
    "details": {
      "limit": {"timeout_seconds": 5},
      "observed": {"duration_ms": 5000, "cpu_time_ms": 4900, "max_memory_used_mb": 90}
    }
  },
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": 5000,
      "billed_duration_ms": 5000,
      "cpu_time_ms": 4900,
      "memory_limit_mb": 128,
      "max_memory_used_mb": 90
    }
  }
}
```

- `200 OK` — **InvocationRecord (failed: upstream no connection)**
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.crm.lookup_customer.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.failed.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": "2026-01-01T00:00:00.010Z",
    "finished_at": "2026-01-01T00:00:00.520Z"
  },
  "result": null,
  "error": {
    "id": "gts.x.core.faas.err.v1~x.core._.http_transport.v1~x.core._.no_connection.v1~",
    "message": "Upstream HTTP request failed",
    "details": {
      "url": "https://example.crm/api/customers/123"
    }
  },
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": 510,
      "billed_duration_ms": 600,
      "cpu_time_ms": 30,
      "memory_limit_mb": 128,
      "max_memory_used_mb": 40
    }
  }
}
```

- `200 OK` — **InvocationRecord (completed)**
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.namespace.func_name.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.succeeded.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": "2026-01-01T00:00:01.000Z",
    "finished_at": "2026-01-01T00:00:02.000Z"
  },
  "result": {},
  "error": null,
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": 1000,
      "billed_duration_ms": 1000,
      "cpu_time_ms": 620,
      "memory_limit_mb": 128,
      "max_memory_used_mb": 80
    }
  }
}
```

Status values:
- `gts.x.core.faas.status.v1~x.core._.queued.v1`
- `gts.x.core.faas.status.v1~x.core._.running.v1`
- `gts.x.core.faas.status.v1~x.core._.suspended.v1`
- `gts.x.core.faas.status.v1~x.core._.succeeded.v1`
- `gts.x.core.faas.status.v1~x.core._.failed.v1`
- `gts.x.core.faas.status.v1~x.core._.canceled.v1`

`gts.x.core.faas.status.v1~x.core._.suspended.v1` indicates execution is waiting on a timer or an external event subscription and will be resumed by the runtime.

### 3) Cancel function/workflow execution
`POST /api/serverless-runtime/v1/invocations/{invocation_id}:cancel`

Response:
- `202 Accepted`

### 4) Timeline / step history (observability)
`GET /api/serverless-runtime/v1/invocations/{invocation_id}/timeline`

Query params:
- `limit` (optional)
- `cursor` (optional)

Response (example):
```json
{
  "invocation_id": "<opaque-id>",
  "items": [
    {"at": "2026-01-01T00:00:00Z", "type": "STARTED"},
    {"at": "2026-01-01T00:00:01Z", "type": "STEP_STARTED", "step_id": "step-1"},
    {"at": "2026-01-01T00:00:02Z", "type": "STEP_SUCCEEDED", "step_id": "step-1"}
  ],
  "page_info": {
    "next_cursor": "<opaque-cursor>",
    "prev_cursor": null,
    "limit": 25
  }
}
```

### 5) Debug invocation (invocation record + debug info)
`GET /api/serverless-runtime/v1/invocations/{invocation_id}/debug`

Response (example):
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.namespace.func_name.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.running.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": "2026-01-01T00:00:00.010Z",
    "finished_at": null
  },
  "result": null,
  "error": null,
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": null,
      "billed_duration_ms": null,
      "cpu_time_ms": null,
      "memory_limit_mb": 128,
      "max_memory_used_mb": null
    }
  },
  "debug": {
    "location": {
      "line": 12,
      "code": "resp = r_http_get_v1(url)"
    },
    "stack": {
      "frames": [
        {"function": "main", "file": "inline", "line": 1},
        {"function": "calculate", "file": "inline", "line": 12}
      ]
    }
  }
}
```

Call trace retrieval (paginated via `debug.page`):
`GET /api/serverless-runtime/v1/invocations/{invocation_id}/debug/calls`

Query params:
- `limit` (optional)
- `cursor` (optional)

Response (example):
```json
{
  "invocation_id": "<opaque-id>",
  "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.namespace.func_name.v1~",
  "status": "gts.x.core.faas.status.v1~x.core._.running.v1",
  "timestamps": {
    "created_at": "2026-01-01T00:00:00.000Z",
    "started_at": "2026-01-01T00:00:00.010Z",
    "finished_at": null
  },
  "result": null,
  "error": null,
  "observability": {
    "correlation_id": "<id>",
    "trace_id": "<trace>",
    "metrics": {
      "duration_ms": null,
      "billed_duration_ms": null,
      "cpu_time_ms": null,
      "memory_limit_mb": 128,
      "max_memory_used_mb": null
    }
  },
  "debug": {
    "location": {
      "line": 12,
      "code": "resp = r_http_get_v1(url)"
    },
    "stack": {
      "frames": [
        {"function": "main", "file": "inline", "line": 1},
        {"function": "calculate", "file": "inline", "line": 12}
      ]
    },
    "page": {
      "items": [
        {
          "call_invocation_id": "<opaque-id>",
          "entrypoint_id": "gts.x.core.faas.func.v1~vendor.app.crm.lookup_customer.v1~",
          "params": {"customer_id": "c_123"},
          "duration_ms": 42,
          "response": {
            "result": {"customer": {"id": "c_123", "name": "Jane"}},
            "error": null
          }
        }
      ],
      "page_info": {
        "next_cursor": "<opaque-cursor>",
        "prev_cursor": null,
        "limit": 25
      }
    }
  }
}
```

## TODO:

## Comparison with similar solutions

This section compares the proposed Serverless Runtime entrypoint model and invocation APIs with similar capabilities in:
- AWS (Lambda + Step Functions)
- Google Cloud (Cloud Functions + Workflows)
- Azure (Azure Functions + Durable Functions)

Notes:
- Public clouds generally split “function” and “workflow/orchestration” into different products.
- Hyperspot deliberately exposes a unified entrypoint definition schema and a single invocation surface, with consistent response shapes.

### Category: Definition model and versioning

| Capability | Hyperspot Serverless Runtime | AWS | Google Cloud | Azure |
|---|---|---|---|---|
| Definition type model | Unified entrypoint definition schema (function/workflow) via GTS-identified JSON Schemas | Split: Lambda functions vs Step Functions state machines | Split: Cloud Functions vs Workflows | Split: Functions vs Durable orchestrations |
| Type system / schema IDs | GTS identifiers for base + derived definition types | No first-class type IDs; resource ARNs + service-specific specs | No first-class type IDs; resource names + service-specific specs | No first-class type IDs; resource IDs + service-specific specs |
| Versioning concept | Definitions are versioned types; invocations point to an explicit definition ID | Lambda versions/aliases; Step Functions revisions via updates | Function revisions; workflow updates | Function versions; durable orchestration code deployments |

### Category: Invocation semantics and lifecycle

| Capability | Hyperspot Serverless Runtime | AWS | Google Cloud | Azure |
|---|---|---|---|---|
| Sync invocation | Supported (`mode: sync`) | Lambda: `RequestResponse` | HTTP-triggered functions are synchronous by default | HTTP-triggered functions are synchronous by default |
| Async invocation | Supported (`mode: async` + poll status) | Lambda: `Event`; Step Functions: start execution then poll/described execution | Workflows: start execution then poll; Functions: async via events/pubsub | Durable: start orchestration then query status |
| Dry-run | Supported (`dry_run: true`) without durable record | Lambda: `DryRun` (permission check) | Generally via validation/testing tools rather than a single universal API | Generally via validation/testing tools rather than a single universal API |
| Durable invocation record shape | Unified `InvocationRecord` for start + status + debug | Different response shapes per service | Different response shapes per service | Different response shapes per service |
| Cancel | Supported (`:cancel`) | Step Functions: stop execution; Lambda: no “cancel running invocation” | Workflows: cancel execution; Functions: stop depends on trigger/runtime | Durable: terminate instance |
| Suspend/resume (event waiting) | Supported (`status: suspended`) for timers and event-driven continuation | Step Functions: wait states + event-driven patterns; Lambda: event-driven via triggers | Workflows support waiting; functions are event-triggered or use separate services | Durable: timers + external events |
| Idempotency | Supported via idempotency key on start | Step Functions supports idempotency via execution name constraints; Lambda typically handled externally | Typically handled externally per trigger/service | Typically handled externally per trigger/service |

### Category: Observability and timeline

| Capability | Hyperspot Serverless Runtime | AWS | Google Cloud | Azure |
|---|---|---|---|---|
| Timeline / step history | Dedicated timeline endpoint (`/timeline`) | Step Functions execution history | Workflows execution history | Durable Functions history/events |
| Correlation and tracing | `observability.correlation_id` + `trace_id` fields | CloudWatch + X-Ray trace IDs (service-dependent) | Cloud Logging + Trace IDs | Application Insights correlation/tracing |
| Compute/memory metrics | Returned in `observability.metrics` (duration, billed duration, CPU time, memory limit/usage) | Lambda reports duration/billed duration/max memory used; CPU time not universally exposed | Execution metrics available via monitoring; CPU time not always directly exposed | Metrics via App Insights/monitoring; CPU time not always directly exposed |

### Category: Debugging and call trace

| Capability | Hyperspot Serverless Runtime | AWS | Google Cloud | Azure |
|---|---|---|---|---|
| Debug endpoint | `GET /debug` returns `InvocationRecord` + `debug` section (`location`, `stack`) | No single universal debug endpoint; relies on logs, traces, and per-service UIs | No single universal debug endpoint; relies on logs/traces/UIs | No single universal debug endpoint; relies on logs/App Insights/Durable history |
| Call trace (ordered) | `GET /debug/calls` returns the same shape as `GET /debug`, with `debug.page` containing a paginated ordered call list including params, duration, and exact response | Achievable via X-Ray subsegments/instrumentation; not a standard structured API output | Achievable via tracing/instrumentation; not a standard structured API output | Achievable via Durable history/instrumentation; not a standard structured API output |
| Completed-execution debug | Supported (`GET /debug` with `location`/`stack` null; `GET /debug/calls` for full trace via `debug.page`) | Historical logs/traces; UI varies by service | Historical logs/traces; UI varies by service | Historical logs/traces; durable history |

### Category: Error taxonomy

| Capability | Hyperspot Serverless Runtime | AWS | Google Cloud | Azure |
|---|---|---|---|---|
| Standard error envelope | Base error type `gts.x.core.faas.err.v1~` with `id`, `message`, `details` | Service-specific error payloads (varies) | Service-specific error payloads (varies) | Service-specific error payloads (varies) |
| Structured error classes | Derived errors: upstream HTTP, runtime limits/timeouts, code errors | Often string-based error+cause (Step Functions) and runtime-specific error types/statuses | Runtime-specific error types/statuses | Runtime-specific error types/statuses |

### Category: Caching and client behavior

| Capability | Hyperspot Serverless Runtime | AWS | Google Cloud | Azure |
|---|---|---|---|---|
| Result caching (TTL) | `traits.caching.max_age_seconds` defines client caching TTL for successful results | No first-class “function result cache TTL”; caching typically external (API gateway/CDN/app cache) | No first-class “function result cache TTL”; caching typically external | No first-class “function result cache TTL”; caching typically external |

### Open-source landscape (context)

Open-source systems that overlap with parts of this ADR:
- Temporal: strong durable workflow orchestration, rich history and retries; different model (SDK-first, not GTS-schema-first) and not a unified “function+workflow definition schema”.
- Apache Airflow: batch/workflow scheduling with strong DAG semantics; less aligned with request/response serverless invocation and per-invocation records.
- Knative / OpenFaaS: function execution and autoscaling; workflows/orchestration typically handled by separate systems.
