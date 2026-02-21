# ADR — Starlark Runtime (Function & Workflow Execution)

## Status
Proposed

## Context
Hyperspot Serverless Runtime supports functions and workflows defined as GTS-identified JSON Schemas and invoked via a unified invocation API.

This ADR defines the design of the Starlark runtime implementation used to execute those definitions:
- the Starlark program structure (entrypoint contract)
- strong typing rules and runtime validation
- runtime-provided helper methods (`r_*`) and their versioning
- asynchronous execution model (promise + await)
- workflow orchestration hooks (steps, retries, compensation, snapshots, event waiting)
- example definitions and Starlark programs for functions and workflows

This ADR is intentionally aligned with `2_ADR_ENTRYPOINT.md` (Entrypoint schema + Invocation API).

## Why Starlark?

Starlark is a good fit for Hyperspot Serverless Runtime because it provides a constrained, embeddable, deterministic scripting language with strong tooling hooks.

### Fit for functions and workflows
- Starlark can express both short-lived function handlers and long-lived workflow orchestration logic using the same `main(ctx, input)` contract.
- Workflows rely on runtime-provided primitives (`r_snapshot_v1`, `r_wait_event_v1`, `r_run_step_v1`) rather than language-level threads, exceptions, or background I/O.
  This keeps orchestration behavior explicit and makes suspend/resume semantics a runtime feature instead of a language hack.

### Rust-first embedding model
- The runtime is implemented in Rust and can interpret Starlark directly via an embedded interpreter.
- Runtime helpers (`r_*`) are the primary integration surface: the Rust host controls all I/O, persistence, event waiting, step tracking, and compensation.
- This model minimizes the host surface area compared to embedding Python/JS while still giving an ergonomic authoring experience.

### Static validation via parse/AST and policy checks
Starlark allows a practical pre-execution validation pipeline:
- parse source into an AST and fail fast on syntax errors
- restrict the available built-ins and prohibit disallowed constructs
- validate that the program exports `main(ctx, input)`
- validate calls to runtime helpers (`r_*`) for arity and argument types where possible
- optionally enforce additional policy (e.g., forbid dynamic `load()`, forbid unbounded loops, or require explicit timeouts on I/O helpers)

The runtime MAY also perform system-aware validation when registering a function/workflow, for example:
- validate that outbound URLs are recognized/allowed by policy and have corresponding credentials configured (e.g., via an outbound API gateway)
- validate that referenced event type IDs are registered in the system
- validate error types are registered in the system
- validate function/workflow contract against entrypoint schema

### Determinism and replayability
- Starlark is designed to be deterministic given the same inputs and host-provided functions.
- The runtime can ensure all nondeterminism flows through `r_*` helpers, which are versioned and can be recorded/replayed.
- This is critical for durable workflows that may be resumed from snapshots and must behave consistently.

### Resource controls and safe execution
- Because execution happens inside our Rust host, the runtime can enforce limits (wall-clock, instruction count/step budget, memory, and CPU quotas) at the interpreter boundary.
- Restricting the language and the host surface makes it easier to sandbox than general-purpose runtimes.

### Debuggability (breakpoints, stack traces, tracing)
- Starlark evaluation naturally exposes execution state (call stack, frames, current location) to the host.
- The runtime can implement:
  - setting breakpoints by source location
  - pausing/resuming execution and inspecting locals
  - structured tracing of `r_*` calls, including step boundaries and awaited events
  - deterministic stack traces aligned with source

### Familiarity (humans and LLMs)
- Starlark syntax is intentionally Python-like, making it familiar to Python users.
- Its Python-like surface also makes it a strong target for LLM-assisted authoring, review, and transformation.

### Stewardship
Starlark is used by Bazel and the language specification is maintained in the `bazelbuild` ecosystem with strong ongoing involvement from the Bazel community and Google.

For Rust-based embedding, `starlark-rust` is a widely used Rust implementation of Starlark (commonly referenced as `facebook/starlark-rust` / Meta) published as standard crates.

