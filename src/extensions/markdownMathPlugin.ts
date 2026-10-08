/**
 * markdown-it rules for `$…$` (inline) and `$$…$$` (block).
 * Emits TipTap-friendly HTML; KaTeX runs in NodeViews, not here.
 */
import type MarkdownIt from "markdown-it";

const FLAG = "__monolithMath";

function escAttr(s: string): string {
  return s
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

// markdown-it state types vary by package build; keep local structural typing.
type BlockState = {
  src: string;
  bMarks: number[];
  eMarks: number[];
  tShift: number[];
  line: number;
  push: (type: string, tag: string, nesting: number) => { content: string; map: number[]; markup: string };
};

type InlineState = {
  src: string;
  pos: number;
  posMax: number;
  push: (type: string, tag: string, nesting: number) => { content: string; markup: string };
};

function mathBlock(
  state: BlockState,
  startLine: number,
  endLine: number,
  silent: boolean,
): boolean {
  const start = state.bMarks[startLine] + state.tShift[startLine];
  const max = state.eMarks[startLine];
  if (start + 2 > max) return false;
  if (state.src.slice(start, start + 2) !== "$$") return false;

  const afterOpen = state.src.slice(start + 2, max);
  if (afterOpen.trim().endsWith("$$") && afterOpen.trim().length > 2) {
    const body = afterOpen.trim().slice(0, -2).trim();
    if (silent) return true;
    const token = state.push("math_block", "div", 0);
    token.content = body;
    token.map = [startLine, startLine + 1];
    token.markup = "$$";
    state.line = startLine + 1;
    return true;
  }
  if (afterOpen.trim() !== "") return false;

  let next = startLine + 1;
  let closed = false;
  while (next < endLine) {
    const lineStart = state.bMarks[next] + state.tShift[next];
    const lineMax = state.eMarks[next];
    if (state.src.slice(lineStart, lineStart + 2) === "$$") {
      const rest = state.src.slice(lineStart + 2, lineMax).trim();
      if (rest === "") {
        closed = true;
        break;
      }
    }
    next += 1;
  }
  if (!closed) return false;
  if (silent) return true;

  const firstContent = state.bMarks[startLine + 1];
  const lastContent = state.eMarks[next - 1];
  const body = state.src.slice(firstContent, lastContent).replace(/^\n+|\n+$/g, "");
  const token = state.push("math_block", "div", 0);
  token.content = body;
  token.map = [startLine, next + 1];
  token.markup = "$$";
  state.line = next + 1;
  return true;
}

function mathInline(state: InlineState, silent: boolean): boolean {
  const start = state.pos;
  if (state.src[start] !== "$") return false;
  if (state.src[start + 1] === "$") return false;
  if (start > 0 && state.src[start - 1] === "\\") return false;

  let pos = start + 1;
  let found = -1;
  while (pos < state.posMax) {
    if (state.src[pos] === "$" && state.src[pos - 1] !== "\\") {
      found = pos;
      break;
    }
    pos += 1;
  }
  if (found < 0 || found === start + 1) return false;
  const body = state.src.slice(start + 1, found);
  if (body.includes("\n")) return false;
  if (silent) return true;

  const token = state.push("math_inline", "span", 0);
  token.content = body;
  token.markup = "$";
  state.pos = found + 1;
  return true;
}

export function installMarkdownMath(md: MarkdownIt): void {
  const anyMd = md as MarkdownIt & { [FLAG]?: boolean };
  if (anyMd[FLAG]) return;
  anyMd[FLAG] = true;

  md.block.ruler.before("fence", "math_block", mathBlock as never, {
    alt: ["paragraph", "reference", "blockquote", "list"],
  });
  md.inline.ruler.after("escape", "math_inline", mathInline as never);

  md.renderer.rules.math_block = (tokens, idx) => {
    const latex = tokens[idx]?.content ?? "";
    return `<div data-type="math-block" data-latex="${escAttr(latex)}"></div>`;
  };
  md.renderer.rules.math_inline = (tokens, idx) => {
    const latex = tokens[idx]?.content ?? "";
    return `<span data-type="math-inline" data-latex="${escAttr(latex)}"></span>`;
  };
}
