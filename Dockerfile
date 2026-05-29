FROM node:22 AS builder

WORKDIR /app

# Enable corepack for Yarn 4+
RUN corepack enable

# Copy workspace manifests first for better layer caching
COPY package.json yarn.lock* .yarnrc.yml* ./
COPY packages/agent-ide-types/package.json ./packages/agent-ide-types/
COPY extensions/agent-ide-core/package.json ./extensions/agent-ide-core/
COPY applications/browser-app/package.json ./applications/browser-app/

# Install all dependencies
RUN yarn install --immutable 2>/dev/null || yarn install

# Copy source
COPY . .

# Build: types → extension → browser app
RUN yarn build

# Production stage
FROM node:22-slim

RUN corepack enable

WORKDIR /app

COPY --from=builder /app /app

EXPOSE 3000

CMD ["yarn", "start"]
