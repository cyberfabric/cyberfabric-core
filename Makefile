CI := 1

OPENAPI_URL ?= http://127.0.0.1:8087/openapi.json
OPENAPI_OUT ?= docs/api/api.json

DEFAULT_FEATURES ?= users-info-example,static-authn,static-authz

# -------- Utility macros --------

define check_tool
    @command -v $(1) >/dev/null || (echo "ERROR: $(1) is not installed. Run 'make setup' to install required tools." && exit 1)
endef

define check_rustup_component
    @command -v rustup >/dev/null || (echo "ERROR: rustup not installed. Install rustup or run 'make setup'." && exit 1)
	@rustup component list --installed | grep -q '^$(1)' || (echo "ERROR: $(1) component not installed. Run 'rustup component add $(1)' or 'make setup'." && exit 1)
endef

# -------- Defaults --------

# Show the help message with list of commands (default target)
help:
	@python3 scripts/make_help.py Makefile


# -------- Set up --------
# Note: .setup-stamp should be added to .gitignore

.PHONY: setup

## Install all required development tools
setup: .setup-stamp

.setup-stamp:
	@echo "Installing required development tools..."
	rustup component add clippy
	cargo install lychee
	cargo install cargo-geiger
	cargo install cargo-deny
	cargo install cargo-dylint
	cargo install dylint-link
	cargo install cargo-fuzz
	@if echo "$$OS" | grep -iq windows || [ -n "$$COMSPEC" ]; then \
		echo "WARNING: kani-verifier and cargo-llvm-cov installation skipped on Windows."; \
		echo "These tools are not supported on Windows. Use WSL2 or Docker to install instead."; \
	else \
		cargo install --locked kani-verifier && \
		cargo kani setup && \
		cargo install cargo-llvm-cov; \
	fi
	@echo "Setup complete. All tools installed."
	@touch .setup-stamp

# -------- Code formatting --------

.PHONY: fmt

# Check code formatting
fmt:
	$(call check_rustup_component,rustfmt)
	cargo fmt --all -- --check

# -------- Module naming validation --------

.PHONY: validate-module-names

## Validate module folder names follow kebab-case convention
validate-module-names:
	@python3 scripts/validate_module_names.py

# -------- Code safety checks --------
#
# Tool Comparison - What Each Tool Checks:
# +-------------+----------------------------------------------------------------------+
# | Tool        | Checks Performed                                                     |
# +-------------+----------------------------------------------------------------------+
# | clippy      | - Idiomatic Rust patterns (e.g., use of .iter() vs into_iter())      |
# |             | - Common mistakes (e.g., unnecessary clones, redundant closures)     |
# |             | - Performance issues (e.g., inefficient string operations)           |
# |             | - Style violations (e.g., naming conventions, formatting)            |
# |             | - Suspicious constructs (e.g., comparison to NaN, unused results)    |
# |             | - Complexity warnings (e.g., too many arguments, cognitive load)     |
# +-------------+----------------------------------------------------------------------+
# | kani        | - Memory safety proofs (buffer overflows, null pointer dereferences) |
# |             | - Arithmetic overflow/underflow in all possible execution paths      |
# |             | - Assertion violations (panics, unwrap failures)                     |
# |             | - Undefined behavior detection                                       |
# |             | - Concurrency issues (data races, deadlocks) with #[kani::proof]     |
# |             | - Custom invariants and postconditions verification                  |
# +-------------+----------------------------------------------------------------------+
# | geiger      | - Unsafe blocks in your code and dependencies                        |
# |             | - FFI (Foreign Function Interface) calls                             |
# |             | - Raw pointer dereferences                                           |
# |             | - Mutable static variables access                                    |
# |             | - Inline assembly usage                                              |
# |             | - Dependency tree visualization of unsafe code usage                 |
# +-------------+----------------------------------------------------------------------+
# | lint        | - Compiler warnings treated as errors (unused variables, imports)    |
# |             | - Dead code detection                                                |
# |             | - Type inference failures                                            |
# |             | - Deprecated API usage                                               |
# |             | - Missing documentation warnings                                     |
# |             | - Ensures clean compilation across all targets and features          |
# +-------------+----------------------------------------------------------------------+
# | dylint      | - Project-specific architectural conventions (custom lints)          |
# |             | - DTO declaration and placement (only in api/rest folders)           |
# |             | - DTO isolation (no references from domain/contract layers)          |
# |             | - API endpoint versioning requirements (e.g., /users/v1/users)       |
# |             | - Contract layer purity (no serde, HTTP types, or ToSchema)          |
# |             | - Layer separation and dependency rules enforcement                  |
# |             | - Use 'make dylint-list' to see all available custom lints           |
# +-------------+----------------------------------------------------------------------+

