# =============================================================================
# Stage 1: Build Rust binaries
# =============================================================================
FROM --platform=linux/amd64 rust:1.85-bookworm AS builder

WORKDIR /app

# Cache dependencies: copy manifests and build a dummy project first.
# This means changing source code won't re-download all dependencies.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && \
    echo "fn main() {}" > src/main.rs && \
    echo "" > src/lib.rs && \
    mkdir -p src/bin && \
    echo "fn main() {}" > src/bin/server.rs && \
    echo "fn main() {}" > src/bin/receiver.rs && \
    cargo build --release 2>/dev/null || true && \
    rm -rf src target/release/.fingerprint/chunkstream* \
           target/release/deps/chunkstream* \
           target/release/chunkstream*

# Copy actual source and build both binaries
COPY src/ src/
COPY examples/ examples/
RUN cargo build --release --bin chunkstream-server --bin chunkstream-receiver

# =============================================================================
# Stage 2: Minimal runtime image
# =============================================================================
FROM --platform=linux/amd64 debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy both binaries from builder
COPY --from=builder /app/target/release/chunkstream-server /usr/local/bin/
COPY --from=builder /app/target/release/chunkstream-receiver /usr/local/bin/

# Create directory for received files
RUN mkdir -p /app/received

# Default to server; docker-compose overrides per service
CMD ["chunkstream-server"]
