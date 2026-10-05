/**
 * UI tree name filter — must stay aligned with Domain `asset_matches_name_query`
 * (display_name + path basename, case-insensitive substring).
 * Locked by `scripts/test-asset-search.mjs` against the Rust unit cases.
 * Empty/whitespace query: UI shows full tree; Domain `search_assets_by_name` rejects empty.
 */

export function basenameOf(path: string): string {
  const parts = path.split(/[/\\]/);
  return parts[parts.length - 1] || path;
}

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
