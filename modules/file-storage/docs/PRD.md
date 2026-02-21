# PRD — File Storage

## 1. Overview

### 1.1 Purpose

FileStorage is a universal file storage and management service for the CyberFabric platform. It provides upload,
download, metadata management, access control, and sharing capabilities for any module or user within the platform.

The service supports pluggable storage backends, multiple access protocols (REST, S3-compatible, WebDAV), tenant-scoped
access control with an ownership model, and policy-driven governance for file types, sizes, and sharing.

### 1.2 Background / Problem Statement

CyberFabric modules and platform users require file storage for various purposes: modules handle multimodal AI content
(images, audio, video, documents), documents and artifacts, reporting outputs, and platform users need direct file
access through standard protocols.

Without a dedicated storage service, each module implements ad-hoc file handling, media gets inlined as base64 in API
payloads (bloating requests and hitting size limits), provider-generated URLs expire leaving consumers with broken
links, and there is no unified access control or policy enforcement across the platform.

FileStorage solves this by providing a centralized, tenant-aware storage service with persistent URLs, pluggable
backends, and standardized access interfaces — functioning as a superset of S3 and WebDAV capabilities within the
CyberFabric security and governance model.

### 1.3 Goals (Business Outcomes)

- Unified file storage accessible by all CyberFabric modules and platform users
- Tenant-scoped access control with user and tenant ownership model
- Flexible sharing via public, tenant-scoped, and signed URLs
- Policy-driven governance over file types, sizes, webhooks, and sharing models
- Audit trail for all write operations
- Pluggable storage backends without service rebuild

### 1.4 Success Metrics

| Metric                                   | Baseline                                 | Target                                                           | Timeframe                      |
|------------------------------------------|------------------------------------------|------------------------------------------------------------------|--------------------------------|
| Module adoption rate                     | 0% (ad-hoc file handling)                | 90%+ of file-dependent modules use FileStorage SDK               | 6 months after GA              |
| Base64-inlined media payloads            | Present in LLM Gateway and other modules | 0 base64 file payloads in modules that adopted FileStorage       | 3 months after module adoption |
| Broken/expired provider URLs             | Recurring in downstream workflows        | 0 broken URLs for files within retention period                  | Ongoing after GA               |
| Audit coverage for file write operations | No centralized audit                     | 100% of write operations audited                                 | At GA                          |
| Multi-backend deployment                 | Single ad-hoc storage per module         | At least 2 backend types validated (e.g., S3 + local filesystem) | At GA                          |

### 1.5 Glossary

| Term                | Definition                                                                                                                                                                                                                                                        |
|---------------------|-------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------|
| File                | Binary content stored in FileStorage with associated metadata                                                                                                                                                                                                     |
| File URL            | Persistent URL pointing to content stored in FileStorage                                                                                                                                                                                                          |
| Metadata            | File properties: system-managed (name, size, mime_type, dates, owner, availability) and user-defined custom key-value pairs                                                                                                                                       |
| Custom Metadata     | User-defined key-value pairs attached to a file, analogous to S3 object metadata                                                                                                                                                                                  |
| Owner               | Entity (user or tenant) that owns a file and controls its sharing and lifecycle                                                                                                                                                                                   |
| Shareable Link      | A unique URL served by FileStorage that grants access to a file with a specific scope and expiration; FileStorage validates the link and enforces access control on every request                                                                                 |
| Signed URL          | A presigned URL pointing directly to the storage backend, generated by FileStorage using its own backend credentials, granting time-limited download access without requiring authentication; the storage backend validates the signature and enforces expiration |
| Direct Transfer URL | A presigned URL pointing directly to the storage backend, generated by FileStorage using its own backend credentials, allowing a client to upload or download without routing traffic through FileStorage                                                         |
| Storage Backend     | An underlying storage system (S3, GCS, Azure Blob, NFS, FTP, SMB, WebDAV) used for persisting file content                                                                                                                                                        |
| Tenant Policy       | A set of rules defined by a tenant governing allowed file types, size limits, webhooks, and sharing models                                                                                                                                                        |

## 2. Actors

> **Note**: Stakeholder needs are managed at the project/task level by the steering committee and are not duplicated in
> module specs. Focus on **actors** (users, systems) that directly interact with this module.

### 2.1 Human Actors

#### Platform User

**ID**: `cpt-cf-file-storage-actor-platform-user`

**Role**: Authenticated user who uploads, downloads, and manages files through the platform UI or API.
**Needs**: Direct file access, sharing capabilities, metadata management, and self-service link management.

### 2.2 System Actors

#### CyberFabric Modules

**ID**: `cpt-cf-file-storage-actor-cf-modules`

**Role**: Any CyberFabric module requiring file upload, download, metadata retrieval, or link management (e.g., LLM
Gateway for multimodal media, document management modules, reporting modules).

#### Authorization Service

**ID**: `cpt-cf-file-storage-actor-authz-service`

**Role**: Evaluates access decisions for read, write, and delete operations on `gts.x.fstorage.file` resources.

## 3. Operational Concept & Environment

> **Note**: Project-wide runtime, OS, architecture, lifecycle policy, and module integration patterns are defined in
> root PRD. Only document module-specific deviations or additional constraints here.

### 3.1 Module-Specific Environment Constraints

No module-specific environment constraints. FileStorage operates within the standard CyberFabric runtime environment.

## 4. Scope

### 4.1 In Scope

