# PRD — CredStore

<!--
=============================================================================
PRODUCT REQUIREMENTS DOCUMENT (PRD)
=============================================================================
PURPOSE: Define WHAT the system must do and WHY — business requirements,
functional capabilities, and quality attributes.

SCOPE:
  ✓ Business goals and success criteria
  ✓ Actors (users, systems) that interact with this module
  ✓ Functional requirements (WHAT, not HOW)
  ✓ Non-functional requirements (quality attributes, SLOs)
  ✓ Scope boundaries (in/out of scope)
  ✓ Assumptions, dependencies, risks

NOT IN THIS DOCUMENT (see other templates):
  ✗ Stakeholder needs (managed at project/task level by steering committee)
  ✗ Technical architecture, design decisions → DESIGN.md
  ✗ Why a specific technical approach was chosen → ADR/
  ✗ Detailed implementation flows, algorithms → features/

STANDARDS ALIGNMENT:
  - IEEE 830 / ISO/IEC/IEEE 29148:2018 (requirements specification)
  - IEEE 1233 (system requirements)
  - ISO/IEC 15288 / 12207 (requirements definition)

REQUIREMENT LANGUAGE:
  - Use "MUST" or "SHALL" for mandatory requirements (implicit default)
  - Do not use "SHOULD" or "MAY" — use priority p2/p3 instead
  - Be specific and clear; no fluff, bloat, duplication, or emoji
=============================================================================
-->

## 1. Overview

### 1.1 Purpose

CredStore provides per-tenant secret storage and retrieval for the platform. It abstracts backend differences behind a unified API, enabling platform modules to store and access credentials without coupling to a specific storage technology.

### 1.2 Background / Problem Statement

Platform modules — most notably the Outbound API Gateway (OAGW) — need access to secrets (API keys, tokens, credentials) for making upstream API calls on behalf of tenants. These secrets must be stored securely, scoped per tenant, and accessible only to authorized consumers.

Standard credential stores provide per-tenant isolation but do not support hierarchical multi-tenant sharing. In the platform's business model, parent tenants (partners) share API credentials with child tenants (customers). For example, a partner with an OpenAI API key and quota allows their customers to make requests through OAGW using the partner's key — without the customer ever seeing the actual secret value. This requires a hierarchical resolution model: when a customer requests a secret, the system walks up the tenant tree to find a shared secret from an ancestor.

Additionally, the platform runs in multiple environments: Kubernetes (where an external credential store like VendorA Credstore is available) and desktop/VM (where OS-level protected storage is appropriate). A plugin-based architecture allows runtime selection of the appropriate backend without changing consumer code.

### 1.3 Goals (Business Outcomes)

- Enable OAGW to retrieve tenant credentials for upstream API calls without exposing secret values to end users
- Support hierarchical credential sharing so partners can share API access with customers
- Decouple platform modules from specific credential storage backends
- Enforce least-privilege access: read vs write authorization, service-to-service vs tenant self-service

### 1.4 Glossary

| Term | Definition |
|------|------------|
| Secret | A key-value pair where the value is sensitive (API key, token, password) |
| Secret reference | A human-readable key identifying a secret within a tenant's namespace (e.g., `partner-openai-key`). **Format**: alphanumeric characters, hyphens, and underscores only (`[a-zA-Z0-9_-]+`). Max length: 255 characters. Colons are prohibited to prevent ExternalID collisions. |
| Sharing mode | Controls secret access scope: `private` (owner only), `tenant` (all users in tenant), or `shared` (tenant + descendants) |
| Owner | The specific actor (identified by `subject_id` from SecurityContext) that created the secret |
| Hierarchical resolution | Lookup algorithm that walks from child to parent to root tenant, returning the first matching accessible secret |
| Secret shadowing | When a child tenant creates a secret with the same reference as a parent's shared secret, the child's own secret takes precedence |
| SecurityCtx | Request security context containing the authenticated tenant ID, subject ID, and permissions |

## 2. Actors

### 2.1 Human Actors

#### Tenant Admin

**ID**: `cpt-cf-credstore-actor-tenant-admin`

