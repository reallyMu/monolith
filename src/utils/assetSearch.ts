/** Shared name-match rules for asset tree UI (align with Domain search_assets_by_name). */

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