- Upload, download, delete, and list files
- Rich file metadata storage, retrieval, and update
- File ownership by user or tenant
- Authorization checks via Authorization Service
- Shareable links with public, tenant, and tenant-hierarchy scopes
- Signed URLs for unauthenticated, time-limited downloads
- Link expiration and lifecycle management
- Audit trail for all write operations
- Tenant-level policies (file types, size limits, webhooks, sharing restrictions)
- Pluggable storage backend abstraction
- Multipart (chunked) upload for large files
- Content-type validation against actual file content
- Direct-to-backend transfer via presigned URLs for compatible backends
- Garbage collection for unconfirmed direct uploads
- Indefinite file retention (phase 1); tenant-level and per-file retention policies (phase 2)
- REST API access interface

### 4.2 Out of Scope

- S3-compatible API (phase 2)
- WebDAV API (phase 2)
- Streaming and range requests (phase 2)
- Runtime tenant-configurable storage backends (phase 2)
- Storage quota enforcement via Quota Enforcement service (phase 2)
- Ownership transfer (phase 2)
- Custom metadata limits (phase 2)
- Content transformation or transcoding
- CDN distribution
- Full-text search within file content
- File versioning

## 5. Functional Requirements

> **Testing strategy**: All requirements verified via automated tests (unit, integration, e2e) targeting 90%+ code
> coverage unless otherwise specified.

### 5.1 Core File Operations

#### Upload File

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-upload-file`

The system **MUST** accept file content with metadata and persist it, returning a persistent, accessible URL. File
content is immutable after upload — to change content, a new file **MUST** be uploaded.

**Rationale**: All platform modules and users need to store files — modules store generated content, documents, and
artifacts, users upload files directly. Immutable content simplifies caching, integrity verification, and backend
replication.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Download File

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-download-file`

The system **MUST** retrieve file content and metadata by URL for consumption by requesting actors.

**Rationale**: All platform modules and users need to retrieve stored files — modules fetch media and documents, users
download files directly.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Delete File

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-delete-file`

The system **MUST** allow the file owner to permanently delete a file and all its associated shareable links.

**Rationale**: Owners need to remove files that are no longer needed; deletion must cascade to all links to prevent
dangling references.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Get File Metadata

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-get-metadata`

The system **MUST** return file metadata (name, size, mime_type, created date, modified date, owner, download
availability, and custom metadata) without transferring file content.

**Rationale**: Consumers validate file properties (size limits, type compatibility) and read custom metadata before
initiating downloads, avoiding wasted bandwidth on incompatible files.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### List Files

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-list-files`

The system **MUST** support listing files with their metadata (no content transfer). The caller **MUST** specify the
owner type as a mandatory filter:

- **User-owned** — files owned by a specific user
- **Tenant-owned** — files owned by the tenant

The response **MUST** be paginated following the platform API guidelines (cursor-based or offset-based pagination with
configurable page size). The system **SHOULD** support additional filters (mime_type, date range, custom metadata keys).

**Rationale**: Users and modules need to discover and browse files they own or have access to. Mandatory owner type
filtering prevents unbounded queries across all files and aligns with the ownership model.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Multipart Upload

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-multipart-upload`

The system **MUST** support multipart (chunked) upload for large files. A multipart upload **MUST**:

- Allow the client to split a file into multiple parts and upload them independently
- Support resumable uploads — if a part fails, only that part needs re-uploading
- Assemble parts into a complete file upon finalization
- Apply the same authorization, metadata, and audit requirements as single-part uploads

**Rationale**: Single-request uploads are impractical for large files (video, datasets, backups) due to timeouts,
memory constraints, and network reliability. Multipart upload enables reliable transfer of arbitrarily large files.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Content-Type Validation

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-content-type-validation`

The system **MUST** validate the declared mime_type against the actual file content (magic bytes / file signature) on
proxied uploads (where file content passes through FileStorage). If the declared type does not match the detected type,
the system **MUST** reject the upload with an error indicating the mismatch. Content-type validation does not apply to
direct uploads via presigned URLs because FileStorage does not receive the file content in that flow.

**Rationale**: Without content inspection, a client can declare `image/png` but upload an executable, trivially
bypassing file type policies. Content-type validation ensures declared types are trustworthy for downstream consumers
and policy enforcement. Direct uploads trade server-side content validation for transfer efficiency — consumers relying
on strict type guarantees should use proxied uploads.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

### 5.2 Ownership & Access Control

#### File Ownership

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-file-ownership`

The system **MUST** associate every file with an owner. Ownership **MUST** be assignable to either a specific user or a
tenant at upload time. Ownership is immutable after creation — transfer of ownership is out of scope.

**Rationale**: Ownership determines who can manage (delete, share, update metadata) a file and establishes the basis for
access control decisions. Immutable ownership simplifies the authorization model and prevents accidental privilege
escalation.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Authorization Checks

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-authorization`

The system **MUST** verify authorization for every file operation by requesting an access decision from the
Authorization Service. Read, write, and delete operations **MUST** be checked against `gts.x.fstorage.file` resources in
the context of the requesting user.

**Rationale**: All file access must be governed by the platform's centralized authorization model to enforce role-based
and tenant-scoped permissions.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules` (initiating actors);
`cpt-cf-file-storage-actor-authz-service` (decision service)

