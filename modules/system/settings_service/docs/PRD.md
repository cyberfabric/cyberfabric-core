# PRD

## A. Vision

**Purpose**: Settings Service is a Hyperspot module providing centralized configuration management for multi-tenant SaaS applications. Built on the Hyperspot modular platform using Rust and ModKit, it enables both internal modules and external services to store, retrieve, and manage settings through a type-safe SDK with automatic REST API generation and comprehensive OpenAPI documentation.

The Settings Service acts as a universal configuration management system that supports multiple domain types (Tenant, User) with hierarchical tenant inheritance, role-based access control, and compliance features. It enables services to define setting types with GTS (Global Type System) schemas and manage configuration values across complex organizational hierarchies while maintaining security, auditability, and data integrity through Hyperspot's secure-by-default architecture.

**Target Users**:

- **Service Developers** - Internal and external developers building services that need configuration management
- **System Administrators** - Operations teams managing settings across tenant hierarchies
- **Compliance Officers** - Teams enforcing configuration policies and audit requirements

**Key Problems Solved**:

- **Distributed Configuration Management**: Eliminates the need for each service to implement its own configuration storage and management logic by providing a centralized, schema-validated settings repository
- **Multi-Tenant Hierarchy Complexity**: Handles complex tenant inheritance patterns with configurable inheritance rules, barrier tenants, and self-service overwrite capabilities across organizational hierarchies
- **Schema Evolution and Validation**: Supports dynamic setting type definition with GTS (Global Type System) schemas, JSON Schema validation, type versioning, and backward compatibility for evolving configuration requirements
- **Compliance and Audit**: Provides compliance mode with setting locks, MFA requirements, audit event generation, and read-only enforcement for regulated configuration data

**Success Criteria**:

- Support 100+ concurrent services storing settings with sub-100ms read latency
- Maintain 99.9% availability for settings retrieval operations
- Process 10,000+ setting value updates per minute across tenant hierarchies
- Achieve zero data loss for setting changes with full audit trail
- Support tenant hierarchies with 10+ levels of inheritance without performance degradation
- Support API rate limits of 1,000 requests per second per tenant with graceful degradation

**Capabilities**:

- Hyperspot module with SDK pattern for type-safe inter-module communication
- Setting type definition with GTS schemas and configurable traits
- Setting value CRUD operations with tenant hierarchy inheritance
- GTS-based type versioning for backward-compatible schema evolution
- Compliance mode with setting locks and read-only enforcement
- Multi-tenant access control with SecurityContext and role-based permissions
- Batch operations for efficient bulk updates and bulk get for multiple tenants
- Trait-based event generation for audit and notification systems
- Domain object association (tenant, user)
- MFA-protected settings for sensitive configurations
- Generic and explicit value resolution with inheritance chains
- Soft deletion with configurable retention periods
- Generic reporting interface for setting values with configurable attribute mappings
- API for creating new setting types with GTS schema definitions
- Automatic OpenAPI documentation via ModKit OperationBuilder
- OData support for pagination, filtering, and field projection
- Secure-by-default database access with automatic tenant and setting type (GTS) scoping

## B. Actors

### Human Actors

#### Service Developer

**ID**: `fdd-settings-service-actor-service-developer`

<!-- fdd-id-content -->
**Role**: Internal and external developers who build and maintain services that consume Settings Service APIs to store and retrieve configuration data. Creates setting types with GTS schemas, integrates settings into service logic, and manages type evolution over time.
<!-- fdd-id-content -->

#### System Administrator

**ID**: `fdd-settings-service-actor-system-administrator`

<!-- fdd-id-content -->
**Role**: Manages settings across tenant hierarchies, updates setting values for tenants, and monitors setting usage. Responsible for operational configuration management and troubleshooting.
<!-- fdd-id-content -->

#### Compliance Officer

**ID**: `fdd-settings-service-actor-compliance-officer`

<!-- fdd-id-content -->
**Role**: Enforces configuration compliance policies by locking critical settings, enabling MFA requirements, and reviewing audit logs. Ensures settings meet regulatory and security requirements.
<!-- fdd-id-content -->

### System Actors

#### Tenant Management Module

**ID**: `fdd-settings-service-actor-tenant-management`

<!-- fdd-id-content -->
**Role**: Provides tenant hierarchy information, user authentication data, and tenant provisioning states. Publishes tenant lifecycle events (create, update, delete) that Settings Service consumes for tenant reconciliation.
<!-- fdd-id-content -->

#### Event Bus Module

**ID**: `fdd-settings-service-actor-event-bus`

<!-- fdd-id-content -->
**Role**: Receives audit and notification events from Settings Service when settings or setting values are modified. Distributes events to subscribers for monitoring, alerting, and compliance tracking. Delivers asynchronous events for tenant and user lifecycle changes.
<!-- fdd-id-content -->

#### GTS Type Registry Module

**ID**: `fdd-settings-service-actor-gts-registry`

<!-- fdd-id-content -->
**Role**: Provides GTS (Global Type System) type definitions and versioning information for setting type schemas. Enables type evolution and backward compatibility through GTS versioning. Reference: <https://github.com/GlobalTypeSystem/gts-spec>
<!-- fdd-id-content -->

#### Domain Object Validation Module

**ID**: `fdd-settings-service-actor-domain-validation`

