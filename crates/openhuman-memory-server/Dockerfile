# syntax=docker/dockerfile:1

# ── Build stage ───────────────────────────────────────────────────────────────
FROM rust:1.82-slim AS builder

WORKDIR /build

# Install system deps needed by rusqlite (bundled) and openssl
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy workspace manifests first so dependency compilation is cached.
COPY Cargo.toml Cargo.lock ./
COPY crates/openhuman-embeddings/Cargo.toml crates/openhuman-embeddings/
COPY crates/openhuman-memory-types/Cargo.toml crates/openhuman-memory-types/
COPY crates/openhuman-memory/Cargo.toml crates/openhuman-memory/
COPY crates/openhuman-memory-server/Cargo.toml crates/openhuman-memory-server/

# Stub source files so cargo can resolve the dependency graph without
# recompiling every time source changes.
RUN mkdir -p src \
    && echo "fn main(){}" > src/main.rs \
    && mkdir -p crates/openhuman-embeddings/src \
    && echo "" > crates/openhuman-embeddings/src/lib.rs \
    && mkdir -p crates/openhuman-memory-types/src \
    && echo "" > crates/openhuman-memory-types/src/lib.rs \
    && mkdir -p crates/openhuman-memory/src \
    && echo "" > crates/openhuman-memory/src/lib.rs \
    && mkdir -p crates/openhuman-memory-server/src \
    && echo "" > crates/openhuman-memory-server/src/lib.rs \
    && echo "fn main(){}" > crates/openhuman-memory-server/src/main.rs

RUN cargo build --release -p openhuman-memory-server 2>/dev/null; true

# Now copy the real source and rebuild only the changed crates.
COPY crates/openhuman-embeddings/src crates/openhuman-embeddings/src
COPY crates/openhuman-memory-types/src crates/openhuman-memory-types/src
COPY crates/openhuman-memory/src crates/openhuman-memory/src
COPY crates/openhuman-memory-server/src crates/openhuman-memory-server/src

RUN touch crates/openhuman-memory-server/src/main.rs \
    && cargo build --release -p openhuman-memory-server

# ── Runtime stage ─────────────────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder \
    /build/target/release/openhuman-memory-server \
    /usr/local/bin/openhuman-memory-server

# Default workspace inside the container; mount a volume here for persistence.
ENV MEMORY_WORKSPACE=/data/workspace
RUN mkdir -p /data/workspace

EXPOSE 8420

ENTRYPOINT ["/usr/local/bin/openhuman-memory-server"]
CMD ["--port", "8420", "--workspace", "/data/workspace"]