### Alternatives (quick comparison)
- WebAssembly (Wasm)
  - Pros: strong sandboxing, mature tooling, good performance
  - Cons: heavier authoring model, less ergonomic for orchestration scripting, debugging and host-call surface can be more complex, deterministic replay requires additional conventions
- JavaScript (embedded engine)
  - Pros: familiar language, large ecosystem
  - Cons: larger runtime/attack surface, more complexity around determinism, resource control, and sandboxing; host embedding is heavier
- Python
  - Pros: very popular, high productivity
  - Cons: embedding/sandboxing is hard, resource controls are complex, too many dynamic escape hatches for a multi-tenant runtime
- Lua
  - Pros: embeddable, small
  - Cons: less aligned with structured typing via `struct(...)` conventions and our desired validation ergonomics; ecosystem familiarity varies
- Custom DSL
  - Pros: maximum control and validation
  - Cons: high implementation cost, poor ergonomics, and tends to grow into a full language over time

## Goals
- Provide a deterministic and safe execution model for runtime created/updated code.
- Make functions and workflows compatible with a single language/runtime surface.
- Enforce schema-based typing and prevent invalid input/output at runtime.
- Support long-running execution patterns: waiting on events, suspend/resume, and snapshots.
- Provide a versioned runtime API so code remains forward-compatible.

## Non-goals
- Defining the full sandboxing / isolation strategy.
- Defining the persistence format for snapshots.
- Defining the complete event transport implementation.

## Starlark program structure

### Entrypoint
All Starlark functions and workflows MUST expose a `main(ctx, input)` function.

- `ctx` is a runtime-provided context object.
- `input` is the validated input object matching the entrypoint `params` schema.

`main()` MUST return a value matching the entrypoint `returns` schema or terminate execution via `r_exit(...)`.

## Strong type system

### Source of truth
The source of truth for function/workflow types is the GTS-identified JSON Schema:
- `params` (input)
- `returns` (output)
- `errors` (error envelope types)

### Validation rules
- On function registration, the runtime MUST validate `params` against the entrypoint schema before executing Starlark.
- Similarly, the runtime MUST validate the returned value against the `returns` schema.
- For workflows, the runtime MUST validate snapshot state and resumption input before continuing.

### Starlark typing model
Starlark is dynamically typed, but the runtime enforces a strong type system by:
- validating boundary values (input/output)
- validating `r_*` method arguments (e.g., `url` must be string)
- validating `r_wait_event(...)` against an event schema ID and payload schema

For maximum ergonomics and stricter typing, the runtime SHOULD materialize validated inputs as a Starlark `struct(...)` with fields matching the JSON Schema properties (instead of a generic dict). This enables `input.customer_id` style access.

All objects returned to the caller MUST be compatible with the JSON Schema types declared in `returns`.
In practice, this means Starlark programs MUST treat returned objects as strongly typed structs:
- no missing required fields
- no additional fields when `additionalProperties` is false
- field types must match schema

## Runtime helper methods (versioned)
All runtime helper methods (`r_*`) MUST be versioned to preserve compatibility across functions and workflows.

### Common conventions
- All helper names are prefixed with `r_`.
- Methods that perform I/O MUST be async and return a promise.
- Methods that influence control flow (`r_exit`, `r_snapshot`) MUST be deterministic.

To preserve determinism and replayability, Starlark programs MUST NOT use nondeterministic sources such as embedded `datetime.now()` or `random.random()`.
If time or randomness is required, it MUST be obtained via runtime helpers provided by the Rust host.

### Promise and await
The runtime provides a minimal async model:

- `Promise<T>` is an opaque runtime object.
- `r_await(promise)` waits for the promise to resolve and returns `T`.

The runtime MAY also support `r_await_all([p1, p2, ...])` to resolve multiple promises concurrently.

If `r_await(...)` is called on a promise that cannot complete without external input (event or timer), the runtime MUST suspend the invocation and resume it when the promise becomes ready.

### HTTP helpers

#### Response shape
`r_http_*_v1()` resolves to an `HttpResult` value which never raises exceptions and can represent either a successful HTTP response or an error.