<!-- cpt-cf-id-content -->
**Role**: Authenticated user managing secrets for their tenant. Creates, updates, and deletes secrets. Configures sharing mode to control descendant access.
**Needs**: CRUD operations on secrets within their own tenant namespace. Ability to share secrets with descendants or keep them private.
<!-- cpt-cf-id-content -->

### 2.2 System Actors

#### Outbound API Gateway (OAGW)

**ID**: `cpt-cf-credstore-actor-oagw`

<!-- cpt-cf-id-content -->
**Role**: Service that proxies outbound API calls to external services. Retrieves secrets on behalf of tenants using service-to-service authentication with explicit tenant_id. Primary consumer of hierarchical secret resolution.
<!-- cpt-cf-id-content -->

#### Platform Module

**ID**: `cpt-cf-credstore-actor-platform-module`

<!-- cpt-cf-id-content -->
**Role**: Any internal module consuming secrets via ClientHub in-process API. Reads or writes secrets using the calling tenant's SecurityCtx.
<!-- cpt-cf-id-content -->

#### CredStore Backend

**ID**: `cpt-cf-credstore-actor-backend`

<!-- cpt-cf-id-content -->
**Role**: External storage system that persists encrypted secrets. Examples: VendorA Credstore (Go service with REST API), OS-protected storage (macOS Keychain, Windows DPAPI). Accessed exclusively through plugins.
<!-- cpt-cf-id-content -->

## 3. Operational Concept & Environment

> **Note**: Project-wide runtime, OS, architecture, lifecycle policy, and integration patterns defined in root PRD. Document only module-specific deviations here.

### 3.1 Module-Specific Environment Constraints

- VendorA Credstore plugin requires network access to the Credstore Go service and valid OAuth2 client credentials
- OS-protected storage plugin requires platform-specific keychain/credential APIs (macOS Keychain, Windows DPAPI)
- Only one storage plugin is active per deployment (selected by configuration)

## 4. Scope

### 4.1 In Scope

- Store, retrieve, and delete per-tenant secrets
- Sharing modes: private (owner-only), tenant (tenant-wide, default), and shared (hierarchical)
- Owner-based access control for private secrets (using `subject_id` from SecurityContext)
- Hierarchical secret resolution with walk-up through tenant ancestry
- Secret shadowing (child overrides parent)
- Service-to-service retrieval with explicit tenant_id (for OAGW)
- Gateway + Plugin architecture with runtime backend selection
- VendorA Credstore REST plugin (P1)
- OS-protected storage plugin (P2)
- Module-level authorization enforcement (read vs write)

### 4.2 Out of Scope

- Full Credstore RAML API parity (only subset needed)
- Encryption key management (delegated to backend)
- Automatic secret rotation or expiration
- Secret versioning or history
- Cross-tenant secret transfer (secrets cannot change ownership)
- Unauthenticated or untrusted client access (all access requires platform authentication via SecurityCtx)
- Secret listing or search operations (only retrieval by known reference)
- Granular access control lists (ACLs) for sharing with specific tenant subsets. Current design supports only hierarchical sharing modes (`private`, `tenant`, `shared`). Custom ACLs (e.g., "share with tenants A, B, C only" or "share outside hierarchy") are not supported in this version.

## 5. Functional Requirements

### 5.1 P1 — Core Operations

#### Store Secret

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-put-secret`

<!-- cpt-cf-id-content -->
The system **MUST** allow a tenant to store a secret with a reference (key), a value, and a sharing mode. For `tenant` and `shared` modes: if a secret with the same reference already exists for that tenant, the value and sharing mode are updated. For `private` mode: each owner (identified by `subject_id`) can store their own secret under the same reference — multiple private secrets with the same reference can coexist within one tenant (one per owner). A private secret and a tenant/shared secret with the same reference can also coexist.

**Rationale**: Core capability — tenants need to manage their own API credentials.
**Actors**: `cpt-cf-credstore-actor-tenant-admin`, `cpt-cf-credstore-actor-platform-module`
<!-- cpt-cf-id-content -->

#### Retrieve Own Secret

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-get-secret`

