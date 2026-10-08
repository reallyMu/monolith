/**
 * UI tree name filter — must stay aligned with Domain `asset_matches_name_query`
 * (remark/display_name + path basename + mount/ancestor folder names).
 * Locked by `scripts/test-asset-search.mjs` against the Rust unit cases.
 * Empty/whitespace query: UI shows full tree; Domain `search_assets_by_name` rejects empty.
 */

export function basenameOf(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

export function nameContainsQuery(query: string, value: string): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  return value.toLowerCase().includes(q);
}

/** Remark (`displayName`) and file basename. */
export function matchAssetName(
  query: string,
  displayName: string,
  absolutePath: string | null | undefined,
): boolean {
  const q = query.trim().toLowerCase();
  if (!q) return true;
  if (displayName.toLowerCase().includes(q)) return true;
  if (absolutePath && basenameOf(absolutePath).toLowerCase().includes(q)) return true;
  return false;
}

export function matchFolderName(query: string, folderDisplayName: string): boolean {
  return nameContainsQuery(query, folderDisplayName);
}
