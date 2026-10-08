# Monolith

**A tiny local text editor with a VS Code–class editing core — about 5 MB on disk.**

Monolith wraps [Monaco Editor](https://microsoft.github.io/monaco-editor/) (the same editing engine behind VS Code) in a native macOS shell built with [Tauri](https://tauri.app/) + Vue 3. You get familiar editing power without Electron’s hundreds of megabytes.

> **Credits:** Editing experience powered by **Monaco Editor** (Microsoft). Thank you to the Monaco / VS Code teams.

---

## Why Monolith?

| | VS Code | Monolith |
|---|---|---|
| Editing engine | Monaco | Monaco |
| Shell | Electron | Tauri (native WebView) |
| Installer size (arm64 DMG) | hundreds of MB | **~5 MB** |
| Extensions / marketplace | yes | no — stay small |
| Always online / account | optional | **fully offline, local files only** |

Monolith is for people who want to **open, read, and edit text files quickly** — markdown notes, configs, logs, source — without launching a full IDE.

---

## Features

### Editor (Monaco)
- Syntax highlighting for common languages (TypeScript/JavaScript, Python, Rust, Go, Java/Kotlin, C/C++, SQL, Shell, Ruby, PHP, Swift, R, HTML/CSS, YAML, TOML/INI, GraphQL, Dockerfile, and more)
- Multi-tab editing, undo/redo, find, indent/outdent
- SQL format action when the buffer is SQL
- Recent files menu

### Markdown
- Split view: Monaco source ↔ WYSIWYG preview (TipTap)
- Block-level source map sync and highlight between panes
- Edit from either side when in split / preview modes

### Previews
- **Markdown** — rich WYSIWYG
- **HTML** — sandboxed preview
- **JSON / XML** — structured tree preview
- Other types — editor-focused view

### Files & OS integration (macOS)
- Open / save / save as for UTF-8 text
- Finder **Open With** / double-click for registered text types
- Default-handler helper script after install
- Rejects binary payloads (null bytes) instead of corrupting the buffer
- Brings the app to the front when a file is opened from Finder

### Local document asset library (v0.2)
- Build a **local document asset catalog** indexed to real files — **no file moves / copies**
- Stay in sync with the original path (save writes back; index tracks validity)
- Global browse tree + **name search** + **version history** (“Save New Version” beside the file)
- Delete asset = unregister only (never deletes the disk file)

### Convert office / PDF → Markdown
- Turn **Word / Excel / PDF / PPT** into Markdown for reading and asset management (on top of v0.1 text editing)

### Chrome web clipper
- **Monolith Web Clipper** saves pages as Markdown into Inbox → open / register as assets (install from Settings)

### MCP for AI Agents
- **`monolith-mcp`** lets Agents list / search / read the same asset library — so Agents can **find and read your local docs accurately**
- Discover Agents, install MCP, and edit Skill from Settings
- Install writes a no-space symlink `~/.local/bin/monolith-mcp` into Agent configs (avoids MCP discovery failures on `Application Support` paths)

### Local & private
- No telemetry, no cloud sync, no account
- All I/O is local filesystem via Tauri commands

---

## Supported text types

Monolith registers **59 extensions** (see `src/file-types.json`), including:

`.md` `.markdown` `.json` `.jsonc` `.yml` `.yaml` `.xml` `.svg` `.html` `.css` `.ts` `.tsx` `.js` `.jsx` `.py` `.rs` `.go` `.java` `.kt` `.c` `.cpp` `.sql` `.sh` `.vue` `.toml` `.ini` `.csv` `.graphql` …

Plus special basenames such as `Dockerfile` / `Makefile`.

> Not every byte on disk is “text.” Binary files are refused. Unknown extensions are not silently treated as plain text — the allow-list in `file-types.json` is the source of truth.

---

## Download

**Release:** [v0.2.2](https://github.com/reallyMu/monolith/releases/tag/v0.2.2) — [release notes](docs/RELEASE-NOTES-v0.2.2.md)

| Platform | Asset | Notes |
|---|---|---|
| **macOS** (Apple Silicon) | `Monolith_0.2.2_aarch64.dmg` | Ad-hoc signed (not notarized). First open: **Privacy & Security → Open Anyway**. |
| **Windows** (x64) | `Monolith_0.2.2_x64-setup.exe` (NSIS) | Built by GitHub Actions. Requires [WebView2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) (usually preinstalled on Windows 10/11). |

Windows installers are produced by [.github/workflows/build-windows.yml](.github/workflows/build-windows.yml) (`workflow_dispatch` or `v*` tags).

### macOS: re-bind file defaults

```bash
swift scripts/macos-set-default-handlers.swift src/file-types.json
```

---

## Develop

Requirements: Node.js 20+, Rust stable, macOS with Xcode CLT.

```bash
npm install
npm run tauri dev
```

Release build:

```bash
npm run tauri build
```

`npm run build` syncs macOS file associations from `src/file-types.json` into `src-tauri/tauri.conf.json` (keeps Finder types and Monaco languages aligned).

---

## Architecture (short)

```text
Finder / Open With
       │
       ▼
  Tauri (Rust) ── read/write UTF-8, quarantine clear, activate window
       │
       ▼
  Vue 3 UI ── tabs, toolbar, i18n
       │
       ├── Monaco Editor (source)
       └── TipTap / HTML / tree previews
```

Design notes live under `docs/superpowers/specs/` (document assets: `2026-10-04-local-document-asset-design.md`).

---

## License

[MIT](./LICENSE) © reallyMu

Monaco Editor is used under its own license ([MIT](https://github.com/microsoft/monaco-editor/blob/main/LICENSE)). See third-party notices in dependency licenses after `npm install` / `cargo tree`.

---

## Roadmap (ideas)

- [ ] Apple Developer ID + notarization for Gatekeeper-clean distribution
- [ ] Intel (`x86_64`) macOS build
- [x] Windows x64 NSIS installer (CI)
- [ ] Optional Linux target via Tauri
- [ ] User-configurable association list beyond the built-in allow-list
- [ ] Single-instance / second-file open while already running on Windows

---

## Acknowledgements

- **[Monaco Editor](https://github.com/microsoft/monaco-editor)** — the editing core that makes Monolith feel like a slice of VS Code
- **[Tauri](https://tauri.app/)** — tiny native shell
- **[Vue](https://vuejs.org/)** · **[TipTap](https://tiptap.dev/)** · **[markdown-it](https://github.com/markdown-it/markdown-it)**
