/**
 * Unit tests for Agent tool parse / format helpers.
 *   node --experimental-strip-types scripts/test-agent-tools.mjs
 */
import assert from "node:assert/strict";
import {
  formatAssetSearchMd,
  guessAssetNameQuery,
  looksLikeToolMarkup,
  parseToolInvokes,
  stripToolMarkup,
} from "../src/utils/agentToolParse.ts";

assert.equal(looksLikeToolMarkup("<|DSML|>"), true);
assert.equal(looksLikeToolMarkup('invoke name="asset_search"'), true);
assert.equal(looksLikeToolMarkup("plain answer"), false);

const dsml = `
<|DSML|>
<invoke name="asset_search">
<parameter name="query">ER</parameter>
<parameter name="recursive">true</parameter>
</invoke>
</|DSML|>
`;
const invokes = parseToolInvokes(dsml);
assert.equal(invokes.length, 1);
assert.equal(invokes[0].name, "asset_search");
assert.equal(invokes[0].params.query, "ER");
assert.equal(invokes[0].params.recursive, "true");

const stripped = stripToolMarkup(dsml + "\nVisible");
assert.equal(stripped.includes("DSML"), false);
assert.equal(stripped.includes("Visible"), true);

assert.match(
  formatAssetSearchMd("q", []),
  /未在资产库中找到/,
);
assert.match(
  formatAssetSearchMd("ER", [
    { id: 1, displayName: "卡包ER", absolutePath: "/a.md", fileType: "md" },
  ]),
  /卡包ER/,
);
assert.match(
  formatAssetSearchMd(
    "x",
    [{ id: 1, displayName: "pdf", absolutePath: "/a.pdf", fileType: "pdf" }],
    { mdOnly: true },
  ),
  /Markdown/,
);

assert.equal(guessAssetNameQuery("随便聊聊"), null);
assert.equal(guessAssetNameQuery("在资产库搜索「卡包ER」"), "卡包ER");
assert.equal(guessAssetNameQuery("帮我查找 Monolith 里的 ai-analyzer"), "ai-analyzer");
assert.equal(guessAssetNameQuery("帮我查一下文档"), null);

console.log("test-agent-tools: ok");
