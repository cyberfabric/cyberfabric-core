# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Cyber Fabric (formerly HyperSpot) is a modular, high-performance platform for building AI services in Rust. Built on the **ModKit** framework, it emphasizes:

- **Modular architecture** — Everything is a Module with composable, independent units
- **Type safety** — Compile-time guarantees via typestate builders and trait-based APIs
- **Multi-tenancy** — Built-in tenant isolation with secure ORM layer
- **Database agnostic** — PostgreSQL, MySQL, SQLite via unified API
- **GTS extensibility** — Global Type System for versioned, pluggable extensions

## Essential Commands

### Development (daily workflow)

```bash
# Full dev cycle: auto-format, auto-fix clippy, run tests
make dev

# Individual steps
make dev-fmt             # Auto-format code
make dev-clippy          # Auto-fix clippy warnings
make dev-test            # Run tests

# Database integration tests
make test-db             # All database integration tests
make test-sqlite         # SQLite only
make test-pg             # PostgreSQL only
make test-mysql          # MySQL only

# E2E tests
make e2e-local           # Local server (requires Python deps)
make e2e-docker          # Docker environment

# Ad-hoc (when make targets don't cover the use case)
cargo build                       # Debug build
cargo test -p <package>           # Single package tests
cargo test -- --nocapture         # Show output
cargo test -- --test-threads=1    # Single-threaded
cargo test -- --ignored           # Run ignored tests
```

### CI & Quality Checks

```bash
# Full CI pipeline (format check, lint, test, security)
make ci
make check               # Same as ci

# Individual CI checks (strict mode — check only, no auto-fix)
make fmt                 # Check formatting
make clippy              # Lint (deny warnings)
make test                # All tests
make build               # Release build

# Architecture and security
make dylint              # Custom architecture lints (see make dylint-list)
make deny                # License and dependency checks
make safety              # All safety checks (clippy + kani + lint + dylint)
make geiger              # Unsafe code scanner
make lychee              # Markdown link checker
```

### Running the Server

```bash
make quickstart          # Quick start with SQLite
make example             # With example modules (users-info, tenant-resolver)

# Ad-hoc: custom config or database backend
cargo run --bin hyperspot-server -- --config <config.yaml> run
cargo run --bin hyperspot-server -- --config <config.yaml> --mock run  # Mock DB

# Health check / OpenAPI docs
curl http://127.0.0.1:8087/health
curl http://127.0.0.1:8087/openapi.json
# Browser: http://127.0.0.1:8087/docs
```

### Other

```bash
make coverage            # Code coverage (unit + e2e-local)
make coverage-unit       # Unit tests only
make openapi             # Generate OpenAPI spec (requires running server)
make setup               # Install all dev tools
make fuzz                # Smoke-test all fuzz targets
make fuzz-run FUZZ_TARGET=<target>  # Run specific fuzz target
make gts-docs            # Validate GTS identifiers in docs
make dylint-list         # List all custom architecture lints
make dylint-test         # Test lint UI cases
```

## Architecture Overview

### Monorepo Structure

```
hyperspot/
├── apps/                    # Binaries
│   ├── hyperspot-server/   # Main server binary
│   └── gts-docs-validator/ # GTS identifier validation tool
├── libs/                    # Shared libraries (ModKit framework, cf-* crate prefix)
│   ├── modkit/             # Core module framework
│   ├── modkit-db/          # Database abstraction + multi-tenancy
│   ├── modkit-auth/        # Authentication utilities
│   ├── modkit-errors/      # Standardized error handling
│   ├── modkit-http/        # HTTP utilities and client
│   ├── modkit-odata/       # OData pagination/filtering
│   ├── modkit-sdk/         # SDK utilities for module clients
│   ├── modkit-security/    # Security context and primitives
│   ├── modkit-transport-grpc/ # gRPC transport layer
│   ├── system-sdks/        # System module SDKs (directory)
│   └── modkit-*/           # Other framework components (macros, utils, node-info)
├── modules/                # Business logic modules
│   ├── system/             # System modules
│   │   ├── api-gateway/        # HTTP server (owns Axum router)
│   │   ├── grpc-hub/           # gRPC gateway
│   │   ├── module-orchestrator/ # Module lifecycle management
│   │   ├── nodes-registry/     # Node information registry (+SDK)
│   │   ├── tenant-resolver/    # Tenant resolution system (gateway + plugins)
│   │   ├── types-registry/     # GTS type system registry (+SDK, openspec)
│   │   └── oagw/               # OAGW module (design phase)
│   ├── file-parser/        # Document parsing module
│   ├── simple-user-settings/ # User settings module (+SDK)
│   ├── file-storage/       # File storage module (planned)
│   ├── llm-gateway/        # LLM gateway module (planned, SDK exists)
│   └── model-registry/     # Model registry module (planned)
├── examples/               # Example implementations
│   ├── modkit/            # Basic examples (users-info)
│   ├── oop-modules/       # Out-of-process examples (calculator)
│   └── plugin-modules/    # Plugin architecture examples
├── dylint_lints/          # Custom linters (separate workspace)
├── fuzz/                  # Fuzzing targets and corpus (separate workspace)
├── proto/                 # Protocol buffer definitions
├── scripts/               # CI and build automation (Python)
├── testing/               # E2E tests (Python/pytest) and Docker environment
├── guidelines/            # DNA development standards
│   └── DNA/              # Stack-agnostic norms (submodule)
├── docs/                  # Architecture docs, ADRs, spec templates
└── config/               # Configuration files
```