<!-- cpt-cf-id-content -->
The system **MUST** allow a tenant to retrieve the decrypted value of their own secret by reference. Returns the secret value or not-found if no secret with that reference exists for the tenant.

**Rationale**: Tenants need to verify or use their own stored credentials.
**Actors**: `cpt-cf-credstore-actor-tenant-admin`, `cpt-cf-credstore-actor-platform-module`
<!-- cpt-cf-id-content -->

#### Delete Secret

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-delete-secret`

<!-- cpt-cf-id-content -->
The system **MUST** allow a tenant to delete their own secret by reference. Descendants using a shared secret lose access immediately upon deletion.

**Rationale**: Tenants must be able to revoke credentials.
**Actors**: `cpt-cf-credstore-actor-tenant-admin`, `cpt-cf-credstore-actor-platform-module`
<!-- cpt-cf-id-content -->

#### Tenant Scoping

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-tenant-scoping`

<!-- cpt-cf-id-content -->
The system **MUST** derive the owning tenant from the request SecurityCtx for all CRUD operations. Tenants MUST NOT create, update, or delete secrets belonging to other tenants.

**Rationale**: Prevents cross-tenant data manipulation.
**Actors**: `cpt-cf-credstore-actor-tenant-admin`, `cpt-cf-credstore-actor-platform-module`
<!-- cpt-cf-id-content -->

#### Secret Reference Validation

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-secretref-validation`

<!-- cpt-cf-id-content -->
The system **MUST** validate SecretRef format: alphanumeric characters, hyphens, and underscores only (`[a-zA-Z0-9_-]+`). Max length: 255 characters. Colons and other special characters are prohibited. Invalid references are rejected with a validation error.

**Rationale**: Prevents ExternalID collisions in the deterministic mapping (e.g., `base64url("{tenant_id}:{key}")` for tenant/shared, `base64url("{tenant_id}:{key}:p:{owner_id}")` for private). Colons in SecretRef could cause different tenant/key pairs to map to the same ExternalID.
**Actors**: `cpt-cf-credstore-actor-tenant-admin`, `cpt-cf-credstore-actor-platform-module`
<!-- cpt-cf-id-content -->

### 5.2 P1 — Hierarchical Sharing

#### Sharing Modes

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-sharing-modes`

<!-- cpt-cf-id-content -->
Each secret **MUST** have a sharing mode: `private`, `tenant` (default), or `shared`.
- `private`: accessible only to the owner (the specific actor identified by `subject_id` from SecurityContext that created the secret)
- `tenant`: accessible to all users and services within the owning tenant
- `shared`: accessible to all users in the owning tenant and all descendant tenants in the hierarchy

**Rationale**: Partners need flexible credential sharing. Personal API keys and sensitive credentials should be owner-only (`private`). Team credentials should be tenant-wide (`tenant`). Platform-level credentials for customer access should be hierarchical (`shared`).
**Actors**: `cpt-cf-credstore-actor-tenant-admin`
<!-- cpt-cf-id-content -->

#### Hierarchical Secret Resolution

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-hierarchical-resolve`

<!-- cpt-cf-id-content -->
The system **MUST** support hierarchical secret resolution: given a secret reference and a tenant_id, the system walks from the specified tenant up through its ancestors (parent, grandparent, ... root), returning the first secret that matches the reference and is accessible (owned by the tenant, or shared by an ancestor). If no accessible secret is found, the system returns not-found.

**Hierarchical Direction**: Resolution is **upward-only** (child → parent → root). A tenant can access ancestor secrets marked as `shared`, but parent tenants **cannot** access child tenant secrets (even if marked as `shared`). This enforces least privilege and enables shadowing.

**Implementation Note**: This walk-up algorithm and sharing mode enforcement are implemented in the Gateway module (credstore). The Gateway queries the tenant hierarchy via `tenant_resolver`, then at each level performs a two-phase lookup: first the Plugin's `get` with the caller's `owner_id` (private secret), then `get` without `owner_id` (tenant/shared secret). The Plugin and Backend provide simple per-tenant key-value storage without hierarchical logic.

**Rationale**: Enables the core business use case — OAGW retrieves a partner's shared API key when making calls on behalf of a customer.
**Actors**: `cpt-cf-credstore-actor-oagw`
<!-- cpt-cf-id-content -->

#### Secret Shadowing

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-secret-shadowing`

