import test from "node:test";
import assert from "node:assert/strict";
import { launchBrowser, Tab } from "./browser.mjs";

const SURFACE = "data:text/html," + encodeURIComponent("<style>body{margin:0;user-select:none}div{width:390px;height:844px}</style><div>Native input surface</div><script>window.moves=[];window.ends=0;window.touchEnds=[];document.addEventListener('mousemove',e=>moves.push(e.buttons));document.addEventListener('touchend',e=>{ends++;touchEnds.push({trusted:e.isTrusted,touches:e.touches.length,changed:e.changedTouches.length});});</script>");

async function bounded(promise, ms = 2000) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => { timer = setTimeout(() => reject(new Error("native input deadline")), ms); })]);
  } finally { clearTimeout(timer); }
}

async function cssSurface(browser, held, run) {
  const tab = await browser.open(null, { engine: "webkit", mobile: true });
  let release;
  let released = !held;
  const css = new Promise((resolve) => { release = () => { released = true; resolve(); }; });
  if (!held) release();
  await tab.page.addInitScript(() => {
    window.nativeInput = [];
    for (const type of ["pointermove", "mousemove", "pointerdown", "mousedown", "pointerup", "mouseup", "gotpointercapture", "lostpointercapture"])
      document.addEventListener(type, (event) => nativeInput.push({ type, buttons: event.buttons, trusted: event.isTrusted, x: event.clientX, y: event.clientY }));
    document.addEventListener("pointerdown", (event) => event.target.setPointerCapture(event.pointerId));
  });
  await tab.page.route("http://native-input.test/**", async (route) => {
    if (route.request().url().endsWith("/held.css")) {
      await css;
      await route.fulfill({ contentType: "text/css", body: "body{margin:0}" }).catch(() => {});
    } else {
      await route.fulfill({ contentType: "text/html", body: '<meta name="viewport" content="width=device-width,initial-scale=1"><link rel="stylesheet" href="/held.css"><button id="native" style="width:300px;height:300px">Native surface</button>' });
    }
  });
  try {
    await tab.page.goto("http://native-input.test/", { waitUntil: "commit" });
    await tab.page.waitForFunction(() => document.querySelector("#native"), null, { polling: 10, timeout: 2000 });
    await run(tab, { release, released: () => released });
  } finally {
    // Even a failed old-sequence control must unblock the real browser.
    release();
    await tab.context.close();
  }
}

test("WebKit drag sends down before waiting for a rendering-deferred hover acknowledgement", { timeout: 20000 }, async () => {
  const browser = await launchBrowser();
  try {
    await cssSurface(browser, false, async (tab) => {
      await bounded(tab.page.mouse.move(40, 40));
      assert.equal(await tab.evaluate("nativeInput.some(e => e.type === 'mousemove' && e.trusted)"), true);
    });
    await cssSurface(browser, true, async (tab, css) => {
      let acknowledged = false;
      const hover = tab.page.mouse.move(40, 40).then(() => { acknowledged = true; });
      await tab.page.waitForFunction(() => nativeInput.some((event) => event.type === "mousemove"), null, { polling: 10, timeout: 2000 });
      await new Promise((resolve) => setTimeout(resolve, 150));
      assert.equal(acknowledged, false, "the old await-move chain must actually wait for rendering");
      assert.equal(css.released(), false);
      css.release();
      await bounded(hover);
      assert.equal(acknowledged, true, "releasing CSS must release the old hover acknowledgement");
    });
    await cssSurface(browser, true, async (tab, css) => {
      await bounded(tab.drag({ x: 40, y: 40 }, { x: 120, y: 120 }));
      assert.equal(css.released(), false, "the drag must finish while CSS still blocks rendering");
      assert.equal(tab.pendingInput, null);
      const events = await tab.evaluate("nativeInput");
      assert.ok(events.every((event) => event.trusted));
      const mouse = events.filter((event) => event.type.startsWith("mouse"));
      assert.equal(mouse[0].type, "mousemove");
      assert.equal(mouse[0].buttons, 0);
      assert.deepEqual(mouse[1], { type: "mousedown", buttons: 1, trusted: true, x: 40, y: 40 });
      assert.ok(mouse.slice(2, -1).length >= 8);
      assert.ok(mouse.slice(2, -1).every((event) => event.type === "mousemove" && event.buttons === 1));
      assert.deepEqual(mouse.at(-1), { type: "mouseup", buttons: 0, trusted: true, x: 120, y: 120 });
      assert.equal(events.filter((event) => event.type === "pointerdown").length, 1);
      assert.equal(events.filter((event) => event.type === "pointerup").length, 1);
      assert.ok(events.some((event) => event.type === "gotpointercapture"));
      assert.ok(events.some((event) => event.type === "lostpointercapture"));
    });
  } finally { await browser.close(); }
});

