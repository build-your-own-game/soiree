// Exercise real WASM-owned subscriptions and timers in both browser engines.
// From the common root: node salon/tests/ui-browser.mjs
// Prerequisites: Rust/WASM, matching wasm-bindgen, tools/cert npm ci + browsers.
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname } from "node:path";
import { fileURLToPath } from "node:url";
import { launchBrowser } from "../../tools/cert/browser.mjs";

const workspace = fileURLToPath(new URL("../", import.meta.url));
const output = new URL("../target/ui-browser/", import.meta.url);
function run(command, args) {
  const result = spawnSync(command, args, { cwd: workspace, stdio: "inherit" });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${command} failed (${result.signal ?? result.status})`);
}
run("cargo", ["build", "-p", "salon-web", "--example", "ui_probe", "--release", "--target", "wasm32-unknown-unknown"]);
run("wasm-bindgen", ["target/wasm32-unknown-unknown/release/examples/ui_probe.wasm", "--target", "web", "--out-dir", "target/ui-browser"]);
const mime = { ".js": "text/javascript", ".wasm": "application/wasm" };
const server = createServer(async (request, response) => {
  const path = new URL(request.url, "http://localhost").pathname;
  if (path === "/") {
    response.writeHead(200, { "Content-Type": "text/html" }).end('<script type="module">import init, * as ui from "./ui_probe.js"; await init(); window.ui = ui;</script>');
    return;
  }
  if (!["/ui_probe.js", "/ui_probe_bg.wasm"].includes(path)) {
    response.writeHead(404).end();
    return;
  }
  try {
    response.writeHead(200, { "Content-Type": mime[extname(path)] });
    response.end(await readFile(new URL(path.slice(1), output)));
  } catch {
    response.writeHead(500).end();
  }
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
let browser;
try {
  browser = await launchBrowser();
  for (const engine of ["webkit", "chromium"]) {
    const tab = await browser.open(null, { engine });
    try {
      // Observe independent native API registrations, not a counter supplied
      // by the Rust wrapper whose cleanup we are checking.
      await tab.context.addInitScript(() => {
        window.uiRegistrations = 0;
        window.connectionRegistrations = 0;
        const add = EventTarget.prototype.addEventListener;
        const remove = EventTarget.prototype.removeEventListener;
        const names = new Set(["soiree-ui:probe", "compositionstart", "compositionend"]);
        const connectionNames = new Set(["pageshow", "online", "visibilitychange"]);
        EventTarget.prototype.addEventListener = function (name, ...args) {
          if (names.has(name)) window.uiRegistrations++;
          if (connectionNames.has(name)) window.connectionRegistrations++;
          return Reflect.apply(add, this, [name, ...args]);
        };
        EventTarget.prototype.removeEventListener = function (name, ...args) {
          if (names.has(name)) window.uiRegistrations--;
          if (connectionNames.has(name)) window.connectionRegistrations--;
          return Reflect.apply(remove, this, [name, ...args]);
        };
      });
      await tab.page.goto(`http://127.0.0.1:${server.address().port}/`);
      await tab.page.waitForFunction(() => !!window.ui, null, { timeout: 5_000 });
      for (const grouped of [false, true]) {
        assert.deepEqual(await tab.page.evaluate((grouped) => {
          const target = new EventTarget();
          let dispatched = 0;
          target.addEventListener("soiree-ui:reentrant", () => dispatched++);
          return { ...JSON.parse(ui.reentrant_listener(target, Number(grouped))), dispatched };
        }, grouped), { during: 2, after_drop: 2, dispatched: 3 }, `${engine}: ${grouped ? "Listeners" : "Listener"} must deliver synchronous reentrancy`);
      }
      assert.deepEqual(await tab.page.evaluate(() => {
        const baseline = connectionRegistrations;
        const closes = () => __certSocketTrace.events.filter((event) => event.kind === "close-call").length;
        const before = closes();
        ui.connect();
        const opened = connectionRegistrations - baseline;
        ui.drop_connection();
        const keptByClone = connectionRegistrations - baseline;
        const notClosed = closes() - before;
        ui.drop_connection_clone();
        return { opened, keptByClone, notClosed, after: connectionRegistrations - baseline, closed: closes() - before };
      }), { opened: 3, keptByClone: 3, notClosed: 0, after: 0, closed: 1 });
      assert.deepEqual(await tab.page.evaluate(() => {
        for (let i = 0; i < 100; i++) { ui.mount(); ui.unmount(); }
        ui.dispatch(0);
        return { registrations: uiRegistrations, ...JSON.parse(ui.stats()) };
      }), { registrations: 0, events: 0, timeouts: 0, intervals: 0 });
      await tab.page.evaluate(() => ui.mount());
      assert.equal(await tab.page.evaluate(() => uiRegistrations), 4);
      assert.deepEqual(await tab.page.evaluate(() => {
        const result = [ui.blocks_key(0), ui.blocks_key(1), ui.blocks_key(2)];
        ui.dispatch(1); result.push(ui.blocks_key(0));
        ui.dispatch(2); result.push(ui.blocks_key(0));
        ui.dispatch(0);
        return { blocked: result, events: JSON.parse(ui.stats()).events };
      }), { blocked: [false, true, true, true, false], events: 2 });
      await tab.page.waitForFunction(() => JSON.parse(ui.stats()).intervals > 0, null, { timeout: 2_000 });
      const stopped = await tab.page.evaluate(() => {
        ui.clear_listeners();
        ui.dispatch(0);
        ui.unmount();
        ui.dispatch(0);
        // A timeout created and cancelled within the same task cannot have
        // fired before cancellation, regardless of host/browser load.
        ui.mount(); ui.unmount();
        return { registrations: uiRegistrations, ...JSON.parse(ui.stats()) };
      });
      assert.equal(stopped.registrations, 0);
      assert.equal(stopped.events, 3);
      await tab.page.waitForTimeout(250);
      assert.deepEqual(await tab.page.evaluate(() => ({ registrations: uiRegistrations, ...JSON.parse(ui.stats()) })), stopped);
      // Positive control: a live timeout actually fires once.
      await tab.page.evaluate(() => ui.mount());
      await tab.page.waitForFunction((expected) => JSON.parse(ui.stats()).timeouts === expected, stopped.timeouts + 1, { timeout: 2_000 });
      await tab.page.evaluate(() => ui.unmount());
      // Connection's deadline consumes its own timer while running. This must
      // remain safe when the RAII callback releases its own owner.
      await tab.page.evaluate(() => ui.self_cancelling_timer());
      await tab.page.waitForFunction(() => ui.self_fired() === 1, null, { timeout: 2_000 });
      await tab.page.evaluate(() => ui.frame_start(0));
      await tab.page.waitForFunction(() => ui.frames() >= 2, null, { timeout: 2_000 });
      const framesStopped = await tab.page.evaluate(() => { ui.frame_stop(); return ui.frames(); });
      await tab.page.waitForTimeout(100);
      assert.equal(await tab.page.evaluate(() => ui.frames()), framesStopped, "dropped frame loop kept running");
      await tab.page.evaluate(() => ui.frame_start(1));
      await tab.page.waitForFunction((before) => ui.frames() > before, framesStopped, { timeout: 2_000 });
      await tab.page.waitForTimeout(100);
      assert.equal(await tab.page.evaluate(() => ui.frames()), framesStopped + 1, "owner dropped during frame rescheduled itself");
      assert.deepEqual(tab.errors, []);
      console.log(`${engine}: reentrant listeners, 100 scopes cleaned, no late timer/frame callback, composition guards passed`);
    } finally {
      await tab.context.close();
    }
  }
} finally {
  await browser?.close();
  await new Promise((done) => server.close(done));
}
