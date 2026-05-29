FROM node:22-bookworm-slim AS builder

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

RUN yarn --cwd packages/agent-ide-types build
RUN yarn --cwd packages/agent-ide-backend build
RUN yarn --cwd extensions/agent-ide-core build
RUN yarn --cwd applications/browser-app build

# ─── Runtime image ────────────────────────────────────────────────────────────
FROM node:22-bookworm-slim

RUN corepack enable

WORKDIR /workspace

COPY --from=builder /workspace/node_modules            ./node_modules
COPY --from=builder /workspace/packages                ./packages
COPY --from=builder /workspace/extensions              ./extensions
COPY --from=builder /workspace/applications            ./applications
COPY --from=builder /workspace/package.json            ./package.json

ENV NODE_ENV=production
# 3000 = Theia IDE  |  3001 = Agent backend API
EXPOSE 3000 3001

# Start both the Theia frontend and the agent backend
CMD ["sh", "-c", "node packages/agent-ide-backend/lib/server.js & yarn start"]