`HttpResult` SHOULD be represented as a Starlark `struct(...)` so callers can use attribute access (`result.ok`, `result.response`, `result.error`).

Successful response:
- `ok` (boolean, `true`)
- `response` (a Starlark `struct(...)`)
  - `status_code` (integer)
  - `headers` (dict)
  - `body` (dict)

Error response:
- `ok` (boolean, `false`)
- `error` (a Starlark `struct(...)`) compatible with the runtime error envelope defined in `2_ADR_ENTRYPOINT.md`.
  - for non-2xx upstream responses: `gts.x.core.faas.err.v1~x.core._.http.v1~`
  - for upstream transport failures: `gts.x.core.faas.err.v1~x.core._.http_transport.v1~x.core._.timeout.v1~` and `gts.x.core.faas.err.v1~x.core._.http_transport.v1~x.core._.no_connection.v1~`
  - for runtime failures: `gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.timeout.v1~`, `gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.memory_limit.v1~`, `gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.cpu_limit.v1~`, and `gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.canceled.v1~`

If the upstream response body is not JSON, the runtime SHOULD return an empty object for `response.body`.

#### r_http_get_v1(...)
Signature:
- `r_http_get_v1(url, headers=None, timeout_ms=None, retry=None, exit_on_http_error=True) -> Promise<HttpResult>`

Notes:
- `url` is required.
- `headers` is an optional dict.
- `timeout_ms` is optional.
- `retry` is optional runtime-defined retry policy (e.g., {"max_attempts": 3, "backoff_ms": 200}).
- `exit_on_http_error=true` means the runtime calls `r_exit(...)` on non-2xx upstream responses or on upstream transport failures.
- `exit_on_http_error=false` means the promise resolves to an `HttpResult` and the Starlark code is responsible for handling `ok=false` and/or non-2xx status codes.

NOTE: The runtime MUST NOT perform outbound HTTP requests directly. Instead, it MUST delegate outbound calls to the outbound API gateway so that security context, credential management, and internal-to-external token/authorization exchange are applied consistently and all egress traffic is centrally enforced and observed.

#### r_http_post_v1(...)
Signature:
- `r_http_post_v1(url, body, headers=None, timeout_ms=None, retry=None, exit_on_http_error=True) -> Promise<HttpResult>`

### Sleep
- `r_sleep(milliseconds) -> Promise<None>`

### Time and randomness (deterministic)
- `r_now_v1() -> string`
- `r_rand_v1(label=None) -> string`

`r_now_v1()` returns a runtime-provided timestamp string. For workflows, the runtime MUST ensure the returned value is replay-safe (e.g., captured/recorded at a deterministic boundary and reused on resume).

`r_rand_v1(...)` returns a runtime-provided pseudo-random value. For workflows, the runtime MUST ensure the returned value is replay-safe (e.g., derived from invocation identity + an optional label, and/or recorded for deterministic replay).

### Exit
- `r_exit(error) -> None`

`error` MUST be an object compatible with the runtime error envelope.

For workflows, calling `r_exit(...)` terminates execution and triggers compensation for previously completed steps:
- the runtime MUST execute all registered compensation actions for steps that completed successfully prior to the `r_exit(...)` call
- compensations MUST be executed in reverse step completion order
- the runtime MUST NOT invoke compensation for steps that did not complete successfully

### Event waiting

#### r_wait_event_v1(...)
Signature:
- `r_wait_event_v1(event_type_id, event_filter_query=None, timeout_ms=None, exit_on_error=True) -> Promise<EventWaitResult>`

- `event_type_id` MUST be a GTS type ID string ending with `~` (e.g., `gts.x.core.events.event.v1~vendor.app.some.event.v1~`).
- `event_filter_query` MUST be a valid event filter query string supported by the event broker.
- `timeout_ms` MUST be a non-negative integer.
- The event payload MUST be validated against the event type schema.

`EventWaitResult` SHOULD be represented as a Starlark `struct(...)` so callers can use attribute access.

- `struct(ok = True, value = <Event>)` when the event is received
- `struct(ok = False, error = <FaaS Error envelope>)` when the wait fails

