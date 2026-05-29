FROM node:22-bookworm-slim AS builder

RUN corepack enable

WORKDIR /workspace

COPY package.json yarn.lock* ./
COPY packages/agent-ide-types/package.json ./packages/agent-ide-types/
COPY extensions/agent-ide-core/package.json ./extensions/agent-ide-core/
COPY applications/browser-app/package.json ./applications/browser-app/

RUN yarn install --frozen-lockfile

COPY packages/ ./packages/
COPY extensions/ ./extensions/
COPY applications/ ./applications/
COPY tsconfig.base.json .

RUN yarn build

# ─── Runtime image ────────────────────────────────────────────────────────────
FROM node:22-bookworm-slim

RUN corepack enable

WORKDIR /workspace

COPY --from=builder /workspace/node_modules ./node_modules
COPY --from=builder /workspace/packages ./packages
COPY --from=builder /workspace/extensions ./extensions
COPY --from=builder /workspace/applications ./applications
COPY --from=builder /workspace/package.json ./package.json

ENV NODE_ENV=production
EXPOSE 3000

CMD ["yarn", "start"]