<!-- fdd-id-content -->
**Role**: Optional module that validates existence of domain objects (users) when settings are created or updated. Provides GET endpoints for domain object retrieval and consumes domain object deletion events.
<!-- fdd-id-content -->

#### Hyperspot ModKit Framework

**ID**: `fdd-settings-service-actor-hyperspot-modkit`

<!-- fdd-id-content -->
**Role**: Provides the foundational framework for module lifecycle management, REST API generation via OperationBuilder, ClientHub for inter-module communication, secure database access with SecureConn, and RFC-9457 Problem error handling. Enables automatic OpenAPI documentation and type-safe module integration.
<!-- fdd-id-content -->

#### Hyperspot API Gateway

**ID**: `fdd-settings-service-actor-api-gateway`

<!-- fdd-id-content -->
**Role**: Owns the Axum router and OpenAPI document, routes HTTP requests to registered module endpoints, serves OpenAPI documentation at `/docs`, and handles CORS configuration. All REST endpoints are registered through the gateway via OperationBuilder.
<!-- fdd-id-content -->

#### Hyperspot Tenant Resolver

**ID**: `fdd-settings-service-actor-tenant-resolver`

<!-- fdd-id-content -->
**Role**: Resolves tenant context from authentication tokens and provides SecurityContext for all module operations. Enables automatic tenant isolation and multi-tenant access control across all Hyperspot modules.
<!-- fdd-id-content -->

## C. Functional Requirements

### Setting Type Definition with GTS Schema and Traits

**ID**: `fdd-settings-service-fr-setting-type-definition`

<!-- fdd-id-content -->
The system MUST support defining setting types with GTS (Global Type System) schemas and configurable traits. Traits include domain_type (TENANT, USER), event configuration (audit, notification), setting options (enable_generic, enable_compliance), and operations (mutable_access_scope, read_only). Setting types are versioned through GTS and support backward compatibility.

**Actors**: `fdd-settings-service-actor-service-developer`

**Entry Point**: API for creating setting types with GTS schema
<!-- fdd-id-content -->

#### Setting Value CRUD Operations

**ID**: `fdd-settings-service-fr-setting-value-crud`

<!-- fdd-id-content -->
The system MUST provide create, read, update, and delete operations for setting values with support for tenant hierarchy inheritance, domain object association, and explicit/inherited value resolution.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-system-administrator`
<!-- fdd-id-content -->

#### Tenant Hierarchy Inheritance and Default Value Resolution

**ID**: `fdd-settings-service-fr-tenant-inheritance`

<!-- fdd-id-content -->
The system MUST resolve setting values through tenant hierarchy inheritance chains, supporting configurable inheritance rules, barrier tenants, and self-service overwrite capabilities. Value resolution follows parent-to-child inheritance with explicit values taking precedence. When inheritance is enabled, the system queries tenant/domain object, then tenant/generic, then traverses parent hierarchy. The system is designed to always return a value: if no explicit or inherited value is found, the system MUST return the default value defined in the setting type's GTS schema. This ensures that read operations never fail due to missing values and applications always receive a valid configuration.

**Actors**: `fdd-settings-service-actor-system-administrator`, `fdd-settings-service-actor-tenant-management`
<!-- fdd-id-content -->

#### Compliance Mode and Setting Locks

**ID**: `fdd-settings-service-fr-compliance-mode`

<!-- fdd-id-content -->
The system MUST support compliance mode where settings can be locked to read-only state for specific tenants and domain objects. Locked settings cannot be modified until unlocked, enforcing configuration compliance policies.

**Actors**: `fdd-settings-service-actor-compliance-officer`, `fdd-settings-service-actor-system-administrator`
<!-- fdd-id-content -->

#### Multi-Tenant Access Control

**ID**: `fdd-settings-service-fr-access-control`

<!-- fdd-id-content -->
The system MUST enforce role-based access control with tenant hierarchy validation, ensuring users can only access and modify settings within their authorized tenant scope. Supports root, subroot, partner, customer, unit, and folder tenant kinds with different permission levels. Tenant users, parent tenant users (if barrier not enforced), and root/subroot users can modify setting values.

**Actors**: `fdd-settings-service-actor-system-administrator`, `fdd-settings-service-actor-tenant-management`
<!-- fdd-id-content -->

#### Batch Operations

**ID**: `fdd-settings-service-fr-batch-operations`

<!-- fdd-id-content -->
The system MUST support batch update operations for setting values across multiple tenants and bulk get operations to fetch values for multiple tenants and multiple setting types simultaneously. Batch operations enable efficient bulk configuration changes with partial success handling: each operation in the batch is processed independently, successful operations are committed, failed operations return detailed error information including the specific tenant/setting that failed and the reason, and the response indicates which operations succeeded and which failed with their respective error details.

**Actors**: `fdd-settings-service-actor-system-administrator`, `fdd-settings-service-actor-service-developer`
<!-- fdd-id-content -->

#### Event Generation and Audit

**ID**: `fdd-settings-service-fr-event-generation`

<!-- fdd-id-content -->
The system MUST generate audit and notification events for setting value changes (create, update, delete) based on event trait configuration in the setting type. Supports SELF, SUBROOT, and NONE event propagation modes. The system MUST also consume domain object deletion events to delete associated settings when configured in the setting type traits.

**Actors**: `fdd-settings-service-actor-event-bus`, `fdd-settings-service-actor-compliance-officer`
<!-- fdd-id-content -->

#### GTS-Based Type Versioning

**ID**: `fdd-settings-service-fr-gts-versioning`

<!-- fdd-id-content -->
The system MUST support GTS (Global Type System) based type versioning enabling backward-compatible type evolution and automatic selection of compatible type versions. Setting types are defined using GTS schemas with version management through the GTS Type Registry.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-gts-registry`
<!-- fdd-id-content -->

