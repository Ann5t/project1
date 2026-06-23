# ──────────────────────────────────────────────────
# Stage 1: Chef — Dependency planning
# ──────────────────────────────────────────────────
FROM rust:1.85-slim-bookworm AS chef
RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev binutils && rm -rf /var/lib/apt/lists/*
# BuildKit cache mount avoids re-downloading crate index on repeated builds
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    cargo install cargo-chef --version 0.1.69
WORKDIR /app

# ──────────────────────────────────────────────────
# Stage 2: Planner — Generate dependency recipe
# ──────────────────────────────────────────────────
FROM chef AS planner
COPY Cargo.toml Cargo.lock* ./
COPY crates/agent-server/Cargo.toml crates/agent-server/
COPY crates/agent-core/Cargo.toml crates/agent-core/
COPY crates/agent-db/Cargo.toml crates/agent-db/
# Create dummy source files so cargo-chef can determine targets
RUN mkdir -p crates/agent-server/src && \
    echo "pub fn dummy() {}" > crates/agent-server/src/lib.rs && \
    echo "fn main() {}" > crates/agent-server/src/main.rs && \
    mkdir -p crates/agent-core/src && \
    echo "pub fn dummy() {}" > crates/agent-core/src/lib.rs && \
    mkdir -p crates/agent-db/src && \
    echo "pub fn dummy() {}" > crates/agent-db/src/lib.rs
RUN cargo chef prepare --recipe-path recipe.json

# ──────────────────────────────────────────────────
# Stage 3: Builder — Compile dependencies + source
# ──────────────────────────────────────────────────
FROM chef AS builder
COPY --from=planner /app/recipe.json recipe.json
# BuildKit cache mounts speed up repeated builds dramatically
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo chef cook --release --recipe-path recipe.json

COPY Cargo.toml Cargo.lock* ./
COPY crates/ crates/
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/app/target \
    cargo build --release --bin agent-server && \
    cp /app/target/release/agent-server /app/agent-server && \
    strip /app/agent-server

# ──────────────────────────────────────────────────
# Stage 4: Runtime — Minimal production image
# ──────────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime
# Pin to a digest-sha256 for strict reproducibility in production:
# FROM debian:bookworm-slim@sha256:... (run `docker pull debian:bookworm-slim` to get current digest)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl libssl3 && \
    rm -rf /var/lib/apt/lists/* && \
    addgroup --gid 1001 appgroup && \
    adduser --uid 1001 --gid 1001 --disabled-password --gecos "" appuser

WORKDIR /app

# Copy binary (built and extracted in builder stage)
COPY --from=builder --chown=appuser:appgroup /app/agent-server /app/agent-server

# Copy frontend assets (static HTML/CSS/JS, no build step needed)
COPY --chown=appuser:appgroup crates/agent-frontend/ /app/frontend/

# Note: SQL migrations are embedded at compile time by sqlx::migrate!()
# No runtime migration files needed

# Create data directory with correct permissions for the volume mount.
# All previous COPY commands use --chown, so only /app/data needs explicit ownership.
RUN mkdir -p /app/data && chown appuser:appgroup /app/data

USER appuser
EXPOSE 3000

HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD curl -f http://localhost:3000/api/health

ENV RUST_LOG=info
ENV DATABASE_PATH=/app/data/agent.db
ENV FRONTEND_DIR=/app/frontend
ENV BIND_ADDRESS=0.0.0.0:3000

CMD ["./agent-server"]
