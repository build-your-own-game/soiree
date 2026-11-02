// Exercise WebRTC's negotiation contracts on the real host, in both engines,
// without a game: what `soiree certify`'s `rtc` scenario runs, on the demo.
// From the common root: node salon/tests/rtc-browser.mjs
// Prerequisites: Rust/WASM, matching wasm-bindgen, tools/cert npm ci + browsers.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { createServer } from "node:net";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { launchBrowser, sleep } from "../../tools/cert/browser.mjs";
import { verifyRtcNegotiation } from "../../tools/cert/rtc-negotiation.mjs";
import { welcomeDevice } from "../../tools/cert/udp.mjs";

const workspace = fileURLToPath(new URL("../", import.meta.url));
const port = await new Promise((resolve, reject) => {
  const probe = createServer();
  probe.once("error", reject);
  probe.listen(0, "127.0.0.1", () => {
    const { port } = probe.address();
    probe.close(() => resolve(port));
  });
});
const data = mkdtempSync(join(tmpdir(), "salon-rtc-browser-"));
const host = spawn("cargo", ["run", "--release", "--locked", "-p", "salon-demo"], {
  cwd: workspace,
  env: { ...process.env, SALON_DEMO_PORT: String(port), SALON_DEMO_DATA: data, SALON_LOG: "1" },
  stdio: ["ignore", "pipe", "inherit"],
});
const ready = new Promise((resolve, reject) => {
  let told = "";
  host.stdout.on("data", (chunk) => {
    told += chunk;
    if (told.includes("Salon demo ready")) resolve();
  });
  host.once("exit", (code) => reject(new Error(`the host stopped: ${code}`)));
});
let browser;
const summary = {};
try {
  await ready;
  browser = await launchBrowser({
    identify: (bytes) => welcomeDevice([...bytes]),
  });
  for (const engine of (process.argv[2] ?? "webkit,chromium").split(",")) {
    const url = `http://127.0.0.1:${port}/play?salon=ws`;
    const tab = await browser.open(url, { engine });
    try {
      for (let waited = 0; tab.device === null && waited < 10_000; waited += 50) await sleep(50);
      assert.ok(tab.device, `${engine}: the phone was never welcomed`);
      const { cases } = await verifyRtcNegotiation({
        page: tab.page, url, upstream: "127.0.0.1", port, expectedDevice: tab.device,
        onCleanupFailure: (error) => { throw error; },
      });
      for (const { name, elapsed_ms: elapsed, closed_ms: closed, resume_ms: resume } of cases) {
        const measured = [closed && `attempt closed at ${Math.round(closed)} ms`, resume && `back in ${Math.round(resume)} ms`].filter(Boolean);
        console.log(`${engine} — ${name}: ${elapsed} ms${measured.length ? ` (${measured.join(", ")})` : ""}`);
      }
      summary[engine] = cases.length;
    } finally {
      await tab.context.close();
    }
  }
  console.log(JSON.stringify(summary));
} finally {
  await browser?.close();
  host.kill("SIGKILL");
  rmSync(data, { recursive: true, force: true });
}