**Note:** Crate names use the `cf-` prefix (e.g., `cf-modkit`, `cf-modkit-db`). Rust edition 2024, minimum toolchain 1.92.0.

### ModKit Framework

ModKit is the core framework providing:

- **Module discovery** — Automatic via `inventory` crate (compile-time registration)
- **Lifecycle management** — Topological initialization based on dependencies
- **Typed ClientHub** — Type-safe inter-module communication via trait resolution
- **REST builder** — Type-state `OperationBuilder` for compile-time route safety
- **Database abstraction** — SeaORM/SQLx with multi-tenant security
- **Background tasks** — `WithLifecycle<T>` for stateful modules

**Module declaration:**

```rust
#[modkit::module(
    name = "my_module",
    deps = ["dependency_module"],
    capabilities = [db, rest, stateful],
    client = contract::client::MyModuleApi,
    lifecycle(entry = "serve", stop_timeout = "30s", await_ready)
)]
pub struct MyModule;
```

### DDD-Light Layer Architecture

Each module follows strict layering enforced by custom dylint linters:

```
modules/<module-name>/src/
├── lib.rs              # Public exports
├── module.rs           # Module trait implementations
├── config.rs           # Typed configuration
├── contract/           # PUBLIC API (inter-module communication)
│   ├── client.rs       # Trait definitions for ClientHub
│   ├── model.rs        # Transport-agnostic domain models
│   └── error.rs        # Domain errors
├── api/                # TRANSPORT ADAPTERS
│   └── rest/           # HTTP layer
│       ├── dto.rs      # DTOs with serde/utoipa (REST-specific)
│       ├── handlers.rs # Axum handlers
│       └── routes.rs   # OperationBuilder registration
├── domain/             # BUSINESS LOGIC
│   ├── service.rs      # Orchestration and business rules
│   ├── model.rs        # Rich domain models
│   └── error.rs        # Internal errors
└── infra/              # INFRASTRUCTURE
    └── storage/        # Database layer
        ├── entity.rs   # SeaORM entities
        ├── mapper.rs   # Entity <-> Contract conversions
        └── migrations/ # Database migrations
```

**Critical separation rules (enforced by linters):**

1. **Contract layer** — NO serde, NO utoipa, NO HTTP types (pure domain)
2. **API/REST layer** — DTOs MUST have serde + utoipa, MUST be in `api/rest/`
3. **REST endpoints** — MUST follow `/{service-name}/v{N}/{resource}` pattern
4. **DTO isolation** — DTOs only referenced within `api/rest/`, not from domain/contract

### Multi-Tenancy & Security

**Secure ORM Layer** (`libs/modkit-db/src/secure/`):

- **Typestate enforcement** — Prevents unscoped queries at compile time
- **Request-scoped security** — Security context passed per-operation
- **Implicit deny-all** — Empty scope = `WHERE 1=0`
- **Automatic isolation** — Tenant and resource-level filtering

