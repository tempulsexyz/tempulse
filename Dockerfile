####################################
# Stage 1: Build Rust binaries
####################################
FROM rust:1.92-slim-bookworm AS builder

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy manifests first for dependency caching
COPY Cargo.toml Cargo.lock ./
COPY bin/api/Cargo.toml bin/api/Cargo.toml
COPY bin/fin/Cargo.toml bin/fin/Cargo.toml
COPY crates/core/Cargo.toml crates/core/Cargo.toml
COPY crates/storage/Cargo.toml crates/storage/Cargo.toml
COPY crates/tempo/Cargo.toml crates/tempo/Cargo.toml

# Create stub source files so cargo can download & compile dependencies
RUN mkdir -p bin/api/src bin/fin/src crates/core/src crates/storage/src crates/tempo/src && \
    echo "fn main() {}" > bin/api/src/main.rs && \
    echo "fn main() {}" > bin/fin/src/main.rs && \
    echo "" > crates/core/src/lib.rs && \
    echo "" > crates/storage/src/lib.rs && \
    echo "" > crates/tempo/src/lib.rs

# Build dependencies only (this layer gets cached)
RUN cargo build --release 2>/dev/null || true

# Copy real source code
COPY . .

# Touch source files to force rebuild of our code (not deps)
RUN touch bin/api/src/main.rs bin/fin/src/main.rs \
    crates/core/src/lib.rs crates/storage/src/lib.rs crates/tempo/src/lib.rs

# Build release binaries
RUN cargo build --release

####################################
# Stage 2: Runtime image
####################################
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y ca-certificates libssl3 && rm -rf /var/lib/apt/lists/*

# Copy compiled binaries
COPY --from=builder /app/target/release/api /usr/local/bin/tempulse-api
COPY --from=builder /app/target/release/fin /usr/local/bin/tempulse-fin

# Copy migrations (needed at runtime for sqlx::migrate!)
COPY --from=builder /app/migrations /app/migrations

WORKDIR /app

# Default to running the API server
CMD ["tempulse-api"]
