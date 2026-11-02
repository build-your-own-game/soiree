import test from "node:test";
import assert from "node:assert/strict";
import { launchBrowser } from "./browser.mjs";
import { summarizeFrames, frameFailures, measureFrames, measureProbe, renderSettled, positiveBudget, panicMatches, stopTasks, injectedPanicRemainder, gestureKeys, FixtureFailure, FixtureGuard, scenarioFailureItem, networkAddressPresent } from "./performance.mjs";

test("frame budgets reject an isolated stall and low cadence", () => {
  const smooth = summarizeFrames(Array(180).fill(1000 / 60));
  assert.deepEqual(frameFailures(smooth), []);
  assert.match(frameFailures(summarizeFrames([...Array(179).fill(1000 / 60), 45]))[0], /45.0 ms/);
  assert.match(frameFailures(summarizeFrames(Array(90).fill(1000 / 30))).at(-1), /30.0 images/);
  assert.throws(() => summarizeFrames([]));
  assert.throws(() => summarizeFrames([NaN]));
  assert.throws(() => frameFailures(smooth, { min_fps: "fast" }), /nombre positif/);
  assert.throws(() => frameFailures(smooth, { max_frame_ms: -1 }), /nombre positif/);
  assert.throws(() => positiveBudget("80", 80, "budget"), /nombre positif/);
});

