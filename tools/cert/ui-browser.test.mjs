// Test the observer against browser behavior, including intentionally false
// observations; these controls must fail before the caller can mark success.
import { test } from "node:test";
import assert from "node:assert/strict";
import { launchBrowser } from "./browser.mjs";
import { runUiScenario, runUiCommand } from "./ui.mjs";

test("real browser UI observations preserve identity and reject false assertions", async () => {
  const browser = await launchBrowser();
  try {
    for (const engine of ["webkit", "chromium"]) {
      const tab = await browser.open(null, { engine });
      try {
        await tab.page.setContent('<input id="field"><div id="editable" contenteditable="true">hello</div><button id="noop">noop</button>');
        await tab.page.evaluate(() => {
          window.compositions = [];
          document.querySelector("#field").addEventListener("compositionend", (event) => window.compositions.push(event.target.value));
        });
        await runUiScenario(tab, { name: "identity and input", steps: [
          { focus: "#field" }, { fill: "#field", text: "hello" }, { select: "#field", start: 1, end: 3 },
          { remember: "node", read: "document.querySelector('#field')" },
          { dispatch: "#noop", event: "click" },
          { unchanged: "node", read: "document.querySelector('#field')" },
          { assert: "document.activeElement.id === 'field' && document.activeElement.selectionStart === 1 && document.activeElement.selectionEnd === 3" },
          { composition: "#field", phase: "start" }, { composition: "#field", phase: "update", text: "に" },
          { composition: "#field", phase: "end", text: "日本" },
          { assert: "window.compositions.length === 1 && window.compositions[0] === '日本'" },
          { focus: "#editable" }, { select: "#editable", start: 1, end: 4 },
          { assert: "window.getSelection().toString() === 'ell'" },
        ] });
        await assert.rejects(runUiScenario(tab, { name: "deliberately false", steps: [{ assert: "document.querySelector('#field').value === 'wrong'" }] }), /assertion fausse/);
      } finally { await tab.context.close(); }
    }
  } finally { await browser.close(); }
});

test("actual UI subprocess failure is propagated despite a green stdout line", async () => {
  await assert.rejects(runUiCommand({ name: "negative exit", command: [process.execPath, "-e", "console.log('green'); process.exit(23)"] }, { cwd: process.cwd(), url: "http://localhost/" }), /sortie 23[\s\S]*green/);
  await assert.rejects(runUiCommand({ name: "timeout", timeout_ms: 30, command: [process.execPath, "-e", "setInterval(()=>{},1000)"] }, { cwd: process.cwd(), url: "http://localhost/" }), /délai dépassé/);
});
