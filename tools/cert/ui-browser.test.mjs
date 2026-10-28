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

test("real browser waits await resolved truth and enforce a bounded deadline", { timeout: 45_000 }, async (t) => {
  const browser = await launchBrowser();
  try {
    for (const engine of ["chromium", "webkit"]) {
      await t.test(engine, { timeout: 20_000 }, async () => {
        const tab = await browser.open(null, { engine });
        try {
          await tab.page.setContent("<p>independent async observation fixture</p>");
          await tab.page.evaluate(() => { window.__falsePolls = 0; });
          const start = performance.now();
          await assert.rejects(runUiScenario(tab, { name: "async false", steps: [
            { wait: "(++window.__falsePolls, Promise.resolve(false))" },
            { assert: "window.__afterFalse = true" },
          ] }), /attente non satisfaite après 5000 ms/);
          const elapsed = performance.now() - start;
          assert.ok(elapsed >= 4500 && elapsed < 8000, `${engine}: false observation took ${elapsed} ms`);
          const falseState = await tab.page.evaluate(() => ({ polls: window.__falsePolls, advanced: window.__afterFalse === true }));
          assert.ok(falseState.polls > 1, `${engine}: false observations must be polled again`);
          assert.equal(falseState.advanced, false);
          await new Promise((resolve) => setTimeout(resolve, 50));
          assert.equal(await tab.page.evaluate(() => window.__falsePolls), falseState.polls, "polling must stop after timeout");

          await tab.page.evaluate(() => { window.__lateReady = false; window.__lateStarted = false; window.__latePolls = 0; });
          await runUiScenario(tab, { name: "asynchronous readiness", steps: [
            { wait: "(() => { ++window.__latePolls; if (!window.__lateStarted) { window.__lateStarted = true; setTimeout(() => window.__lateReady = true, 120); } return Promise.resolve(window.__lateReady); })()" },
            { assert: "window.__lateReady === true && window.__latePolls > 1" },
          ] });
          await runUiScenario(tab, { name: "one delayed resolution", steps: [
            { wait: "new Promise(resolve => setTimeout(() => { window.__resolvedTrue = true; resolve(true); }, 60))" },
            { assert: "window.__resolvedTrue === true" },
          ] });

          const pendingStart = performance.now();
          await assert.rejects(runUiScenario(tab, { name: "never settles", steps: [
            { wait: "new Promise(() => {})" },
            { assert: "window.__afterPending = true" },
          ] }), /attente non satisfaite après 5000 ms/);
          const pendingElapsed = performance.now() - pendingStart;
          assert.ok(pendingElapsed >= 4500 && pendingElapsed < 8000, `${engine}: pending Promise took ${pendingElapsed} ms`);
          assert.equal(await tab.page.evaluate(() => window.__afterPending === true), false);
          assert.equal(await tab.page.evaluate(() => Object.hasOwn(window, "__certUiRemembered")), false);
        } finally { await tab.context.close(); }
      });
    }
  } finally { await browser.close(); }
});
