# MD Formula Editor Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Toolbar Σ panel (templates + LaTeX + KaTeX preview) and preview double-click to edit `$`/`$$` formulas, writing only to Monaco/source markdown.

**Architecture:** Pure string helpers in `mdMathRange.ts`; Vue modal for UX; Math NodeView emits bubbled custom event; App owns open/commit against `tab.content` (+ Monaco selection when available).

**Tech Stack:** Vue 3, existing KaTeX, Monaco executeEdits, TipTap NodeView.

## File map

| File | Role |
|---|---|
| `src/utils/mdMathRange.ts` | list/find/wrap/replace math ranges |
| `scripts/test-md-math-range.mjs` | unit checks |
| `src/components/FormulaEditorModal.vue` | panel UI |
| `src/extensions/MathNodes.ts` | dblclick → `monolith-math-edit` |
| `src/components/MarkdownWysiwyg.vue` | forward edit-math |
| `src/components/MonacoEditor.vue` | cursor offset + replace range |
| `src/components/ToolbarIcon.vue` | `sigma` icon |
| `src/i18n/messages.ts` | zh/en strings |
| `src/App.vue` | toolbar + wire commit |

## Tasks

1. `mdMathRange` + test script (fence skip, find at offset, wrap/replace).
2. `FormulaEditorModal` + i18n + sigma icon.
3. Monaco helpers; App open/commit from toolbar.
4. MathNodes dblclick + Wysiwyg emit; App handle edit existing.
5. `vue-tsc` + test script; rebuild/install if needed.