.PHONY: clippy lychee kani geiger safety lint dylint dylint-list dylint-test gts-docs gts-docs-vendor gts-docs-release gts-docs-vendor-release gts-docs-test cypilot-validate

# Run clippy linter (excludes gts-rust submodule which has its own lint settings)
clippy:
	$(call check_rustup_component,clippy)
	cargo clippy --workspace --all-targets --all-features -- -D warnings -D clippy::perf

# Validate cypilot artifacts (specs, code, templates)
cypilot-validate:
	@if [ ! -d .cypilot/.git ] && [ ! -f .cypilot/.git ]; then \
		echo "Initializing .cypilot submodule (first run)"; \
		git submodule update --init --recursive -- .cypilot; \
	elif git -C .cypilot symbolic-ref -q HEAD >/dev/null 2>&1; then \
		echo "Skipping .cypilot update (branch checkout detected)"; \
	else \
		echo "Updating .cypilot via git submodule update (detached HEAD)"; \
		git submodule update --init --recursive -- .cypilot; \
	fi
	@python3 .cypilot/skills/cypilot/scripts/cypilot.py validate && echo "OK. cypilot validation PASSED" || (echo "ERROR: cypilot validation FAILED"; exit 1)

# Run markdown checks with 'lychee'
lychee:
	$(call check_tool,lychee)
	lychee docs examples dylint_lints guidelines

## The Kani Rust Verifier for checking safety of the code
kani:
	$(call check_tool,kani)
	cargo kani --workspace --all-features

## Run Geiger scanner for unsafe code in dependencies
geiger:
	$(call check_tool,cargo-geiger)
	cd apps/hyperspot-server && cargo geiger --all-features

## Check there are no compile time warnings
lint:
	RUSTFLAGS="-D warnings" cargo check --workspace --all-targets --all-features

## Validate GTS identifiers in .md and .json files (DE0903)
# Uses gts-docs-validator from apps/gts-docs-validator
# Vendor enforcement is available via the gts-docs-vendor target (--vendor x)

# REDUCING THE SCOPE OF THE VALIDATION UNTIL IT IS STABLE
gts-docs:
	cargo run -p gts-docs-validator -- \
		--exclude "target/*" \
		--exclude "docs/api/*" \
		--exclude "*.md" \
		--exclude "*.json" \
		docs modules libs examples

## Validate GTS docs with vendor check (ensures all IDs use vendor "x")
gts-docs-vendor:
	cargo run -p gts-docs-validator -- \
		--vendor x \
		--exclude "target/*" \
		--exclude "docs/api/*" \
		docs modules libs examples

## Validate GTS identifiers (release build)
gts-docs-release:
	cargo run --release -p gts-docs-validator -- \
		--exclude "target/*" \
		--exclude "docs/api/*" \
		docs modules libs examples

## Validate GTS docs with vendor check (release build)
gts-docs-vendor-release:
	cargo run --release -p gts-docs-validator -- \
		--vendor x \
		--exclude "target/*" \
		--exclude "docs/api/*" \
		docs modules libs examples

## Run tests for GTS documentation validator
gts-docs-test:
	cargo test -p gts-docs-validator

## List all custom project compliance lints (see dylint_lints/README.md)
dylint-list:
	@cd dylint_lints && \
	DYLINT_LIBS=$$(find target/release -maxdepth 1 \( -name "libde*@*.so" -o -name "libde*@*.dylib" -o -name "de*@*.dll" \) -type f | sort -u); \
	if [ -z "$$DYLINT_LIBS" ]; then \
		echo "ERROR: No dylint libraries found. Run 'make dylint' first to build them."; \
		exit 1; \
	fi; \
	for lib in $$DYLINT_LIBS; do \
		echo "=== $$lib ==="; \
		cargo dylint list --lib-path "$$lib"; \
	done

## Test dylint lints on UI test cases (compile and verify violations)
dylint-test:
	@cd dylint_lints && cargo test

# Run project compliance dylint lints on the workspace (see `make dylint-list`)
dylint:
	$(call check_tool,cargo-dylint)
	$(call check_tool,dylint-link)
	cargo +nightly-2025-09-18 dylint --all --workspace

# Run all code safety checks
safety: clippy kani lint dylint # geiger
	@echo "OK. Rust Safety Pipeline complete"

