import * as monaco from "monaco-editor";
import editorWorker from "monaco-editor/esm/vs/editor/editor.worker?worker";
import jsonWorker from "monaco-editor/esm/vs/language/json/json.worker?worker";
import cssWorker from "monaco-editor/esm/vs/language/css/css.worker?worker";
import htmlWorker from "monaco-editor/esm/vs/language/html/html.worker?worker";
import tsWorker from "monaco-editor/esm/vs/language/typescript/ts.worker?worker";

let configured = false;

export function ensureMonaco(): typeof monaco {
  if (!configured) {
    (self as unknown as { MonacoEnvironment: unknown }).MonacoEnvironment = {
      getWorker(_: unknown, label: string) {
        if (label === "json") return new jsonWorker();
        if (label === "css" || label === "scss" || label === "less") return new cssWorker();
        if (label === "html" || label === "handlebars" || label === "razor") return new htmlWorker();
        if (label === "typescript" || label === "javascript") return new tsWorker();
        return new editorWorker();
      },
    };
    configured = true;
  }
  return monaco;
}

export function applyEditorTheme(opts: {
  /** Solid pane background — avoid transparent; WKWebView/Tauri ghosts glyphs over layered fills. */
  background: string;
  foreground: string;
  lineHighlightBg: string;
  lineHighlightBorder?: string;
  selectionBg?: string;
}) {
  const m = ensureMonaco();
  const bg = opts.background?.startsWith("#") ? opts.background : "#1a1d23";
  m.editor.defineTheme("monolith-dark", {
    base: "vs-dark",
    inherit: true,
    rules: [{ token: "", foreground: opts.foreground.replace("#", "") }],
    colors: {
      "editor.background": bg.length === 7 || bg.length === 9 ? bg : "#1a1d23",
      "editor.foreground": opts.foreground,
      "editorLineNumber.foreground": "#8b93a3",
      "editorLineNumber.activeForeground": opts.foreground,
      // hex only — rgba() is invalid in Monaco themes and can render as loud red
      "editor.selectionBackground": opts.selectionBg ?? "#6a8ab048",
      "editor.lineHighlightBackground": opts.lineHighlightBg || "#00000000",
      "editor.lineHighlightBorder": opts.lineHighlightBorder ?? "#8aa4c499",
      "editorCursor.foreground": opts.foreground,
    },
  });
  m.editor.setTheme("monolith-dark");
}

export { monaco };
