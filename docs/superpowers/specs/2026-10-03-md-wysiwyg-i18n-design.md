# MD WYSIWYG + system locale UI

## Decisions

1. **Markdown right pane = TipTap** (`tiptap-markdown`), not read-only `markdown-it` preview.
2. Split: left Monaco (source) ↔ right TipTap (WYSIWYG), bidirectional sync with loop guards.
3. Format toolbar routes to the focused MD surface (source vs WYSIWYG). Preview-only mode edits TipTap.
4. **UI locale** follows OS: `navigator.language` starting with `zh` → Chinese, else English. No manual language switch in v1.

## Out of scope

- Per-file locale override
- Perfect MD round-trip for every edge case (HTML blocks, exotic extensions)