#### Domain Object Association

**ID**: `fdd-settings-service-fr-domain-objects`

<!-- fdd-id-content -->
The system MUST associate setting values with domain objects (tenant, user) and support generic values that apply to all domain objects when no explicit value exists. Domain object validation is optional and configurable per setting type through traits.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-domain-validation`
<!-- fdd-id-content -->

#### MFA-Protected Settings

**ID**: `fdd-settings-service-fr-mfa-protection`

<!-- fdd-id-content -->
The system MUST support MFA (Multi-Factor Authentication) requirements for sensitive settings, enforcing that only tenants with MFA enabled can modify settings marked with is_mfa_required option.

**Actors**: `fdd-settings-service-actor-compliance-officer`, `fdd-settings-service-actor-system-administrator`
<!-- fdd-id-content -->

#### Soft Deletion and Retention

**ID**: `fdd-settings-service-fr-soft-deletion`

<!-- fdd-id-content -->
The system MUST support soft deletion of settings and setting values with configurable retention periods, allowing recovery of deleted data within the retention window and permanent cleanup after expiration.

**Actors**: `fdd-settings-service-actor-system-administrator`, `fdd-settings-service-actor-service-developer`
<!-- fdd-id-content -->

#### Tenant Reconciliation

**ID**: `fdd-settings-service-fr-tenant-reconciliation`

<!-- fdd-id-content -->
The system MUST synchronize tenant hierarchy data from Tenant Management Module through event bus and on-demand reconciliation, maintaining consistent tenant paths and provisioning states for access control.

**Actors**: `fdd-settings-service-actor-tenant-management`, `fdd-settings-service-actor-event-bus`
<!-- fdd-id-content -->

#### Setting Type API

**ID**: `fdd-settings-service-fr-setting-type-api`

<!-- fdd-id-content -->
The system MUST provide API endpoints to create new setting types with GTS schema definitions and configurable traits. The API allows developers to define setting types programmatically including domain type, event configuration, setting options, and operation rules.

**Actors**: `fdd-settings-service-actor-service-developer`
<!-- fdd-id-content -->

#### Hyperspot Module Integration

**ID**: `fdd-settings-service-fr-hyperspot-module`

<!-- fdd-id-content -->
The system MUST be implemented as a Hyperspot module following the SDK pattern for type-safe inter-module communication. The module MUST integrate with the platform's module lifecycle management, implement lifecycle hooks for initialization and shutdown, and register REST endpoints with automatic OpenAPI documentation generation.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-hyperspot-modkit`
<!-- fdd-id-content -->

#### Security Context Integration

**ID**: `fdd-settings-service-fr-security-context`

<!-- fdd-id-content -->
The system MUST require security context for all API operations and SDK methods to enable tenant isolation, access control, and audit logging. All database operations MUST use secure database connections with automatic tenant scoping to prevent cross-tenant data access. The system MUST integrate with the platform's tenant resolver for authentication and authorization.

**Actors**: `fdd-settings-service-actor-tenant-resolver`, `fdd-settings-service-actor-hyperspot-modkit`
<!-- fdd-id-content -->

#### ClientHub Inter-Module Communication

**ID**: `fdd-settings-service-fr-clienthub`

<!-- fdd-id-content -->
The system MUST provide a typed SDK interface registered with the platform's inter-module communication hub for in-process communication. Other platform modules MUST access Settings Service through the SDK interface without direct dependencies on implementation details. The SDK MUST be transport-agnostic with no serialization or HTTP-specific types.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-hyperspot-modkit`
<!-- fdd-id-content -->

#### RFC-9457 Error Handling

**ID**: `fdd-settings-service-fr-rfc9457-errors`

<!-- fdd-id-content -->
The system MUST use RFC-9457 Problem format for all REST API errors with standardized error codes, titles, and detail messages. Domain errors MUST be mapped to the Problem format for consistent error responses. All REST handlers MUST support automatic error conversion and OpenAPI error documentation.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-hyperspot-modkit`
<!-- fdd-id-content -->

#### OData Query Support

**ID**: `fdd-settings-service-fr-odata`

<!-- fdd-id-content -->
The system MUST support OData query parameters for list endpoints including $filter for field-based filtering, $orderby for sorting, $select for field projection, $top/$skip for pagination, and cursor-based pagination. The system MUST support compile-time query validation for type safety. Default page size MUST be 50 items with a maximum limit of 200 items per request to prevent resource exhaustion.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-hyperspot-modkit`
<!-- fdd-id-content -->

#### Dynamic Domain Type Registration

**ID**: `fdd-settings-service-fr-dynamic-domain-types`

