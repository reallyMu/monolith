# Monolith Pi Agent runtime

Production Bridge sidecar for the Monolith desktop Agent.

- **Bridge:** `bridge/server.mjs`
- **Pi SDK:** vendored from [earendil-works/pi](https://github.com/earendil-works/pi) `@main` (npm registry often lags). Install script clones, builds, packs, and installs into `bridge/node_modules`.
- **Node:** release builds ship Node ≥ 22.19 via `npm run pi-node:install` (`third-party/node`).

```bash
npm run pi-agent:install          # vendor Pi @ main
PI_FROM_NPM=1 npm run pi-agent:install   # use registry only (older)
PI_REF=v1.1.0 npm run pi-agent:install   # pin a git tag/branch
```

Runtime config lives under App Support `…/com.muqiang.monolith/pi-agent/`.
