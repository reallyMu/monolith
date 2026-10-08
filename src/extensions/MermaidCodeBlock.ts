import CodeBlock from "@tiptap/extension-code-block";

/**
 * Mermaid fence → SVG preview.
 * Interaction (kept minimal — no TipTap event wrestling):
 *   − / + / 1:1  → zoom
 *   ⌘/Ctrl+wheel → zoom
 *   plain scroll  → pan (native overflow)
 */

let mermaidReady = false;

async function loadMermaid() {
  const mermaid = (await import("mermaid")).default;
  if (!mermaidReady) {
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: "strict",
      theme: "neutral",
    });
    mermaidReady = true;
  }
  return mermaid;
}

let renderSeq = 0;

const ZOOM_MIN = 0.25;
const ZOOM_MAX = 4;
const ZOOM_STEP = 0.1;

function isMermaidLang(lang: unknown): boolean {
  return (
    String(lang || "")
      .trim()
      .toLowerCase() === "mermaid"
  );
}

function clampZoom(z: number): number {
  return Math.min(ZOOM_MAX, Math.max(ZOOM_MIN, Math.round(z * 100) / 100));
}

export const MermaidCodeBlock = CodeBlock.extend({
  addNodeView() {
    return ({ node }) => {
      if (!isMermaidLang(node.attrs.language)) {
        const pre = document.createElement("pre");
        const code = document.createElement("code");
        if (node.attrs.language) {
          code.className = `language-${node.attrs.language}`;
        }
        code.textContent = node.textContent;
        pre.append(code);
        return { dom: pre, contentDOM: code };
      }

      const dom = document.createElement("div");
      dom.className = "mermaid-block";
      dom.setAttribute("data-language", "mermaid");
      dom.contentEditable = "false";

      const toolbar = document.createElement("div");
      toolbar.className = "mermaid-zoom-bar";

      const btnOut = document.createElement("button");
      btnOut.type = "button";
      btnOut.className = "mermaid-zoom-btn";
      btnOut.title = "Zoom out";
      btnOut.textContent = "−";

      const label = document.createElement("span");
      label.className = "mermaid-zoom-label";

      const btnIn = document.createElement("button");
      btnIn.type = "button";
      btnIn.className = "mermaid-zoom-btn";
      btnIn.title = "Zoom in";
      btnIn.textContent = "+";

      const btnReset = document.createElement("button");
      btnReset.type = "button";
      btnReset.className = "mermaid-zoom-btn";
      btnReset.title = "Reset zoom";
      btnReset.textContent = "1:1";

      toolbar.append(btnOut, label, btnIn, btnReset);

      const viewport = document.createElement("div");
      viewport.className = "mermaid-viewport";
      viewport.title = "Scroll to pan · ⌘+scroll to zoom";

      const stage = document.createElement("div");
      stage.className = "mermaid-stage";
      stage.textContent = "…";
      viewport.append(stage);
      dom.append(toolbar, viewport);

      let cancelled = false;
      let lastSrc = "";
      let zoom = 1;
      let naturalW = 0;
      let naturalH = 0;

      const applyZoom = () => {
        zoom = clampZoom(zoom);
        label.textContent = `${Math.round(zoom * 100)}%`;
        btnOut.disabled = zoom <= ZOOM_MIN;
        btnIn.disabled = zoom >= ZOOM_MAX;
        const svg = stage.querySelector("svg");
        if (!svg || naturalW <= 0 || naturalH <= 0) return;
        svg.setAttribute("width", String(naturalW * zoom));
        svg.setAttribute("height", String(naturalH * zoom));
      };

      const setZoom = (z: number) => {
        zoom = z;
        applyZoom();
      };

      const onBtn = (el: HTMLButtonElement, fn: () => void) => {
        el.addEventListener("click", (e) => {
          e.preventDefault();
          e.stopPropagation();
          fn();
        });
      };
      onBtn(btnOut, () => setZoom(zoom - ZOOM_STEP));
      onBtn(btnIn, () => setZoom(zoom + ZOOM_STEP));
      onBtn(btnReset, () => setZoom(1));

      viewport.addEventListener(
        "wheel",
        (e) => {
          if (!(e.metaKey || e.ctrlKey)) return; // plain wheel → native scroll pan
          e.preventDefault();
          e.stopPropagation();
          setZoom(zoom + (e.deltaY > 0 ? -ZOOM_STEP : ZOOM_STEP));
        },
        { passive: false },
      );

      applyZoom();

      const paint = (src: string) => {
        if (src === lastSrc) return;
        lastSrc = src;
        const id = `mmd-${++renderSeq}`;
        void loadMermaid()
          .then((mermaid) => mermaid.render(id, src))
          .then(({ svg }) => {
            if (cancelled) return;
            dom.classList.remove("mermaid-error");
            stage.innerHTML = svg;
            const el = stage.querySelector("svg");
            if (el) {
              const vb = el.viewBox?.baseVal;
              naturalW = (vb && vb.width) || Number(el.getAttribute("width")) || el.clientWidth || 0;
              naturalH = (vb && vb.height) || Number(el.getAttribute("height")) || el.clientHeight || 0;
              if (naturalW <= 0 || naturalH <= 0) {
                const r = el.getBoundingClientRect();
                naturalW = r.width || 400;
                naturalH = r.height || 300;
              }
            }
            applyZoom();
          })
          .catch((err: unknown) => {
            if (cancelled) return;
            dom.classList.add("mermaid-error");
            naturalW = 0;
            naturalH = 0;
            stage.textContent = err instanceof Error ? err.message : String(err);
          });
      };

      paint(node.textContent || "");

      return {
        dom,
        stopEvent: () => true,
        ignoreMutation: () => true,
        update: (updated) => {
          if (updated.type.name !== "codeBlock") return false;
          if (!isMermaidLang(updated.attrs.language)) return false;
          paint(updated.textContent || "");
          return true;
        },
        destroy: () => {
          cancelled = true;
        },
      };
    };
  },
});
