// Independent wire assertions against real Swift-produced scripts and WASM.
// Prerequisites: the same browsers as `soiree doctor`; run with
// `cargo xtask web-contract-browser` on macOS.
import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { launchBrowser } from "../../tools/cert/browser.mjs";

const root = fileURLToPath(new URL("../target/demo-page/", import.meta.url));
const fixtures = JSON.parse(await readFile(new URL("../target/web-contract/native-fixtures.json", import.meta.url)));
const mime = { ".wasm": "application/wasm", ".js": "text/javascript", ".html": "text/html", ".css": "text/css" };
const server = createServer(async (request, response) => {
  const pathname = new URL(request.url, "http://localhost").pathname;
  const path = resolve(root, `.${pathname === "/" ? "/index.html" : pathname}`);
  if (!path.startsWith(root.endsWith(sep) ? root : root + sep)) {
    response.writeHead(403).end();
    return;
  }
  try {
    const data = await readFile(path);
    response.writeHead(200, { "Content-Type": mime[extname(path)] ?? "application/octet-stream" }).end(data);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
let browser;
try {
  browser = await launchBrowser();
  for (const engine of ["webkit", "chromium"]) {
    const tab = await browser.open(null, { engine, mobile: true });
    try {
      await tab.context.addInitScript({ content: `${fixtures.bootstrap}
        window.nativeCommands = [];
        window.webkit = { messageHandlers: { [${JSON.stringify(fixtures.handler)}]: {
          postMessage(command) { window.nativeCommands.push(command); }
        } } };` });
      await tab.page.goto(`http://127.0.0.1:${server.address().port}/`);
      await tab.page.waitForFunction(() => window.nativeCommands?.length >= 2, null, { timeout: 5_000 });
      assert.equal(await tab.page.locator("#info").innerText(), `joueur 11111111…\nprénom « ${fixtures.name} »\nLandscapeLeft`);
      assert.deepEqual(await tab.page.evaluate(() => window.nativeCommands), [
        { motion: true }, { datagram: { op: "open", id: 1, port: 8095 } },
      ]);
      await tab.page.evaluate(fixtures.motion);
      assert.equal(await tab.page.locator("#motion").innerText(), "gravité +0.00 -1.00 +0.00");
      await tab.page.evaluate(fixtures.orientation);
      assert.equal(await tab.page.locator("#info").innerText(), "tourné : Portrait");
      await tab.page.evaluate(fixtures.datagram);
      assert.equal(await tab.page.locator("#echo").innerText(), "native ↔ page");
      await tab.page.locator("#buzz").dispatchEvent("pointerdown");
      await tab.page.locator("#buzz").dispatchEvent("pointerup");
      await tab.page.locator("#turn").click();
      const commands = await tab.page.evaluate(() => window.nativeCommands.slice(2));
      assert.deepEqual(commands, [
        { vibrate: "press" },
        { datagram: { op: "send", id: 1, data: [...Buffer.from("tap 1 depuis Rust")] } },
        { haptic: [{ time: 0, duration: 0, intensity: 0.5, sharpness: 0.9 }, { time: 0.08, duration: 0, intensity: 0.3, sharpness: 0.9 }] },
        { orientation: "portrait" },
      ]);
      assert.deepEqual(tab.errors, []);
      console.log(`${engine}: native → WASM events and WASM → native commands passed`);
    } finally {
      await tab.context.close();
    }
  }
  const standalone = await browser.open(`http://127.0.0.1:${server.address().port}/`);
  await standalone.page.waitForFunction(() => document.querySelector("#info").innerText.includes("Hors de l'app"), null, { timeout: 5_000 });
  await standalone.page.locator("#buzz").dispatchEvent("pointerdown");
  await standalone.page.locator("#turn").click();
  assert.deepEqual(standalone.errors, []);
  console.log("standalone browser fallback passed");
} finally {
  await browser?.close();
  await new Promise((done) => server.close(done));
}
