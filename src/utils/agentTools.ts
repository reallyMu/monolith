/**
 * Execute Agent tool invokes via Monolith MCP (`agent_tool_call`).
 * Pure parse/format helpers live in `agentToolParse.ts`.
 */
import { agentToolCall } from "./agentApi";
import type { AssetDto } from "./assetsApi";
import {
  formatAssetSearchMd,
  type ToolInvoke,
} from "./agentToolParse";

export {
  formatAssetSearchMd,
  guessAssetNameQuery,
  looksLikeToolMarkup,
  parseToolInvokes,
  stripToolMarkup,
  type ToolInvoke,
} from "./agentToolParse";

export async function runAssetSearchTool(
  params: Record<string, string>,
  opts?: { preferMd?: boolean },
): Promise<string> {
  const query = (params.query || "").trim();
  if (!query) return "asset_search 失败：缺少 query";
  const recursive = /^(1|true|yes)$/i.test(params.recursive || "");
  const raw = (await agentToolCall("asset_search", {
    query,
    mode: params.mode?.trim() || "name",
    recursive,
    ...(params.folder ? { folder: params.folder } : {}),
  })) as { assets?: AssetDto[] };
  if (!Array.isArray(raw?.assets)) {
    throw new Error("asset_search returned no assets array");
  }
  const mdOnly =
    opts?.preferMd === true || /^(md|markdown)$/i.test(params.fileType || params.kind || "");
  return formatAssetSearchMd(query, raw.assets, { mdOnly });
}

function formatToolResult(name: string, result: unknown): string {
  try {
    const text = JSON.stringify(result, null, 2);
    const clipped = text.length > 12000 ? `${text.slice(0, 12000)}\n…(truncated)` : text;
    return `### Tool \`${name}\` result\n\n\`\`\`json\n${clipped}\n\`\`\``;
  } catch {
    return `### Tool \`${name}\` result\n\n${String(result)}`;
  }
}

/** Execute MCP tools configured for the embedded Agent. */
export async function executeToolInvokes(
  invokes: ToolInvoke[],
  opts?: { preferMd?: boolean },
): Promise<string> {
  const parts: string[] = [];
  for (const inv of invokes) {
    if (inv.name === "asset_search") {
      parts.push(await runAssetSearchTool(inv.params, opts));
      continue;
    }
    try {
      const result = await agentToolCall(inv.name, inv.params);
      parts.push(formatToolResult(inv.name, result));
    } catch (e) {
      parts.push(`### Tool \`${inv.name}\` error\n\n${String(e)}`);
    }
  }
  return parts.join("\n\n");
}