<!-- cpt-cf-id-content -->
When a tenant owns a secret with the same reference as an ancestor's shared secret, and that secret is **accessible** to the requester, the tenant's own secret **MUST** take precedence during hierarchical resolution. The ancestor's secret is never checked in this case.

If the tenant owns a secret with the same reference but it is **inaccessible** to the requester (e.g., `private` mode with owner mismatch, or insufficient permissions), the hierarchical resolution **MUST** continue to check ancestors according to the walk-up algorithm.

**Rationale**: Allows customers to override partner defaults with their own credentials while maintaining hierarchical fallback when the tenant's secret is not accessible to the requester.
**Actors**: `cpt-cf-credstore-actor-oagw`
<!-- cpt-cf-id-content -->

#### Service-to-Service Retrieval

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-service-retrieve`

<!-- cpt-cf-id-content -->
The system **MUST** provide a retrieval operation that accepts an explicit tenant_id parameter (not derived from SecurityCtx). This operation is restricted to authorized service accounts (e.g., OAGW). The response **MUST** include the decrypted secret value.

**Implementation Note**: OAGW is a ModKit module that uses the standard CredStore SDK client (not a separate integration path). For service-to-service retrieval with explicit tenant_id, OAGW constructs a SecurityCtx with the target tenant_id and calls the Gateway's standard get operation. Hierarchical resolution is implemented in the Gateway module: the Gateway uses `tenant_resolver` to walk up the tenant hierarchy, performing a two-phase Plugin lookup at each level (private for caller, then tenant/shared) until a matching accessible secret is found.

**Rationale**: OAGW operates as a service account and needs to retrieve secrets on behalf of arbitrary tenants with hierarchical resolution.
**Actors**: `cpt-cf-credstore-actor-oagw`
<!-- cpt-cf-id-content -->

### 5.3 P1 — Authorization

#### Read Authorization

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-authz-read`

<!-- cpt-cf-id-content -->
The system **MUST** require `Secrets:Read` permission for get and resolve operations.

**Rationale**: Least-privilege access control.
**Actors**: `cpt-cf-credstore-actor-tenant-admin`, `cpt-cf-credstore-actor-oagw`, `cpt-cf-credstore-actor-platform-module`
<!-- cpt-cf-id-content -->