<!-- fdd-id-content -->
The system MUST support dynamic domain types as GTS entities, allowing new domain types to be registered at runtime without code changes. Each domain type registration MUST provide: (1) domain_id data type specification (e.g., string, uuid), (2) REST API endpoint for validating domain object existence, (3) event type for domain object deletion that Settings Service monitors to automatically remove settings scoped to deleted domain objects. The system MUST support the predefined domain types: TENANT, USER, and allow registration of additional custom domain types through the domain type registry.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-gts-registry`, `fdd-settings-service-actor-event-bus`, `fdd-settings-service-actor-domain-validation`
<!-- fdd-id-content -->

#### Setting Type Discovery

**ID**: `fdd-settings-service-fr-setting-type-discovery`

<!-- fdd-id-content -->
The system MUST provide API endpoints to list and search available setting types with filtering by domain type, name pattern, and trait configuration. The discovery API enables service developers to explore existing setting types before creating new ones and understand available configuration options.

**Actors**: `fdd-settings-service-actor-service-developer`
<!-- fdd-id-content -->

#### Schema Validation Error Handling

**ID**: `fdd-settings-service-fr-schema-validation-errors`

<!-- fdd-id-content -->
The system MUST provide clear, actionable error messages when GTS schema validation fails, including the specific field that failed validation, the expected format or constraint, the actual value provided, and suggestions for correction. Validation errors MUST be returned in RFC-9457 Problem format with detailed diagnostic information.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-system-administrator`
<!-- fdd-id-content -->

#### Idempotent Operations

**ID**: `fdd-settings-service-fr-idempotent-operations`

<!-- fdd-id-content -->
The system MUST ensure that write operations (create, update, delete) are idempotent, allowing clients to safely retry operations without causing duplicate or inconsistent state. Operations MUST use idempotency keys or natural identifiers to detect and handle duplicate requests.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-system-administrator`
<!-- fdd-id-content -->

#### Authentication

**ID**: `fdd-settings-service-fr-authentication`

<!-- fdd-id-content -->
The system MUST authenticate all API requests using bearer tokens (JWT) with tenant context embedded in the token claims. The system MUST validate token signatures, expiration, and required claims before processing requests. Unauthenticated requests MUST be rejected with 401 Unauthorized status.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-system-administrator`, `fdd-settings-service-actor-tenant-resolver`
<!-- fdd-id-content -->

#### Data Deletion

**ID**: `fdd-settings-service-fr-data-deletion`

<!-- fdd-id-content -->
The system MUST support permanent deletion of user data and tenant data upon request to comply with data privacy regulations (right to be forgotten). Permanent deletion MUST remove all setting values, audit logs, and metadata associated with the specified user or tenant, bypassing the soft deletion retention period.

**Actors**: `fdd-settings-service-actor-system-administrator`, `fdd-settings-service-actor-compliance-officer`
<!-- fdd-id-content -->

#### API Response Format

**ID**: `fdd-settings-service-fr-api-response-format`

<!-- fdd-id-content -->
The system MUST provide a consistent API response format for all endpoints with standardized structure including data payload, metadata (pagination, filtering applied), and error details. Success responses MUST include HTTP 2xx status codes with response body, and error responses MUST use RFC-9457 Problem format with appropriate 4xx/5xx status codes.

**Actors**: `fdd-settings-service-actor-service-developer`
<!-- fdd-id-content -->

#### GTS Base Setting Type

**ID**: `fdd-settings-service-fr-gts-base-setting-type`

<!-- fdd-id-content -->
The system MUST define a base GTS type `gts.x.sm.setting.v1.0~` that serves as the abstract foundation for all concrete setting types. The base type MUST include core properties: `type` (type identifier), `domain_object_id` (UUID, string identifier, or 'generic'), `data` (object with schema validation), `tenant_id` (UUID), `updated_at` (datetime), and `created_at` (datetime). The base type MUST define `gts-traits` at the same level as `"type": "object"` to specify trait configuration that derived types must provide values for. The `gts-traits` MUST include: `domain_type` (enum of supported domain types), `events` (audit and notification configuration with propagation modes), `options` (configuration flags including inheritance, overwrite, MFA, retention, and compliance settings), and `operation` (access control and mutability rules). The base type MUST be marked as non-final to allow extension.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-gts-registry`
<!-- fdd-id-content -->

#### GTS Extension Point Registration

**ID**: `fdd-settings-service-fr-gts-extension-point`

<!-- fdd-id-content -->
The system MUST support an extension point mechanism that allows services to register concrete derived setting types that extend the base `gts.x.sm.setting.v1.0~` type. When registering a derived type, services MUST provide: (1) a unique GTS type identifier following the pattern `gts.x.sm.setting.{service}.{name}.v{version}~`, (2) concrete values for all `gts-traits` defined in the base type (domain_type, events, options, operation), (3) a JSON Schema for the `data` property that defines the structure and validation rules for setting values, and (4) optional legacy_name and legacy_namespace for backward compatibility. The system MUST validate that derived types properly extend the base type and provide all required trait values. The registration process MUST store the type definition in the GTS Type Registry and make it available for setting type creation through the Settings Service API. Derived types MUST inherit the base type's properties while allowing the `data` property schema to be overridden with specific constraints.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-gts-registry`
<!-- fdd-id-content -->

#### GTS Traits Definition

**ID**: `fdd-settings-service-fr-gts-traits-definition`

<!-- fdd-id-content -->
The system MUST support definition of setting type traits using GTS `gts-traits` format. The `gts-traits` definition MUST include all trait semantics: `domain_type` defines the domain type enum, `events` defines event configuration with audit and notification modes, `options` defines setting options with all configuration flags and retention period, `operation` defines operations configuration with access control and mutability rules, and legacy identifiers are preserved for backward compatibility. The `gts-traits` MUST be positioned at the same structural level as `"type": "object"` in the GTS schema definition, not nested within properties. The system MUST validate that all required traits are present and have valid values according to their type definitions. This enables consistent trait-based configuration for all setting types while maintaining compatibility with existing setting types.