#### Tenant Boundary Enforcement

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-tenant-boundary`

The system **MUST** enforce tenant isolation: file deletion and metadata update operations **MUST NOT** cross tenant
boundaries. A user in one tenant **MUST NOT** delete or update metadata of files owned by another tenant. Cross-tenant
read access is intentionally permitted via shareable links with tenant-hierarchy scope (see
`cpt-cf-file-storage-fr-shareable-links`).

**Rationale**: Multi-tenant platforms require strict data isolation for write operations to prevent unauthorized
cross-tenant modification, while supporting controlled read sharing across tenant hierarchies.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules` (initiating actors);
`cpt-cf-file-storage-actor-authz-service` (decision service)

#### Data Classification

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-data-classification`

FileStorage treats all stored files as opaque binary blobs and does **NOT** inspect, classify, or label file content by
sensitivity level. Data classification (public, internal, confidential, restricted) is the responsibility of consuming
modules and tenant policies. FileStorage enforces access control through its authorization model and tenant boundaries
regardless of data sensitivity.

**Rationale**: FileStorage is a general-purpose storage service that serves modules with diverse data sensitivity
requirements. Embedding classification logic in the storage layer would couple it to domain-specific semantics. Instead,
consuming modules classify their own data and rely on FileStorage's authorization and tenant isolation to enforce access
boundaries appropriate to the sensitivity level.  
**Actors**: `cpt-cf-file-storage-actor-cf-modules`

#### Ownership Transfer

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-ownership-transfer`

The system **MUST** allow the current file owner to transfer ownership of a file to another user or to the tenant.
Ownership transfer **MUST** be an audited operation and **MUST** require authorization of both the current owner and the
receiving entity.

**Rationale**: As teams evolve, files may need to change hands — e.g., when a user leaves the organization or when
personal files should become shared tenant resources.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`

### 5.3 Link Management & Sharing

#### Create Shareable Links

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-shareable-links`

The system **MUST** support creating unique shareable links for files with the following access scopes:

- **Public** — accessible to anyone, including unauthenticated users
- **Tenant** — accessible to any authenticated user within the file's tenant
- **Tenant hierarchy** — accessible to any authenticated user within the file's tenant and its child tenants

Shareable links are served by FileStorage — all requests pass through FileStorage, which validates the link, enforces
scope-based access control, and serves the file content from the storage backend. The desired sharing scope(s) **MUST**
be specifiable at file creation time and when creating additional links for existing files.

**Rationale**: Different use cases require different visibility: public links for external sharing, tenant links for
internal collaboration, hierarchy links for parent-child tenant structures. Routing through FileStorage enables
scope-based access control, revocation, and audit logging on every access.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Signed Download URLs

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-signed-urls`

The system **MUST** support generating presigned download URLs that point directly to the storage backend, granting
time-limited download access without requiring authentication. FileStorage generates these URLs using its own backend
credentials. A signed URL **MUST**:

- Be generated by FileStorage using its own credentials with the storage backend
- Point directly to the storage backend (bypassing FileStorage for content delivery)
- Contain a cryptographic signature that the storage backend validates against its own key material
- Include an expiration timestamp after which the URL becomes invalid
- Be scoped to a single file (download only)
- Not require the consumer to present authentication credentials

Signed URLs are **not revocable** — once issued, they remain valid until expiration because the storage backend
validates the signature independently of FileStorage. To limit exposure, signed URLs **MUST** use short expiration
times. If revocable access is needed, use shareable links instead (served through FileStorage with revocation support).

For backends that do not support presigned URLs, FileStorage **MUST** fall back to serving the content through its own
endpoint with signature validation.

**Rationale**: Signed URLs enable secure file sharing with external systems and unauthenticated consumers (e.g.,
embedding in emails, third-party integrations) while maintaining time-bounded access control. Routing downloads through
the storage backend directly eliminates FileStorage as a bottleneck for shared content — following the pattern
established by S3 presigned URLs, GCS signed URLs, and Azure SAS tokens. The non-revocable nature follows the same
constraint inherent in S3 presigned URLs, GCS signed URLs, and Azure SAS tokens.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Link Expiration

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-link-expiration`

The system **MUST** support configurable expiration for any shareable link or signed URL. Expiration **MUST** be
specifiable at link creation time. For shareable links, FileStorage enforces expiration and **MUST** return an
access-denied response after the link expires. For signed URLs, the storage backend enforces expiration — the
backend validates the signature and rejects expired URLs independently of FileStorage.

**Rationale**: Time-limited access prevents stale links from remaining accessible indefinitely, reducing the attack
surface for shared files. Expiration enforcement follows the traffic path: FileStorage enforces for shareable links
(which it serves), and the storage backend enforces for signed URLs (which bypass FileStorage).  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Manage Links

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-manage-links`

The file owner **MUST** be able to list all active shareable links and issued signed URLs for a file. The owner **MUST**
be able to revoke (delete) individual shareable links. Signed URLs cannot be revoked (they remain valid until
expiration) but the owner **MUST** be able to view their expiration status.

**Rationale**: Owners need visibility into how their files are shared and the ability to revoke shareable link access
when no longer needed. Signed URLs are non-revocable by design (backend validates independently), so short expiration
is the primary access control mechanism.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`

### 5.4 Direct-to-Backend Transfer

