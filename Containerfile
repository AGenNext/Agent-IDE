# ─── Builder ──────────────────────────────────────────────────────────────────
FROM node:22-alpine AS builder

RUN apk add --no-cache python3 make g++
RUN corepack enable

WORKDIR /workspace

COPY package.json yarn.lock* ./
COPY packages/agent-ide-types/package.json      ./packages/agent-ide-types/
COPY packages/agent-ide-backend/package.json    ./packages/agent-ide-backend/
COPY extensions/agent-ide-core/package.json     ./extensions/agent-ide-core/
COPY applications/browser-app/package.json      ./applications/browser-app/

RUN yarn install --frozen-lockfile

COPY packages/      ./packages/
COPY extensions/    ./extensions/
COPY applications/  ./applications/
COPY tsconfig.base.json .

RUN yarn --cwd packages/agent-ide-types build && \
    yarn --cwd packages/agent-ide-backend build && \
    yarn --cwd extensions/agent-ide-core build && \
    yarn --cwd applications/browser-app build

# ─── Runtime ──────────────────────────────────────────────────────────────────
FROM node:22-alpine

RUN apk add --no-cache tini wget && corepack enable

WORKDIR /workspace

COPY --from=builder /workspace/node_modules            ./node_modules
COPY --from=builder /workspace/packages                ./packages
COPY --from=builder /workspace/extensions              ./extensions
COPY --from=builder /workspace/applications            ./applications
COPY --from=builder /workspace/package.json            ./package.json
COPY docker-entrypoint.sh /usr/local/bin/docker-entrypoint.sh

ENV NODE_ENV=production \
    PORT=3001 \
    WORKSPACE_ROOT=/data/workspace

RUN addgroup -S agent && \
    adduser -S agent -G agent -h /workspace && \
    mkdir -p /data/workspace && \
    chmod +x /usr/local/bin/docker-entrypoint.sh && \
    chown -R agent:agent /workspace /data /usr/local/bin/docker-entrypoint.sh

USER agent

EXPOSE 3000 3001

HEALTHCHECK --interval=30s --timeout=10s --retries=3 --start-period=20s \
  CMD wget -qO- "http://127.0.0.1:${PORT}/health" || exit 1

ENTRYPOINT ["tini", "--"]
CMD ["/usr/local/bin/docker-entrypoint.sh"]
