# Monolith Clipper

Browser extension that clips the current page to Markdown under `~/Downloads/MonolithInbox/` for the [Monolith](../../README.md) desktop app to watch and import.

This is a thin fork of [Obsidian Web Clipper](https://github.com/obsidianmd/obsidian-clipper) (**MIT License**, Copyright (c) 2024 Obsidian). Obsidian trademarks and official branding assets are **not** used.

## Build (Chrome)

```bash
cd extensions/monolith-clipper
npm install
npm run build:chrome
```

Output: `dist/chrome/` — load unpacked in `chrome://extensions` (Developer mode).

## Install from Monolith App

Use **Install Web Clipper** in Monolith (releases the built extension to `~/Library/Application Support/com.muqiang.monolith/MonolithClipper/` and opens Finder + Chrome extensions). Mainland China: no Chrome Web Store required.
