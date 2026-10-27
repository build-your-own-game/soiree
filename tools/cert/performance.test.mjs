import test from "node:test";
import assert from "node:assert/strict";
import { launchBrowser } from "./browser.mjs";
import { summarizeFrames, frameFailures, measureFrames, measureProbe, positiveBudget, panicMatches, stopTasks, injectedPanicRemainder, gestureKeys, FixtureFailure, FixtureGuard } from "./performance.mjs";

test("frame budgets reject an isolated stall and low cadence", () => {
  const smooth = summarizeFrames(Array(180).fill(1000 / 60));
  assert.deepEqual(frameFailures(smooth), []);
  assert.match(frameFailures(summarizeFrames([...Array(179).fill(1000 / 60), 45]))[0], /45.0 ms/);
  assert.match(frameFailures(summarizeFrames(Array(90).fill(1000 / 30)))[1], /30.0 images/);
  assert.throws(() => summarizeFrames([]));
  assert.throws(() => summarizeFrames([NaN]));
  assert.throws(() => frameFailures(smooth, { min_fps: "fast" }), /nombre positif/);
  assert.throws(() => frameFailures(smooth, { max_frame_ms: -1 }), /nombre positif/);
  assert.throws(() => positiveBudget("80", 80, "budget"), /nombre positif/);
});

test("both engines preserve removable init scripts and measure a causal rendered response", { timeout: 120000 }, async (t) => {
  const browser = await launchBrowser();
  try {
    for (const engine of ["chromium", "webkit"]) {
      await t.test(engine, async () => {
        const tab = await browser.open("data:text/html," + encodeURIComponent("<button id='act'>Change</button><output id='value'>0</output><span id='clock'></span><script>setInterval(()=>document.querySelector('#clock').textContent=Date.now(),8);document.querySelector('#act').onclick=()=>setTimeout(()=>document.querySelector('#value').textContent=String(Number(document.querySelector('#value').textContent)+1),120)</script>"), { engine, mobile: true });
        const remove = await tab.beforeScripts("window.__testInstalled=42");
        await tab.reload();
        assert.equal(await tab.evaluate("window.__testInstalled"), 42);
        await remove();
        await tab.reload();
        assert.equal(await tab.evaluate("window.__testInstalled === undefined"), true);
        const probe = { screen: "tv", action: { click: "#act" }, rendered: "document.querySelector('#value').textContent" };
        const measured = await measureProbe(tab, tab, probe);
        assert.ok(measured.latency_ms >= 115, `120 ms response appeared after ${measured.latency_ms} ms`);
        assert.equal(measured.baseline, '"0"');
        assert.equal(measured.rendered, '"1"');
        const frames = await measureFrames(tab.page, 250);
        assert.ok(frames.frames > 2);
        assert.ok(Number.isFinite(frames.fps));
        await tab.page.evaluate(() => { window.__testPaints = 0; window.__paintTimer = setInterval(() => window.__testPaints++, 34); });
        const paints = await measureFrames(tab.page, 400, "window.__testPaints");
        assert.equal(paints.observer, "present");
        assert.ok(paints.fps < 40, `a 30 Hz renderer was observed at ${paints.fps} Hz`);
        assert.ok(frameFailures(paints).length > 0);
        await tab.page.evaluate(() => clearInterval(window.__paintTimer));
        const stopped = await measureFrames(tab.page, 150, "window.__testPaints");
        assert.equal(stopped.frames, 0);
        assert.ok(stopped.trailing_gap_ms >= 150);
        assert.ok(frameFailures(stopped).length > 0);
        await assert.rejects(measureProbe(tab, tab, { ...probe, action: { key: "Escape" } }, 250), /Timeout/);
        // A WebKit page must never accidentally call Chromium's CDP path.
        if (engine === "webkit") assert.throws(() => tab.call("Debugger.pause"), /nécessite Chromium/);
        await tab.close();
      });
    }
  } finally { await browser.close(); }
});

test("a fully blocked frame observer and its cleanup have a finite Node watchdog", { timeout: 6000 }, async () => {
  const began = Date.now();
  const blocked = { evaluate: () => new Promise(() => {}) };
  await assert.rejects(measureFrames(blocked, 1), /ne termine pas/);
  assert.ok(Date.now() - began < 4500, "cleanup also must be bounded");
});

test("panic evidence matches the exact injected nonce and page", () => {
  const report = { page: "/play", attempt: 1, message: "panicked at file.rs:1:\nsalon certification panic 12-1-0" };
  assert.equal(panicMatches(report, "12-1-0", "/play"), true);
  assert.equal(panicMatches(report, "12-1", "/play"), false);
  assert.equal(panicMatches(report, "12-1-0", "/other"), false);
  assert.equal(panicMatches({ ...report, message: "unrelated panic" }, "12-1-0", "/play"), false);
});

test("the probe watchdog also bounds setup and cleanup on a fully blocked page", { timeout: 6000 }, async () => {
  const began = Date.now();
  const blocked = { page: { evaluate: () => new Promise(() => {}) } };
  await assert.rejects(measureProbe(blocked, blocked, { rendered: "document.title", action: { key: "Enter" } }, 1), /délai global/);
  assert.ok(Date.now() - began < 4500, "blocked setup and cleanup must terminate");
});

test("background actor stop and forced context close stay bounded", { timeout: 2000 }, async () => {
  let aborted = false;
  const never = new Promise(() => {});
  const began = Date.now();
  assert.equal(await stopTasks([never], () => { aborted = true; return never; }, 1), false);
  assert.equal(aborted, true);
  assert.ok(Date.now() - began < 1200);
  assert.equal(await stopTasks([Promise.resolve()], () => assert.fail("no abort needed"), 1), true);
});

test("only one nonce-correlated WASM trap is consumed in either browser spelling", () => {
  const chrome = "exception : RuntimeError: unreachable | wasm-function";
  const webkit = "exception : RuntimeError: Unreachable code should not be executed (evaluating wasm)";
  const unrelated = "exception : TypeError: undefined is not an object";
  assert.deepEqual(injectedPanicRemainder([chrome, unrelated], "12-1-0", true), [unrelated]);
  assert.deepEqual(injectedPanicRemainder([webkit, unrelated], "12-1-0", true), [unrelated]);
  assert.deepEqual(injectedPanicRemainder([webkit, chrome, unrelated], "12-1-0", true), [chrome, unrelated]);
  assert.deepEqual(injectedPanicRemainder([webkit, unrelated], "12-1-0", false), [webkit, unrelated]);
  const wrongNotice = "console.error : salon certification panic 12-1-00";
  assert.deepEqual(injectedPanicRemainder([wrongNotice], "12-1-0", true), [wrongNotice]);
});

test("mobile Backspace requires editable text focus; desktop bindings remain exercisable", () => {
  assert.equal(gestureKeys({ mobile: true, textFocus: false }).includes("Backspace"), false);
  assert.equal(gestureKeys({ mobile: true, textFocus: true }).includes("Backspace"), true);
  assert.equal(gestureKeys({ mobile: false, textFocus: false }).includes("Backspace"), true);
});

test("destroying a fixture preserves its first cause and stops later product observations", () => {
  const guard = new FixtureGuard();
  const observed = ["join", "performance"];
  guard.check();
  guard.invalidate("native input stalled; phone context closed");
  guard.invalidate("secondary closed-page error");
  assert.throws(() => {
    guard.check();
    observed.push("restart");
    observed.push("storage");
  }, (error) => error instanceof FixtureFailure && error.message === "native input stalled; phone context closed");
  assert.deepEqual(observed, ["join", "performance"]);
});