```rust
// Define secure entity
#[derive(DeriveEntityModel, Scopable)]
#[sea_orm(table_name = "users")]
#[secure(tenant_col = "tenant_id", resource_col = "id")]
pub struct Model { /* ... */ }

// Request-scoped query
let ctx = SecurityCtx::for_tenants(vec![tenant_id], user_id);
let users = secure_conn.find::<user::Entity>(&ctx)?.all(conn).await?;
```

### Inter-Module Communication

**ClientHub Pattern:**

- Type-safe resolution by trait: `hub.get::<dyn MyApi>()?`
- Scoped clients for plugins: `hub.get_scoped::<dyn PluginApi>(&scope)?`
- Zero-cost abstraction (direct Arc clones)

```rust
// Provider module (in init):
ctx.client_hub().register::<dyn MyModuleApi>(Arc::new(client));

// Consumer module:
let api = ctx.client_hub().get::<dyn MyModuleApi>()?;
```

**Gateway + Plugin Architecture:**

- **Gateway** — Exposes public API, routes to selected plugin
- **Plugins** — Implement plugin trait, register with GTS instance ID
- See `docs/MODKIT_PLUGINS.md` for details

### Out-of-Process Modules

Modules can run as separate processes with gRPC communication:

```yaml
modules:
  calculator:
    runtime:
      type: oop
      execution:
        executable_path: "~/.hyperspot/bin/calculator-oop.exe"
        args: []
        environment:
          RUST_LOG: "info"
```

**SDK Pattern** — Each OoP module has a `-sdk` crate containing:
- API trait + types
- gRPC proto stubs
- gRPC client implementation
- Wiring helpers (`wire_client()`, `build_client()`)

See `examples/oop-modules/calculator/` for reference implementation.

### REST API Conventions

**Type-Safe OperationBuilder:**

```rust
OperationBuilder::get("/my-service/v1/users")
    .operation_id("my_module.list")
    .summary("List users")
    .tag("my_module")
    .json_response_with_schema::<Vec<UserDto>>(openapi, 200, "Success")
    .problem_response(openapi, 400, "Bad Request")
    .handler(get(list_users_handler))
    .register(router, openapi);
```

**Endpoint structure:** `/{service-name}/v{N}/{resource}`
- Service name: kebab-case
- Version: `v1`, `v2`, etc. (REQUIRED)
- Resource: kebab-case with path params `{id}`

**Error Handling:** RFC-9457 Problem Details

```rust
use modkit::api::problem::{Problem, bad_request, conflict, internal_error};

async fn handler() -> Result<Json<T>, Problem> {
    Err(bad_request("Invalid input"))
}
```

**Use `.standard_errors(openapi)` to add all common error responses (400, 401, 403, 404, 409, 422, 429, 500).**

### Database Management

**Configuration:**

```yaml
database:
  servers:
    sqlite_main:
      file: "database/database.db"
      params:
        WAL: "true"
      pool:
        max_conns: 10
        busy_timeout_ms: 5000

modules:
  my_module:
    database:
      server: "sqlite_main"
```

**Environment overrides:**
```bash
export HYPERSPOT_DATABASE_URL="postgres://user:pass@localhost/db"
export HYPERSPOT_MODULES_API_INGRESS_BIND_ADDR="0.0.0.0:8080"
```

**Migrations:**

```rust
impl DbModule for MyModule {
    async fn migrate(&self, db: &DbHandle) -> anyhow::Result<()> {
        // SeaORM migrations in infra/storage/migrations/
        Ok(())
    }
}
```

### GTS (Global Type System)

The types-registry module provides a powerful extension point:

- **Versioned type definitions** — Shared schemas with backward compatibility
- **Runtime discovery** — New types and implementations
- **Plugin identification** — GTS instance IDs for scoped resolution

**GTS Instance ID format:**
```
gts.x.core.modkit.plugin.v1~<vendor>.<package>.<module>.plugin.v1~
```

See `modules/system/types-registry/` for implementation.

### OData Pagination & Filtering

**Type-safe OData system** with three layers:

1. **DTO Layer** — `#[derive(ODataFilterable)]` generates FilterField enum
2. **Domain Layer** — `FilterNode<F>` AST (transport-agnostic)
3. **Infrastructure** — Map to SeaORM via `ODataFieldMapping`