# -------- Code security checks --------

.PHONY: deny security

## Check licenses and dependencies
deny:
	$(call check_tool,cargo-deny)
	cargo deny check

security: deny

# -------- API and docs --------

.PHONY: openapi

# Generate OpenAPI spec from running hyperspot-server
openapi:
	@command -v curl >/dev/null || (echo "curl is required to generate OpenAPI spec" && exit 1)
	@echo "Starting hyperspot-server to generate OpenAPI spec..."
	# Run server in background
	cargo run --bin hyperspot-server --features users-info-example,static-authn,static-authz -- --config config/quickstart.yaml &
	@SERVER_PID=$$!; \
	trap 'kill $$SERVER_PID >/dev/null 2>&1 || true' EXIT; \
	echo "hyperspot-server PID: $$SERVER_PID"; \
	echo "Waiting for $(OPENAPI_URL) to become ready..."; \
	ELAPSED=0; MAX_WAIT=300; SLEEP=1; \
	while ! curl -fsS "$(OPENAPI_URL)" -o /dev/null >/dev/null 2>&1; do \
		if [ $$ELAPSED -ge $$MAX_WAIT ]; then \
			echo "ERROR: hyperspot-server did not become ready in time"; exit 1; \
		fi; \
		echo "Waiting for hyperspot-server... ($$ELAPSED s)"; \
		sleep $$SLEEP; \
		ELAPSED=$$((ELAPSED + SLEEP)); \
		SLEEP=$$((SLEEP < 8 ? SLEEP*2 : 8)); \
	done; \
	echo "Server is ready, fetching OpenAPI spec..."; \
	mkdir -p $$(dirname "$(OPENAPI_OUT)"); \
	curl -fsS "$(OPENAPI_URL)" -o "$(OPENAPI_OUT)"; \
	echo "OpenAPI spec saved to $(OPENAPI_OUT)"; \
	echo "Stopping hyperspot-server..."; \
	kill $$SERVER_PID >/dev/null 2>&1 || true; \
	wait $$SERVER_PID 2>/dev/null || true

# -------- Development and auto fix --------

.PHONY: dev dev-fmt dev-clippy dev-test

## Run tests in development mode
dev-test:
	cargo test --workspace

## Auto-fix code formatting
dev-fmt:
	cargo fmt --all

## Auto-fix clippy warnings
dev-clippy:
	cargo clippy --workspace --all-targets --all-features --fix --allow-dirty

# Auto-fix formatting and clippy warnings
dev: dev-fmt dev-clippy dev-test

# -------- Tests --------

.PHONY: test test-no-macros test-macros test-sqlite test-pg test-mysql test-db test-users-info-pg

# Run all tests
test:
	cargo test --workspace

test-no-macros:
	cargo test --workspace --exclude cf-modkit-macros-tests --exclude cf-modkit-db-macros

test-macros:
	cargo test -p cf-modkit-db-macros
	cargo test -p cf-modkit-macros-tests

## Run SQLite integration tests
test-sqlite:
	cargo test -p cf-modkit-db --features sqlite,integration

## Run PostgreSQL integration tests
test-pg:
	cargo test -p cf-modkit-db --features pg,integration

## Run MySQL integration tests
test-mysql:
	cargo test -p cf-modkit-db --features mysql,integration

# Run all database integration tests
test-db: test-sqlite test-pg test-mysql

## Run users-info module integration tests
test-users-info-pg:
	cargo test -p users-info --features "integration" -- --nocapture

# -------- E2E tests --------

.PHONY: e2e e2e-local e2e-docker e2e-docker-pg e2e-docker-mysql e2e-smoke

# Run E2E tests in Docker (default)
e2e: e2e-docker

## Run E2E smoke tests in Docker with sqlite (only tests marked @pytest.mark.smoke)
e2e-smoke:
	python3 scripts/ci.py e2e --docker --cargo-build-args="--features $(DEFAULT_FEATURES)" $(E2E_ARGS) -- -m smoke

# Run E2E tests locally
e2e-local:
	python3 scripts/ci.py e2e

## Run E2E tests in Docker environment (default: sqlite)
e2e-docker:
	python3 scripts/ci.py e2e --docker --cargo-build-args="--features $(DEFAULT_FEATURES)" $(E2E_ARGS)

## Run E2E tests in Docker environment with PostgreSQL
e2e-docker-pg:
	python3 scripts/ci.py e2e --docker --cargo-build-args="--features db-pg,$(DEFAULT_FEATURES)" --profile postgres $(E2E_ARGS)