When an error occurs while waiting (including a timeout):
- if `exit_on_error=true`, the runtime MUST call `r_exit(...)` with an error envelope
- if `exit_on_error=false`, the promise MUST resolve with `struct(ok = False, error = ...)`

When `timeout_ms` is provided and the timeout occurs, the error envelope id MUST be `gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.timeout.v1~`.

Waiting on an event is implemented by:
- the runtime suspending the invocation
- registering an event subscription
- resuming execution when the event arrives or timeout occurs

### Snapshotting

#### r_snapshot_v1(...)
Signature:
- `r_snapshot_v1(label=None) -> None`

Calling `r_snapshot_v1()` requests the runtime to capture a durable snapshot of execution state.
The snapshot includes:
- call stack / instruction pointer
- relevant variable bindings
- workflow runtime metadata needed for resume

The runtime may automatically take snapshots at safe points, but `r_snapshot_v1()` allows user code to request one explicitly.

## Workflow orchestration hooks
For workflows, the runtime is responsible for orchestrating typical workflow processing:
- identifying steps
- tracking step status and retries
- scheduling retries
- registering compensation actions
- checkpointing and suspend/resume via snapshots
- event subscription and event-driven continuation

To support this, the runtime provides workflow helpers.

### Workflow helpers

#### r_step_v1(name, fn, compensate_fn=None)
Registers a step function, optional compensation function, and returns a callable wrapper.

If `compensate_fn` is provided, it MUST accept the step output as its second argument:
- `fn(ctx, step_input) -> <step output>`
- `compensate_fn(ctx, step_input, step_output) -> None`

The runtime automatically passes:
- `step_input` to the step function when executing the step and as the first argument to the compensation function
- `step_output` to the compensation function when invoking compensation

This enables the main action to provide context to the compensation action by returning it as part of the step output (e.g., a created object ID).

#### r_run_step_v1(step_name, fn, input, exit_on_error=True)
Executes a step with runtime tracking, retry policy, and status updates.

`exit_on_error=true` means the runtime calls `r_exit(...)` if the step fails.
`exit_on_error=false` means the function returns a result object and the Starlark code is responsible for handling `ok=false`.

The `r_run_step_v1(...)` function returns a result object:
- `struct(ok = True, value = <step output>)` on success
- `struct(ok = False, error = <FaaS Error envelope>)` on failure

The exact internal representation is runtime-defined; the Starlark surface remains stable.

Compensation invocation:
- When a workflow terminates unsuccessfully via `r_exit(...)` (or a runtime abort that maps to a workflow failure), the runtime MUST execute registered compensation actions for all previously completed steps.
- Compensations MUST be executed in reverse step completion order.
- Compensation failures MUST NOT cause additional compensations to be skipped; they MUST be recorded and surfaced via workflow status and logs.

## Examples

### Function example #1: two upstream calls and composite output

#### Function definition (GTS schema)
```json
{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "$id": "gts://gts.x.core.faas.func.v1~vendor.app.example.compose_customer_profile.v1~",
  "allOf": [
    {"$ref": "gts://gts.x.core.faas.func.v1~"}
  ],
  "type": "object",
  "properties": {
    "id": {"const": "gts.x.core.faas.func.v1~vendor.app.example.compose_customer_profile.v1~"},
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
          "customer": {"type": "object", "additionalProperties": true},
          "orders": {"type": "array", "items": {"type": "object", "additionalProperties": true}}
        },
        "required": ["customer", "orders"]
      }
    },
    "traits": {
      "type": "object",
      "properties": {
        "runtime": {"const": "starlark"},
        "invocation": {
          "type": "object",
          "properties": {"supported": {"const": ["sync", "async"]}, "default": {"const": "sync"}},
          "required": ["supported"]
        },
        "caching": {
          "type": "object",
          "properties": {"max_age_seconds": {"const": 0}},
          "required": ["max_age_seconds"]
        }
      },
      "required": ["runtime"]
    },
    "implementation": {
      "type": "object",
      "properties": {
        "code": {
          "type": "object",
          "properties": {
            "language": {"const": "starlark"},
            "source": {"const": ""}
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

#### Starlark code
```python
# main() is the required entrypoint.

