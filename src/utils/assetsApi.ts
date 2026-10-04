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
};

export type AssetTreeDto = {
  terms: BrowseTermDto[];
  assets: AssetDto[];
  rootTermId: number;
};

export function assetListTree(): Promise<AssetTreeDto> {
  return invoke("asset_list_tree");
}

export function assetCreateFromPath(path: string, sourcePath?: string | null): Promise<AssetDto> {
  return invoke("asset_create_from_path", { path, sourcePath: sourcePath ?? null });
}

export function assetDelete(assetId: number): Promise<void> {
  return invoke("asset_delete", { assetId });
}

export function assetMove(assetId: number, browseTermId: number): Promise<void> {
  return invoke("asset_move", { assetId, browseTermId });
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