## Run E2E tests in Docker environment with MariaDB
e2e-docker-mysql:
	python3 scripts/ci.py e2e --docker --cargo-build-args="--features db-mysql,$(DEFAULT_FEATURES)" --profile mariadb $(E2E_ARGS)

# -------- Code coverage --------

.PHONY: coverage coverage-unit coverage-e2e-local check-prereq-e2e-local

# Generate code coverage report (unit + e2e-local tests)
coverage:
	$(call check_tool,cargo-llvm-cov)
	python3 scripts/coverage.py combined

# Generate code coverage report (unit tests only)
coverage-unit:
	$(call check_tool,cargo-llvm-cov)
	python3 scripts/coverage.py unit

## Ensure needed packages and programs installed for local e2e testing
check-prereq-e2e-local:
	python scripts/check_local_env.py --mode e2e-local

# Generate code coverage report (e2e-local tests only)
coverage-e2e-local: check-prereq-e2e-local
	$(call check_tool,cargo-llvm-cov)
	python3 scripts/coverage.py e2e-local

# -------- Fuzzing --------

.PHONY: fuzz fuzz-build fuzz-list fuzz-run fuzz-clean fuzz-corpus

## Check cargo-fuzz is installed (required for fuzzing)
fuzz-install:
	$(call check_tool,cargo-fuzz)

## Build all fuzz targets
fuzz-build: fuzz-install
	cd fuzz && cargo +nightly fuzz build

## List all available fuzz targets
fuzz-list: fuzz-install
	cd fuzz && cargo +nightly fuzz list

## Run a specific fuzz target (use FUZZ_TARGET=name)
## Example: make fuzz-run FUZZ_TARGET=fuzz_odata_filter FUZZ_SECONDS=60
fuzz-run: fuzz-install
	@if [ -z "$(FUZZ_TARGET)" ]; then \
		echo "ERROR: FUZZ_TARGET is required. Example: make fuzz-run FUZZ_TARGET=fuzz_odata_filter"; \
		exit 1; \
	fi
	cd fuzz && cargo +nightly fuzz run $(FUZZ_TARGET) -- -max_total_time=$(or $(FUZZ_SECONDS),60)

## Run all fuzz targets for a short time (smoke test)
fuzz: fuzz-build
	@echo "Running all fuzz targets for 30 seconds each..."
	@cd fuzz && \
	FAILED=0; \
	for target in $$(cargo +nightly fuzz list); do \
		echo "=== Fuzzing $$target ==="; \
		cargo +nightly fuzz run $$target -- -max_total_time=30 || FAILED=1; \
	done; \
	if [ $$FAILED -ne 0 ]; then \
		echo "Fuzzing found crashes. Check fuzz/artifacts/ for details."; \
		exit 1; \
	fi
	@echo "Fuzzing complete. No crashes found."

## Clean fuzzing artifacts and corpus
fuzz-clean:
	rm -rf fuzz/artifacts/
	rm -rf fuzz/corpus/*/
	rm -rf fuzz/target/

## Minimize corpus for a specific target
fuzz-corpus: fuzz-install
	@if [ -z "$(FUZZ_TARGET)" ]; then \
		echo "ERROR: FUZZ_TARGET is required. Example: make fuzz-corpus FUZZ_TARGET=fuzz_odata_filter"; \
		exit 1; \
	fi
	cd fuzz && cargo +nightly fuzz cmin $(FUZZ_TARGET)

# -------- Main targets --------

.PHONY: all check ci build quickstart example

# Start server with quickstart config
quickstart:
	mkdir -p data
	cargo run --bin hyperspot-server -- --config config/quickstart.yaml run

## Run server with example module
example:
	cargo run --bin hyperspot-server --features users-info-example,static-authn,static-authz -- --config config/quickstart.yaml run

oop-example:
	cargo build -p calculator --features oop_module
	cargo run --bin hyperspot-server --features oop-example,users-info-example,static-authn,static-authz -- --config config/quickstart.yaml run

# Run all quality checks
check: .setup-stamp fmt cypilot-validate clippy lychee security dylint-test dylint gts-docs test

ci_test: fmt clippy

ci_docs: lychee

# Run CI pipeline locally, requires docker
ci: fmt clippy test-no-macros test-macros test-db deny test-users-info-pg lychee dylint dylint-test

# Make a release build using stable toolchain
build:
	cargo +stable build --release

# Run all necessary quality checks and tests and then build the release binary
all: build check test-sqlite e2e-local
	@echo "consider to run 'make test-db' as well"
