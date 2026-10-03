import { Extension } from "@tiptap/core";
import { Plugin, PluginKey } from "@tiptap/pm/state";
import { Decoration, DecorationSet } from "@tiptap/pm/view";

export type SourceHighlightState = { index: number };

export const sourceHighlightKey = new PluginKey<SourceHighlightState>("mdSourceHighlight");

/** TipTap decoration for the active top-level block (survives ProseMirror DOM refresh). */
export const SourceHighlight = Extension.create({
  name: "sourceHighlight",
  addProseMirrorPlugins() {
    return [
      new Plugin<SourceHighlightState>({
        key: sourceHighlightKey,
        state: {
          init: () => ({ index: -1 }),
          apply(tr, prev) {
            const meta = tr.getMeta(sourceHighlightKey) as SourceHighlightState | undefined;
            return meta ?? prev;
          },
        },
        props: {
          decorations(state) {
            const { index } = sourceHighlightKey.getState(state) ?? { index: -1 };
            if (index < 0) return DecorationSet.empty;
            const decos: Decoration[] = [];
            let i = 0;
            state.doc.forEach((node, offset) => {
              if (i === index) {
                decos.push(Decoration.node(offset, offset + node.nodeSize, { class: "is-active" }));
              }
              i += 1;
            });
            return DecorationSet.create(state.doc, decos);
          },
        },
      }),
    ];
  },
});