#### Write Authorization

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-authz-write`

<!-- cpt-cf-id-content -->
The system **MUST** require `Secrets:Write` permission for put and delete operations.

**Rationale**: Least-privilege access control.
**Actors**: `cpt-cf-credstore-actor-tenant-admin`, `cpt-cf-credstore-actor-platform-module`
<!-- cpt-cf-id-content -->

#### Gateway-Level Enforcement

- [ ] `p1` - **ID**: `cpt-cf-credstore-fr-authz-gateway`

<!-- cpt-cf-id-content -->
Authorization **MUST** be enforced in the gateway layer, not in plugins. Plugins are storage adapters and MUST NOT implement authorization logic.

**Rationale**: Prevents inconsistent authorization behavior across different backends.
**Actors**: `cpt-cf-credstore-actor-platform-module`
<!-- cpt-cf-id-content -->

### 5.4 P2 — Planned

#### OS Protected Storage Plugin

- [ ] `p2` - **ID**: `cpt-cf-credstore-fr-os-storage`

<!-- cpt-cf-id-content -->
The system **MUST** provide an OS-protected storage plugin for desktop/VM environments using platform-native secure storage (macOS Keychain, Windows DPAPI). This plugin supports basic per-tenant get/put/delete operations. In desktop/single-tenant environments, hierarchical resolution is not applicable (no multi-tenant hierarchy exists), so the Gateway returns only the requesting tenant's own secrets.

**Rationale**: Desktop/VM environments lack access to VendorA Credstore.
**Actors**: `cpt-cf-credstore-actor-platform-module`
<!-- cpt-cf-id-content -->

#### Read-Only / Read-Write Credential Separation

- [ ] `p2` - **ID**: `cpt-cf-credstore-fr-rw-separation`

<!-- cpt-cf-id-content -->
The VendorA Credstore plugin **MUST** support optional separate OAuth2 client credentials for read-only and read-write operations, enabling least-privilege deployment configurations.

**Rationale**: Production environments benefit from separate credentials: RO for get/resolve, RW for put/delete.
**Actors**: `cpt-cf-credstore-actor-backend`
<!-- cpt-cf-id-content -->

## 6. Non-Functional Requirements

### 6.1 Module-Specific NFRs

#### Secret Value Confidentiality

- [ ] `p1` - **ID**: `cpt-cf-credstore-nfr-confidentiality`

<!-- cpt-cf-id-content -->
Secret values **MUST NOT** appear in logs, error messages, or debug output at any level (gateway, plugin, transport). Secret values **MUST** be encrypted at rest in the backend storage.

**Threshold**: Zero plaintext secret values in any log output
**Rationale**: Secrets are the most sensitive data in the platform. Leaking them through logs or error messages would be a critical security incident.
**Architecture Allocation**: See DESIGN.md for implementation approach
<!-- cpt-cf-id-content -->

## 7. Public Library Interfaces

### 7.1 Public API Surface

#### CredStoreClientV1

- [ ] `p1` - **ID**: `cpt-cf-credstore-interface-client`

<!-- cpt-cf-id-content -->
**Type**: Rust trait (async)
**Stability**: stable
**Description**: Public API for platform modules to store, retrieve, and delete secrets. Registered in ClientHub without scope. Operations: `get`, `put`, `delete`. Hierarchical secret resolution is implemented in the Gateway module and accessed through standard `get` operations.
**Breaking Change Policy**: Major version bump required
<!-- cpt-cf-id-content -->

#### CredStorePluginClientV1

- [ ] `p1` - **ID**: `cpt-cf-credstore-interface-plugin-client`

<!-- cpt-cf-id-content -->
**Type**: Rust trait (async)
**Stability**: unstable
**Description**: Plugin SPI for backend implementations. Registered in ClientHub with GTS instance scope. Operations: `get` (takes optional `owner_id` to distinguish private vs tenant/shared lookup; returns SecretMetadata), `put` (stores value with metadata; ExternalID derived from sharing mode), `delete` (takes optional `owner_id`). The Plugin must return metadata so the Gateway can enforce sharing mode access control rules.
**Breaking Change Policy**: Minor version bump (unstable API)
<!-- cpt-cf-id-content -->

### 7.2 External Integration Contracts

#### REST API

- [ ] `p1` - **ID**: `cpt-cf-credstore-contract-rest-api`

<!-- cpt-cf-id-content -->
**Direction**: provided by library
**Protocol/Format**: HTTP/REST, JSON
**Compatibility**: Versioned URL path (`/api/credstore/v1/...`), backward-compatible within major version
<!-- cpt-cf-id-content -->

#### VendorA Credstore REST

- [ ] `p1` - **ID**: `cpt-cf-credstore-contract-vendor_a-rest`

<!-- cpt-cf-id-content -->
**Direction**: required from client (outbound to VendorA Credstore)
**Protocol/Format**: HTTP/REST, JSON. OAuth2 client credentials for authentication. Credstore RAML API subset.
**Compatibility**: Plugin adapts to Credstore API version. Breaking Credstore API changes require plugin update.
<!-- cpt-cf-id-content -->

## 8. Use Cases

#### UC-001: Partner Creates Shared Secret

- [ ] `p1` - **ID**: `cpt-cf-credstore-usecase-create-shared`

<!-- cpt-cf-id-content -->
**Actor**: `cpt-cf-credstore-actor-tenant-admin`

**Preconditions**:
- Tenant is authenticated with `Secrets:Write` permission

**Main Flow**:
1. Partner tenant calls put with reference `partner-openai-key`, value `PARTNER_DEMO_KEY_XYZ`, sharing `shared`
2. Gateway verifies `Secrets:Write` authorization
3. Gateway delegates to plugin
4. Plugin stores secret in backend

**Postconditions**:
- Secret is stored and accessible to partner and all descendant tenants

**Alternative Flows**:
- **Secret already exists**: Value and sharing mode are updated
<!-- cpt-cf-id-content -->

#### UC-002: OAGW Retrieves Secret for Customer (Hierarchical Resolution)

- [ ] `p1` - **ID**: `cpt-cf-credstore-usecase-hierarchical-resolve`

<!-- cpt-cf-id-content -->
**Actor**: `cpt-cf-credstore-actor-oagw`

**Preconditions**:
- OAGW has valid service token with `Secrets:Read` permission
- Partner tenant has created a shared secret with reference `partner-openai-key`
- Customer is a descendant of partner in tenant hierarchy

**Main Flow**:
1. OAGW constructs SecurityCtx for `customer-123` tenant and calls Gateway's `get` with reference `partner-openai-key`
2. Gateway verifies service authorization (`Secrets:Read` permission)
3. Gateway extracts tenant_id from SecurityCtx (`customer-123`)
4. Gateway queries `tenant_resolver` to get ancestor chain: `[customer-123, partner-acme, root]`
5. Gateway walks up hierarchy: calls Plugin `get(customer-123, "partner-openai-key")` → not found
6. Gateway tries next ancestor: calls Plugin `get(partner-acme, "partner-openai-key")` → found with `sharing: shared`
7. Gateway checks sharing mode: `shared` allows access by descendants → returns secret value to OAGW
8. OAGW uses the secret value for upstream API call

**Postconditions**:
- OAGW has the decrypted secret. Customer never sees the actual value.
- Hierarchical resolution is transparent to OAGW (handled by Gateway)

**Alternative Flows**:
- **Customer has own secret**: Customer's secret returned (shadowing), parent not checked
- **Secret is private**: Walk-up continues to next ancestor
- **No secret in hierarchy**: Not-found error returned
<!-- cpt-cf-id-content -->

#### UC-003: Customer Overrides Parent Secret (Shadowing)

- [ ] `p1` - **ID**: `cpt-cf-credstore-usecase-shadowing`

<!-- cpt-cf-id-content -->
**Actor**: `cpt-cf-credstore-actor-tenant-admin`

**Preconditions**:
- Partner has shared secret with reference `partner-openai-key`
- Customer is a descendant of partner

**Main Flow**:
1. Customer creates own secret with same reference `partner-openai-key`, value `CUSTOMER_DEMO_KEY_ABC`, sharing `tenant`
2. OAGW calls resolve for `partner-openai-key`, tenant_id `customer-123`
3. System finds customer's own secret first — returns `CUSTOMER_DEMO_KEY_ABC`
4. Partner's secret is never checked

**Postconditions**:
- Customer uses own key. Partner's shared secret remains available to other descendants.

**Alternative Flows**:
- **Customer uses private mode**: If customer creates secret with `sharing: private`, it's accessible only to the customer's user/service that created it (owner-only)
<!-- cpt-cf-id-content -->

#### UC-004: Private Secret Access Denied vs Shadowing with Fallback

- [ ] `p1` - **ID**: `cpt-cf-credstore-usecase-private-denied`

<!-- cpt-cf-id-content -->
**Actor**: `cpt-cf-credstore-actor-oagw`

**Scenario A: Parent's private secret (no shadowing)**

**Preconditions**:
- Partner has secret with reference `internal-admin-key`, sharing `private`, owned by PartnerAdmin
- Customer is a descendant of partner, has no own secret with this reference

**Main Flow**:
1. OAGW calls resolve for `internal-admin-key`, tenant_id `customer-123`
2. Gateway walks up: Customer — phase 1 (private for OAGW): miss. Phase 2 (tenant/shared): miss.
3. Partner — phase 1 (private for OAGW): miss (OAGW is not the owner). Phase 2 (tenant/shared): miss (no tenant/shared secret).
4. Walk-up exhausted — system returns not-found error (404)

**Postconditions**:
- Customer cannot access partner's private secret. OAGW never sees it because the ExternalID doesn't match.

**Scenario B: Another user's private secret with fallback to parent's shared**

**Preconditions**:
- Customer has secret `api-key`, sharing `private`, owner_id: User A
- Partner (parent) has secret `api-key`, sharing `shared`
- User B in customer tenant requests `api-key`

**Main Flow**:
1. User B calls get for `api-key` in customer tenant
2. Gateway phase 1 for customer: looks up private `api-key` for User B → miss (no private secret for User B)
3. Gateway phase 2 for customer: looks up tenant/shared `api-key` → miss (no tenant/shared secret at customer level)
4. Gateway walks up to parent (partner) — phase 2: finds `api-key` with sharing `shared`
5. Shared secret is accessible to descendants — return partner's secret value

**Postconditions**:
- User B accesses the partner's shared secret as fallback. User A's private secret is invisible to User B (stored under a different ExternalID).

**Rationale**: Private secrets are namespaced per-owner via ExternalID. The two-phase lookup (private for caller → tenant/shared) ensures that inaccessible private secrets don't block fallback to ancestor shared secrets.
<!-- cpt-cf-id-content -->

#### UC-005: Tenant CRUD Own Secrets

- [ ] `p1` - **ID**: `cpt-cf-credstore-usecase-crud`

<!-- cpt-cf-id-content -->
**Actor**: `cpt-cf-credstore-actor-tenant-admin`

**Preconditions**:
- Tenant is authenticated with appropriate permissions

**Main Flow**:
1. Tenant creates secret: put(reference, value, sharing) — `sharing` can be `private`, `tenant` (default), or `shared`
2. Tenant reads secret: get(reference) — returns value (subject to sharing mode access control)
3. Tenant updates secret: put(reference, new_value, new_sharing) — overwrites value and/or sharing mode
4. Tenant deletes secret: delete(reference) — removed

**Postconditions**:
- Secret lifecycle managed. Descendants of shared secrets lose access on delete.

**Alternative Flows**:
- **Get non-existent secret**: Not-found error
- **Delete non-existent secret**: Not-found error (Gateway returns 404 regardless of plugin behavior)
- **Get private secret created by different owner**: Not-found error (owner mismatch returns 404 to prevent enumeration)
<!-- cpt-cf-id-content -->

#### UC-006: Owner-Only Private Secret Access Control

- [ ] `p1` - **ID**: `cpt-cf-credstore-usecase-private-owner-only`

<!-- cpt-cf-id-content -->
**Actor**: `cpt-cf-credstore-actor-tenant-admin`

**Preconditions**:
- User A and User B are both authenticated users in the same tenant
- Both have `Secrets:Write` permission

**Main Flow**:
1. User A calls put with reference `my-personal-api-key`, value `user-a-demo-key`, sharing `private`
2. Gateway stores secret with `owner_id = UserA.subject_id` (private ExternalID includes owner_id)
3. User B calls put with same reference `my-personal-api-key`, value `user-b-demo-key`, sharing `private`
4. Gateway stores a separate secret with `owner_id = UserB.subject_id` (different ExternalID — no conflict)
5. User A calls get for `my-personal-api-key` — Gateway looks up private secret for UserA → returns `user-a-demo-key`
6. User B calls get for `my-personal-api-key` — Gateway looks up private secret for UserB → returns `user-b-demo-key`

**Postconditions**:
- Each user has their own independent private secret under the same reference
- Users cannot see or access each other's private secrets

**Alternative Flows**:
- **Owner updates secret**: User A can update value or change sharing mode to `tenant` or `shared`
- **Non-owner attempts delete**: User B cannot delete User A's private secret (owner-only authorization)
- **No private secret for caller**: If User C (who never created a private secret) calls get, Gateway falls back to tenant/shared secret or returns not-found (404)

**Rationale**: Personal API keys and sensitive user-specific credentials should not be shared tenant-wide. The `private` sharing mode enables users to store secrets under common names (e.g., `github-token`) without namespace conflicts between owners.
<!-- cpt-cf-id-content -->

## 9. Acceptance Criteria

- [ ] Tenant can store, retrieve, and delete secrets via both ClientHub and REST API
- [ ] Private secrets are accessible only to the owner (subject_id match required)
- [ ] Multiple users in the same tenant can each store a private secret under the same reference without conflict
- [ ] Tenant secrets are accessible to all users/services within the owning tenant
- [ ] Shared secrets are accessible to the owning tenant and descendant tenants via hierarchical resolution
- [ ] Secret shadowing works: child's own secret takes precedence over parent's
- [ ] OAGW can retrieve secrets on behalf of any tenant using service-to-service authentication
- [ ] Authorization is enforced at the gateway level for all operations
- [ ] Secret values never appear in log output
- [ ] Owner ID is captured from SecurityContext.subject_id() for all secret creation operations

## 10. Dependencies

| Dependency | Description | Criticality |
|------------|-------------|-------------|
| VendorA Credstore | External Go service for secret persistence (Kubernetes environments) | `p1` |
| OAGW | Primary consumer of hierarchical secret retrieval (uses CredStore SDK client) | `p1` |
| OAuth/token provider | Shared component for Credstore REST authentication tokens | `p1` |
| `tenant_resolver` | Provides tenant hierarchy information (used by Gateway module for hierarchical resolution walk-up) | `p1` |
| `types_registry` | GTS-based plugin registration and discovery | `p1` |

## 11. Assumptions

- Hierarchical secret resolution (walk-up algorithm and sharing mode enforcement) is implemented in the Gateway module (credstore), not in the Backend
- Plugins and Backends provide simple per-tenant key-value storage without hierarchical logic
- OAGW is a ModKit module that uses the standard CredStore SDK client (all access flows through Gateway→Plugin→Backend)
- Gateway provides tenant-scoped CRUD operations, hierarchical resolution, and routes requests to the active storage plugin
- Tenant hierarchy is managed externally and accessible via `tenant_resolver` (used by Gateway for hierarchical walk-up)
- `sharing` field is stored in VendorA Credstore schema as metadata (used by Gateway for access control decisions)
- One storage plugin is active per deployment

## 12. Risks

| Risk | Impact | Mitigation |
|------|--------|------------|
| Credstore API changes break plugin | Plugin stops working, secrets inaccessible | Pin Credstore API version, integration tests against Credstore |
| Secret values leaked through logs | Critical security incident | NFR enforcement, code review, log scrubbing |
| Hierarchy walk-up performance at deep nesting | Increased latency for resolve operations | Gateway implements efficient walk-up with early termination; cache tenant hierarchy queries; monitor resolution depth |
| ExternalID encoding collision | Wrong secret returned | Deterministic encoding with base64url; comprehensive test coverage |

## 13. Open Questions

- **P2/Future - Human vs Service Access**: Should human users (tenant admins via UI) be restricted from retrieving raw secret values for inherited shared secrets, while service accounts (OAGW) can retrieve them? Constructor pattern: tenant admins can see metadata (reference, sharing mode, owner) but cannot get the decrypted value for shared secrets. This would require distinguishing human vs service authentication in SecurityCtx and different authorization rules.
- **P2/Future - Audit Trails**: All credential operations (create, read, update, delete, access) should leave audit trails with timestamps, actor, tenant, and outcome. Audit entries must never contain plaintext secret values. Audit logs should be tamper-evident and stored securely.
- **P2/Future - Schema Validation**: Should Constructor-managed secrets support JSON schema validation (using GTS)? On create/update, validate secret structure against registered schema. Useful for complex credentials (SMTP config, OAuth client credentials) where multiple fields must be present and correctly formatted. Schema can also specify raw string values for simple secrets.
- **P2/Future**: Should secret updates support atomic compare-and-swap (CAS) validation? VendorA Credstore has a newer endpoint that validates the current secret value before allowing updates, preventing race conditions. This could be added as an optional parameter to the `put` operation without breaking existing clients.

## 14. Traceability

- **Design**: [DESIGN.md](./DESIGN.md)
- **ADRs**: [ADR/](./ADR/)
- **Features**: [features/](./features/)