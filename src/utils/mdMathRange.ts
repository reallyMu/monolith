/** Detect / wrap / replace `$…$` and `$$…$$` ranges in Markdown source. */

export type MathMode = "inline" | "block";

export type MathRange = {
  mode: MathMode;
  latex: string;
  /** Inclusive start of opening delimiter (UTF-16 offset). */
  start: number;
  /** Exclusive end after closing delimiter. */
  end: number;
};

/** Skip fenced code so `$` inside ``` does not count as math. */
function skipFence(s: string, i: number): number | null {
  if (!s.startsWith("```", i)) return null;
  const nl = s.indexOf("\n", i + 3);
  if (nl < 0) return s.length;
  const close = s.indexOf("\n```", nl);
  if (close < 0) return s.length;
  return close + 4;
}

/**
 * List math ranges in document order.
 * Block `$$` wins over single `$`. Inline `$…$` does not span newlines.
 */
export function listMathRanges(content: string): MathRange[] {
  const s = content ?? "";
  const out: MathRange[] = [];
  let i = 0;
  while (i < s.length) {
    const fenceEnd = skipFence(s, i);
    if (fenceEnd != null) {
      i = fenceEnd;
      continue;
    }
    if (s.startsWith("$$", i)) {
      const close = s.indexOf("$$", i + 2);
      if (close < 0) {
        i += 2;
        continue;
      }
      let latex = s.slice(i + 2, close);
      if (latex.startsWith("\n")) latex = latex.slice(1);
      if (latex.endsWith("\n")) latex = latex.slice(0, -1);
      out.push({ mode: "block", latex, start: i, end: close + 2 });
      i = close + 2;
      continue;
    }
    if (s[i] === "$") {
      let j = i + 1;
      let found = -1;
      while (j < s.length) {
        if (s[j] === "\n") break;
        if (s[j] === "\\" && j + 1 < s.length) {
          j += 2;
          continue;
        }
        if (s[j] === "$") {
          found = j;
          break;
        }
        j += 1;
      }
      if (found > i) {
        out.push({
          mode: "inline",
          latex: s.slice(i + 1, found),
          start: i,
          end: found + 1,
        });
        i = found + 1;
        continue;
      }
    }
    i += 1;
  }
  return out;
}

/** Range containing `offset` (cursor on either delimiter counts). */
export function findMathAtOffset(content: string, offset: number): MathRange | null {
  if (offset < 0) return null;
  for (const r of listMathRanges(content)) {
    if (offset >= r.start && offset <= r.end) return r;
  }
  return null;
}

export function wrapMath(latex: string, mode: MathMode): string {
  const body = (latex ?? "").trim();
  if (mode === "block") return `$$\n${body}\n$$`;
  return `$${body}$`;
}

export function replaceMathRange(
  content: string,
  range: MathRange,
  latex: string,
  mode: MathMode,
): string {
  return content.slice(0, range.start) + wrapMath(latex, mode) + content.slice(range.end);
}

/** 1-based line → UTF-16 offset of line start. */
export function offsetAtLineStart(content: string, line1: number): number {
  if (line1 <= 1) return 0;
  let line = 1;
  for (let i = 0; i < content.length; i++) {
    if (content[i] === "\n") {
      line += 1;
      if (line === line1) return i + 1;
    }
  }
  return content.length;
}

/**
 * Find a math range by latex + mode; if several, pick closest to preferStart.
 */
export function findMathByLatex(
  content: string,
  latex: string,
  mode: MathMode,
  preferStart?: number,
): MathRange | null {
  const needle = (latex ?? "").trim();
  const matches = listMathRanges(content).filter(
    (r) => r.mode === mode && r.latex.trim() === needle,
  );
  if (matches.length === 0) return null;
  if (preferStart == null) return matches[0]!;
  let best = matches[0]!;
  let bestDist = Math.abs(best.start - preferStart);
  for (let k = 1; k < matches.length; k++) {
    const m = matches[k]!;
    const d = Math.abs(m.start - preferStart);
    if (d < bestDist) {
      best = m;
      bestDist = d;
    }
  }
  return best;
}
