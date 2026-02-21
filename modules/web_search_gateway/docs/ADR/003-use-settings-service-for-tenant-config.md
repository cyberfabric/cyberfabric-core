# ADR-003: Settings Service for Tenant Provider Configuration
**Date:** 2026-02-02

**Status:** Accepted

**ID:** wsg-adr-003-settings-service

## Context and Problem Statement
Different tenants may require different search provider configurations (enabled providers, priorities, rate limits). Where should per-tenant configuration be stored and managed?

## Decision Drivers
- Multi-tenancy: Each tenant may have different provider preferences
- Dynamic configuration: Changes without gateway redeployment
- Consistency: Single source of truth for tenant settings
- Platform alignment: Use existing platform services

## Considered Options
1. Gateway Config: Store tenant settings in gateway configuration files
2. Database: Gateway manages its own tenant settings table
3. Settings Service: Delegate to platform Settings Service

## Decision Outcome
Chosen option: "Settings Service", because it provides a consistent, platform-wide approach to tenant configuration management.

## Implementation
- Tenant provider config stored with key: `tenant:{tenant_id}:web_search.providers`
- Schema defined in `settings.v1.json` (TenantWebSearchConfig)
- Gateway queries Settings Service at request time
- Admin API for CRUD operations on tenant provider config
- Default config used when no tenant-specific config exists

## Consequences
- Good, because consistent with platform patterns for tenant configuration
- Good, because dynamic updates without gateway redeployment
- Good, because Settings Service provides caching and change notifications
- Bad, because adds Settings Service as a runtime dependency
- Mitigation: Settings Service responses are cached; fallback to defaults on failure
