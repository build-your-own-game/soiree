import test from "node:test";
import assert from "node:assert/strict";
import { spawn, spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus, tmpdir } from "node:os";
import { join } from "node:path";
import { LOADED_CPU_SHARE, busyShare, loadSummary, loadWarning, measureLoad } from "./load.mjs";

const cpu = (busy, idle) => ({ times: { user: busy, nice: 0, sys: 0, idle, irq: 0 } });

test("the busy share counts what happened between two readings, on every core", () => {
  assert.equal(busyShare([cpu(100, 900), cpu(0, 0)], [cpu(100, 1900), cpu(1000, 0)]), 0.5);
  assert.equal(busyShare([cpu(0, 0)], [cpu(0, 1000)]), 0);
  assert.equal(busyShare([cpu(0, 0)], [cpu(1000, 0)]), 1);
});

test("a reading that says nothing is unknown, never an idle machine", () => {
  assert.equal(busyShare([], []), null);
  assert.equal(busyShare([cpu(5, 5)], [cpu(5, 5)]), null);
  assert.equal(busyShare([cpu(5, 5)], [cpu(5, 5), cpu(1, 1)]), null);
});

test("one measure takes its window, and keeps the context beside the share", async () => {
  const readings = [[cpu(0, 0)], [cpu(600, 400)]];
  const waited = [];
  const sample = await measureLoad({ windowMs: 7, read: () => readings.shift(), average: () => [3.5, 2, 1], wait: async (ms) => waited.push(ms) });
  assert.deepEqual(waited, [7]);
  assert.equal(sample.cpu_share, 0.6);
  assert.equal(sample.load_1m, 3.5);
  assert.equal(sample.cores, 1);
});

test("a machine busy at either end of a run is loaded", () => {
  const at = (cpu_share) => ({ cores: 8, cpu_share, load_1m: 1 });
  const calm = LOADED_CPU_SHARE - 0.01;
  assert.equal(loadSummary(at(calm), at(calm)).state, "calm");
  assert.equal(loadSummary(at(LOADED_CPU_SHARE), at(calm)).state, "loaded");
  assert.equal(loadSummary(at(calm), at(0.9)).state, "loaded");
  assert.equal(loadSummary(at(null), at(calm)).state, "unknown");
  assert.equal(loadSummary(at(null), at(0.9)).state, "loaded");
  assert.equal(loadSummary(at(calm), at(calm)).threshold, LOADED_CPU_SHARE);
});

test("the warning names both measures and stays silent on a calm machine", () => {
  const at = (cpu_share) => ({ cores: 8, cpu_share, load_1m: 1 });
  assert.equal(loadWarning(loadSummary(at(0.1), at(0.2))), null);
  assert.match(loadWarning(loadSummary(at(0.82), at(0.12))), /chargée.*82 %.*12 %.*50 %/);
  assert.match(loadWarning(loadSummary(at(null), at(0.1))), /non mesurée/);
});

test("a real load on every core is seen as one", { timeout: 20000 }, async () => {
  const burn = "const end = Date.now() + 3000; while (Date.now() < end);";
  const workers = cpus().map(() => spawn(process.execPath, ["-e", burn], { stdio: "ignore" }));
  const done = Promise.all(workers.map((worker) => new Promise((resolve) => worker.once("exit", resolve))));
  try {
    await new Promise((resolve) => setTimeout(resolve, 500));
    const sample = await measureLoad({ windowMs: 1000 });
    assert.ok(sample.cpu_share >= LOADED_CPU_SHARE, `every core burnt, read as ${sample.cpu_share}`);
  } finally {
    for (const worker of workers) worker.kill();
    await done;
  }
});

test("the real harness keeps both measures, even when the game never starts", { timeout: 30000 }, () => {
  const game = mkdtempSync(join(tmpdir(), "cert-load-"));
  try {
    writeFileSync(join(game, "cert.json"), JSON.stringify({ launch: ["false"], relaunch: ["false"], ready: "never", tv: "/" }));
    const observations = join(game, "observations.json");
    const run = spawnSync(process.execPath, ["cert.mjs", game, "--only", "ui", "--seed", "1"], { cwd: import.meta.dirname, env: { ...process.env, SOIREE_OBSERVATIONS_PATH: observations }, encoding: "utf8" });
    assert.equal(run.status, 2, run.stdout + run.stderr);
    const load = JSON.parse(readFileSync(observations, "utf8")).machine_load;
    assert.equal(typeof load.start.cpu_share, "number");
    assert.equal(typeof load.end.cpu_share, "number");
    assert.ok(["calm", "loaded"].includes(load.state));
    assert.equal(load.threshold, LOADED_CPU_SHARE);
  } finally { rmSync(game, { recursive: true, force: true }); }
});
