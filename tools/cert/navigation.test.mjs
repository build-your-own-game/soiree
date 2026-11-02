import test from "node:test";
import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import http from "node:http";
import { mkdtempSync, readFileSync, writeFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { execFile } from "node:child_process";
import { promisify } from "node:util";
import { launchBrowser, NavigationFailure, phoneTransport, preflightPhoneUpstream, Tab } from "./browser.mjs";
import { FixtureFailure } from "./performance.mjs";

const execute = promisify(execFile);

async function serverFixture(handler, run) {
  const sockets = new Set();
  const server = http.createServer(handler);
  server.on("connection", (socket) => { sockets.add(socket); socket.on("close", () => sockets.delete(socket)); });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try { await run(`http://127.0.0.1:${server.address().port}`, sockets); }
  finally {
    for (const socket of sockets) socket.destroy();
    await new Promise((resolve) => server.close(resolve));
  }
}

test("phone transport never silently falls back from LAN to loopback", () => {
  assert.deepEqual(phoneTransport({ phone: "/play" }, "192.0.2.1"), { mode: "lan", upstream: "192.0.2.1", explicit: false });
  assert.deepEqual(phoneTransport({ phone: "/play" }, null), { mode: "lan", upstream: null, explicit: false });
  assert.deepEqual(phoneTransport({ phone: "/play", phone_upstream: "loopback" }, null), { mode: "loopback", upstream: "127.0.0.1", explicit: true });
  assert.equal(phoneTransport({ phone: null }, null).mode, "none");
  assert.throws(() => phoneTransport({ phone: "/play", phone_upstream: "typo" }, "192.0.2.1"), FixtureFailure);
});

test("HTTP preflight distinguishes a served product error from a missing response", { timeout: 5000 }, async () => {
  await serverFixture((request, response) => {
    if (request.url === "/stall") return;
    response.writeHead(request.url === "/error" ? 503 : 200);
    response.end("fixture");
  }, async (base, sockets) => {
    const answer = await preflightPhoneUpstream(base + "/ok");
    assert.equal(answer.status, 200);
    await assert.rejects(preflightPhoneUpstream(base + "/error"), (error) => {
      assert.ok(error instanceof NavigationFailure && error instanceof FixtureFailure);
      assert.equal(error.kind, "product");
      assert.equal(error.phase, "phone-upstream");
      assert.equal(error.status, 503);
      return true;
    });
    const before = performance.now();
    await assert.rejects(preflightPhoneUpstream(base + "/stall", 80), (error) => {
      assert.ok(error instanceof NavigationFailure);
      assert.equal(error.kind, "infrastructure");
      assert.equal(error.status, null);
      assert.equal(error.timeoutMs, 80);
      return true;
    });
    assert.ok(performance.now() - before < 1000, "the request has a real deadline");
    for (let i = 0; i < 20 && sockets.size; i++) await new Promise((resolve) => setTimeout(resolve, 10));
    assert.equal(sockets.size, 0, "settled preflights leave no open request sockets");
  });
});

/** A short deadline proves a cause only if that cause came before it expired,
 * and a slow machine can spend 250 ms before the browser reaches it. Such a try
 * concludes nothing: the next one doubles the deadline, and each has its own
 * tab, so that a navigation left running cannot land inside the following one. */
const DEADLINES = [250, 500, 1000, 2000, 4000];

async function firstConclusive(what, attempt) {
  for (const timeoutMs of DEADLINES) if (await attempt(timeoutMs)) return timeoutMs;
  assert.fail(`${what}: never observed within ${DEADLINES.at(-1)} ms`);
}

test("initial navigation waits for load and reports HTTP or resource timeouts on both engines", { timeout: 120000 }, async () => {
  const browser = await launchBrowser();
  let heldRequests = 0;
  try {
    await serverFixture((request, response) => {
      const page = (css) => {
        response.writeHead(200, { "content-type": "text/html" });
        response.end(`<link rel="stylesheet" href="${css}"><p id="ready">fixture</p>`);
      };
      if (request.url === "/error") { response.writeHead(500); response.end("served failure"); }
      else if (request.url === "/slow-error") {
        response.writeHead(500, { "content-type": "text/html" });
        response.write("<!doctype html><p>served error before the body finishes</p>" + " ".repeat(1024));
      }
      else if (request.url === "/held.css") {
        heldRequests++;
      }
      else if (request.url === "/delayed.css") {
        const timer = setTimeout(() => { response.writeHead(200, { "content-type": "text/css" }); response.end("body{color:green}"); }, 100);
        response.on("close", () => clearTimeout(timer));
      }
      else if (request.url === "/slow") page("/held.css");
      else if (request.url === "/slow-document") {
        // What a slow runner does to the fixture above, on any machine.
        const timer = setTimeout(() => page("/held.css"), 300);
        response.on("close", () => clearTimeout(timer));
      }
      else page("/delayed.css");
    }, async (base) => {
      for (const engine of ["webkit", "chromium"]) {
        const tab = await browser.open(null, { engine });
        try {
          const started = performance.now();
          assert.equal((await tab.goto(base + "/ok")).status(), 200);
          assert.ok(performance.now() - started >= 70, "onload waits for the delayed stylesheet");
          assert.equal(await tab.page.evaluate(() => document.readyState), "complete");
          await assert.rejects(tab.goto(base + "/error"), (error) => {
            assert.ok(error instanceof NavigationFailure);
            assert.equal(error.kind, "product");
            assert.equal(error.status, 500);
            assert.equal(error.phase, "initial-navigation");
            return true;
          });
        } finally { await tab.context.close(); }
        await firstConclusive(`${engine}: the served HTTP 500`, async (timeoutMs) => {
          const tab = await browser.open(null, { engine });
          try {
            let served = false;
            await assert.rejects(tab.goto(base + "/slow-error", { timeoutMs }), (error) => {
              assert.ok(error instanceof NavigationFailure);
              served = error.kind === "product";
              if (served) assert.equal(error.status, 500, "a served HTTP 500 is not hidden by a later load timeout");
              else assert.equal(error.status, null, "no status was seen before this deadline");
              return true;
            });
            return served;
          } finally { await tab.context.close(); }
        });
        const blockedByItsResource = async (path, timeoutMs) => {
          const tab = await browser.open(null, { engine });
          try {
            let loaded = false;
            tab.page.once("load", () => { loaded = true; });
            const requestedBefore = heldRequests;
            let requested = false;
            await assert.rejects(tab.goto(base + path, { timeoutMs }), (error) => {
              // Read at the rejection: a browser which keeps navigating past
              // this deadline asks for the resource later, too late to be why.
              requested = heldRequests > requestedBefore;
              assert.ok(error instanceof NavigationFailure);
              assert.equal(error.kind, "infrastructure");
              assert.equal(error.timeoutMs, timeoutMs);
              return true;
            });
            assert.equal(loaded, false, "a timed out document must not be treated as loaded");
            assert.equal(tab.pendingInput, null);
            assert.equal(tab.lastNavigationFailure.kind, "infrastructure");
            return requested;
          } finally { await tab.context.close(); }
        };
        await firstConclusive(`${engine}: the browser really requested the blocked resource`, (timeoutMs) => blockedByItsResource("/slow", timeoutMs));
        const late = await firstConclusive(`${engine}: the resource of a late document`, (timeoutMs) => blockedByItsResource("/slow-document", timeoutMs));
        assert.ok(late > 250, "a deadline which expired before the document says nothing of its resource");
      }
    });
  } finally { await browser.close(); }
});

test("a navigation left running cannot pass its late response to the next one", async () => {
  const frame = {};
  const response = (request, status) => ({ status: () => status, request: () => request, frame: () => frame, url: () => "http://fixture.test/late" });
  const navigation = () => ({ isNavigationRequest: () => true });
  for (const [own, kind, status] of [[false, "infrastructure", null], [true, "product", 500]]) {
    const page = new EventEmitter();
    page.mainFrame = () => frame;
    page.url = () => "about:blank";
    page.goto = async () => {
      const request = navigation();
      if (own) page.emit("request", request);
      page.emit("response", response(request, 500));
      throw new Error("navigation interrupted");
    };
    const tab = new Tab({}, page, false);
    await assert.rejects(tab.goto("http://fixture.test/next"), (error) => {
      assert.ok(error instanceof NavigationFailure);
      assert.equal(error.kind, kind);
      assert.equal(error.status, status);
      return true;
    });
  }
});

test("unavailable LAN and served HTTP errors stop before UI or panic scenarios", { timeout: 20000 }, async () => {
  const root = mkdtempSync(join(tmpdir(), "soiree-navigation-fixture-"));
  try {
    // Isolate network-interface discovery in this child only. The fixture uses
    // loopback sockets, with no dependence on a real Wi-Fi or firewall setting.
    const network = join(root, "network.mjs");
    writeFileSync(network, `import { createRequire, syncBuiltinESMExports } from 'node:module';
const require = createRequire(import.meta.url);
require('node:os').networkInterfaces = () => ({ fixture: [{family:'IPv4',internal:false,address:'127.0.0.1'}] });
syncBuiltinESMExports();`);
    const host = join(root, "host.mjs");
    writeFileSync(host, `import http from 'node:http';
import {writeFileSync} from 'node:fs';
writeFileSync('host.pid', String(process.pid));
http.createServer((request,response) => {
 if (process.argv[3] === 'stall') return;
 response.writeHead(500); response.end('served fixture HTTP error');
}).listen(Number(process.argv[2]), '127.0.0.1', () => console.log('fixture ready'));`);
    for (const mode of ["stall", "http-error"]) {
      writeFileSync(join(root, "cert.json"), JSON.stringify({ launch: [process.execPath, host, "{port}", mode], ready: "fixture ready", tv: "/", phone: "/play" }));
      const evidencePath = join(root, mode + ".json");
      const started = performance.now();
      let result;
      try {
        await execute(process.execPath, ["--import", network, fileURLToPath(new URL("./cert.mjs", import.meta.url)), root, "--only", "ui,panic"], {
          env: { ...process.env, SOIREE_OBSERVATIONS_PATH: evidencePath }, timeout: 9000,
        });
        assert.fail("the negative control must fail");
      } catch (error) { result = error; }
      assert.equal(result.killed, false, result.stderr);
      assert.equal(result.code, mode === "stall" ? 2 : 1, result.stderr);
      // Two seconds of this budget are the machine's load, measured at both ends.
      assert.ok(performance.now() - started < 8000, "the LAN fixture fails before any browser wait");
      const evidence = JSON.parse(readFileSync(evidencePath, "utf8"));
      assert.equal(evidence.transport.mode, "lan");
      assert.equal(evidence.transport.preflight.status, "failed");
      assert.equal(evidence.navigation_failure.phase, "phone-upstream");
      assert.equal(evidence.navigation_failure.kind, mode === "stall" ? "infrastructure" : "product");
      assert.equal(evidence.infrastructure_error === null, mode !== "stall");
      assert.deepEqual(evidence.records.map((record) => record.item), [mode === "stall" ? "H1" : "HTTP"]);
      assert.ok(!evidence.records.some((record) => ["U1", "C10"].includes(record.item)));
      const pid = Number(readFileSync(join(root, "host.pid"), "utf8"));
      assert.throws(() => process.kill(pid, 0), (error) => error.code === "ESRCH");
    }
  } finally { rmSync(root, { recursive: true, force: true }); }
});
