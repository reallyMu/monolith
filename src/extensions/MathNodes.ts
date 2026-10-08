import { mergeAttributes, Node } from "@tiptap/core";
import katex from "katex";
import { installMarkdownMath } from "./markdownMathPlugin";

function renderKatex(dom: HTMLElement, latex: string, displayMode: boolean) {
  dom.classList.remove("math-error");
  try {
    katex.render(latex || "", dom, {
      displayMode,
      throwOnError: false,
      strict: "ignore",
    });
  } catch (e) {
    dom.classList.add("math-error");
    dom.textContent = latex || String(e);
  }
}

export const MathBlock = Node.create({
  name: "mathBlock",
  group: "block",
  atom: true,
  selectable: true,
  draggable: false,

  addAttributes() {
    return {
      latex: {
        default: "",
        parseHTML: (el) => el.getAttribute("data-latex") || "",
        renderHTML: (attrs) => ({ "data-latex": attrs.latex || "" }),
      },
    };
  },

  parseHTML() {
    return [{ tag: 'div[data-type="math-block"]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["div", mergeAttributes({ "data-type": "math-block" }, HTMLAttributes)];
  },

  addNodeView() {
    return ({ node, getPos }) => {
      const dom = document.createElement("div");
      dom.className = "math-block";
      dom.setAttribute("data-type", "math-block");
      dom.title = "Double-click to edit";
      let latex = node.attrs.latex as string;
      renderKatex(dom, latex, true);
      const onDbl = (e: MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        const pos = typeof getPos === "function" ? getPos() : null;
        dom.dispatchEvent(
          new CustomEvent("monolith-math-edit", {
            bubbles: true,
            detail: { latex, mode: "block" as const, pos: typeof pos === "number" ? pos : null },
          }),
        );
      };
      dom.addEventListener("dblclick", onDbl);
      return {
        dom,
        update: (updated) => {
          if (updated.type.name !== "mathBlock") return false;
          latex = updated.attrs.latex as string;
          renderKatex(dom, latex, true);
          return true;
        },
        destroy: () => dom.removeEventListener("dblclick", onDbl),
        ignoreMutation: () => true,
        stopEvent: () => false,
      };
    };
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: { write: (s: string) => void; closeBlock: (n: unknown) => void }, node: { attrs: { latex: string } }) {
          const latex = (node.attrs.latex || "").trim();
          state.write("$$\n" + latex + "\n$$");
          state.closeBlock(node);
        },
        parse: {
          setup(markdownit: Parameters<typeof installMarkdownMath>[0]) {
            installMarkdownMath(markdownit);
          },
        },
      },
    };
  },
});

export const MathInline = Node.create({
  name: "mathInline",
  group: "inline",
  inline: true,
  atom: true,
  selectable: true,

  addAttributes() {
    return {
      latex: {
        default: "",
        parseHTML: (el) => el.getAttribute("data-latex") || "",
        renderHTML: (attrs) => ({ "data-latex": attrs.latex || "" }),
      },
    };
  },

  parseHTML() {
    return [{ tag: 'span[data-type="math-inline"]' }];
  },

  renderHTML({ HTMLAttributes }) {
    return ["span", mergeAttributes({ "data-type": "math-inline" }, HTMLAttributes)];
  },

  addNodeView() {
    return ({ node, getPos }) => {
      const dom = document.createElement("span");
      dom.className = "math-inline";
      dom.setAttribute("data-type", "math-inline");
      dom.title = "Double-click to edit";
      let latex = node.attrs.latex as string;
      renderKatex(dom, latex, false);
      const onDbl = (e: MouseEvent) => {
        e.preventDefault();
        e.stopPropagation();
        const pos = typeof getPos === "function" ? getPos() : null;
        dom.dispatchEvent(
          new CustomEvent("monolith-math-edit", {
            bubbles: true,
            detail: { latex, mode: "inline" as const, pos: typeof pos === "number" ? pos : null },
          }),
        );
      };
      dom.addEventListener("dblclick", onDbl);
      return {
        dom,
        update: (updated) => {
          if (updated.type.name !== "mathInline") return false;
          latex = updated.attrs.latex as string;
          renderKatex(dom, latex, false);
          return true;
        },
        destroy: () => dom.removeEventListener("dblclick", onDbl),
        ignoreMutation: () => true,
        stopEvent: () => false,
      };
    };
  },

  addStorage() {
    return {
      markdown: {
        serialize(state: { write: (s: string) => void }, node: { attrs: { latex: string } }) {
          state.write("$" + (node.attrs.latex || "") + "$");
        },
        parse: {
          setup(markdownit: Parameters<typeof installMarkdownMath>[0]) {
            installMarkdownMath(markdownit);
          },
        },
      },
    };
  },
});
