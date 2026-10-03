/**
 * MD split sync checks (TipTap right pane).
 *   node --experimental-strip-types scripts/test-md-sync.mjs
 * Optional browser e2e (dev on :1420 + playwright):
 *   MD_SYNC_E2E=1 node --experimental-strip-types scripts/test-md-sync.mjs
 */
import assert from "node:assert/strict";

const { sourceSpansFromMarkdown, spanForLine } = await import("../src/utils/mdSourceMap.ts");

const FIXTURE = `# Title

intro

\`\`\`
AAAAAAAA
BBBBBBBB
CCCCCCCC
DDDDDDDD
EEEEEEEE
FFFFFFFF
GGGGGGGG
HHHHHHHH
IIIIIIII
JJJJJJJJ
KKKKKKKK
LLLLLLLL
\`\`\`

## 9. DBX (design)

> note

| a | b |
|---|---|
| 1 | 2 |

${Array.from({ length: 30 }, (_, i) => `para ${i}`).join("\n\n")}
`;

function fail(msg) {
  console.error("FAIL:", msg);
  process.exit(1);
}

function unitTests() {
  const lines = FIXTURE.split("\n");
  const h2 = lines.findIndex((l) => l.startsWith("## 9. DBX")) + 1;
  assert.ok(h2 > 0, "fixture h2");

  const spans = sourceSpansFromMarkdown(FIXTURE);
  assert.ok(spans.length >= 4, `expected several spans, got ${spans.length}`);

  const h2Span = spanForLine(spans, h2);
  assert.ok(h2Span, "h2 span");
  assert.equal(h2Span.start, h2, `h2 mapped to ${h2Span.start}, want ${h2}`);

  const fenceLine = 10;
  const fenceSpan = spanForLine(spans, fenceLine);
  assert.ok(fenceSpan, "fence span");
  assert.ok(fenceSpan.start <= fenceLine && fenceLine <= fenceSpan.end);
  assert.notEqual(fenceSpan.start, h2);

  const bqLine = lines.findIndex((l) => l.startsWith("> ")) + 1;
  const bqSpans = spans.filter((s) => s.start === bqLine);
  assert.equal(bqSpans.length, 1, `blockquote should be one span, got ${bqSpans.length}`);

  console.log("unit: ok — spans", spans.length, "h2@", h2);
}

async function resolveDevBase() {
  for (const base of ["http://localhost:1420/", "http://127.0.0.1:1420/", "http://[::1]:1420/"]) {
    try {
      const r = await fetch(base);
      if (r.ok) return base;
    } catch {
      /* try next */
    }
  }
  return null;
}

async function e2eTests() {
  let chromium;
  try {
    ({ chromium } = await import("playwright"));
  } catch {
    fail("e2e requested but playwright is not installed");
  }

  const base = await resolveDevBase();
  if (!base) fail("dev server not up on :1420");

  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.goto(base, { waitUntil: "networkidle" });

  const result = await page.evaluate(async (sample) => {
    const wait = (ms) => new Promise((r) => setTimeout(r, ms));
    const st = document.querySelector("#app").__vue_app__._instance.setupState;
    const tab = st.tabs[0];
    tab.content = sample;
    tab.viewMode = "split";
    tab.language = "markdown";
    await wait(120);
    const editorApi = st.editorRef?.value ?? st.editorRef;
    const previewApi = st.mdWysiwygRef?.value ?? st.mdWysiwygRef;
    const e = editorApi.getEditor();
    e.setValue(sample);
    await wait(100);
    const h2line = sample.split("\n").findIndex((l) => l.startsWith("## 9")) + 1;
    const bqline = sample.split("\n").findIndex((l) => l.startsWith("> ")) + 1;
    st.mdDriver = "source";
    e.setPosition({ lineNumber: h2line, column: 1 });
    e.revealLineNearTop(h2line);
    tab.cursorLine = h2line;
    await wait(80);
    previewApi.alignToSourceLine(h2line, editorApi.getLineViewportTop(h2line));
    await wait(200);
    const preview = document.querySelector(".md-wysiwyg");
    const prose = document.querySelector(".md-wysiwyg-prose");
    const active = prose?.querySelector(".is-active");
    const h2 = [...(prose?.children || [])].find((n) => (n.textContent || "").includes("9. DBX"));
    const pr = preview.getBoundingClientRect();
    const hr = h2.getBoundingClientRect();
    const monacoPos = e.getScrolledVisiblePosition({ lineNumber: h2line, column: 1 });
    const delta = Math.abs(hr.top - pr.top - (monacoPos?.top ?? 0));

    st.mdDriver = "preview";
    const bq = [...prose.children].find((n) => n.tagName === "BLOCKQUOTE");
    bq.click();
    await wait(80);
    const clickLine = e.getPosition().lineNumber;

    const editable = prose?.getAttribute("contenteditable") === "true";

    return {
      delta: Math.round(delta),
      activeIsH2: !!(active && active.textContent.includes("9. DBX")),
      clickLine,
      bqline,
      editable,
      hasWysiwyg: !!preview,
      noReadonlyPreview: !document.querySelector(".md-preview"),
    };
  }, FIXTURE);

  await browser.close();

  assert.equal(result.hasWysiwyg, true);
  assert.equal(result.noReadonlyPreview, true);
  assert.equal(result.editable, true);
  assert.equal(result.activeIsH2, true);
  assert.ok(result.delta <= 6, `align delta ${result.delta}`);
  assert.equal(result.clickLine, result.bqline);
  console.log("e2e: ok —", result);
}

unitTests();
if (process.env.MD_SYNC_E2E === "1") await e2eTests();
else {
  const base = await resolveDevBase();
  if (base) console.log(`dev server: reachable at ${base} (set MD_SYNC_E2E=1 for full browser test)`);
  else console.log("dev server: not running (unit tests only)");
}

console.log("ALL PASSED");
