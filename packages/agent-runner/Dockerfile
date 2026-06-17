# Autonomyx Platform Runner — Production Dockerfile
# Multi-stage: hermetic Rust build → minimal runtime image (~20MB)
#
# Build:   docker build -t autonomyx-runner .
# Run:     docker run -e GATEWAY_API_KEY=<key> -p 3001:3001 autonomyx-runner
#
# Production run:
#   docker run -d \
#     --name autonomyx \
#     --restart unless-stopped \
#     --read-only \
#     --security-opt no-new-privileges \
#     --cap-drop ALL \
#     -e GATEWAY_API_KEY=<64-char-key> \
#     -e PRODUCTION=true \
#     -e CORS_ALLOWED_ORIGINS=https://your-domain.com \
#     -e AUTONOMYX_IDENTITY_KEY=<ed25519-key> \
#     -p 3001:3001 \
#     autonomyx-runner

# ── Stage 1: Build ────────────────────────────────────────────────────────────
FROM rust:1.80-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    pkg-config libssl-dev ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build

# Cache dependencies separately (layer cache invalidates only on Cargo.toml changes)
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs \
    && cargo build --release --locked 2>/dev/null; rm -rf src

COPY src ./src
RUN touch src/main.rs && cargo build --release --locked

# Strip debug symbols → smaller binary
RUN strip target/release/autonomyx-runner
RUN ls -lh target/release/autonomyx-runner

# ── Stage 2: Minimal runtime image ────────────────────────────────────────────
FROM debian:bookworm-slim AS runtime

# Non-root user — least privilege
RUN groupadd --gid 1001 autonomyx \
    && useradd --uid 1001 --gid autonomyx \
       --no-create-home --shell /bin/false autonomyx

# CA certs for HTTPS egress (LLM providers, peers)
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /build/target/release/autonomyx-runner /usr/local/bin/autonomyx-runner

# k8s liveness probe
HEALTHCHECK --interval=10s --timeout=3s --start-period=10s --retries=3 \
    CMD curl -sf http://localhost:${PORT:-3001}/health || exit 1

USER autonomyx

EXPOSE 3001

# Defaults — override all via env in production
ENV RUST_LOG="agent_runner=info,tower_http=warn" \
    PORT="3001" \
    REQUEST_TIMEOUT_SECS="30" \
    RATE_LIMIT_RPM="60" \
    AUTH_FAILURE_THRESHOLD="10" \
    AUTH_BAN_SECS="300"

ENTRYPOINT ["/usr/local/bin/autonomyx-runner"]
