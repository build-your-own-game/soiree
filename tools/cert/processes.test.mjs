import test from "node:test";
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import net from "node:net";
import { createOwnedGroupKiller, killOwnedGroup, parseProcessTable } from "./processes.mjs";
import { FixtureFailure } from "./performance.mjs";

const fake = () => ({ pid: 123456, exitCode: null, signalCode: null });
const member = child => ({ pid: child.pid, pgid: child.pid, state: "S", live: true });

test("a nonterminating signal and a stuck process-table query fail within the original deadline", async () => {
  for (const stuck of [false, true]) {
    const child = fake(), started = performance.now();
    const kill = createOwnedGroupKiller({ signal: () => false, inspect: () => stuck ? new Promise(() => {}) : [member(child)] });
    await assert.rejects(kill(child, { timeoutMs: 45 }), error => error instanceof FixtureFailure && /did not terminate/.test(error.message));
    assert.ok(performance.now() - started < 500, "cleanup deadline was not bounded");
  }
});

test("EPERM is explicit, ESRCH alone cannot prove closure, and a reused leader is never signalled", async () => {
  const child = fake();
  const denied = createOwnedGroupKiller({ inspect: () => [member(child)], signal: () => { throw Object.assign(new Error("denied"), { code: "EPERM" }); } });
  await assert.rejects(denied(child), error => error instanceof FixtureFailure && /EPERM/.test(error.message));
  const missing = createOwnedGroupKiller({ inspect: () => [member(child)], signal: () => { throw Object.assign(new Error("gone"), { code: "ESRCH" }); } });
  await assert.rejects(missing(fake(), { timeoutMs: 30 }), /did not terminate/);
  let signals = 0;
  const reused = createOwnedGroupKiller({ inspect: () => [member(child)], signal: () => signals++ });
  await assert.rejects(reused({ ...child, exitCode: 0 }), /reused/);
  assert.equal(signals, 0);
});

test("zombies are distinguished from living group members; the child itself must also be reaped", async () => {
  assert.deepEqual(parseProcessTable(" 42 42 Z+\n 43 42 S\n").map(row => row.live), [false, true]);
  const child = fake();
  const inspect = () => [{ ...member(child), state: "Z", live: false }];
  const kill = createOwnedGroupKiller({ inspect, signal: () => assert.fail("zombies must not be signalled") });
  await assert.rejects(kill(child, { timeoutMs: 25 }), /did not terminate/);
  const receipt = await kill({ ...child, exitCode: 0 });
  assert.equal(receipt.zombies.length, 1);
  assert.equal(receipt.child_exited, true);
});

test("cleanup refuses invalid targets and a child outside its own detached group", async () => {
  let signals = 0;
  const child = fake();
  const kill = createOwnedGroupKiller({ inspect: () => [{ ...member(child), pgid: 42 }], signal: () => signals++ });
  await assert.rejects(kill(child), /not launched in its own detached group/);
  for (const pid of [1, 0, -1, process.pid, undefined]) await assert.rejects(kill({ ...child, pid }), FixtureFailure);
  await assert.rejects(killOwnedGroup(child), /ChildProcess object/);
  assert.equal(signals, 0);
});

const socketClosed = port => new Promise(resolve => {
  const socket = net.connect({ host: "127.0.0.1", port });
  socket.once("connect", () => { socket.destroy(); resolve(false); });
  socket.once("error", error => resolve(error.code === "ECONNREFUSED"));
});

test("real parent and grandchild sockets close in the owned group while a neighbor stays alive", { timeout: 10000 }, async () => {
  const grandchildCode = `const net=require('node:net');const s=net.createServer();s.listen(0,'127.0.0.1',()=>process.send({port:s.address().port}));setInterval(()=>{},1000);`;
  const parentCode = `const{spawn}=require('node:child_process');const net=require('node:net');const s=net.createServer();s.listen(0,'127.0.0.1',()=>{const c=spawn(process.execPath,['-e',${JSON.stringify(grandchildCode)}],{stdio:['ignore','ignore','ignore','ipc']});c.once('message',m=>process.send({parent:s.address().port,grandchild:m.port,pid:c.pid}));});setInterval(()=>{},1000);`;
  const child = spawn(process.execPath, ["-e", parentCode], { detached: true, stdio: ["ignore", "ignore", "ignore", "ipc"] });
  const neighbor = spawn(process.execPath, ["-e", "setInterval(()=>{},1000)"], { detached: true, stdio: "ignore" });
  try {
    const [ports] = await once(child, "message");
    assert.equal(await socketClosed(ports.parent), false);
    assert.equal(await socketClosed(ports.grandchild), false);
    const pending = killOwnedGroup(child);
    assert.equal(killOwnedGroup(child), pending, "repeat cleanup launched another signal sequence");
    const receipt = await pending;
    assert.deepEqual(receipt.live, []);
    assert.equal(receipt.child_exited, true);
    assert.ok(receipt.members_before_signal.some(member => member.pid === child.pid && member.pgid === child.pid));
    assert.ok(receipt.members_before_signal.some(member => member.pid === ports.pid && member.pgid === child.pid), "grandchild was not observed in the owned process group");
    assert.equal(await socketClosed(ports.parent), true);
    assert.equal(await socketClosed(ports.grandchild), true);
    assert.equal(process.kill(neighbor.pid, 0), true, "neighbor was killed");
    assert.equal(await killOwnedGroup(child), receipt);
  } finally { await Promise.allSettled([killOwnedGroup(child), killOwnedGroup(neighbor)]); }
});

test("cleanup of an already terminated child does not send a signal", { timeout: 5000 }, async () => {
  const child = spawn(process.execPath, ["-e", ""], { detached: true, stdio: "ignore" });
  await once(child, "exit");
  let signals = 0;
  const kill = createOwnedGroupKiller({ signal: () => signals++ });
  const receipt = await kill(child);
  assert.equal(signals, 0);
  assert.equal(receipt.child_exited, true);
});
