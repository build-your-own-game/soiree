import test from "node:test";
import assert from "node:assert/strict";
import { silenceReaction } from "./browser.mjs";

const socket = { kind: "construct", id: 1, at: 0, order: 0, ready: 0, url: "ws://host/salon/ws" };
const own = { kind: "close-call", id: 1, at: 6_000, order: 1, ready: 1 };
const retry = { kind: "construct", id: 2, at: 6_002, order: 2, ready: 0, url: socket.url };

test("C3 counts a constructor attempt even when its upgrade is swallowed", () => {
  assert.equal(silenceReaction([socket, own, retry], [1], 0).independent, true);
});

test("C3 rejects server close/error followed by a normal reconnect", () => {
  const close = { kind: "close-event", id: 1, at: 5_999, order: 1, ready: 3 };
  assert.equal(silenceReaction([socket, close, { ...own, ready: 3, order: 2 }, { ...retry, order: 3 }], [1], 0).independent, false);
  // Even an unusual browser ordering that still reports OPEN cannot turn
  // an earlier native error into an independent silence watchdog.
  assert.equal(silenceReaction([socket, { ...close, kind: "error-event" }, { ...own, order: 2 }, { ...retry, order: 3 }], [1], 0).independent, false);
});

test("C3 requires same endpoint, own original socket and a timely retry", () => {
  assert.equal(silenceReaction([socket, own], [1], 0).independent, false);
  assert.equal(silenceReaction([socket, own, { ...retry, at: 20_001 }], [1], 0).independent, false);
  assert.equal(silenceReaction([socket, own, { ...retry, url: "ws://other/telemetry" }], [1], 0).independent, false);
  assert.equal(silenceReaction([socket, own, retry], [99], 0).independent, false);
});
