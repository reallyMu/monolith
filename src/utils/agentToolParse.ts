/**
 * Pure Agent tool markup parse / format helpers (no Tauri).
 */
import type { AssetDto } from "./assetsApi";

export type ToolInvoke = {
  name: string;
  params: Record<string, string>;
};

const DSML_MARK =
  /(?:&lt;|<)\s*[|｜]{1,2}\s*DSML\s*[|｜]{1,2}|<\/?\s*[|｜]{1,2}\s*DSML/i;

export function looksLikeToolMarkup(text: string): boolean {
  return DSML_MARK.test(text) || /invoke\s+name\s*=\s*["']asset_/i.test(text);
}

/** Strip DSML / function-call scaffolding for display. */
export function stripToolMarkup(text: string): string {
  let s = text
    .replace(/&lt;/g, "<")
    .replace(/&gt;/g, ">")
    .replace(/&amp;/g, "&")
    .replace(/&quot;/g, '"');
  s = s.replace(/<[^>]*DSML[^>]*>[\s\S]*?<\/[^>]*DSML[^>]*>/gi, "");
  s = s.replace(/<[^>]*DSML[^>]*\/?>/gi, "");
  s = s.replace(/<\/?invoke\b[^>]*>/gi, "");
  s = s.replace(/<\/?parameter\b[^>]*>[\s\S]*?/gi, "");
  s = s.replace(/<\/?calls\b[^>]*>/gi, "");
  return s.replace(/\n{3,}/g, "\n\n").trim();
}

export function parseToolInvokes(text: string): ToolInvoke[] {
  const raw = text.replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&amp;/g, "&");
  const out: ToolInvoke[] = [];
  const invokeRe =
    /<\s*[^>]*invoke[^>]*name\s*=\s*["']([^"']+)["'][^>]*>([\s\S]*?)<\s*\/[^>]*invoke[^>]*>/gi;
  let m: RegExpExecArray | null;
  while ((m = invokeRe.exec(raw)) !== null) {
    const name = m[1]!.trim();
    const body = m[2] ?? "";
    const params: Record<string, string> = {};
    const paramRe =
      /<\s*[^>]*parameter[^>]*name\s*=\s*["']([^"']+)["'][^>]*>([\s\S]*?)<\s*\/[^>]*parameter[^>]*>/gi;
    let pm: RegExpExecArray | null;
    while ((pm = paramRe.exec(body)) !== null) {
      params[pm[1]!.trim()] = pm[2]!.trim();
    }
    out.push({ name, params });
  }
  return out;
}

function isMdAsset(a: AssetDto): boolean {
  if ((a.fileType || "").toLowerCase() === "md") return true;
  const p = (a.absolutePath || "").toLowerCase();
  return p.endsWith(".md") || p.endsWith(".markdown");
}

export function formatAssetSearchMd(
  query: string,
  assets: AssetDto[],
  opts?: { mdOnly?: boolean },
): string {
  let list = assets;
  if (opts?.mdOnly) list = list.filter(isMdAsset);
  if (!list.length) {
    return `未在资产库中找到名称含「${query}」的${opts?.mdOnly ? " Markdown " : ""}文档。`;
  }
  const rows = list
    .slice(0, 50)
    .map(
      (a, i) =>
        `${i + 1}. **${a.displayName || "(unnamed)"}** (id=${a.id})\n   - \`${a.absolutePath || "—"}\``,
    )
    .join("\n");
  const more = list.length > 50 ? `\n\n…另有 ${list.length - 50} 条未列出` : "";
  return `在资产库中按名称「${query}」找到 **${list.length}** 条${opts?.mdOnly ? " Markdown " : ""}：\n\n${rows}${more}`;
}

export function guessAssetNameQuery(userText: string): string | null {
  const t = userText.trim();
  if (!/(资产|知识库|ML|Monolith|文档|markdown|\.md|md文档)/i.test(t)) {
    if (!/(查询|搜索|查找|搜一下|帮我查)/.test(t)) return null;
  }
  if (!/(查询|搜索|查找|搜|找|相似|名称|名字|name)/i.test(t)) return null;
  const quoted =
    t.match(/[「『"“]([^」』"”]+)[」』"”]/)?.[1] ||
    t.match(/名称[含有相似像]*[「『"“]?([^」』"”\s，。]+)[」』"”]?/)?.[1];
  if (quoted?.trim()) return quoted.trim();
  const aiDash = t.match(/(ai-[\w.-]*)/i)?.[1];
  if (aiDash) return aiDash;
  return null;
}