```rust
// DTO with filterable fields
#[derive(ODataFilterable)]
pub struct UserDto {
    #[odata(filter(kind = "Uuid"))]
    pub id: Uuid,
    #[odata(filter(kind = "String"))]
    pub email: String,
    // No #[odata] = not filterable
    pub display_name: String,
}

// In handler
async fn list(OData(q): OData) -> Result<Json<Page<UserDto>>, Problem> {
    let page = service.list_users(q.filter, q.order, q.limit).await?;
    Ok(Json(page))
}
```

See `docs/modkit_unified_system/07_odata_pagination_select_filter.md` for complete documentation.

## Code Quality Standards

### DNA Guidelines

Stack-agnostic development norms in `guidelines/DNA/`:

- **REST API Design** — `guidelines/DNA/REST/API.md`
  - Status codes, pagination, filtering, errors
  - `field.op=value` syntax (e.g., `status.in=open,urgent`)
  - Cursor-based pagination with `limit`, `after`, `before`
  - ETags for concurrency control
  - Idempotency-Key header

- **Rust Conventions** — `guidelines/DNA/languages/RUST.md`
  - Line length: 100 chars max
  - Indentation: 4 spaces
  - Trailing commas required in multi-line expressions

- **Module Development** — `guidelines/NEW_MODULE.md`
  - Step-by-step production-grade module creation

### Custom Linters (dylint)

**Architecture enforcement** in `dylint_lints/`:

- **DE01xx** — Contract layer purity (no serde, no utoipa, no HTTP types, no GTS schema_for)
- **DE02xx** — API layer conventions (DTOs in api/rest/, must have serde+utoipa+ToSchema)
- **DE05xx** — Client layer rules (plugin client suffix, client versioning)
- **DE08xx** — REST endpoint versioning, OData usage, snake_case enforcement
- **DE09xx** — GTS layer rules (string patterns, schema_for restrictions)
- **DE13xx** — Common patterns (no print macros)

View all lints: `make dylint-list`

### Clippy Configuration

Strict clippy rules in `Cargo.toml` workspace lints:
- Pedantic mode enabled
- 140+ additional lints (async safety, performance, complexity)
- `unwrap_used` and `expect_used` denied (use proper error handling)

### Testing Strategy

Target: **90%+ code coverage**

- **Unit tests** — Domain logic, mappers, utilities
- **Integration tests** — Database interactions, module wiring
- **E2E tests** — Full request flows in `testing/e2e/` (Python/pytest)
- **Fuzz tests** — OData parser fuzzing in `fuzz/` (cargo-fuzz + ClusterFuzzLite CI)
- Use `tracing-test` for capturing logs in tests
- Use `testcontainers` for database integration tests

## Common Patterns

### Error Handling

```rust
use modkit::api::problem::{Problem, bad_request, not_found, internal_error};

// In handlers
async fn handler() -> Result<Json<T>, Problem> {
    match service.operation().await {
        Ok(result) => Ok(Json(result)),
        Err(DomainError::NotFound) => Err(not_found("Resource not found")),
        Err(DomainError::Invalid(msg)) => Err(bad_request(msg)),
        Err(e) => {
            tracing::error!("Operation failed: {}", e);
            Err(internal_error("Internal server error"))
        }
    }
}
```

### Conversions (Entity ↔ Domain ↔ DTO)

```rust
// Entity -> Domain
impl From<UserEntity> for User {
    fn from(e: UserEntity) -> Self {
        Self {
            id: e.id,
            email: e.email,
            // ...
        }
    }
}

// Domain -> DTO
impl From<User> for UserDto {
    fn from(u: User) -> Self {
        Self {
            id: u.id,
            email: u.email,
            // ...
        }
    }
}

// Usage
let user: User = entity.into();
let dto: UserDto = user.into();
```

### Module State (ArcSwap for read-heavy)

```rust
use arc_swap::{ArcSwap, ArcSwapOption};

#[modkit::module(name = "my_module", capabilities = [rest])]
pub struct MyModule {
    service: ArcSwapOption<MyService>,
}

impl Module for MyModule {
    async fn init(&self, ctx: &ModuleCtx) -> anyhow::Result<()> {
        let service = Arc::new(MyService::new(ctx.db.clone()));
        self.service.store(Some(service));
        Ok(())
    }
}
```

### File Uploads

