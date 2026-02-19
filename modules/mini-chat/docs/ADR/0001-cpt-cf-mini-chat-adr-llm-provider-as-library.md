---
status: accepted
date: 2026-02-12
---
# llm_provider as a Library Crate, Not a Standalone Service

**ID**: `cpt-cf-mini-chat-adr-llm-provider-as-library`

## Context and Problem Statement

The `llm_provider` component builds OpenAI API requests, parses SSE streams, and maps errors. Should it be deployed as a standalone microservice (with its own network endpoint) or embedded as a library crate directly consumed by `chat_service`?

## Decision Drivers

* Cancellation propagation complexity - streaming cancellation must reach the provider's outbound HTTP connection quickly (hard cancel requirement)
* Operational overhead - services require independent deployment, health checks, scaling policies, and monitoring
* Security surface - whether `llm_provider` needs its own authentication or network isolation
* Future multi-consumer demand - whether other modules will need the same LLM abstraction

## Considered Options

* Library crate linked into `chat_service`
* Standalone gRPC/HTTP service with its own process

## Decision Outcome

Chosen option: "Library crate", because `llm_provider` has no independent lifecycle, holds no state, and does not benefit from service isolation. Embedding it avoids an unnecessary network hop in the streaming path.

### Consequences

* Good, because cancellation is simpler - `CancellationToken` propagates in-process without crossing a network boundary, enabling hard cancel within a single `tokio::select!`
* Good, because one fewer service to deploy, monitor, and scale
* Good, because no additional network-facing authentication surface - `llm_provider` inherits the `SecurityContext` from `chat_service` in-process
* Good, because streaming latency is lower - no serialization/deserialization or network overhead between `chat_service` and `llm_provider`
* Bad, because `llm_provider` updates require redeploying `chat_service` (acceptable given they are tightly coupled)
* Bad, because a bug in SSE parsing or provider protocol mapping can impact the entire `chat_service` process (blast radius is larger than an isolated service)
* Bad, because if a second consumer needs LLM access, the library must be extracted into a shared crate (not a service, unless criteria below are met)

### Confirmation

* Code review: `llm_provider` is a Rust crate with `chat_service` as its only dependent
* No `Dockerfile`, no `main.rs`, no health endpoint in the `llm_provider` crate
* Cancellation integration test: verify `CancellationToken` propagates from `chat_service` to `llm_provider`'s HTTP client abort

## Pros and Cons of the Options

### Library crate linked into `chat_service`

`llm_provider` is a Rust library crate. `chat_service` calls it via direct function invocation. No network boundary.

* Good, because zero network overhead on the streaming hot path
* Good, because `CancellationToken` propagates in-process (no RPC cancel semantics needed)
* Good, because no independent deployment artifact or scaling policy
* Good, because no additional network-facing security surface - no ports, no auth, no TLS between services (the security surface remains inside the `chat_service` process)
* Neutral, because tightly couples `llm_provider` release cycle to `chat_service`
* Bad, because cannot scale `llm_provider` independently (irrelevant - it is stateless and CPU-bound only during request parsing)

### Standalone gRPC/HTTP service

`llm_provider` runs as its own process with a gRPC or HTTP API. `chat_service` calls it over the network.

* Good, because can be deployed and scaled independently
* Good, because could serve multiple consumers without shared-crate coupling
* Bad, because adds a network hop to every streaming token (latency + failure mode)
* Bad, because cancellation requires gRPC cancellation semantics or a custom abort protocol
* Bad, because requires its own deployment manifest, health checks, monitoring, and TLS
* Bad, because `llm_provider` holds no state and has no security boundary separate from `chat_service` - the service boundary adds cost without benefit

## Criteria for Future Extraction to a Service

Re-evaluate this decision if any of the following become true:

* Multiple products (not just mini-chat) need the same LLM provider abstraction
* Consumers are written in different languages (shared Rust crate is insufficient)
* Policy enforcement (rate limiting, content filtering) must happen in the provider layer independently of consumers
* `llm_provider` needs its own secret management or credential scope distinct from `chat_service`

## Traceability

- **PRD**: [PRD.md](../PRD.md)
- **DESIGN**: [DESIGN.md](../DESIGN.md)

This decision directly addresses the following requirements or design elements:

* `cpt-cf-mini-chat-component-llm-provider` - Defines `llm_provider` as a library, not a service
* `cpt-cf-mini-chat-nfr-streaming-latency` - Eliminates network hop in streaming path
* `cpt-cf-mini-chat-seq-cancellation` - Simplifies hard cancel via in-process `CancellationToken`
* `cpt-cf-mini-chat-constraint-no-buffering` - No serialization boundary that could introduce buffering
