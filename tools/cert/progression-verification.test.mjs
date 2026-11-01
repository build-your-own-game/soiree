import test from "node:test";
import assert from "node:assert/strict";
import { verifyTerminalJourney } from "./progression-verification.mjs";

const fixture = () => {
  let phase = 0;
  return { read: async () => ({ phase }), advance: async () => phase++, terminal: state => state.phase === 3, restart: async () => { phase = 4; }, restarted: state => state.phase === 4, maxSteps: 8, timeoutMs: 500 };
};
test("a terminal and a different resumed running state must both be observed", async () => {
  const result = await verifyTerminalJourney(fixture());
  assert.equal(result.steps, 3); assert.deepEqual(result.terminal, { phase: 3 }); assert.deepEqual(result.resumed, { phase: 4 });
});
test("an endlessly changing A↔B cannot stand in for a terminal", async () => {
  let phase = "A";
  await assert.rejects(verifyTerminalJourney({ ...fixture(), read: async () => ({ phase }), advance: async () => { phase = phase === "A" ? "B" : "A"; }, terminal: state => state.phase === "end", maxSteps: 6 }), error => /without a terminal/.test(error.message) && error.diagnostics.steps === 6 && error.diagnostics.terminal_seen === false);
});
test("seeing a terminal without a working restart still fails", async () => {
  await assert.rejects(verifyTerminalJourney({ ...fixture(), restart: async () => {}, timeoutMs: 30 }), error => /deadline/.test(error.message) && error.diagnostics.terminal_seen);
});
test("missing terminal metadata, already-terminal setup and a stuck driver cannot pass", async () => {
  await assert.rejects(verifyTerminalJourney({ ...fixture(), read: async () => null }), /state absent/);
  await assert.rejects(verifyTerminalJourney({ ...fixture(), terminal: () => true }), /already terminal/);
  await assert.rejects(verifyTerminalJourney({ ...fixture(), advance: () => new Promise(() => {}), timeoutMs: 25 }), /deadline during advance/);
  await assert.rejects(verifyTerminalJourney({ ...fixture(), terminal: () => Promise.resolve(false) }), /must return a boolean/);
});
