# ─── Stage 1: Rust agent-runner (Phase 2 native backend) ─────────────────────
FROM rust:1.77-alpine AS rust-builder

RUN apk add --no-cache musl-dev openssl-dev openssl-libs-static pkgconfig

WORKDIR /rust
COPY packages/agent-runner/Cargo.toml packages/agent-runner/Cargo.lock ./
# Cache deps layer
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release && rm -rf src

COPY packages/agent-runner/src ./src
RUN touch src/main.rs && cargo build --release

# ─── Stage 2: TypeScript IDE + Node backend ───────────────────────────────────
FROM node:22-alpine AS node-builder

RUN apk add --no-cache python3 make g++
RUN corepack enable

WORKDIR /workspace

COPY package.json yarn.lock* ./
COPY packages/agent-ide-types/package.json   ./packages/agent-ide-types/
COPY packages/agent-ide-backend/package.json ./packages/agent-ide-backend/
COPY extensions/agent-ide-core/package.json  ./extensions/agent-ide-core/
COPY applications/browser-app/package.json   ./applications/browser-app/

RUN yarn install --frozen-lockfile

COPY packages/      ./packages/
COPY extensions/    ./extensions/
COPY applications/  ./applications/
COPY tsconfig.base.json .

RUN yarn --cwd packages/agent-ide-types build && \
    yarn --cwd packages/agent-ide-backend build && \
    yarn --cwd extensions/agent-ide-core build && \
    yarn --cwd applications/browser-app build

# ─── Stage 3: Runtime ─────────────────────────────────────────────────────────
FROM node:22-alpine

RUN apk add --no-cache tini && corepack enable

WORKDIR /workspace

# Phase 1: Node backend
COPY --from=node-builder /workspace/node_modules  ./node_modules
COPY --from=node-builder /workspace/packages      ./packages
COPY --from=node-builder /workspace/extensions    ./extensions
COPY --from=node-builder /workspace/applications  ./applications
COPY --from=node-builder /workspace/package.json  ./package.json

# Phase 2: Rust binary (runs instead of node when RUNTIME_PHASE=2)
COPY --from=rust-builder /rust/target/release/agent-runner /usr/local/bin/agent-runner

COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh
RUN chmod +x /usr/local/bin/docker-entrypoint.sh /usr/local/bin/agent-runner

ENV NODE_ENV=production \
    PORT=3001 \
    WORKSPACE_ROOT=/data/workspace \
    RUNTIME_PHASE=1

RUN mkdir -p /data/workspace

EXPOSE 3000 3001

ENTRYPOINT ["tini", "--"]
CMD ["/usr/local/bin/docker-entrypoint.sh"]
