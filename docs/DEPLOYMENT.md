# Deployment Guide

## Prerequisites

- Node.js 22+
- Corepack enabled (`corepack enable`)
- Yarn 4+ (managed via corepack)
- Python 3 (required by some native Theia dependencies)
- C++ build tools (required by some native Theia dependencies)
  - macOS: `xcode-select --install`
  - Ubuntu: `apt install build-essential`

## Local Development

```bash
# 1. Install dependencies
yarn install

# 2. Build all packages (types → extension → browser app)
yarn build

# 3. Start the browser app on port 3000
yarn start
```

Open `http://localhost:3000` in your browser.

### Watch Mode (Extension Development)

```bash
# Terminal 1: watch the extension (fast TSC rebuild)
cd extensions/agent-ide-core && yarn watch

# Terminal 2: watch the browser app (webpack)
cd applications/browser-app && yarn watch

# Terminal 3: start the server
yarn start
```

## Docker

### Build

```bash
docker build -t agent-ide:latest .
```

### Run

```bash
docker run -p 3000:3000 agent-ide:latest
```

Open `http://localhost:3000`.

### Production Build

The Dockerfile uses a single-stage build for simplicity. For production, consider
a multi-stage build to reduce image size:

```dockerfile
# Stage 1: build
FROM node:22 AS builder
...
# Stage 2: run
FROM node:22-slim AS runner
COPY --from=builder /app/applications/browser-app/lib ./lib
...
```

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `3000` | HTTP port for the Theia backend |
| `THEIA_DEFAULT_PLUGINS` | (empty) | Comma-separated VS Code plugin IDs to install |

## CI/CD

GitHub Actions workflow: `.github/workflows/ci.yml`

Runs on every push and pull request:
1. Checkout
2. Setup Node 22
3. Enable corepack
4. `yarn install`
5. `yarn build`
6. `yarn lint` (if available)

## Troubleshooting

### `node-gyp` errors during `yarn install`

Install platform build tools (see Prerequisites). These are required by
`@theia/core`'s native dependencies.

### Port 3000 already in use

```bash
yarn start -- --port 3001
```

### Slow first build

The first `theia build` downloads webpack chunks and compiles all extensions.
Expect 5–15 minutes on a clean install. Subsequent builds use the cache and
are much faster (30–90 seconds).