#### Direct Transfer URLs

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-direct-transfer`

The system **MUST** support generating presigned direct transfer URLs that point to the storage backend, allowing
clients to upload or download file content directly to/from the backend without routing traffic through FileStorage.
FileStorage generates these URLs using its own backend credentials (e.g., AWS access keys, GCS service account). A
direct transfer URL **MUST**:

- Be generated by FileStorage using its own credentials with the storage backend
- Point directly to the storage backend (e.g., S3 bucket endpoint)
- Contain a cryptographic signature that the backend validates against its own key material
- Support both upload (PUT) and download (GET) operations
- Be time-limited with a configurable expiration
- Be scoped to a single file and a single operation type (upload or download)

The client authenticates with FileStorage (using its API token), FileStorage verifies authorization, registers the file
metadata (including the target backend path), and then issues the presigned URL. The client uses the presigned URL
directly with the backend — no further authentication or callback is required because the backend trusts the signature
generated with FileStorage's credentials.

**Rationale**: For large files (video, datasets, backups), proxying all traffic through FileStorage creates a bottleneck
and doubles bandwidth consumption. Direct-to-backend transfer via presigned URLs eliminates this overhead for compatible
backends, following the pattern established by S3 presigned URLs, GCS signed URLs, and Azure SAS tokens — where the
service with backend credentials signs on behalf of the client.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Garbage Collection for Unconfirmed Uploads

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-gc-direct-uploads`

The system **MUST** detect and clean up orphaned records from direct uploads that were never completed (presigned URL
issued but upload not started, or upload partially completed). The system **MUST** reconcile file metadata records
against actual backend object existence and remove records with no corresponding backend object. Cleanup **MUST** be
automatic and based on a configurable TTL for unconfirmed uploads.

**Rationale**: Since metadata is registered before the presigned URL is issued, failed or abandoned uploads leave
metadata records pointing to non-existent backend objects. Garbage collection prevents stale metadata accumulation and
ensures consistency between FileStorage records and backend state.  
**Actors**: `cpt-cf-file-storage-actor-cf-modules`

### 5.5 Tenant Policies (Phase 2)

#### Allowed File Types Policy

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-allowed-types-policy`

The system **MUST** allow tenants to define policies specifying which file types (by mime_type) are permitted for
upload. Uploads of disallowed types **MUST** be rejected.

**Rationale**: Tenants need to restrict uploads to approved file types for security and compliance (e.g., blocking
executable files).  
**Actors**: `cpt-cf-file-storage-actor-platform-user`

#### File Size Limits Policy

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-size-limits-policy`

The system **MUST** allow tenants to define file size limits with two levels of granularity: a global maximum size for
any file, and per-mime-type size limits that override the global limit. Uploads exceeding limits **MUST** be rejected.

**Rationale**: Tenants need to control storage consumption and prevent oversized uploads, with the flexibility to allow
larger files for specific types (e.g., video vs. text).  
**Actors**: `cpt-cf-file-storage-actor-platform-user`

#### Webhook Triggers

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-webhook-triggers`

The system **MUST** allow tenants to configure webhooks triggered on file write operations (upload, update, delete).
Tenant policy **MUST** define which events trigger which webhook URLs.

**Rationale**: Enables integration with external systems for workflows such as antivirus scanning, content moderation,
or backup triggers.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`

#### Sharing Model Restrictions

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-sharing-restrictions`

The system **MUST** allow tenants to restrict which sharing models (public, tenant, tenant hierarchy, signed URLs) are
available within their tenant. Attempts to create links with restricted sharing models **MUST** be rejected.

**Rationale**: Tenants in regulated environments may need to prohibit public sharing or signed URLs to enforce data
governance policies.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`

#### Storage Quota Enforcement

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-storage-quota`

The system **MUST** integrate with the Quota Enforcement service to enforce per-tenant storage consumption limits.
Uploads that would exceed the tenant's storage quota **MUST** be rejected. The system **MUST** report current storage
consumption to the Quota Enforcement service.

**Rationale**: Without storage quotas, tenants can consume unbounded storage, increasing costs and risking resource
exhaustion for the platform.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`

### 5.6 Metadata

#### Rich Metadata Storage

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-metadata-storage`

The system **MUST** store and return the following system-managed metadata for every file:

- File name (original upload name)
- File size (bytes)
- File type (mime_type)
- Creation date
- Last modified date
- Owner (user or tenant reference)
- Download availability (whether the file is currently accessible for download; controlled by the file owner)

In addition, the system **MUST** support user-defined custom metadata as arbitrary key-value string pairs. Custom
metadata **MUST** be specifiable at upload time and updatable after upload. The system **MUST** return custom metadata
alongside system-managed metadata in metadata queries.

**Rationale**: Rich metadata enables file browsing, search, validation, and governance across the platform. Custom
metadata enables consumers to attach domain-specific context (tags, categories, processing status, source identifiers)
without schema changes — following the established pattern used by S3 object metadata, GCS custom metadata, and Azure
Blob metadata.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Update Custom Metadata

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-update-metadata`

The file owner **MUST** be able to update custom metadata (user-defined key-value pairs) and download availability on
an existing file. All other system-managed metadata (name, size, mime_type, creation date, last modified date, owner) is
**NOT** updatable by users — it is maintained by the system. Updating custom metadata or download availability **MUST**
update the file's last modified date.

**Rationale**: Custom metadata evolves as files are processed, categorized, or annotated by consuming modules. System
metadata reflects the immutable physical properties of the file and must remain authoritative.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Custom Metadata Limits

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-metadata-limits`

The system **MUST** enforce configurable limits on custom metadata: maximum number of key-value pairs per file, maximum
key name length, maximum value length, and maximum total custom metadata size per file. Metadata operations exceeding
limits **MUST** be rejected.

**Rationale**: Without limits, custom metadata can be abused for general-purpose data storage, inflating metadata
storage costs and degrading query performance.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

### 5.7 File Retention & Lifecycle

#### Indefinite Retention

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-retention-indefinite`

