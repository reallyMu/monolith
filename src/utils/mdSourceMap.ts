import MarkdownIt from "markdown-it";

/**
 * Top-level blocks as TipTap/ProseMirror emits them (one list node, not per item).
 * Used for split sync: span index ↔ `.ProseMirror` child index.
 */
const WYSIWYG_BLOCK_OPEN = new Set([
  "heading_open",
  "paragraph_open",
  "bullet_list_open",
  "ordered_list_open",
  "blockquote_open",
  "fence",
  "code_block",
  "table_open",
  "hr",
  "html_block",
]);

export type SourceSpan = { start: number; end: number };

function parseTokens(content: string) {
  const md = new MarkdownIt({ html: false, linkify: true, typographer: true, breaks: false });
  return md.parse(content || "", {});
}

/** Source spans aligned to TipTap top-level blocks (1-based inclusive lines). */
export function sourceSpansFromMarkdown(content: string): SourceSpan[] {
  const tokens = parseTokens(content);
  const out: SourceSpan[] = [];
  let listItemDepth = 0;
  let blockquoteDepth = 0;
  for (const token of tokens) {
    if (token.type === "list_item_open") listItemDepth += 1;
    if (token.type === "list_item_close") listItemDepth = Math.max(0, listItemDepth - 1);
    if (token.type === "blockquote_open") blockquoteDepth += 1;
    if (token.type === "blockquote_close") blockquoteDepth = Math.max(0, blockquoteDepth - 1);
    if (!token.map || !WYSIWYG_BLOCK_OPEN.has(token.type)) continue;
    if (token.type === "paragraph_open" && (listItemDepth > 0 || blockquoteDepth > 0)) continue;
    const start = token.map[0] + 1;
    const end = Math.max(start, token.map[1]);
    out.push({ start, end });
  }
  return out;
}

/** Span that contains `line`, preferring the narrowest match. */
export function spanForLine(spans: SourceSpan[], line: number): SourceSpan | null {
  if (line < 1 || spans.length === 0) return null;
  let best: SourceSpan | null = null;
  let bestWidth = Number.POSITIVE_INFINITY;
  for (const s of spans) {
    if (s.start <= line && line <= s.end) {
      const w = s.end - s.start;
      if (w < bestWidth) {
        best = s;
        bestWidth = w;
      }
    }
  }
  return best;
}

/** Content Y of `el` inside scroll `container`. */
export function offsetTopWithin(el: Element, container: HTMLElement): number {
  const er = el.getBoundingClientRect();
  const cr = container.getBoundingClientRect();
  return er.top - cr.top + container.scrollTop;
}