**Actors**: `fdd-settings-service-actor-service-developer`, `fdd-settings-service-actor-gts-registry`
<!-- fdd-id-content -->

## D. Use Cases

## UC-001: Create Setting Type with GTS Schema

**ID**: `fdd-settings-service-usecase-create-setting-type`

<!-- fdd-id-content -->
**Actor**: `fdd-settings-service-actor-service-developer`

**Preconditions**:

- User has appropriate permissions
- GTS schema is valid
- GTS Type Registry is accessible

**Flow**:

1. Service developer defines GTS schema for setting type including required default value
2. Developer specifies setting type name and selects domain type from registered domain types (predefined: TENANT, USER, or custom registered types)
3. Developer configures setting type traits:
   - Event configuration: audit (SELF, SUBROOT, NONE), notification (SELF, SUBROOT, NONE)
   - Options: is_value_inheritable (default: true), is_mfa_required (default: false), retention_period (default: 90 days), is_barrier_inheritance (default: true), enable_generic (default: true), enable_compliance (default: false)
   - Operations: mutable_access_scope conditions, read_only conditions, hierarchy logic (and: parents)
4. Developer optionally configures domain object validation endpoints and deletion event details
5. System validates GTS schema structure and trait definitions
6. System registers setting type with GTS Type Registry
7. System creates setting type with schema and trait configuration
8. System makes setting type available for value assignments

**Postconditions**: Setting type is created and available for value assignments across tenant hierarchy
<!-- fdd-id-content -->

## UC-002: Update Setting Value with Inheritance

**ID**: `fdd-settings-service-usecase-update-value-inheritance`

<!-- fdd-id-content -->
**Actor**: `fdd-settings-service-actor-system-administrator`

**Preconditions**:

- Setting type exists
- User is one of: tenant user, parent tenant user (if tenancy barrier not enforced), or root/subroot tenant user
- Tenant is in enabled state
- Setting is not locked in compliance mode

**Flow**:

1. Administrator specifies tenant, domain object, and setting type identifier
2. Administrator provides new setting value matching GTS schema
3. System validates tenant access permissions (tenant user, parent tenant user if barrier not enforced, or root/subroot user)
4. System checks compliance lock status
5. System validates value against GTS schema
6. System checks mutable access scope traits
7. System updates setting value in database
8. System generates audit and notification events based on event trait configuration
9. Child tenants inherit new value if inheritance enabled in setting type

**Postconditions**: Setting value is updated and inherited by child tenants according to inheritance rules
<!-- fdd-id-content -->

## UC-003: Retrieve Setting Values with Hierarchy Resolution

**ID**: `fdd-settings-service-usecase-get-values-hierarchy`

<!-- fdd-id-content -->
**Actor**: `fdd-settings-service-actor-service-developer`

**Preconditions**:

- Setting type exists
- User has read access to tenant
- Setting type allows generic or explicit queries

**Flow (when inheritance is enabled)**:

1. Service requests setting values for tenant and domain object
2. System validates tenant read access
3. System resolves tenant hierarchy path
4. System queries explicit value for tenant/domain object
5. If no explicit value, system queries value for tenant/generic object
6. If no explicit value, system traverses parent hierarchy for tenant/domain object
7. If not found, system traverses parent hierarchy for tenant/generic object
8. System returns first explicit value found or default value from GTS schema
9. System includes inheritance metadata in response

**Flow (when inheritance is not enabled)**:

1. Service requests setting values for tenant and domain object
2. System validates tenant read access
3. System queries explicit value for tenant/domain object
4. If no explicit value, system queries value for tenant/generic object
5. If no explicit value, system returns default value from GTS schema

**Postconditions**: Service receives setting value with inheritance chain information
<!-- fdd-id-content -->

## UC-004: Lock Setting for Compliance

**ID**: `fdd-settings-service-usecase-lock-compliance`

<!-- fdd-id-content -->
**Actor**: `fdd-settings-service-actor-compliance-officer`

**Preconditions**:

- Setting type has enable_compliance trait enabled
- User has appropriate permissions
- Target tenant and domain object exist

**Flow**:

1. Compliance officer identifies setting requiring lock
2. Officer specifies tenant, domain object, and setting type
3. System validates compliance mode is enabled in setting type traits
4. System creates lock record in database
5. System marks setting as read-only for specified scope
6. System generates compliance lock audit event based on event trait configuration
7. Future update attempts return forbidden error

**Postconditions**: Setting is locked and cannot be modified until explicitly unlocked
<!-- fdd-id-content -->

## UC-005: Batch Update Setting Values

**ID**: `fdd-settings-service-usecase-batch-update`

<!-- fdd-id-content -->
**Actor**: `fdd-settings-service-actor-system-administrator`

**Preconditions**:

- Setting type exists
- User has appropriate permissions
- All target tenants are accessible

**Flow**:

1. Administrator provides list of tenant IDs and setting values
2. System validates batch size limits (max 100 tenants)
3. System validates each tenant access permission
4. System processes updates sequentially with transaction per tenant
5. System collects success and failure results
6. System generates events for successful updates based on event trait configuration
7. System returns partial success response with detailed errors