test("WebKit drag retains both start failures and still releases the native mouse", { timeout: 20000 }, async () => {
  const browser = await launchBrowser();
  try {
    const tab = await browser.open(SURFACE, { engine: "webkit", mobile: true });
    const input = tab.input.bind(tab);
    tab.input = async (operation, task, details) => {
      const value = await input(operation, task, details);
      if (operation === "drag.mouse.move.start" || operation === "drag.mouse.down") throw new Error(operation);
      return value;
    };
    await assert.rejects(tab.drag({ x: 20, y: 20 }, { x: 100, y: 100 }), (error) => {
      assert.ok(error instanceof AggregateError);
      assert.deepEqual(error.errors.map((failure) => failure.message), ["drag.mouse.move.start", "drag.mouse.down"]);
      return true;
    });
    tab.input = input;
    await bounded(tab.page.mouse.move(30, 30));
    assert.equal(await tab.evaluate("moves.at(-1)"), 0);
    assert.equal(tab.pendingInput, null);
    await tab.context.close();
  } finally { await browser.close(); }
});

test("a failed native release does not hide the earlier drag failure", { timeout: 20000 }, async () => {
  const browser = await launchBrowser();
  try {
    const tab = await browser.open(SURFACE, { engine: "webkit", mobile: true });
    const input = tab.input.bind(tab);
    tab.input = async (operation, task, details) => {
      const value = await input(operation, task, details);
      if (operation === "drag.mouse.move.steps" || operation === "drag.mouse.up") throw new Error(operation);
      return value;
    };
    await assert.rejects(tab.drag({ x: 20, y: 20 }, { x: 100, y: 100 }), (error) => {
      assert.ok(error instanceof AggregateError);
      assert.deepEqual(error.errors.map((failure) => failure.message), ["drag.mouse.move.steps", "drag.mouse.up"]);
      return true;
    });
    tab.input = input;
    await bounded(tab.page.mouse.move(30, 30));
    assert.equal(await tab.evaluate("moves.at(-1)"), 0);
    await tab.context.close();
  } finally { await browser.close(); }
});

test("native diagnostics retain the oldest pending call when another acknowledgement arrives", async () => {
  const tab = new Tab({}, { url: () => "http://native-input.test/" }, true);
  let finishMove;
  let finishDown;
  const move = tab.input("move", () => new Promise((resolve) => { finishMove = resolve; }));
  const down = tab.input("down", () => new Promise((resolve) => { finishDown = resolve; }));
  assert.equal(tab.pendingInput.operation, "move");
  finishDown();
  await down;
  assert.equal(tab.pendingInput.operation, "move");
  assert.equal(tab.pendingInputs.size, 1);
  finishMove();
  await move;
  assert.equal(tab.pendingInput, null);
  assert.equal(tab.pendingInputs.size, 0);
});

test("interrupted native drags release the held mouse on both engines", { timeout: 30000 }, async () => {
  const browser = await launchBrowser();
  try {
    for (const engine of ["webkit", "chromium"]) {
      const tab = await browser.open(SURFACE, { engine, mobile: true });
      const input = tab.input.bind(tab);
      for (const stage of ["drag.mouse.move.start", "drag.mouse.down", "drag.mouse.move.steps"]) {
        // Fail after an actual native acknowledgement, when the driver may
        // already retain a pressed button. No synthetic DOM event is used.
        tab.input = async (operation, task) => {
          const value = await input(operation, task);
          if (operation === stage) throw new Error(`interrupted ${stage}`);
          return value;
        };
        await assert.rejects(tab.drag({ x: 20, y: 20 }, { x: 100, y: 100 }), /interrupted/);
        tab.input = input;
        await tab.page.mouse.move(30, 30);
        assert.equal(await tab.evaluate("moves.at(-1)"), 0, `${engine}: ${stage} left a pressed button`);
      }
      await tab.close();
    }
  } finally { await browser.close(); }
});

