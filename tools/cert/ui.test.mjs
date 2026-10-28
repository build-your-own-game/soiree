import { test } from "node:test";
import assert from "node:assert/strict";
import { validateUi, runUiScenario, runUiCommand } from "./ui.mjs";

const exempt = { editing: "no fields", effects: "pure static view", lifecycle: "page lifetime", async: "no background operations" };
test("UI coverage cannot disappear silently or contradict itself", () => {
  validateUi({ not_applicable: exempt });
  assert.throws(() => validateUi(undefined));
  assert.throws(() => validateUi({ not_applicable: { ...exempt, editing: "" } }));
  assert.throws(() => validateUi({ scenarios: [{ name: "edit", screen: "tv", covers: ["editing"], steps: [{ assert: "true" }] }], not_applicable: exempt }));
  assert.throws(() => validateUi({ scenarios: [{ name: "edit", screen: "tv", covers: ["invented"], steps: [{ assert: "true" }] }] }));
  assert.throws(() => validateUi({ scenarios: [{ name: "edit", screen: "tv", covers: ["editing"], steps: [{ click: "#x" }] }] }));
});
test("a failed browser observation fails the public scenario runner", async () => {
  const page = { evaluate: async (expression) => typeof expression === "function" && expression.toString().includes("Boolean") ? false : undefined };
  await assert.rejects(runUiScenario({ page }, { name: "broken", steps: [{ assert: "false" }] }), /assertion fausse/);
});

test("success evidence rejects invalid patterns and zero-test green exits", async () => {
  const check = { name: "one exact test", covers: ["effects"], command: [process.execPath, "-e", "console.log('test result: ok. 0 passed')"], success: "test result: ok\\. 1 passed" };
  const options = { cwd: process.cwd(), url: "http://localhost/" };
  await assert.rejects(runUiCommand(check, options), /sans la preuve attendue[\s\S]*0 passed/);
  const output = await runUiCommand({ ...check, command: [process.execPath, "-e", "console.error('test result: ok. 1 passed')"] }, options);
  assert.match(output, /1 passed/);
  for (const success of ["", "[", 1]) {
    assert.throws(() => validateUi({ checks: [{ ...check, success }], not_applicable: { editing: "no input", lifecycle: "page lifetime", async: "no tasks" } }), /success/);
  }
});

test("UI waits keep polling resolved false values until a later observation is true", async () => {
  let polls = 0;
  let assertion = false;
  const page = {
    evaluate: async (_callback, expression) => {
      if (expression === "eventual condition") return ++polls >= 3;
      if (expression === "following assertion") { assertion = polls >= 3; return assertion; }
    },
  };
  await runUiScenario({ page }, { name: "eventually true", steps: [{ wait: "eventual condition" }, { assert: "following assertion" }] });
  assert.equal(polls, 3);
  assert.equal(assertion, true);
});

test("UI command timeouts must be finite positive timer values", async () => {
  const check = { name: "bounded", covers: ["effects"], command: [process.execPath, "-e", "process.exit(0)"] };
  const declaration = (timeout_ms) => ({ checks: [{ ...check, timeout_ms }], not_applicable: { editing: "no input", lifecycle: "page lifetime", async: "no tasks" } });
  validateUi(declaration(120_000));
  for (const timeout_ms of [0, -1, NaN, Infinity, null, "20", 2_147_483_648]) {
    assert.throws(() => validateUi(declaration(timeout_ms)), /timeout_ms/);
    await assert.rejects(runUiCommand({ ...check, timeout_ms }, { cwd: process.cwd(), url: "http://localhost/" }), /timeout_ms/);
  }
});

test("a true result arriving after the UI deadline cannot beat a delayed timer", async (t) => {
  let now = 0;
  t.mock.method(performance, "now", () => now);
  const page = {
    evaluate: async (_callback, expression) => {
      if (expression === "late true") { now = 5001; return true; }
    },
  };
  await assert.rejects(runUiScenario({ page }, { name: "late true", steps: [{ wait: "late true" }] }), /attente non satisfaite après 5000 ms/);
});

test("a stopped page cannot hide an assertion failure behind observer cleanup", { timeout: 2000 }, async () => {
  const page = {
    evaluate: async (callback, expression) => {
      if (expression === "false") return false;
      if (String(callback).includes("delete window.__certUiRemembered")) return new Promise(() => {});
    },
  };
  await assert.rejects(runUiScenario({ page }, { name: "stopped cleanup", steps: [{ assert: "false" }] }), /assertion fausse/);
});