**Postconditions**: Setting values are updated for successful tenants, failures are reported with reasons
<!-- fdd-id-content -->

## UC-006: Reconcile Tenant Hierarchy

**ID**: `fdd-settings-service-usecase-reconcile-tenant`

<!-- fdd-id-content -->
**Actor**: `fdd-settings-service-actor-tenant-management`

**Preconditions**:

- Tenant exists in Tenant Management Module
- Event bus connection is active

**Flow**:

1. Tenant Management Module publishes tenant lifecycle event (create/update/delete)
2. Settings Service consumes event from event bus
3. System fetches tenant details from Tenant Management Module API
4. System calculates tenant hierarchy path
5. System updates or creates tenant record in local database
6. System updates tenant UUID to internal ID mapping
7. System marks tenant reconciliation as complete

**Postconditions**: Tenant hierarchy is synchronized and access control uses updated tenant paths
<!-- fdd-id-content -->

## UC-007: Query Setting Type with GTS Version Resolution

**ID**: `fdd-settings-service-usecase-gts-version-query`

<!-- fdd-id-content -->
**Actor**: `fdd-settings-service-actor-service-developer`

**Preconditions**:

- Setting type uses GTS-based naming
- Multiple versions of setting type exist in GTS Type Registry
- User has read access

**Flow**:

1. Service requests setting type using GTS type identifier
2. System queries GTS Type Registry for type definition
3. System resolves compatible type version based on GTS versioning rules
4. System returns setting type schema and traits
5. Service uses returned setting type for operations

**Postconditions**: Service receives compatible setting type version from GTS Type Registry
<!-- fdd-id-content -->

## UC-008: Bulk Get Setting Values

**ID**: `fdd-settings-service-usecase-bulk-get`

<!-- fdd-id-content -->
**Actor**: `fdd-settings-service-actor-service-developer`

**Preconditions**:

- Setting types exist
- User has read access to requested tenants

**Flow**:

1. Service requests setting values for multiple tenants and multiple setting types
2. System validates read access for each tenant
3. System queries setting values for each tenant/setting type combination
4. System applies inheritance resolution for each query
5. System collects all results with inheritance metadata
6. System returns bulk response with values organized by tenant and setting type

**Postconditions**: Service receives setting values for multiple tenants and setting types in single request
<!-- fdd-id-content -->

## UC-009: Register Dynamic Domain Type

**ID**: `fdd-settings-service-usecase-register-domain-type`

<!-- fdd-id-content -->
**Actor**: `fdd-settings-service-actor-service-developer`

**Preconditions**:

- User has appropriate permissions
- Domain type name is unique
- Validation API endpoint is accessible
- Event type is defined in Event Bus

**Flow**:

1. Service developer defines new domain type name (e.g., WORKSPACE, PROJECT, DEVICE)
2. Developer specifies domain_id data type (string, uuid, integer, etc.)
3. Developer provides REST API endpoint URL for domain object existence validation (e.g., GET /workspaces/{id})
4. Developer specifies event type for domain object deletion (e.g., workspace.deleted)
5. System validates domain type name uniqueness
6. System validates REST API endpoint accessibility
7. System validates event type exists in Event Bus
8. System registers domain type as GTS entity in domain type registry
9. System subscribes to domain object deletion events
10. System makes domain type available for setting type creation

**Postconditions**: New domain type is registered and available for use in setting type definitions. Settings Service monitors deletion events for automatic cleanup.
<!-- fdd-id-content -->

## E. Non-functional requirements

## Performance and Scalability

**ID**: `fdd-settings-service-nfr-performance`

<!-- fdd-id-content -->
The system MUST achieve sub-100ms response time for 95th percentile of setting value read operations and support 10,000+ write operations per minute. Database queries MUST use indexes for tenant hierarchy traversal and setting lookups. The system MUST handle tenant hierarchies with 10+ levels without performance degradation.
<!-- fdd-id-content -->

## Availability and Reliability

**ID**: `fdd-settings-service-nfr-availability`

<!-- fdd-id-content -->
The system MUST maintain 99.9% availability for read operations and 99.5% for write operations. The system MUST implement database transaction management to ensure data consistency. The system MUST handle message queue failures gracefully with retry mechanisms and dead letter queues.
<!-- fdd-id-content -->

## Security and Access Control

**ID**: `fdd-settings-service-nfr-security`

<!-- fdd-id-content -->
The system MUST enforce role-based access control with tenant hierarchy validation on all API endpoints. The system MUST support MFA requirements for sensitive settings configured through setting type traits. The system MUST validate all input data against GTS schemas to prevent injection attacks. The system MUST use secure communication channels (TLS) for all external API calls.
<!-- fdd-id-content -->

## Audit and Compliance

**ID**: `fdd-settings-service-nfr-audit`

<!-- fdd-id-content -->
The system MUST generate audit events for all setting and setting value modifications with complete metadata including user ID, tenant ID, timestamp, and change details. The system MUST support compliance mode with setting locks that prevent unauthorized modifications. Audit events MUST be immutable and stored for minimum 90 days. Access to audit logs MUST be restricted to compliance officers and system administrators with appropriate role-based permissions. Audit log retention beyond 90 days MUST be configurable per tenant for regulatory compliance requirements.
<!-- fdd-id-content -->

## Data Integrity and Consistency

**ID**: `fdd-settings-service-nfr-data-integrity`

