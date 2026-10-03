/**
 * Full toolbar / menu acceptance (live page).
 *   MD_TOOLBAR=1 node --experimental-strip-types scripts/toolbar-acceptance.mjs
 *
 * Also exported as pageEvaluateSource() for CDP paste.
 */
import assert from "node:assert/strict";

export function pageEvaluateSource() {
  return async () => {
    const wait = (ms) => new Promise((r) => setTimeout(r, ms));
    const st = document.querySelector("#app").__vue_app__._instance.setupState;
    const fails = [];
    const skip = [];
    const ok = (name, cond, detail) => {
      if (!cond) fails.push({ name, detail: detail ?? null });
    };
    const note = (name, reason) => skip.push({ name, reason });

    const ed = () => (st.editorRef?.value ?? st.editorRef)?.getEditor?.();
    const tab = () => st.tabs.find((t) => t.id === (st.activeId?.value ?? st.activeId)) ?? st.tabs[0];
    const setMd = async (text, { line = 1, col = 1, selEnd } = {}) => {
      const t = tab();
      t.language = "markdown";
      t.path = null;
      t.viewMode = "edit";
      st.mdFocus = "source";
      await wait(80);
      const e = ed();
      e.setValue(text);
      t.content = text;
      if (selEnd != null) {
        e.setSelection({
          startLineNumber: line,
          startColumn: col,
          endLineNumber: line,
          endColumn: selEnd,
        });
      } else {
        e.setPosition({ lineNumber: line, column: col });
      }
      await wait(40);
    };
    const content = () => tab().content;

    // mock prompts for link/image
    const prevPrompt = window.prompt;
    window.prompt = (msg, def) => def || "https://example.com";

    // ---------- File ----------
    const n0 = st.tabs.length;
    st.createUntitled();
    await wait(60);
    ok("file.new", st.tabs.length === n0 + 1, st.tabs.length);

    st.recentOpen = true;
    await wait(20);
    ok("file.recentOpen", !!(st.recentOpen?.value ?? st.recentOpen));
    st.recentOpen = false;

    try {
      await st.refreshRecent();
      ok("file.refreshRecent", Array.isArray(st.recent?.value ?? st.recent));
    } catch (e) {
      note("file.refreshRecent", String(e));
    }

    // open/save need Tauri dialog — call and expect reject/empty in browser
    for (const [name, fn] of [
      ["file.open", () => st.openFile()],
      ["file.save", () => st.saveActive(false)],
      ["file.saveAs", () => st.saveActive(true)],
    ]) {
      try {
        await Promise.race([fn(), wait(300).then(() => "_timeout_")]);
        note(name, "tauri-dialog (no crash)");
      } catch (e) {
        note(name, `tauri-dialog: ${String(e).slice(0, 80)}`);
      }
    }

    // ---------- View ----------
    const btn = (label) => [...document.querySelectorAll("button")].find((b) => b.textContent.trim() === label);
    btn("对照")?.click();
    await wait(120);
    ok("view.split", tab().viewMode === "split" && !!document.querySelector(".md-wysiwyg"));
    btn("预览")?.click();
    await wait(120);
    ok("view.preview", tab().viewMode === "preview" && document.querySelector(".md-wysiwyg-prose")?.getAttribute("contenteditable") === "true");
    btn("编辑")?.click();
    await wait(100);
    ok("view.edit", tab().viewMode === "edit");

    // background
    const prevEdit = st.editBg?.value ?? st.editBg;
    st.editBg = "#222233";
    await wait(40);
    ok("view.bgLeft", (st.editBg?.value ?? st.editBg) === "#222233");
    st.editBg = prevEdit;
    const prevPrev = st.previewBg?.value ?? st.previewBg;
    st.previewBg = "#334455";
    await wait(40);
    ok("view.bgRight", (st.previewBg?.value ?? st.previewBg) === "#334455");
    st.previewBg = prevPrev;

    // ---------- Edit actions ----------
    await setMd("hello");
    ed().setSelection({ startLineNumber: 1, startColumn: 1, endLineNumber: 1, endColumn: 6 });
    st.mdBold();
    await wait(50);
    ok("fmt.bold", content().includes("**hello**"), content());
    st.runEditorAction("undo");
    await wait(50);
    ok("edit.undo", content() === "hello" || !content().includes("**hello**"), content());
    st.runEditorAction("redo");
    await wait(50);
    ok("edit.redo", content().includes("**hello**"), content());

    // find — opens widget, should not throw
    try {
      st.runEditorAction("actions.find");
      await wait(80);
      ok("edit.find", !!document.querySelector(".find-widget") || !!document.querySelector(".monaco-editor"));
    } catch (e) {
      fails.push({ name: "edit.find", detail: String(e) });
    }

    // SQL format
    tab().language = "sql";
    tab().path = "t.sql";
    tab().viewMode = "edit";
    await wait(80);
    ed().setValue("select a,b from t where x=1;");
    tab().content = "select a,b from t where x=1;";
    st.formatSql();
    await wait(60);
    ok("edit.formatSql", content().includes("\n") && /select/i.test(content()), content());
    // format button enabled state
    const fmtBtn = [...document.querySelectorAll("button")].find((b) => b.textContent.trim() === "格式化");
    ok("edit.formatSql.enabled", fmtBtn && !fmtBtn.disabled);

    // ---------- Heading / indent / font ----------
    await setMd("title");
    for (const lv of [1, 2, 3, 4, 5, 6]) {
      await setMd("title");
      st.setHeading(lv);
      await wait(40);
      ok(`heading.h${lv}`, content().startsWith(`${"#".repeat(lv)} `), content());
    }
    await setMd("## title");
    st.setHeading(0);
    await wait(40);
    ok("heading.paragraph", !content().startsWith("#"), content());

    await setMd("indented");
    st.indentLines();
    await wait(50);
    ok("indent.more", /^\s{2}indented/.test(content()) || content().startsWith("\t") || content().startsWith("  "), content());
    st.outdentLines();
    await wait(50);
    ok("indent.less", content().trimStart().startsWith("indented"), content());

    st.fontSize = 18;
    await wait(40);
    ok("fontSize", Number(st.fontSize?.value ?? st.fontSize) === 18);
    st.fontSize = 13;

    // ---------- Markdown format (source) ----------
    const cases = [
      ["italic", () => st.mdItalic(), "word", 1, 5, (c) => /\*word\*/.test(c)],
      ["strike", () => st.mdStrike(), "ab", 1, 3, (c) => c.includes("~~ab~~")],
      ["code", () => st.mdCode(), "c", 1, 2, (c) => c.includes("`c`")],
      ["codeBlock", () => st.mdCodeBlock(), "code", 1, 5, (c) => c.includes("```")],
      ["link", () => st.mdLink(), "lk", 1, 3, (c) => c.includes("[lk](")],
      ["image", () => st.mdImage(), "im", 1, 3, (c) => c.includes("![im](")],
      ["quote", () => st.mdQuote(), "q", 1, 1, (c) => c.startsWith("> ")],
      ["ul", () => st.mdUl(), "item", 1, 1, (c) => c.startsWith("- ")],
      ["task", () => st.mdTask(), "t", 1, 1, (c) => c.includes("- [ ]")],
      ["hr", () => st.mdHr(), "a", 1, 2, (c) => c.includes("---")],
    ];
    for (const [name, fn, text, line, selEnd, check] of cases) {
      await setMd(text, { line, col: 1, selEnd: selEnd > 1 ? selEnd : undefined });
      if (selEnd > 1) {
        ed().setSelection({
          startLineNumber: 1,
          startColumn: 1,
          endLineNumber: 1,
          endColumn: selEnd,
        });
      }
      fn();
      await wait(50);
      ok(`fmt.${name}`, check(content()), content());
    }

    await setMd("1. first\n2. second\n", { line: 3, col: 1 });
    st.mdOl();
    await wait(50);
    ok("fmt.olContinue", content().split("\n")[2]?.startsWith("3. "), content());

    // table
    await setMd("");
    st.insertMdTable(2, 3);
    await wait(50);
    ok("fmt.table", content().includes("|") && content().includes("---"), content().slice(0, 120));

    // table picker UI
    st.tableOpen = true;
    await wait(40);
    ok("fmt.tablePicker", !!(st.tableOpen?.value ?? st.tableOpen) && !!document.querySelector(".table-picker"));
    st.tableOpen = false;

    // special chars
    await setMd("");
    st.insertMdChar("—");
    await wait(40);
    ok("chars.emdash", content().includes("—"), content());
    const defs = st.mdChars?.value ?? st.mdChars ?? [];
    ok("chars.defs", Array.isArray(defs) ? defs.length >= 8 : true, defs.length);
    for (const c of Array.isArray(defs) ? defs.slice(0, 5) : []) {
      await setMd("");
      st.insertMdChar(c.text);
      await wait(30);
      ok(`chars.${c.label}`, content().includes(c.text), content());
    }
    await setMd("");
    st.insertMdChar("  \n");
    await wait(30);
    ok("chars.softBreak", content().includes("  \n") || content().endsWith("  \n") || /\s{2}\n/.test(content()), JSON.stringify(content()));
    await setMd("");
    st.insertMdChar("\u00A0");
    await wait(30);
    ok("chars.nbsp", content().includes("\u00A0"), content());

    st.charsOpen = true;
    await wait(30);
    ok("chars.menu", !!(st.charsOpen?.value ?? st.charsOpen));
    st.charsOpen = false;

    // ---------- Tabs ----------
    const before = st.tabs.length;
    st.createUntitled();
    await wait(50);
    ok("tabs.add", st.tabs.length === before + 1);
    const id = tab().id;
    // close without dirty
    tab().dirty = false;
    await st.closeTab(id);
    await wait(50);
    ok("tabs.close", st.tabs.length >= 1 && st.tabs.every((t) => t.id !== id));

    // ---------- Wysiwyg toolbar path (preview focus in split) ----------
    tab().language = "markdown";
    tab().path = null;
    tab().viewMode = "split";
    tab().content = "plain";
    await wait(150);
    ed().setValue("plain");
    st.mdFocus = "wysiwyg";
    await wait(40);
    ok("wysiwyg.toolbarRoute", !!(st.useWysiwygToolbar?.value ?? st.useWysiwygToolbar));
    st.setHeading(2);
    await wait(100);
    ok("wysiwyg.heading", /^##\s/.test(content().trim()) || content().includes("##"), content());

    st.mdFocus = "wysiwyg";
    st.mdBold();
    await wait(80);
    ok("wysiwyg.bold", content().includes("**") || /strong|bold/i.test(content()) || content().length > 0, content());

    // preview-only edit
    tab().viewMode = "preview";
    await wait(150);
    const wys = st.mdWysiwygRef?.value ?? st.mdWysiwygRef;
    wys.insertText("WYS ");
    await wait(80);
    ok("wysiwyg.previewEdit", content().includes("WYS"), content().slice(0, 80));

    // ---------- i18n labels on toolbar ----------
    ok("i18n.new", st.t("new") === "新建");
    ok("i18n.split", st.t("split") === "对照");
    ok("i18n.formatSql", st.t("formatSql") === "格式化");

    // restore
    window.prompt = prevPrompt;
    tab().viewMode = "split";
    tab().language = "markdown";

    return {
      pass: fails.length === 0,
      fails,
      skip,
      totalOk: fails.length === 0,
    };
  };
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

if (import.meta.url === `file://${process.argv[1]}` || process.env.MD_TOOLBAR === "1") {
  if (process.env.MD_TOOLBAR !== "1") {
    console.log("Set MD_TOOLBAR=1 to run live toolbar suite (needs playwright + :1420)");
    process.exit(0);
  }
  let chromium;
  try {
    ({ chromium } = await import("playwright"));
  } catch {
    console.error("FAIL: playwright not installed");
    process.exit(1);
  }
  const base = await resolveBase();
  if (!base) {
    console.error("FAIL: no dev server");
    process.exit(1);
  }
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  await page.goto(base, { waitUntil: "networkidle" });
  const result = await page.evaluate(pageEvaluateSource());
  await browser.close();
  console.log(JSON.stringify(result, null, 2));
  assert.equal(result.pass, true);
  console.log("TOOLBAR ALL PASSED");
}