def main(ctx, input):
    customer_id = input.customer_id

    p_customer = r_http_get_v1(
        "https://example.crm/api/customers/" + customer_id,
        timeout_ms = 2000,
        retry = {"max_attempts": 3, "backoff_ms": 200},
        exit_on_http_error = True,
    )

    p_orders = r_http_get_v1(
        "https://example.orders/api/orders?customer_id=" + customer_id,
        timeout_ms = 2000,
        retry = {"max_attempts": 3, "backoff_ms": 200},
        exit_on_http_error = True,
    )

    customer_result = r_await(p_customer)
    orders_result = r_await(p_orders)

    customer_resp = customer_result.response
    orders_resp = orders_result.response

    orders_body = orders_resp.body
    orders_items = orders_body["items"] if "items" in orders_body else []

    return {
        "customer": customer_resp.body,
        "orders": orders_items,
    }
```

### Function example #2: handle upstream HTTP error/timeout

This example demonstrates disabling automatic exit and returning a structured result.

```python

def main(ctx, input):
    customer_id = input.customer_id

    p_customer = r_http_get_v1(
        "https://example.crm/api/customers/" + customer_id,
        timeout_ms = 500,
        retry = {"max_attempts": 1, "backoff_ms": 0},
        exit_on_http_error = False,
    )

    customer_result = r_await(p_customer)

    if not customer_result.ok:
        return {
            "customer": {"id": customer_id, "status": "unavailable"},
            "orders": [],
        }

    customer_resp = customer_result.response

    if customer_resp.status_code >= 400:
        return {
            "customer": {"id": customer_id, "status": "unavailable"},
            "orders": [],
        }

    return {
        "customer": customer_resp.body,
        "orders": [],
    }
```

### Example #3: workflow with steps, event waiting, snapshots, and compensation

#### Workflow definition (high-level)
Workflows use the same entrypoint schema model, but are executed with durable semantics.

#### Starlark workflow code
```python

def step_reserve_inventory(ctx, input):
    p = r_http_post_v1(
        "https://example.inventory/api/reservations",
        body = {"sku": input.sku, "qty": input.qty},
        timeout_ms = 2000,
        retry = {"max_attempts": 3, "backoff_ms": 200},
        exit_on_http_error = True,
    )
    resp = r_await(p).response
    reservation_id = resp.body.get("reservation_id")
    return struct(reservation_id = reservation_id)


def compensate_release_inventory(ctx, step_input, step_output):
    reservation = step_output
    p = r_http_post_v1(
        "https://example.inventory/api/reservations:release",
        body = {"reservation_id": reservation.reservation_id},
        timeout_ms = 2000,
        retry = {"max_attempts": 3, "backoff_ms": 200},
        exit_on_http_error = False,
    )
    release_result = r_await(p)
    if not release_result.ok:
        return None
    return None


def main(ctx, input):
    reserve = r_step_v1("reserve_inventory", step_reserve_inventory, compensate_release_inventory)

    reservation_result = r_run_step_v1("reserve_inventory", reserve, input) # exists on error
    reservation = reservation_result.value

    r_snapshot_v1("after_reserve") # make a snapshot before waiting for approval

    approval_result = r_await(
        r_wait_event_v1(
            "gts.x.core.events.event.v1~vendor.app.orders.approved.v1~",
            event_filter_query = "payload.reservation_id = " + reservation.reservation_id,
            timeout_ms = 86400000,
            exit_on_error = False, # handle error explicitly below
        )
    )
    if not approval_result.ok:
        if approval_result.error.id == "gts.x.core.faas.err.v1~x.core._.runtime.v1~x.core._.timeout.v1~":
            approval_event = None
        else:
            r_exit(approval_result.error) # this triggers compensation actions for all completed steps
    else:
        approval_event = approval_result.value

    return {
        "reservation": reservation,
        "approval": approval_event,
    }
```
