# ADR-001: Stateless Gateway Design
**Date:** 2026-02-02

**Status:** Accepted

**ID:** wsg-adr-001-stateless-gateway

## Context and Problem Statement
Web Search Gateway needs to handle high request volume while remaining scalable. Should the gateway maintain quota/usage state internally or delegate to external services?

## Decision Drivers
- Horizontal scalability without state synchronization
- Simplified deployment and failover
- Separation of concerns (search vs billing)
- Durability requirements for quota tracking

## Considered Options
1. Stateful: Gateway maintains quota counters internally (DB-backed)
2. Stateless: Gateway delegates quota checks to Billing Service

## Decision Outcome
Chosen option: "Stateless", because it enables horizontal scaling without coordination overhead and ensures quota durability through a dedicated Billing Service.

## Implementation
- Rate limiting: Enforced via Redis (token bucket, ephemeral)
- Quota management: Delegated to Billing Service (persistent storage)
- Gateway instances are interchangeable and can scale independently

## Consequences
- Good, because instances are interchangeable and can scale independently
- Good, because no quota state synchronization between gateway nodes
- Good, because Billing Service provides durable, auditable quota tracking
- Bad, because adds latency for Billing Service calls on each request