test("an interrupted Chromium touch sequence emits its native touch end", { timeout: 30000 }, async (t) => {
  let browser;
  let tab;
  let phase;
  let watchdog;
  const timeline = [];
  const short = (record) => record && ({ ...record, url: record.url?.slice(0, 160) });
  const report = (reason) => t.diagnostic(JSON.stringify({
    reason, phase, timeline,
    chromium: tab?.context.browser()?.version(),
    page_closed: tab?.page.isClosed(), viewport: tab?.page.viewportSize(),
    pending: [...(tab?.pendingInputs ?? [])].map(short),
    history: (tab?.inputHistory ?? []).slice(-8).map(short),
  }));
  const progress = (name) => {
    phase = name;
    timeline.push({ phase, at: Date.now() });
    clearTimeout(watchdog);
    // Observe the driver's existing state without querying the renderer:
    // a layout/evaluation probe must not accidentally release a blocked ACK.
    watchdog = setTimeout(() => report("native stage still pending"), 2000);
    watchdog.unref();
  };
  const failed = async (reason) => {
    report(reason);
    if (tab && !tab.page.isClosed()) {
      const state = await bounded(tab.page.evaluate(() => ({
        ready: document.readyState, visibility: document.visibilityState,
        focused: document.hasFocus(), fonts: document.fonts.status,
        ends: globalThis.ends, touch_ends: globalThis.touchEnds,
      })), 500).catch((error) => ({ unavailable: error.message }));
      t.diagnostic(JSON.stringify({ reason: "page state after failure", state }));
    }
  };
  t.after(async () => {
    clearTimeout(watchdog);
    if (t.signal.aborted) await failed("test aborted");
    // A timed-out await does not unwind the test callback's finally block.
    // The test hook closes the native session even when CDP never answers.
    await browser?.close();
  });
  progress("browser.launch");
  browser = await launchBrowser();
  if (t.signal.aborted) { await browser.close(); return; }
  try {
    progress("tab.open");
    tab = await browser.open(SURFACE, { engine: "chromium", mobile: true });
    const input = tab.input.bind(tab);
    tab.input = async (operation, task, details) => {
      progress(operation);
      const value = await input(operation, task, details);
      if (operation === "swipe.cdp.touchMove") throw new Error("interrupted native touch move");
      return value;
    };
    await assert.rejects(tab.swipe([{ x: 20, y: 20 }, { x: 100, y: 100 }]), /interrupted/);
    progress("first.touchend.observation");
    assert.equal(await tab.evaluate("ends"), 1);
    assert.deepEqual(await tab.evaluate("touchEnds"), [{ trusted: true, touches: 0, changed: 1 }]);
    tab.input = input;
    progress("next.touchscreen.tap");
    await tab.tap(30, 30);
    progress("second.touchend.observation");
    assert.equal(await tab.evaluate("ends"), 2, "the next real touch must form an independent sequence");
    assert.deepEqual(await tab.evaluate("touchEnds"), Array.from({ length: 2 }, () => ({ trusted: true, touches: 0, changed: 1 })));
    progress("tab.close");
    await tab.close();
  } catch (error) {
    await failed(error.message);
    throw error;
  } finally { clearTimeout(watchdog); }
});

test("WebKit pointer fallback is one native gesture without an extra tap", { timeout: 30000 }, async () => {
  const browser = await launchBrowser();
  try {
    const tab = await browser.open(null, { engine: "webkit", mobile: true });
    await tab.page.setContent('<meta name="viewport" content="width=device-width,initial-scale=1"><button style="width:300px;height:300px">Tap</button>');
    await tab.page.evaluate(() => {
      window.inputs = { clicks: 0, down: [], up: [], moves: [], touches: 0 };
      document.addEventListener("click", () => inputs.clicks++);
      document.addEventListener("pointerdown", (event) => inputs.down.push([event.pointerType, event.isTrusted]));
      document.addEventListener("pointerup", (event) => inputs.up.push([event.pointerType, event.isTrusted]));
      document.addEventListener("pointermove", (event) => inputs.moves.push([event.buttons, event.isTrusted]));
      document.addEventListener("touchstart", () => inputs.touches++);
    });
    await tab.swipe([{ x: 40, y: 40 }, { x: 90, y: 90 }]);
    const state = await tab.evaluate("inputs");
    assert.equal(state.clicks, 1, "the fallback must not manufacture a preparatory activation");
    assert.equal(state.touches, 0, "the fallback must be reported as a pointer drag, not a touch swipe");
    assert.deepEqual(state.down, [["mouse", true]]);
    assert.deepEqual(state.up, [["mouse", true]]);
    assert.ok(state.moves.some(([buttons, trusted]) => buttons === 1 && trusted));
    await tab.tap(40, 40);
    assert.equal(await tab.evaluate("inputs.touches"), 1, "native touch taps remain independently covered");
    await tab.context.close();
  } finally { await browser.close(); }
});
