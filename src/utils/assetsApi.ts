import { invoke } from "@tauri-apps/api/core";

export type BrowseTermDto = {
  id: number;
  parentId: number | null;
  code: string;
  displayName: string;
  sortOrder: number;
};

export type AssetDto = {
  id: number;
  displayName: string;
  fileType: string;
  browseTermId: number;
  currentVersionId: number | null;
  absolutePath: string | null;
  fileExists: boolean;
  createdAt: string;
  sourcePath: string | null;
  sourceMtime: number | null;
  sourceSize: number | null;
  sourceExists: boolean;
  sourceStale: boolean;
  indexStatus: "VALID" | "INVALID";
};

export type AssetTreeDto = {
  terms: BrowseTermDto[];
  assets: AssetDto[];
  rootTermId: number;
};

export type AssetVersionDto = {
  id: number;
  assetId: number;
  absolutePath: string;
  createdAt: string;
  fileExists: boolean;
  isCurrent: boolean;
};

export function assetListTree(): Promise<AssetTreeDto> {
  return invoke("asset_list_tree");
}

export function assetCreateFromPath(
  path: string,
  opts?: { sourcePath?: string | null; displayName?: string | null },
): Promise<AssetDto> {
  return invoke("asset_create_from_path", {
    path,
    sourcePath: opts?.sourcePath ?? null,
    displayName: opts?.displayName ?? null,
  });
}

export function fileNameFromPath(path: string | null | undefined): string {
  if (!path) return "";
  return path.split(/[/\\]/).pop() || path;
}

/** Parent directory of a registered path — open-dialog start for relocate (move or rename). */
export function parentDirOfPath(path: string | null | undefined): string | undefined {
  if (!path) return undefined;
  const i = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  if (i <= 0) return path;
  return path.slice(0, i);
}

export function indexInvalid(asset: AssetDto): boolean {
  return asset.indexStatus === "INVALID";
}

export function assetDelete(assetId: number): Promise<void> {
  return invoke("asset_delete", { assetId });
}

export function assetMove(assetId: number, browseTermId: number): Promise<void> {
  return invoke("asset_move", { assetId, browseTermId });
}

export function assetUnmount(
  assetId: number,
  browseTermId: number,
): Promise<{ unregistered: boolean }> {
  return invoke("asset_unmount", { assetId, browseTermId });
}

export type FolderImportResult = {
  created: number;
  mounted: number;
  skippedUnsupported: number;
  convertFailed: number;
  depthSkipped: number;
  messages: string[];
};

export function folderImport(
  dir: string,
  includeSubdirs: boolean,
): Promise<FolderImportResult> {
  return invoke("folder_import", { dir, includeSubdirs });
}

export function assetRename(assetId: number, displayName: string): Promise<void> {
  return invoke("asset_rename", { assetId, displayName });
}

export function assetRelocate(assetId: number, newPath: string): Promise<AssetDto> {
  return invoke("asset_relocate", { assetId, newPath });
}

export function assetFindByPath(path: string): Promise<AssetDto | null> {
  return invoke("asset_find_by_path", { path });
}

export function assetSaveNewVersion(assetId: number, content: string): Promise<AssetDto> {
  return invoke("asset_save_new_version", { assetId, content });
}

export function assetListVersions(assetId: number): Promise<AssetVersionDto[]> {
  return invoke("asset_list_versions", { assetId });
}

export function assetSetCurrentVersion(
  assetId: number,
  versionId: number,
): Promise<AssetDto> {
  return invoke("asset_set_current_version", { assetId, versionId });
}

export function revealInOs(path: string): Promise<void> {
  return invoke("reveal_in_os", { path });
}

export function termCreate(parentId: number, displayName: string): Promise<BrowseTermDto> {
  return invoke("term_create", { parentId, displayName });
}

export function termRename(termId: number, displayName: string): Promise<void> {
  return invoke("term_rename", { termId, displayName });
}

export function termMove(termId: number, newParentId: number): Promise<void> {
  return invoke("term_move", { termId, newParentId });
}

export function termDelete(termId: number): Promise<void> {
  return invoke("term_delete", { termId });
}
