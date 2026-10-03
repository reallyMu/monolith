#!/usr/bin/env node
/**
 * Single source: src/file-types.json → tauri.conf.json bundle.fileAssociations
 *
 * Tauri merges extension_to_uti() into every association. Mixing json+xml+html
 * in one entry poisons LSItemContentTypes (the md→plain-text class of bug).
 * This script emits one association per group and fails if a group would mix
 * distinct Tauri-inferred UTIs.
 */
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const typesPath = join(root, "src/file-types.json");
const confPath = join(root, "src-tauri/tauri.conf.json");

/** Mirrors tauri-utils extension_to_uti (must stay in sync with bundler). */
const TAURI_EXT_UTI = {
  svg: "public.svg-image",
  txt: "public.plain-text",
  html: "public.html",
  htm: "public.html",
  json: "public.json",
  xml: "public.xml",
};

const TAURI_MIME_UTI = {
  "text/plain": "public.plain-text",
  "text/html": "public.html",
  "application/json": "public.json",
  "application/xml": "public.xml",
  "text/xml": "public.xml",
  "image/svg+xml": "public.svg-image",
};

const doc = JSON.parse(readFileSync(typesPath, "utf8"));
const conf = JSON.parse(readFileSync(confPath, "utf8"));

const byGroup = new Map();
for (const t of doc.types) {
  if (!doc.groups[t.group]) {
    throw new Error(`file-types.json: type .${t.ext} references unknown group "${t.group}"`);
  }
  if (!byGroup.has(t.group)) byGroup.set(t.group, []);
  byGroup.get(t.group).push(t.ext);
}

const associations = [];
for (const [groupId, meta] of Object.entries(doc.groups)) {
  const exts = byGroup.get(groupId);
  if (!exts?.length) {
    throw new Error(`file-types.json: group "${groupId}" has no extensions`);
  }

  const inferred = new Set();
  for (const ext of exts) {
    const u = TAURI_EXT_UTI[ext];
    if (u) inferred.add(u);
  }
  if (meta.mimeType && TAURI_MIME_UTI[meta.mimeType]) {
    inferred.add(TAURI_MIME_UTI[meta.mimeType]);
  }
  for (const ct of meta.contentTypes ?? []) {
    inferred.add(ct);
  }

  if (inferred.size > 1) {
    throw new Error(
      `file-types.json: group "${groupId}" mixes UTIs ${[...inferred].join(", ")} ` +
        `(exts: ${exts.join(",")}). Split the group — Tauri merges all into LSItemContentTypes.`,
    );
  }

  const entry = {
    ext: exts,
    name: meta.name,
    description: meta.description ?? meta.name,
    role: meta.role ?? "Editor",
    rank: meta.rank ?? "Default",
  };
  if (meta.mimeType) entry.mimeType = meta.mimeType;
  if (meta.contentTypes?.length) entry.contentTypes = meta.contentTypes;
  associations.push(entry);
}

conf.bundle.fileAssociations = associations;
writeFileSync(confPath, `${JSON.stringify(conf, null, 2)}\n`);

const covered = new Set(doc.types.map((t) => t.ext));
console.log(
  `sync-file-associations: ${associations.length} groups, ${covered.size} extensions → tauri.conf.json`,
);
