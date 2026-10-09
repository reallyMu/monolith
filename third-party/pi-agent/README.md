# Monolith Pi Agent runtime

Production Bridge sidecar for the Monolith desktop Agent.

- **Bridge:** `bridge/server.mjs` (Node ≥ 22.19)
- **SDK:** `@earendil-works/pi-coding-agent` (npm)
- **Seed config:** `seed/.pi-agent/` (copied to Application Support on first run)

## Install deps (dev / before packaging)

```bash
/Users/muqiang/.openclaw/workspace/monolith/scripts/pi-agent-install.sh
```

Requires system Node ≥ 22.19. Tauri `bundle.resources` ships this tree with the app.