In phase 1, files **MUST** be retained indefinitely until explicitly deleted by the file owner. The system **MUST NOT**
automatically delete or expire file content based on age or inactivity. Shareable links and signed URLs expire per their
configured expiration, but the underlying file content remains available.

**Rationale**: In the absence of tenant-level retention policies (phase 2), indefinite retention is the safest default —
it prevents accidental data loss and gives consuming modules predictable storage semantics.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### Retention Policies

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-retention-policies`

The system **MUST** allow tenants to define retention policies specifying automatic file expiration based on age,
inactivity, or custom metadata criteria. The system **MUST** also support per-file retention overrides set by the file
owner. When a file's retention period expires, the system **MUST** delete the file content, metadata, and all associated
links, and emit an audit record.

**Rationale**: Regulated environments and cost-conscious tenants need automated lifecycle management to enforce data
retention compliance and control storage growth.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`

### 5.8 Audit

#### Audit Trail

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-audit-trail`

The system **MUST** produce an audit record for every write operation (upload, delete, metadata update, link creation,
link revocation). Audit records **MUST** include the operation type, actor identity, file identifier, timestamp, and
outcome (success or failure).

**Rationale**: Audit trails are required for security forensics, compliance reporting, and operational troubleshooting.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

### 5.9 Pluggable Storage Backends

#### Backend Abstraction

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-backend-abstraction`

The system **MUST** abstract the storage layer behind a common interface, enabling support for multiple backend types (
S3, GCS, Azure Blob, NFS, FTP, SMB, WebDAV, local filesystem).

**Rationale**: Different deployments and tenants have different storage infrastructure; a common interface allows
backend selection without changing the module's core logic.  
**Actors**: `cpt-cf-file-storage-actor-cf-modules`

#### Runtime Backend Configuration

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-runtime-backends`

The system **MUST** allow tenants to connect and configure storage backends at runtime without requiring service rebuild
or redeployment.

**Rationale**: Enterprise tenants need to bring their own storage (BYOS) and switch backends based on cost, compliance,
or geographic requirements.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`

### 5.10 Access Interfaces

#### REST API

- [ ] `p1` - **ID**: `cpt-cf-file-storage-fr-rest-api`

The system **MUST** expose a REST API for all file operations (upload, download, delete, metadata, link management).

**Rationale**: REST is the standard access interface for CyberFabric modules and platform UI.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### S3-Compatible API

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-s3-api`

The system **MUST** expose an S3-compatible API for file upload and download operations, enabling integration with
existing S3 tooling and SDKs.

**Rationale**: S3 is the de facto standard for object storage APIs; compatibility enables direct integration with tools,
libraries, and workflows that already support S3.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

#### WebDAV API

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-webdav-api`

The system **MUST** expose a WebDAV API for file access, enabling native filesystem-like mounting on client operating
systems.

**Rationale**: WebDAV enables direct OS-level access to stored files without custom client software, supporting use
cases like document editing and file management through native file explorers.
**Actors**: `cpt-cf-file-storage-actor-platform-user`

#### Streaming and Range Requests

- [ ] `p2` - **ID**: `cpt-cf-file-storage-fr-range-requests`

The system **MUST** support HTTP Range requests (RFC 7233) for partial content download, enabling seeking within large
files, resumable downloads, and parallel download of file segments.

**Rationale**: For large files (video, datasets), clients need partial access for seeking, preview generation, and
resuming interrupted downloads without re-transferring the entire file.  
**Actors**: `cpt-cf-file-storage-actor-platform-user`, `cpt-cf-file-storage-actor-cf-modules`

## 6. Non-Functional Requirements

> **Global baselines**: Project-wide NFRs (performance, security, reliability, scalability) defined in root PRD
> and [guidelines/](../../../guidelines/). Document only module-specific NFRs here: **exclusions** from defaults or
> **standalone** requirements.
>
> **Testing strategy**: NFRs verified via automated benchmarks, security scans, and monitoring unless otherwise
> specified.

### 6.1 Module-Specific NFRs

#### Metadata Query Latency

- [ ] `p1` - **ID**: `cpt-cf-file-storage-nfr-metadata-latency`

File metadata queries **MUST** complete within 25ms at p95.

**Threshold**: <25ms p95  
**Rationale**: Metadata queries are used for pre-fetch validation in latency-sensitive paths (e.g., a module checks file
size before processing).  
**Architecture Allocation**: See DESIGN.md § NFR Allocation for how this is realized

#### Content Transfer Latency

- [ ] `p1` - **ID**: `cpt-cf-file-storage-nfr-transfer-latency`

Content download latency **MUST** have no fixed overhead exceeding 50ms at p95; total transfer time is proportional to
file size.

**Threshold**: <50ms + transfer time p95  
**Rationale**: FileStorage is called synchronously in request paths of consuming modules; excessive overhead compounds
across requests with multiple files.  
**Architecture Allocation**: See DESIGN.md § NFR Allocation for how this is realized

#### URL Availability

- [ ] `p1` - **ID**: `cpt-cf-file-storage-nfr-url-availability`

Stored file URLs and shareable links **MUST** remain accessible for the duration of their configured lifetime with
availability matching the platform SLA.

**Threshold**: URL availability matches platform SLA for the duration of the retention/expiration period  
**Rationale**: Consumers depend on URL stability — broken links disrupt downstream workflows and user experience.  
**Architecture Allocation**: See DESIGN.md § NFR Allocation for how this is realized

