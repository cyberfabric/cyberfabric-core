# Multi-stage build for hyperspot-server API backend
# Stage 1: Builder
FROM rust:1.92-bookworm@sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2 AS builder

# Build arguments for additional cargo arguments
ARG CARGO_BUILD_ARGS

# Install protobuf-compiler for prost-build
RUN apt-get update && \
    apt-get install -y --no-install-recommends protobuf-compiler libprotobuf-dev && \
    rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Copy workspace files
COPY Cargo.toml Cargo.lock ./
COPY rust-toolchain.toml ./

# Copy all workspace members
COPY apps ./apps
COPY libs ./libs
COPY modules ./modules
COPY examples ./examples
COPY config ./config
COPY proto ./proto

# Build the hyperspot-server binary in release mode
# Using --bin to build only the specific binary
# Additional cargo args can be customized via CARGO_BUILD_ARGS build arg
RUN if [ -n "$CARGO_BUILD_ARGS" ]; then \
        cargo build --bin hyperspot-server --package=hyperspot-server $CARGO_BUILD_ARGS; \
    else \
        cargo build --bin hyperspot-server --package=hyperspot-server; \
    fi

# Stage 2: Runtime - must match builder's base OS
FROM debian:13.3-slim

WORKDIR /app

# Copy the built binary from builder stage
COPY --from=builder /build/target/debug/hyperspot-server /app/hyperspot-server
# Copy config used in CMD
COPY --from=builder /build/config /app/config

# Expose the HTTP port for E2E tests
EXPOSE 8086

# Run the binary with minimal config suitable for E2E tests
# Using --mock flag to use in-memory SQLite for any modules that need DB
CMD ["/app/hyperspot-server", "--config", "/app/config/quickstart.yaml"]