test("one skipped 60 Hz vsync fits the 33 ms budget; a really slow image does not", () => {
  const withWorst = (worst) => frameFailures(summarizeFrames([...Array(179).fill(1000 / 60), worst]));
  // 33.3 and 33.4 ms are what Chrome really reports for one skipped vsync.
  for (const skippedVsync of [1000 / 30, 33.3, 33.4, 34]) assert.deepEqual(withWorst(skippedVsync), [], `${skippedVsync} ms`);
  // Just past the resolution, a 120 Hz image, and two skipped vsyncs at 60 Hz.
  for (const slow of [34.1, 41.7, 45, 50]) assert.match(withWorst(slow)[0], /image .* ms > 33 ms/, `${slow} ms`);
  assert.deepEqual(frameFailures(summarizeFrames([20.5]), { max_frame_ms: 20, min_fps: 1 }), []);
  assert.match(frameFailures(summarizeFrames([21.5]), { max_frame_ms: 20, min_fps: 1 })[0], /> 20 ms/);
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

test("a real long image in the renderer breaches the budget despite the measurement tolerance", { timeout: 10000 }, async () => {
  const browser = await launchBrowser({ webkitPhones: false });
  try {
    const tab = await browser.open("data:text/html,<p>stall</p>");
    await tab.page.evaluate(() => {
      let frame = 0;
      const tick = () => {
        if (++frame === 20) { const began = performance.now(); while (performance.now() - began < 60) { /* deliberate long image */ } }
        requestAnimationFrame(tick);
      };
      requestAnimationFrame(tick);
    });
    const sample = await measureFrames(tab.page, 600);
    assert.ok(sample.max_ms >= 40, `the stall was observed at ${sample.max_ms} ms`);
    assert.ok(frameFailures(sample).some((failure) => failure.startsWith("image ")));
    await tab.close();
  } finally { await browser.close(); }
});

test("a rendered value settles by coming back or by resting, never while it keeps moving", { timeout: 30000 }, async () => {
  const browser = await launchBrowser();
  try {
    const tab = await browser.open("data:text/html," + encodeURIComponent("<output id='value'>0</output>"));
    const read = "document.querySelector('#value').textContent";
    const set = (text) => tab.page.evaluate((next) => { document.querySelector("#value").textContent = next; }, text);
    // A jump: back to where it began.
    await renderSettled(tab, read, '"0"', { quietMs: 60000, timeoutMs: 1000 });
    // A score: it stays changed, and resting is enough.
    await set("1");
    await renderSettled(tab, read, '"0"', { quietMs: 200, timeoutMs: 1000 });
    // A value moving all the time never counts as finished.
    await tab.page.evaluate(() => { let n = 2; window.__moving = setInterval(() => { document.querySelector("#value").textContent = String(n++); }, 20); });
    await assert.rejects(renderSettled(tab, read, '"0"', { quietMs: 200, timeoutMs: 800 }), /Timeout/);
    await tab.page.evaluate(() => clearInterval(window.__moving));
    await tab.close();
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

test("LAN fixture follows the assigned address, not interface enumeration order",()=>{
 const initial='192.0.2.1';
 assert.equal(networkAddressPresent(initial,{b:[{address:'192.0.2.2',internal:false}],a:[{address:initial,internal:false}]}),true);
 assert.equal(networkAddressPresent(initial,{a:[{address:'192.0.2.2',internal:false}]}),false);
 assert.equal(networkAddressPresent(initial,{}),false);
});

test("RTC failures belong to C1 while fixture failures retain infrastructure scope", () => {
  const transportFailure = new Error("aucun paquet salon vivant sur le vrai DataChannel UDP en 15 s");
  assert.equal(scenarioFailureItem("rtc", transportFailure), "C1");
  // A product failure followed by failed cleanup must preserve both receipts:
  // C1 for the primary cause and H1 for the broken controller, never E1.
  const cleanupFailure = new FixtureFailure("le relais UDP ne peut pas être restauré");
  assert.deepEqual([transportFailure, cleanupFailure].map(error => scenarioFailureItem("rtc", error)), ["C1", "H1"]);
  assert.equal(scenarioFailureItem("ui", new Error("éditeur remplacé")), "U1");
  assert.equal(scenarioFailureItem("hostile", new Error("message malformé non isolé")), "E1");
  for (const [scenario, item] of [["capacity", "R3"], ["text", "T2"], ["layout", "T3"], ["performance", "P1"]]) {
    assert.equal(scenarioFailureItem(scenario, new Error("scénario interrompu avant son observation")), item);
  }
  for (const scenario of ["rtc", "ui", "hostile", "capacity", "text", "layout", "performance"]) assert.equal(scenarioFailureItem(scenario, cleanupFailure), "H1");
});

test("a delayed Node controller recovers only its already completed browser sample", { timeout: 30000 }, async () => {
  const browser = await launchBrowser({ webkitPhones: false });
  try {
    const tab = await browser.open("data:text/html,<p>independent renderer</p>");
    const measurement = measureFrames(tab.page, 250);
    await tab.page.waitForFunction(() => Boolean(window.__certFrameSample?.active));
    // This sleeps Node without consuming renderer CPU. The old watchdog
    // rejected a healthy 250 ms browser sample when Node resumed after 4 s.
    const nodeDelayMs = 4000;
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, nodeDelayMs);
    const sample = await measurement;
    assert.equal(sample.recovered_controller_delay, true);
    assert.ok(sample.browser_elapsed_ms < 3250);
    assert.ok(sample.controller_elapsed_ms >= nodeDelayMs);
    assert.ok(sample.duration_ms >= 250, "the renderer ran its whole sample while Node slept");
    // How fast this machine renders is not the question: a runner can take
    // 200 ms between frames with Node awake. Every interval must be the
    // browser's own, closed before Node woke, so none of them can hold its 4 s.
    const receipt = await tab.page.evaluate(() => window.__certFrameSample.completed);
    assert.equal(sample.frames, receipt.frames);
    const browserIntervals = [...receipt.intervals, receipt.leading_gap_ms].filter((interval) => interval > 0);
    assert.equal(sample.max_ms, Math.max(...browserIntervals), "the worst interval comes from the browser's receipt");
    assert.ok(sample.max_ms <= sample.browser_elapsed_ms, `Node delay became a fictitious rendering stall: ${sample.max_ms} ms in a sample of ${sample.browser_elapsed_ms} ms`);
    await tab.close();
  } finally { await browser.close(); }
});

test("controller delay cannot recover a browser which actually exceeded the same deadline", { timeout: 30000 }, async () => {
  const browser = await launchBrowser({ webkitPhones: false });
  try {
    const tab = await browser.open("data:text/html,<p>blocked renderer</p>");
    const measurement = measureFrames(tab.page, 250).then(() => assert.fail("late browser accepted"), (error) => error);
    await tab.page.waitForFunction(() => Boolean(window.__certFrameSample?.active));
    await tab.page.evaluate(() => setTimeout(() => {
      const began = performance.now();
      while (performance.now() - began < 3500) { /* deliberate product-side stall */ }
    }, 30));
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 4000);
    const error = await measurement;
    assert.match(error.message, /ne termine pas/);
    assert.equal(error.diagnostics.state, "completed");
    assert.ok(error.diagnostics.completed.browser_elapsed_ms > 3250);
    await tab.close();
  } finally { await browser.close(); }
});

test("a replaced frame sample cannot cancel its successor during cleanup", { timeout: 10000 }, async () => {
  const browser = await launchBrowser({ webkitPhones: false });
  try {
    const tab = await browser.open("data:text/html,<p>measurement ownership</p>");
    const first = measureFrames(tab.page, 500).then(() => assert.fail("superseded sample accepted"), (error) => error);
    await tab.page.waitForFunction(() => Boolean(window.__certFrameSample?.active));
    const second = measureFrames(tab.page, 750);
    assert.match((await first).message, /remplacé/);
    const sample = await second;
    assert.ok(sample.frames > 10);
    await tab.close();
  } finally { await browser.close(); }
});

test("time blocked before the first callback still breaches the frame budget", async () => {
  const page = { evaluate: async (_operation, argument) => typeof argument === "object" ? ({ intervals: [16, 16], frames: 2, duration_ms: 32, tail_ms: 0, leading_gap_ms: 80, browser_elapsed_ms: 112 }) : undefined };
  const sample = await measureFrames(page, 32);
  assert.equal(sample.leading_gap_ms, 80);
  assert.equal(sample.max_ms, 80);
  assert.match(frameFailures(sample)[0], /80.0 ms/);
});