#### Audit Completeness

- [ ] `p1` - **ID**: `cpt-cf-file-storage-nfr-audit-completeness`

Audit records **MUST** be emitted for 100% of write operations with no silent drops under normal operating conditions.

**Threshold**: 100% audit coverage for write operations  
**Rationale**: Incomplete audit trails undermine compliance and forensic investigations.  
**Architecture Allocation**: See DESIGN.md § NFR Allocation for how this is realized

#### Data Durability and Recovery

- [ ] `p1` - **ID**: `cpt-cf-file-storage-nfr-durability`

File content and metadata **MUST** achieve a Recovery Point Objective (RPO) of zero for committed writes — no
acknowledged upload may be silently lost. The Recovery Time Objective (RTO) for service restoration after an outage
**MUST NOT** exceed 15 minutes. These targets apply to the FileStorage service layer; underlying storage backend
durability (e.g., S3 99.999999999% durability) is inherited from the backend and not controlled by FileStorage.

**Threshold**: RPO = 0 (no data loss for committed writes); RTO ≤ 15 minutes  
**Rationale**: File loss after a successful upload acknowledgment breaks consumer trust and disrupts downstream
workflows. The RPO=0 target ensures write-ahead semantics where acknowledgment implies durability. The 15-minute RTO
balances recovery speed with operational complexity for a non-user-facing backend service.  
**Architecture Allocation**: See DESIGN.md § NFR Allocation for how this is realized

### 6.2 NFR Exclusions

None — all project-default NFRs apply to this module.

## 7. Public Library Interfaces

### 7.1 Public API Surface

#### FileStorage SDK Trait

- [ ] `p1` - **ID**: `cpt-cf-file-storage-interface-sdk-trait`

**Type**: Rust trait (SDK crate)  
**Stability**: unstable  
**Description**: Async trait providing upload, download, delete, metadata, and link management operations.  
**Breaking Change Policy**: Major version bump required for trait signature changes.

#### REST API

- [ ] `p1` - **ID**: `cpt-cf-file-storage-interface-rest-api`

**Type**: REST API (OpenAPI 3.0)  
**Stability**: unstable  
**Description**: HTTP REST API for all file operations, metadata management, and link management.  
**Breaking Change Policy**: Major version bump required for endpoint removal or request/response schema incompatible
changes.

### 7.2 External Integration Contracts

#### CyberFabric Module Contract

- [ ] `p1` - **ID**: `cpt-cf-file-storage-contract-cf-modules`

**Direction**: provided by library (consumed by CyberFabric modules)  
**Protocol/Format**: In-process Rust SDK trait via ClientHub  
**Compatibility**: Trait versioned with SDK crate; breaking changes require coordinated release with consuming modules.

#### Authorization Service Contract

- [ ] `p1` - **ID**: `cpt-cf-file-storage-contract-authz`

**Direction**: required from external service (Authorization Service)  
**Protocol/Format**: Access decision requests for `gts.x.fstorage.file` resources  
**Compatibility**: Contract follows platform authorization protocol; changes require coordinated release.

## 8. Use Cases

#### Upload and Share a File

- [ ] `p1` - **ID**: `cpt-cf-file-storage-usecase-upload-share`

**Actor**: `cpt-cf-file-storage-actor-platform-user`

**Preconditions**:

- User is authenticated
- Authorization Service grants write access

**Main Flow**:

1. User uploads file content with metadata (name, mime_type)
2. FileStorage checks authorization for write on `gts.x.fstorage.file`
3. *(Phase 2)* FileStorage validates file against tenant policies (type, size); in phase 1 all uploads are accepted
4. FileStorage persists content, assigns ownership to the user, and stores metadata
5. FileStorage emits audit record for the upload
6. FileStorage returns persistent URL and file identifier
7. User creates a shareable link with desired scope and expiration
8. FileStorage returns the shareable link URL

**Postconditions**:

- File stored with metadata and ownership
- Shareable link active with configured scope and expiration
- Audit record emitted

**Alternative Flows**:

- **Authorization denied**: FileStorage returns access-denied error
- *(Phase 2)* **Policy violation**: FileStorage returns error indicating which policy was violated (type or size)

#### Fetch File for Module Processing

- [ ] `p1` - **ID**: `cpt-cf-file-storage-usecase-fetch-media`

**Actor**: `cpt-cf-file-storage-actor-cf-modules`

**Preconditions**:

- File exists at the specified URL

**Main Flow**:

1. Module calls download with a file URL
2. FileStorage checks authorization for read
3. FileStorage retrieves file content from the storage backend
4. FileStorage returns content with metadata (mime_type, size)

**Postconditions**:

- Content and metadata returned to the requesting module

**Alternative Flows**:

- **File not found**: FileStorage returns file_not_found error
- **Authorization denied**: FileStorage returns access-denied error

#### Generate and Access Signed URL

- [ ] `p1` - **ID**: `cpt-cf-file-storage-usecase-signed-url`

**Actor**: `cpt-cf-file-storage-actor-platform-user`

**Preconditions**:

- User owns the file or has write access
- Tenant policy permits signed URL sharing

**Main Flow**:

1. Owner requests a signed URL for a file with a specified expiration
2. FileStorage checks authorization for the owner on `gts.x.fstorage.file`
3. *(Phase 2)* FileStorage validates tenant sharing policy allows signed URLs; in phase 1 all sharing models are
   permitted