```rust
// Multipart form upload
OperationBuilder::post("/files/v1/upload")
    .multipart_file_request("file", Some("File to upload"))
    .handler(upload_handler)
    .register(router, openapi);

// Raw binary upload
OperationBuilder::post("/files/v1/upload")
    .octet_stream_request(Some("Raw file bytes"))
    .handler(upload_binary_handler)
    .register(router, openapi);
```

### Server-Sent Events (SSE)

```rust
use modkit::SseBroadcaster;

// Create broadcaster
let broadcaster = SseBroadcaster::<UserEvent>::new(1024);

// Register route
OperationBuilder::get("/users/v1/events")
    .sse_json::<UserEvent>(openapi, "Real-time user events")
    .handler(events_handler)
    .register(router, openapi)
    .layer(Extension(broadcaster));

// Handler
async fn events_handler(
    Extension(sse): Extension<SseBroadcaster<UserEvent>>,
) -> Sse<impl Stream<Item=Result<Event, Infallible>>> {
    sse.sse_response()
}
```

## Important Notes

### Avoid Common Pitfalls

1. **Don't use serde/utoipa in contract layer** — Linter will catch this (DE0101, DE0102)
2. **Always version REST endpoints** — `/service/v1/resource`, not `/service/resource` (DE0801)
3. **DTOs only in api/rest/** — Not in domain or contract (DE0201)
4. **Use Problem for errors** — Not raw StatusCode tuples
5. **No unwrap/expect** — Use proper Result types (clippy denies this)
6. **Request-scoped security** — Don't store SecurityCtx in services

### Module Lifecycle Order

```
Stopped → init() → migrate() → register_rest() → start() → Running → stop() → Stopped
```

Dependencies initialize first (topological sort).

### Configuration Precedence

1. YAML config file (`--config`)
2. Environment variables (`HYPERSPOT_*` prefix)
3. Default values in code

### Documentation

- **Architecture** — `docs/ARCHITECTURE_MANIFEST.md`
- **ModKit Guide** — `docs/modkit_unified_system/README.md` (10-part guide)
- **Plugin System** — `docs/MODKIT_PLUGINS.md`
- **Secure ORM** — `docs/modkit_unified_system/06_secure_orm_db_access.md`
- **Authorization** — `docs/arch/authorization/` (design, ADRs, tenant model)
- **New Module** — `guidelines/NEW_MODULE.md`
- **Spec Templates** — `docs/spec-templates/` (ADR, DESIGN, PRD, FEATURE templates)
- **Releasing** — `docs/RELEASING.md` (release-plz automation)
- **AI Agent Instructions** — `AGENTS.md`
- **Contributing** — `CONTRIBUTING.md`

### Tools Used

- **cargo-llvm-cov** — Code coverage
- **cargo-deny** — License/dependency checks
- **cargo-audit** — Security vulnerabilities
- **dylint** — Custom architecture lints
- **cargo-fuzz** — Fuzzing (targets in `fuzz/`)
- **kani** — Rust formal verification
- **lychee** — Markdown link checking
- **cargo-geiger** — Unsafe code detection
- **release-plz** — Automated versioning and crates.io publishing
- **utoipa** — OpenAPI 3.1 generation
- **testcontainers** — Integration test databases

## Development Tips

- Use `make dev` for quick auto-fix + test cycle
- Check `make help` for all available commands
- Run `make dylint-list` to understand architecture constraints
- Read module examples in `examples/modkit/users-info/` for canonical patterns
- For OoP modules, see `examples/oop-modules/calculator/` SDK pattern
- Use `tracing::info!(field = value, "message")` for structured logging
- Prefer `ArcSwap` over `RwLock` for read-heavy shared state
- Keep handlers thin — business logic belongs in domain services

## Git Workflow

### Commit Convention

Format: `<type>(<scope>): <description>`

Types: `feat`, `fix`, `tech`, `cleanup`, `refactor`, `test`, `docs`, `style`, `chore`, `perf`, `ci`, `build`, `revert`, `security`, `breaking`

**Developer Certificate of Origin (DCO) required:**
```bash
git commit -s -m "feat(api): add user authentication"
# Or enable globally:
git config --global format.signoff true
```

### CI Checks

All PRs must pass:
- `make fmt`
- `make clippy`
- `make test`
- `make deny`
- `make dylint`
- `make lychee`

Additional CI workflows: CodeQL, ClusterFuzzLite, Scorecard, API contracts, release-plz.

Run locally: `make ci` or `make check` (runs all of the above)