<!-- fdd-id-content -->
The system MUST use database transactions for all write operations to ensure atomicity. The system MUST validate all setting values against their GTS schemas before persistence. The system MUST maintain referential integrity between setting types, setting values, tenants, and domain objects. Soft-deleted data MUST be retained for configured retention period before permanent deletion.
<!-- fdd-id-content -->

## Backward Compatibility

**ID**: `fdd-settings-service-nfr-backward-compatibility`

<!-- fdd-id-content -->
The system MUST support GTS-based type versioning to enable backward-compatible type evolution. API endpoints MUST maintain compatibility across versions. Type schema changes MUST not break existing clients using older type versions. The system MUST support GTS version resolution for automatic compatibility.
<!-- fdd-id-content -->

## Monitoring and Observability

**ID**: `fdd-settings-service-nfr-monitoring`

<!-- fdd-id-content -->
The system MUST expose metrics for API request rates, response times, error rates, and database query performance. The system MUST implement distributed tracing for request flows across service boundaries. The system MUST log all errors with stack traces and context for debugging. Health check endpoints MUST validate database connectivity and message queue status.
<!-- fdd-id-content -->

## Scalability and Multi-Tenancy

**ID**: `fdd-settings-service-nfr-scalability`

<!-- fdd-id-content -->
The system MUST support horizontal scaling with stateless API servers. The system MUST handle 100+ concurrent setting types and 10,000+ setting values per type. The system MUST support tenant hierarchies with 100,000+ tenants. Database connection pooling MUST be configured to handle concurrent requests efficiently.
<!-- fdd-id-content -->

## Data Encryption

**ID**: `fdd-settings-service-nfr-data-encryption`

<!-- fdd-id-content -->
The system MUST encrypt sensitive setting values at rest using industry-standard encryption algorithms (AES-256 or equivalent). Encryption keys MUST be managed through a secure key management system with key rotation capabilities. Setting types MUST support marking fields as sensitive to enable automatic encryption. Encrypted data MUST be decrypted only when accessed by authorized users with valid security context.
<!-- fdd-id-content -->

## Data Privacy Compliance

**ID**: `fdd-settings-service-nfr-data-privacy`

<!-- fdd-id-content -->
The system MUST comply with data privacy regulations including GDPR, CCPA, and similar frameworks for user-scoped and tenant-scoped settings. The system MUST support data subject access requests (DSAR) to export all settings associated with a user or tenant. The system MUST implement the right to be forgotten by permanently deleting all user data upon request. Personal data MUST be processed with explicit consent and purpose limitation. Data residency requirements MUST be supported for region-specific data storage.
<!-- fdd-id-content -->

## Network Security

**ID**: `fdd-settings-service-nfr-network-security`

<!-- fdd-id-content -->
The system MUST use TLS 1.2 or higher for all network communications including API endpoints, inter-module communication, and external service integrations. The system MUST enforce network isolation between tenants to prevent cross-tenant network access. API endpoints MUST be protected by rate limiting and DDoS protection mechanisms. The system MUST support IP allowlisting and network access control lists for restricted environments.
<!-- fdd-id-content -->

## Rust Implementation and Type Safety

**ID**: `fdd-settings-service-nfr-rust-safety`

<!-- fdd-id-content -->
The system MUST be implemented in Rust to leverage compile-time safety, memory safety without garbage collection, and deep static analysis. The system MUST pass all workspace lints including clippy with warnings denied, custom dylint lints for project compliance, and cargo deny for dependency licensing. The system MUST maintain 90%+ test coverage with unit, integration, and E2E tests.
<!-- fdd-id-content -->

## Hyperspot Module Standards

**ID**: `fdd-settings-service-nfr-module-standards`

<!-- fdd-id-content -->
The system MUST follow Hyperspot module conventions including SDK pattern with separate crates, DDD-light architecture with domain/api/infra layers, SecureConn for all database access, OperationBuilder for REST endpoints, and YAML configuration under modules.settings_service. The system MUST NOT use raw SQL or bypass SecureConn - all database operations MUST use SeaORM queries executed via `&SecureConn`.
<!-- fdd-id-content -->

## OpenAPI Documentation

**ID**: `fdd-settings-service-nfr-openapi`

<!-- fdd-id-content -->
The system MUST provide comprehensive OpenAPI 3.0 documentation automatically generated from Rust types via utoipa. All REST endpoints MUST be documented with request/response schemas, error codes, authentication requirements, and example payloads. Documentation MUST be accessible at /docs endpoint via Hyperspot API Gateway.
<!-- fdd-id-content -->

## F. Additional context

## Hyperspot Platform Integration

**ID**: `fdd-settings-service-prd-context-hyperspot-platform`

<!-- fdd-id-content -->
Settings Service is implemented as a Hyperspot module within the modular, high-performance Rust-based platform. It integrates with Hyperspot's core systems including ModKit for module lifecycle and REST API generation, API Gateway for HTTP routing and OpenAPI documentation, Tenant Resolver for multi-tenant security context, and ClientHub for type-safe inter-module communication. The service follows Hyperspot's "Everything is a Module" philosophy with composable, independent units. Reference: Hyperspot Server is a modular platform for building enterprise-grade SaaS services with automatic REST API generation, comprehensive OpenAPI documentation, and flexible modular architecture.
<!-- fdd-id-content -->

## Integration with Platform Modules