4. FileStorage generates a presigned download URL using its own backend credentials, scoped to the file and time-limited
5. FileStorage records the signed URL metadata (file, expiration, owner) for visibility and audit
6. FileStorage emits audit record for signed URL creation
7. Owner shares the signed URL with an external consumer
8. External consumer downloads the file directly from the storage backend using the signed URL — no authentication
   required, backend validates signature

**Postconditions**:

- File content delivered to external consumer directly from storage backend without authentication
- Signed URL metadata recorded for visibility and audit
- Audit record emitted for signed URL creation

**Alternative Flows**:

- **Expired URL**: Storage backend rejects the request (signature expiration enforced by backend)
- **Invalid signature**: Storage backend rejects the request
- **Backend does not support presigned URLs**: FileStorage falls back to serving content through its own endpoint with
  signature validation
- *(Phase 2)* **Sharing model restricted by tenant policy**: FileStorage returns policy-violation error

#### Validate File Metadata Before Processing

- [ ] `p1` - **ID**: `cpt-cf-file-storage-usecase-get-metadata`

**Actor**: `cpt-cf-file-storage-actor-cf-modules`

**Preconditions**:

- File exists at the specified URL

**Main Flow**:

1. Module calls get_metadata with a file URL
2. FileStorage checks authorization for read on `gts.x.fstorage.file`
3. FileStorage returns metadata (name, size, mime_type, owner, availability) without transferring content

**Postconditions**:

- Metadata returned; no content transferred

**Alternative Flows**:

- **File not found**: FileStorage returns file_not_found error
- **Authorization denied**: FileStorage returns access-denied error

#### Direct Upload from External Client

- [ ] `p1` - **ID**: `cpt-cf-file-storage-usecase-direct-upload`

**Actor**: `cpt-cf-file-storage-actor-platform-user`

**Preconditions**:

- Client is authenticated with a valid API token
- Storage backend supports presigned URLs (e.g., S3, GCS, Azure Blob)

**Main Flow**:

1. Client requests a direct transfer URL for upload from FileStorage, providing file metadata (name, mime_type, size)
2. FileStorage checks authorization for write on `gts.x.fstorage.file`
3. *(Phase 2)* FileStorage validates file against tenant policies (type, size); in phase 1 all uploads are accepted
4. FileStorage registers the file metadata and ownership, assigns the target backend path
5. FileStorage generates a presigned upload URL using its own backend credentials (e.g., AWS access key), scoped to the
   assigned path and time-limited
6. FileStorage emits audit record for the upload
7. FileStorage returns the presigned URL and file identifier to the client
8. Client uploads file content directly to the storage backend using the presigned URL
9. Storage backend validates the signature against its own key material and accepts the upload

**Postconditions**:

- File metadata and ownership registered in FileStorage before upload
- File content stored on backend via presigned URL — never transited through FileStorage
- Audit record emitted

**Alternative Flows**:

- **Authorization denied at step 2**: FileStorage returns access-denied error; no presigned URL issued
- *(Phase 2)* **Policy violation at step 3**: FileStorage returns error indicating which policy was violated
- **Presigned URL expired**: Backend rejects the upload; client must request a new presigned URL from FileStorage
- **Backend does not support presigned URLs**: FileStorage falls back to proxied upload (standard `fr-upload-file` flow)

#### Delete a File

- [ ] `p1` - **ID**: `cpt-cf-file-storage-usecase-delete-file`

**Actor**: `cpt-cf-file-storage-actor-platform-user`

**Preconditions**:

- User is authenticated
- User owns the file

**Main Flow**:

1. Owner requests deletion of a file by its identifier
2. FileStorage checks authorization for delete on `gts.x.fstorage.file`
3. FileStorage revokes all active shareable links and signed URLs associated with the file
4. FileStorage deletes the file content from the storage backend
5. FileStorage removes file metadata and ownership records
6. FileStorage emits audit record for the deletion

**Postconditions**:

- File content removed from storage backend
- All associated links invalidated
- Metadata and ownership records removed
- Audit record emitted

**Alternative Flows**:

- **Authorization denied**: FileStorage returns access-denied error
- **File not found**: FileStorage returns file_not_found error
- **Cross-tenant attempt**: FileStorage returns access-denied error (tenant boundary enforcement)

#### Manage Shareable Links

- [ ] `p1` - **ID**: `cpt-cf-file-storage-usecase-manage-links`

**Actor**: `cpt-cf-file-storage-actor-platform-user`

**Preconditions**:

- User is authenticated
- User owns the file

**Main Flow**:

1. Owner requests the list of all active shareable links and signed URLs for a file
2. FileStorage returns the list with each link's scope, expiration, and creation date
3. Owner identifies a link to revoke
4. Owner requests revocation of the link by its identifier
5. FileStorage invalidates the link immediately
6. FileStorage emits audit record for the link revocation

**Postconditions**:

- Revoked link returns access-denied on subsequent access
- Remaining links unaffected
- Audit record emitted

**Alternative Flows**:

- **No active links**: FileStorage returns an empty list
- **Link not found**: FileStorage returns link_not_found error
- **Owner creates a new link**: Owner requests a shareable link with desired scope and expiration; FileStorage creates
  and returns the link URL; audit record emitted for link creation

#### Multi-Backend Deployment

- [ ] `p1` - **ID**: `cpt-cf-file-storage-usecase-backend-config`

**Actor**: `cpt-cf-file-storage-actor-cf-modules`

