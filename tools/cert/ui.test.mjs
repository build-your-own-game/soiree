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