**ID**: `fdd-settings-service-prd-context-platform-integration`

<!-- fdd-id-content -->
Settings Service integrates with other Hyperspot modules including Tenant Management Module for tenant hierarchy, Event Bus Module for audit and lifecycle events, and optional Domain Object Validation Module for domain object existence validation. Inter-module communication uses ClientHub with typed SDK interfaces, enabling loose coupling and independent deployment.
<!-- fdd-id-content -->

## GTS Type System Integration

**ID**: `fdd-settings-service-prd-context-gts-integration`

<!-- fdd-id-content -->
The service integrates with GTS (Global Type System) Type Registry for setting type definitions and versioning. Setting types are defined using GTS schemas which provide JSON Schema validation, type versioning, and backward compatibility. Reference: <https://github.com/GlobalTypeSystem/gts-spec>. GTS enables dynamic type evolution without breaking existing clients.
<!-- fdd-id-content -->

## Event Bus Architecture

**ID**: `fdd-settings-service-prd-context-event-bus`

<!-- fdd-id-content -->
The service uses event bus for consuming tenant and user lifecycle events based on setting type trait configuration. Event handlers process messages asynchronously to maintain internal state consistency. The service publishes setting value change events (create, update, delete) to Event Bus Module for distribution to subscribers when configured in setting type traits.
<!-- fdd-id-content -->

## Database Schema and Migrations

**ID**: `fdd-settings-service-prd-context-database`

<!-- fdd-id-content -->
The service uses PostgreSQL database with schema migrations managed through the build system. The database schema includes tables for setting types, setting values, tenants, users, and domain objects. Indexes are optimized for tenant hierarchy queries and setting type lookups. Setting type schemas are stored as GTS type references.
<!-- fdd-id-content -->

## Generic Reporting Interface

**ID**: `fdd-settings-service-prd-context-reporting-interface`

<!-- fdd-id-content -->
The service provides a generic reporting interface for setting values with configurable attribute mappings. The interface allows external systems to retrieve setting values in custom formats by mapping setting attributes to report column names. This replaces platform-specific reporting integrations with a flexible, configuration-driven approach.
<!-- fdd-id-content -->

## Default Value Resolution Strategy

**ID**: `fdd-settings-service-prd-context-default-value-resolution`

<!-- fdd-id-content -->
The service is designed with a "always return a value" philosophy to ensure applications never encounter null or missing configuration errors. The value resolution follows a cascading fallback strategy: (1) explicit value for tenant/domain object, (2) explicit value for tenant/generic object, (3) inherited value from parent tenant hierarchy (if inheritance enabled), (4) default value from setting type's GTS schema. Every setting type MUST define a default value in its GTS schema, which serves as the ultimate fallback. This design ensures predictable behavior, eliminates null-checking in consuming applications, and provides sensible defaults for all configurations. The response metadata indicates the value source (explicit, inherited, generic, or default) for transparency and debugging.
<!-- fdd-id-content -->

## Domain Type Registry

**ID**: `fdd-settings-service-prd-context-domain-type-registry`

<!-- fdd-id-content -->
The service maintains a domain type registry as GTS entities that stores metadata for each supported domain type. Each domain type registration includes: (1) domain_id data type specification defining the type of identifier used (e.g., uuid for TENANT, uuid for USER), (2) REST API endpoint for validating domain object existence before allowing setting creation, (3) event type for domain object deletion that triggers automatic cleanup of associated settings. The system ships with predefined domain types (TENANT, USER) and supports runtime registration of custom domain types without code changes. When a domain object deletion event is received, the system automatically removes all settings scoped to that domain object across all tenants.
<!-- fdd-id-content -->

## SDK Pattern and Module Structure

**ID**: `fdd-settings-service-prd-context-sdk-pattern`

<!-- fdd-id-content -->
The service follows Hyperspot's SDK pattern with two separate crates: `settings-service-sdk` containing the public API trait, transport-agnostic models, and error types; and `settings-service` containing the implementation with domain logic, REST handlers, local client adapter, and infrastructure. This separation enables consumers to depend only on the lightweight SDK crate and access the service via `hub.get::<dyn SettingsServiceClient>()` without implementation dependencies. All SDK trait methods accept `&SecurityContext` as the first parameter for authorization and tenant isolation.
<!-- fdd-id-content -->

## Module Configuration

**ID**: `fdd-settings-service-prd-context-configuration`

<!-- fdd-id-content -->
The service uses YAML-based configuration under the `modules.settings_service` section with support for environment variable overrides using `HYPERSPOT_MODULES_settings_service_` prefix. Configuration includes database connection settings, pagination settings (default: 50 items, maximum: 200 items), and module-specific options. The service supports multiple database backends (PostgreSQL, MariaDB, SQLite) through SeaORM with database-agnostic migrations.
<!-- fdd-id-content -->

## Testing and Quality Assurance

**ID**: `fdd-settings-service-prd-context-testing`

<!-- fdd-id-content -->
The service includes comprehensive unit tests, integration tests, and E2E tests following Hyperspot's quality standards. E2E tests are written in Python using pytest framework and can be run against local development environment or Docker containers. The service uses cargo clippy with warnings denied, custom dylint lints for project-specific compliance, cargo deny for dependency licensing, and maintains 90%+ test coverage target. Continuous fuzzing via ClusterFuzzLite identifies bugs and security issues.
<!-- fdd-id-content -->
