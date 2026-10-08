/** Smoke: math MD parse + katex (mermaid needs DOM — checked via import only). */
import MarkdownIt from "markdown-it";
import katex from "katex";
import { installMarkdownMath } from "../src/extensions/markdownMathPlugin.ts";

const sample = `
# Title

Inline $a+b$ here.

$$
\\int_{-\\infty}^{\\infty} e^{-x^2} dx = \\sqrt{\\pi}
$$

\`\`\`mermaid
graph TD
  A[Start] --> B{Ok?}
\`\`\`
`;

const md = new MarkdownIt({ html: false, linkify: true });
installMarkdownMath(md);
const html = md.render(sample);

if (!html.includes('data-type="math-block"')) {
  console.error("FAIL: math-block missing\n", html);
  process.exit(1);
}
if (!html.includes('data-type="math-inline"')) {
  console.error("FAIL: math-inline missing\n", html);
  process.exit(1);
}
if (!html.includes("language-mermaid")) {
  console.error("FAIL: mermaid fence missing\n", html);
  process.exit(1);
}

const mathLatex = html.match(/data-latex="([^"]+)"/g) || [];
if (mathLatex.length < 2) {
  console.error("FAIL: expected inline+block latex attrs", mathLatex);
  process.exit(1);
}

katex.renderToString("\\sqrt{\\pi}", { throwOnError: true, displayMode: true });
await import("mermaid");

console.log("ok: math parse + katex + mermaid import");
