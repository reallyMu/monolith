# downmark (Monolith converter)

Single ~8MB Go binary — all non-text → Markdown conversion for Monolith.

- Upstream: https://github.com/giraffesyo/downmark
- Contract: `./run <absolute-input> <absolute-output.md>`
- Stub (dev only): `MONOLITH_CONVERT_STUB=1`

Install / refresh binary:

```bash
/Users/muqiang/.openclaw/workspace/monolith/scripts/install-converters.sh
```

Not OCR: scanned PDFs/images without a text layer will fail clearly.

Candidate (not shipped): OpenDataLoader PDF — see `docs/superpowers/specs/2026-10-04-nontext-md-conversion-design.md` §6. Engine zip ~22MB + Java 11; ~100MB total is accepted; hybrid/OCR is not.
