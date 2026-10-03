import fileTypes from "../file-types.json";

type PreviewKind = "markdown" | "html" | "json" | "xml" | "code";

type FileTypeRow = {
  ext: string;
  language: string;
  preview: PreviewKind;
  group: string;
};

type FileTypesDoc = {
  specialNames: Record<string, { language: string; preview: PreviewKind }>;
  untitledLanguage: string;
  types: FileTypeRow[];
  groups: Record<
    string,
    {
      name: string;
      description?: string;
      role?: string;
      rank?: string;
      mimeType?: string;
      contentTypes?: string[];
    }
  >;
  dialogFilterGroups: { name: string; groups: string[] }[];
};

const doc = fileTypes as FileTypesDoc;

const BY_EXT = new Map(doc.types.map((t) => [t.ext, t]));

/** Extensions Monaco path mapping supports (open dialog / Finder). */
export const SUPPORTED_EXTENSIONS = doc.types.map((t) => t.ext).sort();

export type { PreviewKind };

function basename(path: string): string {
  return path.split(/[/\\]/).pop() ?? "";
}

export function extensionOf(path: string | null | undefined): string | null {
  if (!path) return null;
  const lower = basename(path).toLowerCase();
  if (lower in doc.specialNames) return lower;
  const dot = lower.lastIndexOf(".");
  if (dot < 0) return null;
  return lower.slice(dot + 1);
}

function lookup(path: string | null | undefined): {
  language: string;
  preview: PreviewKind;
} | null {
  if (!path) return null;
  const lower = basename(path).toLowerCase();
  const special = doc.specialNames[lower];
  if (special) return special;
  const ext = extensionOf(path);
  if (!ext) return null;
  const row = BY_EXT.get(ext);
  if (!row) return null;
  return { language: row.language, preview: row.preview };
}

/** Monaco language id. Untitled → configured default. Unknown path must not be opened. */
export function languageFromPath(path: string | null | undefined): string {
  if (!path) return doc.untitledLanguage;
  const hit = lookup(path);
  if (!hit) {
    throw new Error(`Unsupported file type: ${basename(path)}`);
  }
  return hit.language;
}

export function isSupportedPath(path: string | null | undefined): boolean {
  if (!path) return true;
  return lookup(path) !== null;
}

export function isMarkdownPath(path: string | null | undefined): boolean {
  if (!path) return true;
  return lookup(path)?.preview === "markdown";
}

export function previewKind(path: string | null | undefined): PreviewKind {
  if (!path) return "markdown";
  const hit = lookup(path);
  if (!hit) {
    throw new Error(`Unsupported file type: ${basename(path)}`);
  }
  return hit.preview;
}

/** Tauri / native open-dialog filters — derived from file-types.json only. */
export function openDialogFilters(): { name: string; extensions: string[] }[] {
  return doc.dialogFilterGroups.map((g) => {
    const exts =
      g.groups.length === 1 && g.groups[0] === "*"
        ? [...SUPPORTED_EXTENSIONS]
        : doc.types.filter((t) => g.groups.includes(t.group)).map((t) => t.ext);
    return { name: g.name, extensions: [...new Set(exts)].sort() };
  });
}

export function titleFromPath(path: string | null, untitledIndex: number): string {
  if (!path) return `Untitled-${untitledIndex}`;
  return basename(path) || path;
}
