/**
 * Monolith acceptance — unit + optional live (playwright).
 *
 *   node --experimental-strip-types scripts/acceptance.mjs
 *   MD_ACCEPTANCE=1 node --experimental-strip-types scripts/acceptance.mjs
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

1. alpha
2. beta

${Array.from({ length: 20 }, (_, i) => `para ${i}`).join("\n\n")}
`;

function unit() {
  const spans = sourceSpansFromMarkdown(FIXTURE);
  const h2 = FIXTURE.split("\n").findIndex((l) => l.startsWith("## 9")) + 1;
  assert.equal(spanForLine(spans, h2)?.start, h2);
  assert.ok(spanForLine(spans, 10)?.start !== h2);
  const olLine = FIXTURE.split("\n").findIndex((l) => l.startsWith("1. alpha")) + 1;
  const olSpan = spanForLine(spans, olLine);
  assert.ok(olSpan);
  assert.ok(olSpan.end >= olLine + 1);
  console.log("unit: ok spans", spans.length);
}

async function resolveBase() {
  for (const b of ["http://localhost:1420/", "http://127.0.0.1:1420/", "http://[::1]:1420/"]) {
    try {
      if ((await fetch(b)).ok) return b;
    } catch {
      /* */
    }
  }
  return null;
}

/** In-page suite (stringified into playwright evaluate). */
function liveSuiteSource() {
  return `async () => {
  const wait = (ms) => new Promise((r) => setTimeout(r, ms));
  const st = document.querySelector("#app").__vue_app__._instance.setupState;
  const fails = [];
  const ok = (name, cond, detail) => { if (!cond) fails.push({ name, detail: detail ?? null }); };
  const ed = () => (st.editorRef?.value ?? st.editorRef).getEditor();
  const sample = ${JSON.stringify(FIXTURE)};

  const tab = st.tabs[0];
  tab.language = "markdown"; tab.path = null; tab.viewMode = "split"; tab.content = sample;
  await wait(200);
  const editorApi = st.editorRef?.value ?? st.editorRef;
  const wys = st.mdWysiwygRef?.value ?? st.mdWysiwygRef;
  ed().setValue(sample); await wait(150);
  const prose = document.querySelector(".md-wysiwyg-prose");
  ok("tiptap", !!prose);
  ok("editable", prose?.getAttribute("contenteditable") === "true");
  ok("no-md-preview", !document.querySelector(".md-preview"));

  st.mdFocus = "wysiwyg"; wys.insertText("EDIT_RIGHT "); await wait(100);
  ok("right→left", tab.content.includes("EDIT_RIGHT"));

  tab.content = sample; ed().setValue(sample); await wait(120);
  const h2line = sample.split("\\n").findIndex((l) => l.startsWith("## 9")) + 1;
  st.mdDriver = "source"; st.mdFocus = "source";
  ed().setPosition({ lineNumber: h2line, column: 1 });
  ed().revealLineNearTop(h2line); tab.cursorLine = h2line; await wait(80);
  wys.alignToSourceLine(h2line, editorApi.getLineViewportTop(h2line)); await wait(220);
  const active = prose.querySelector(".is-active");
  ok("highlight", !!(active && active.textContent.includes("9. DBX")));
  const h2el = [...prose.children].find((n) => (n.textContent || "").includes("9. DBX"));
  const box = document.querySelector(".md-wysiwyg").getBoundingClientRect();
  const hr = h2el.getBoundingClientRect();
  const mp = ed().getScrolledVisiblePosition({ lineNumber: h2line, column: 1 });
  const delta = Math.round(Math.abs(hr.top - box.top - (mp?.top ?? 0)));
  ok("align", delta <= 6, delta);

  st.mdDriver = "preview";
  ed().setPosition({ lineNumber: 1, column: 1 }); await wait(40);
  const bqline = sample.split("\\n").findIndex((l) => l.startsWith("> ")) + 1;
  [...prose.children].find((n) => n.tagName === "BLOCKQUOTE").click(); await wait(100);
  ok("click", ed().getPosition().lineNumber === bqline);

  const preview = document.querySelector(".md-wysiwyg");
  preview.scrollTop = 0;
  const a0 = wys.anchorAtViewport(8);
  st.onMdPreviewScroll({ scrollTop: 0, scrollHeight: preview.scrollHeight, clientHeight: preview.clientHeight, anchorLine: a0.line, anchorViewportTop: a0.viewportTop });
  await wait(60);
  const top = ed().getPosition().lineNumber;
  preview.scrollTop = 800;
  const a1 = wys.anchorAtViewport(8);
  st.onMdPreviewScroll({ scrollTop: 800, scrollHeight: preview.scrollHeight, clientHeight: preview.clientHeight, anchorLine: a1.line, anchorViewportTop: a1.viewportTop });
  await wait(60);
  ok("scroll", ed().getPosition().lineNumber > top && ed().getPosition().lineNumber === a1.line);

  tab.viewMode = "edit"; await wait(100);
  ed().setValue("1. first\\n2. second\\n"); tab.content = "1. first\\n2. second\\n";
  ed().setPosition({ lineNumber: 3, column: 1 }); await wait(40); st.mdOl(); await wait(60);
  ok("ol", tab.content.split("\\n")[2]?.startsWith("3. "));

  ed().setValue("hello"); tab.content = "hello";
  ed().setSelection({ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 6 });
  st.mdFocus = "source"; st.mdBold(); await wait(50);
  ok("bold", tab.content.includes("**hello**"));

  tab.language = "sql"; tab.path = "t.sql";
  ed().setValue("select a,b from t where x=1;"); tab.content = "select a,b from t where x=1;";
  await wait(40); st.formatSql(); await wait(60);
  ok("sql", tab.content.includes("\\n"));

  tab.language = "markdown"; tab.path = null; tab.viewMode = "preview";
  tab.content = "# Only\\n\\nbody"; await wait(180);
  ok("preview-edit", document.querySelector(".md-wysiwyg-prose")?.getAttribute("contenteditable") === "true");
  ok("i18n", st.t("preview") === "预览");

  const n0 = st.tabs.length; st.createUntitled(); await wait(60);
  ok("new-tab", st.tabs.length === n0 + 1);

  tab.viewMode = "edit"; await wait(80);
  ed().setValue(""); tab.content = ""; st.insertMdTable(2, 3); await wait(50);
  ok("table", tab.content.includes("|") && tab.content.includes("---"));

  return { pass: fails.length === 0, fails, delta };
}`;
}

unit();

if (process.env.MD_ACCEPTANCE === "1") {
  let chromium;
  try {
    ({ chromium } = await import("playwright"));
  } catch {
    console.error("FAIL: install playwright for live acceptance");
    process.exit(1);
  }
  const base = await resolveBase();
  if (!base) {
    console.error("FAIL: dev server not on :1420");
    process.exit(1);
  }
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.goto(base, { waitUntil: "networkidle" });
  const result = await page.evaluate(liveSuiteSource());
  await browser.close();
  console.log(JSON.stringify(result, null, 2));
  if (!result.pass) process.exit(1);
  console.log("live: ALL PASSED");
} else {
  const base = await resolveBase();
  console.log(base ? `dev server: ${base}` : "dev server: not running");
}

console.log("ALL PASSED");
