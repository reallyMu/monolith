# downmark (Monolith converter)

Single ~8MB Go binary — Office / HTML / CSV / EPUB / … → Markdown.

- Upstream: https://github.com/giraffesyo/downmark
- Contract: `./run <absolute-input> <absolute-output.md>`
- Stub (dev only): `MONOLITH_CONVERT_STUB=1`

**PDF** uses OpenDataLoader instead — see `third-party/opendataloader-pdf/MONOLITH.md`.

Install / refresh binaries:

```bash
/Users/muqiang/.openclaw/workspace/monolith/scripts/install-converters.sh
```

Not OCR: scanned images without a text layer are out of scope.
