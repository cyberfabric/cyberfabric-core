# ADR-002: Stateless Credential Management via OAGW
**Date:** 2026-02-02

**Status:** Accepted

**ID:** wsg-adr-002-credential-management

## Context and Problem Statement
Web Search Gateway needs to authenticate with external search providers (Tavily, Serper, etc.). Should the gateway manage API keys directly or delegate credential handling?

## Decision Drivers
- Security: Minimize exposure of sensitive credentials
- Statelessness: Gateway should not store secrets
- Auditability: Centralized credential management
- Rotation: Simplified key rotation without gateway redeployment

## Considered Options
1. Direct: Gateway stores and manages provider API keys in config/secrets
2. OAGW Delegation: Gateway delegates credential injection to Outbound API Gateway

## Decision Outcome
Chosen option: "OAGW Delegation", because it keeps the gateway stateless regarding secrets and centralizes credential management.

## Implementation
- Gateway sends requests to OAGW with provider identifier
- OAGW injects appropriate credentials before forwarding to provider
- Gateway never sees or handles raw API keys
- Credential rotation handled entirely in OAGW without gateway changes

## Consequences
- Good, because gateway code never handles sensitive credentials
- Good, because centralized audit trail for all outbound API calls
- Good, because credential rotation requires no gateway redeployment
- Bad, because adds OAGW as a dependency in the request path
- Mitigation: OAGW is a platform-wide service with high availability
