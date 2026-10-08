/**
 * Lock UI name-match rules to Domain search_assets_by_name cases.
 *   node --experimental-strip-types scripts/test-asset-search.mjs
 */
import assert from "node:assert/strict";
import { matchAssetName, matchFolderName } from "../src/utils/assetSearch.ts";

// Mirrors assets.rs::search_assets_by_name_matches_display_and_basename
assert.equal(matchAssetName("ER", "卡包ER设计", "/tmp/card-er-design.md"), true);
assert.equal(matchAssetName("card-er", "卡包ER设计", "/tmp/card-er-design.md"), true);
assert.equal(matchAssetName("ER", "杂记", "/tmp/notes.md"), false);
assert.equal(matchAssetName("notes", "杂记", "/tmp/notes.md"), true);

// Case fold + trim (Domain lowercases trimmed query)
assert.equal(matchAssetName("  er  ", "卡包ER设计", null), true);
assert.equal(matchAssetName("NOTES", "杂记", "C:\\docs\\Notes.MD"), true);

// Empty query: UI filter shows all (Domain API rejects empty — different entry)
assert.equal(matchAssetName("", "anything", "/a.md"), true);
assert.equal(matchAssetName("   ", "anything", "/a.md"), true);

// No path: basename branch skipped
assert.equal(matchAssetName("er", "plain", null), false);
assert.equal(matchAssetName("er", "plain", undefined), false);

assert.equal(matchFolderName("ai-analyzer", "ai-analyzer"), true);
assert.equal(matchFolderName("ANALYZER", "ai-analyzer"), true);
assert.equal(matchFolderName("ai-analyzer", "adaptyv"), false);
assert.equal(matchFolderName("", "ai-analyzer"), true);

console.log("test-asset-search: ok");