**Preconditions**:

- FileStorage is deployed with a configured storage backend

**Main Flow**:

1. Deployment A configures FileStorage with an S3-compatible backend (e.g., AWS S3)
2. Deployment B configures FileStorage with a different backend (e.g., Azure Blob Storage)
3. Both deployments expose identical FileStorage SDK and REST API interfaces
4. CyberFabric modules interact with FileStorage through the SDK trait without awareness of the underlying backend
5. Upload, download, delete, metadata, and link operations behave identically regardless of backend

**Postconditions**:

- All functional requirements are met identically across different backend configurations
- Consuming modules require zero code changes when the backend changes

**Alternative Flows**:

- **Backend-specific feature unavailable**: FileStorage degrades gracefully (e.g., direct transfer URLs unavailable
  for backends that do not support presigned URLs; falls back to proxied transfer)

#### Configure Tenant Policy

- [ ] `p2` - **ID**: `cpt-cf-file-storage-usecase-configure-policy`

**Actor**: `cpt-cf-file-storage-actor-platform-user`

**Preconditions**:

- User has tenant administration privileges

**Main Flow**:

1. Tenant admin defines policies: allowed file types, size limits (global and per-type), webhook URLs for file events,
   and permitted sharing models
2. FileStorage validates and stores the policy configuration
3. Subsequent file operations within the tenant are enforced against the policy

**Postconditions**:

- Tenant policy active and enforced on all file operations

**Alternative Flows**:

- **Invalid policy**: FileStorage returns validation error with details

## 9. Acceptance Criteria

- [ ] File upload returns persistent URL and stores metadata (name, size, type, dates, owner)
- [ ] File download returns content with correct metadata
- [ ] File deletion cascades to all associated shareable links
- [ ] Authorization checked for every file operation via Authorization Service
- [ ] Tenant boundary enforced — cross-tenant modification/deletion rejected
- [ ] Shareable links work with public, tenant, and tenant-hierarchy scopes
- [ ] Signed URLs point directly to storage backend, grant download access without authentication, and storage backend
  rejects after expiration
- [ ] File owner can list active shareable links and signed URLs; can revoke shareable links (signed URLs are
  non-revocable)
- [ ] Audit record emitted for every write operation
- [ ] Tenant policies enforce file type and size restrictions on upload
- [ ] Direct transfer presigned URLs allow upload/download directly to/from storage backend without proxying through
  FileStorage
- [ ] Presigned URLs are generated using FileStorage's own backend credentials and validated by the backend via
  signature verification
- [ ] Fallback to proxied transfer when backend does not support presigned URLs
- [ ] file_not_found error returned for non-existent files
- [ ] access_denied error returned for unauthorized operations
- [ ] Metadata-only queries complete without transferring file content
- [ ] File content is immutable — no in-place content update; changes require a new upload
- [ ] Custom metadata is updatable by the file owner; system-managed metadata is not user-updatable
- [ ] Custom metadata update changes the file's last modified date
- [ ] File ownership is immutable after creation
- [ ] FileStorage SDK and REST API behave identically regardless of configured storage backend
- [ ] File listing returns metadata only, is paginated, and requires owner type filter
- [ ] Multipart upload assembles parts into a complete file with correct metadata
- [ ] Upload rejected when declared mime_type does not match actual file content
- [ ] Orphaned backend objects from unconfirmed direct uploads are cleaned up automatically
- [ ] File owner can toggle download availability via metadata update

## 10. Dependencies

| Dependency            | Description                                          | Criticality |
|-----------------------|------------------------------------------------------|-------------|
| ModKit Framework      | Module lifecycle, ClientHub for service registration | p1          |
| Authorization Service | Access decisions for `gts.x.fstorage.file` resources | p1          |
| Audit Infrastructure  | Platform audit event sink                            | p1          |
| Quota Enforcement     | Per-tenant storage quota enforcement (phase 2)       | p2          |

## 11. Assumptions

- Authorization Service is available and supports `gts.x.fstorage.file` resource type
- All file access respects tenant boundaries at the platform level
- Initial storage backend is configured at deployment time; runtime backend switching is phase 2
- File URLs are internal to CyberFabric; external access is via shareable links or signed URLs
- Tenant policy configuration is available to tenant administrators through the platform

## 12. Risks

| Risk                                                                | Impact                                                         | Mitigation                                                                            |
|---------------------------------------------------------------------|----------------------------------------------------------------|---------------------------------------------------------------------------------------|
| Storage service unavailability blocks all file-dependent operations | High — multimodal AI, document workflows disrupted             | Design for graceful degradation; clear error propagation to consumers                 |
| Large file sizes increase request latency for consuming modules     | Medium — slow responses for multimodal and document operations | Metadata pre-fetch enables size validation; streaming support for large files         |
| Signed URL key compromise enables unauthorized file access          | High — data exposure                                           | Key rotation support; short default expiration; shareable links for revocable access  |
| Tenant policy misconfiguration blocks legitimate uploads            | Medium — user frustration                                      | Policy validation on save; clear error messages identifying which policy was violated |

## 13. Open Questions

- What is the maximum supported file size per media type?
- What is the default file retention and expiration policy?
- What is the maximum and default expiration for signed URLs?
- Should file versioning be supported in a future phase?
- What is the key rotation strategy for signed URL generation?

## 14. Traceability

- **Design**: [DESIGN.md](./DESIGN.md)
- **ADRs**: [ADR/](./ADR/)
- **Features**: [features/](./features/)
