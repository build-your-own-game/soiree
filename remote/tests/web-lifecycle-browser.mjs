// Real WASM SDK: a view dropping its sensors must not stop another live view.
// From the common root: node remote/tests/web-lifecycle-browser.mjs
import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { fileURLToPath } from "node:url";
import { launchBrowser } from "../../tools/cert/browser.mjs";

const workspace = fileURLToPath(new URL("../", import.meta.url));
const output = new URL("../target/web-lifecycle/", import.meta.url);
function run(command, args) {
  const result = spawnSync(command, args, { cwd: workspace, stdio: "inherit" });
  if (result.error) throw result.error;
  assert.equal(result.status, 0, `${command} failed (${result.signal ?? result.status})`);
}
run("cargo", ["build", "-p", "remote-web", "--example", "lifecycle_probe", "--release", "--target", "wasm32-unknown-unknown"]);
run("wasm-bindgen", ["target/wasm32-unknown-unknown/release/examples/lifecycle_probe.wasm", "--target", "web", "--out-dir", "target/web-lifecycle"]);
const server = createServer(async (request, response) => {
  const path = new URL(request.url, "http://localhost").pathname;
  if (path === "/") {
    response.writeHead(200, { "Content-Type": "text/html" }).end('<script type="module">import init, * as probe from "./lifecycle_probe.js"; await init(); window.probe = probe;</script>');
    return;
  }
  if (!["/lifecycle_probe.js", "/lifecycle_probe_bg.wasm"].includes(path)) {
    response.writeHead(404).end();
    return;
  }
  try {
    response.writeHead(200, { "Content-Type": path.endsWith(".wasm") ? "application/wasm" : "text/javascript" });
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
      await tab.context.addInitScript(() => {
        window.commands = [];
        window.webkit = { messageHandlers: { telecommande: { postMessage: (message) => commands.push(message) } } };
      });
      await tab.page.goto(`http://127.0.0.1:${server.address().port}/`);
      await tab.page.waitForFunction(() => !!window.probe, null, { timeout: 5_000 });
      const actual = await tab.page.evaluate(() => {
        const deliver = () => dispatchEvent(new CustomEvent("telecommande:motion", { detail: {
          gravity: [0, -1, 0], rotation: [0, 0, 0], acceleration: [0, 0, 0], attitude: [0, 0, 0, 1], time: 1,
        } }));
        const snapshots = [];
        const snapshot = () => snapshots.push({ commands: structuredClone(commands), samples: [probe.samples(0), probe.samples(1)] });
        probe.subscribe(0); probe.subscribe(1); deliver(); snapshot();
        probe.unsubscribe(0); deliver(); snapshot();
        probe.unsubscribe(1); deliver(); snapshot();
        probe.subscribe(0); deliver(); snapshot();
        // Replacing a subscriber overlaps old/new ownership temporarily and
        // must keep the sensor running, with only one callback for that view.
        probe.subscribe(0); deliver(); snapshot();
        probe.unsubscribe(0); deliver(); snapshot();
        return snapshots;
      });
      assert.deepEqual(actual, [
        { commands: [{ motion: true }], samples: [1, 1] },
        { commands: [{ motion: true }], samples: [1, 2] },
        { commands: [{ motion: true }, { motion: false }], samples: [1, 2] },
        { commands: [{ motion: true }, { motion: false }, { motion: true }], samples: [2, 2] },
        { commands: [{ motion: true }, { motion: false }, { motion: true }], samples: [3, 2] },
        { commands: [{ motion: true }, { motion: false }, { motion: true }, { motion: false }], samples: [3, 2] },
      ]);
      assert.deepEqual(tab.errors, []);
      console.log(`${engine}: concurrent motion subscribers keep sensors active until final owner drop`);
    } finally {
      await tab.context.close();
    }
  }
} finally {
  await browser?.close();
  await new Promise((done) => server.close(done));
}
