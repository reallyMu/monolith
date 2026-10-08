import {
  findMathAtOffset,
  findMathByLatex,
  listMathRanges,
  replaceMathRange,
  wrapMath,
} from "../src/utils/mdMathRange.ts";

function assert(cond, msg) {
  if (!cond) {
    console.error("FAIL:", msg);
    process.exit(1);
  }
}

const sample = `# T

Inline $a+b$ here.

$$
\\int_{-\\infty}^{\\infty} e^{-x^2} dx = \\sqrt{\\pi}
$$

\`\`\`js
const x = '$not$';
\`\`\`

Again $a+b$.
`;

const ranges = listMathRanges(sample);
assert(ranges.length === 3, `expected 3 math ranges, got ${ranges.length}`);
assert(ranges[0].mode === "inline" && ranges[0].latex === "a+b", "first inline");
assert(ranges[1].mode === "block" && ranges[1].latex.includes("\\int"), "block int");
assert(ranges[2].mode === "inline" && ranges[2].latex === "a+b", "second inline");

const insideInline = sample.indexOf("$a+b$") + 2;
const hit = findMathAtOffset(sample, insideInline);
assert(hit?.mode === "inline" && hit.latex === "a+b", "find at offset inline");

const insideBlock = sample.indexOf("\\int");
assert(findMathAtOffset(sample, insideBlock)?.mode === "block", "find at offset block");

assert(!findMathAtOffset(sample, sample.indexOf("'$not$'") + 2), "fence $ ignored");

assert(wrapMath("x", "block") === "$$\nx\n$$", "wrap block");
assert(wrapMath("x", "inline") === "$x$", "wrap inline");

const second = findMathByLatex(sample, "a+b", "inline", sample.lastIndexOf("$a+b$"));
assert(second?.start === ranges[2].start, "preferStart picks second inline");

const next = replaceMathRange(sample, ranges[0], "c", "inline");
assert(next.includes("$c$") && !next.includes("$a+b$ here"), "replace first inline");

console.log("ok: mdMathRange");
