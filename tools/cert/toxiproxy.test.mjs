import test from "node:test";
import assert from "node:assert/strict";
import { createServer } from "node:http";
import { once, EventEmitter } from "node:events";
import { spawn, spawnSync } from "node:child_process";
import net from "node:net";
import os from "node:os";
import { Toxiproxy } from "./toxiproxy.mjs";
import { FixtureFailure } from "./performance.mjs";

async function fixture(handler, check, timeoutMs = 1000) {
  const server = createServer(handler);
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  const api = new Toxiproxy(`http://127.0.0.1:${server.address().port}`, null, { timeoutMs });
  try {
    await check(api);
  } finally {
    await api.stop();
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
}

test("controller HTTP, JSON and schema failures remain infrastructure failures", async () => {
  for (const [status, body] of [[503, "<h1>Timeout</h1>"], [200, "not JSON"], [200, "null"], [200, "[]"], [404, "{}"]]) {
    await fixture((request, response) => response.writeHead(status).end(body), async api => {
      await assert.rejects(api.api("POST", "/proxies/missing"), FixtureFailure);
    });
  }
});

test("only idempotent DELETE accepts a missing object", async () => {
  await fixture((request, response) => response.writeHead(404).end("missing"), async api => {
    assert.equal(await api.api("DELETE", "/proxies/removed"), null);
    await assert.rejects(api.api("GET", "/version"), FixtureFailure);
    await assert.rejects(api.api("POST", "/proxies/removed", {}), FixtureFailure);
  });
});

test("a controller timeout includes response body consumption", async () => {
  await fixture((request, response) => { response.writeHead(200); response.write('{"unfinished":'); }, async api => {
    const started = Date.now();
    await assert.rejects(api.api("GET", "/version"), FixtureFailure);
    assert.ok(Date.now() - started < 1000, "an unending response body escaped the controller deadline");
  }, 50);
});

test("a success response must confirm proxy creation and each injected fault", async () => {
  await fixture((request, response) => response.writeHead(200).end("{}"), async api => {
    await assert.rejects(api.proxy("127.0.0.1", 1234), FixtureFailure);
    assert.equal(api.connectors.size, 0, "failed creation retained a connector");
  });
  await fixture(async (request, response) => {
    let text = "";
    for await (const chunk of request) text += chunk;
    const body = request.url === "/proxies"
      ? { ...JSON.parse(text), listen: "127.0.0.1:1235" }
      : {};
    response.writeHead(200).end(JSON.stringify(body));
  }, async api => {
    const proxy = await api.proxy("127.0.0.1", 1234);
    await assert.rejects(proxy.cut(), FixtureFailure);
    await assert.rejects(proxy.blackhole(), FixtureFailure);
  });
});

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

async function until(predicate) {
  const deadline = performance.now() + 1000;
  while (!predicate()) {
    assert.ok(performance.now() < deadline, "owned sockets did not finish closing");
    await sleep(2);
  }
}

async function connected(port) {
  const socket = net.connect(port, "127.0.0.1");
  socket.on("error", () => {});
  await once(socket, "connect");
  return socket;
}

async function echoFixture(check) {
  const clients = new Set();
  const chunks = [];
  const server = net.createServer({ allowHalfOpen: true }, socket => {
    clients.add(socket);
    socket.on("close", () => clients.delete(socket));
    socket.on("error", () => {});
    socket.on("data", bytes => chunks.push(Buffer.from(bytes)));
    socket.pipe(socket);
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  try { await check(server.address().port, clients, chunks); }
  finally {
    for (const socket of clients) socket.destroy();
    await new Promise(resolve => server.close(resolve));
  }
}

// A listening kernel socket whose owning event loop cannot accept. This
// creates a real pending TCP connect without firewall changes or remote IPs.
async function saturatedBacklog(check) {
  const child = spawn(process.execPath, ["--input-type=module", "-e", `
    import net from 'node:net';
    const server=net.createServer();
    server.listen({host:'127.0.0.1',port:0,backlog:1},()=>{
      console.log(server.address().port);
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)),0,0,60000);
    });
  `], { stdio: ["ignore", "pipe", "inherit"] });
  const sockets = [];
  try {
    const [chunk] = await once(child.stdout, "data");
    const port = Number(chunk.toString().trim());
    let connectedCount = 0;
    for (let i = 0; i < 32; i++) {
      const socket = net.connect(port, "127.0.0.1");
      socket.on("connect", () => connectedCount++);
      socket.on("error", () => {});
      sockets.push(socket);
    }
    await sleep(100);
    assert.ok(connectedCount > 0 && connectedCount < 32, "kernel backlog was not saturated");
    await check(port);
  } finally {
    for (const socket of sockets) socket.destroy();
    child.kill();
    await once(child, "exit");
  }
}

async function connectorFixture(check) {
  await fixture(async (request, response) => {
    let text = "";
    for await (const chunk of request) text += chunk;
    const body = JSON.parse(text);
    response.end(JSON.stringify({ ...body, listen: body.upstream }));
  }, check);
}

test("upstream connector preserves bytes, half-close and ownership", async () => {
  await echoFixture(async port => connectorFixture(async api => {
    const proxy = await api.proxy("127.0.0.1", port);
    const socket = await connected(proxy.port);
    const bytes = Buffer.concat([Buffer.from("日本語 🎲"), Buffer.from(Array.from({ length: 256 }, (_, i) => i))]);
    const received = [];
    socket.on("data", chunk => received.push(chunk));
    socket.end(bytes);
    await once(socket, "close");
    assert.deepEqual(Buffer.concat(received), bytes);
    await api.stop();
    assert.equal(proxy.connector.sockets.size, 0);
    await assert.rejects(connected(proxy.port), /ECONNREFUSED/);
  }));
});

test("a pending upstream dial has a deadline and teardown cancels it", async () => {
  await saturatedBacklog(async port => connectorFixture(async api => {
    const proxy = await api.proxy("127.0.0.1", port);
    const socket = await connected(proxy.port);
    const started = performance.now();
    await once(socket, "close");
    assert.ok(performance.now() - started < 3000, "upstream connect used the OS deadline");
    await until(() => proxy.connector.sockets.size === 0);
    assert.equal(proxy.connector.sockets.size, 0);
    const pending = await connected(proxy.port);
    const closed = once(pending, "close");
    await sleep(50);
    const stopping = performance.now();
    await api.stop();
    await closed;
    assert.ok(performance.now() - stopping < 1000, "teardown did not cancel the pending dial");
    assert.equal(proxy.connector.sockets.size, 0);
  }));
});

test("connector bounds pending sockets and closes them even when admin deletion fails", async () => {
  await saturatedBacklog(async port => fixture(async (request, response) => {
    let text = "";
    for await (const chunk of request) text += chunk;
    if (request.method === "DELETE") response.writeHead(503).end("broken admin");
    else {
      const body = JSON.parse(text);
      response.end(JSON.stringify({ ...body, listen: body.upstream }));
    }
  }, async api => {
    const proxy = await api.proxy("127.0.0.1", port);
    const sockets = [];
    try {
      for (let i = 0; i < 140; i++) {
        const socket = await connected(proxy.port);
        sockets.push(socket);
      }
      await until(() => sockets.some(socket => socket.destroyed));
      assert.equal(proxy.connector.sockets.size, 256, "128 accepted connections must each own exactly two sockets");
      const closures = sockets.filter(socket => !socket.destroyed).map(socket => once(socket, "close"));
      await assert.rejects(proxy.close(), FixtureFailure);
      await Promise.all(closures);
      assert.equal(proxy.connector.sockets.size, 0);
      assert.equal(api.connectors.size, 0);
    } finally { for (const socket of sockets) socket.destroy(); }
  }));
});

const lan = Object.values(os.networkInterfaces()).flat().find(address => address.family === "IPv4" && !address.internal)?.address;
test("connector dials the requested LAN address without substituting loopback", { skip: !lan && "no IPv4 LAN interface" }, async () => {
  const server = net.createServer(socket => socket.end(JSON.stringify({ local: socket.localAddress, remote: socket.remoteAddress })));
  server.listen(0, "0.0.0.0");
  await once(server, "listening");
  try {
    await connectorFixture(async api => {
      const proxy = await api.proxy(lan, server.address().port);
      const socket = await connected(proxy.port);
      const [reply] = await once(socket, "data");
      assert.deepEqual(JSON.parse(reply), { local: lan, remote: lan });
      socket.destroy();
    });
  } finally { await new Promise(resolve => server.close(resolve)); }
});

const officialAvailable = spawnSync("toxiproxy-server", ["-version"]).status === 0;
test("official fault engine remains isolated and responsive with an unconnectable upstream", { skip: !officialAvailable && "toxiproxy-server is not installed" }, async t => {
  const api = await Toxiproxy.start();
  try {
    await saturatedBacklog(async port => {
      // Negative control: the unwrapped official proxy blocks its admin
      // update behind net.Dial while the independent /version stays healthy.
      const raw = await api.api("POST", "/proxies", { name: "negative", listen: "127.0.0.1:0", upstream: `127.0.0.1:${port}`, enabled: true });
      const client = await connected(Number(raw.listen.split(":").pop()));
      try {
        await sleep(50);
        assert.ok((await api.api("GET", "/version")).version);
        await assert.rejects(api.api("POST", "/proxies/negative", { enabled: false }, 250), FixtureFailure);
        const proxy = await api.proxy("127.0.0.1", port);
        const bounded = await connected(proxy.port);
        try {
          await sleep(50);
          const start = performance.now();
          await proxy.cut();
          const cutMs = performance.now() - start;
          assert.ok(cutMs < 1000, "cut waited on the upstream OS connect timeout");
          t.diagnostic(`official ${(await api.api("GET", "/version")).version}: direct negative control exceeded 250 ms, bounded cut ${cutMs.toFixed(2)} ms`);
          await proxy.close();
          assert.equal(proxy.connector.sockets.size, 0);
        } finally { bounded.destroy(); }
      } finally { client.destroy(); }
    });
    await echoFixture(async (port, clients, chunks) => {
      const faulted = await api.proxy("127.0.0.1", port);
      const neighbor = await api.proxy("127.0.0.1", port);
      const exchange = async proxy => {
        const socket = await connected(proxy.port);
        try {
          const reply = once(socket, "data");
          socket.write("echo");
          assert.equal((await reply)[0].toString(), "echo");
        } finally { socket.destroy(); }
      };
      for (const fault of ["cut", "blackhole", "deafen", "delay", "reset"]) {
        const socket = await connected(faulted.port);
        let returned = false;
        socket.on("data", () => returned = true);
        const closed = new Promise(resolve => socket.once("close", resolve));
        try {
          const start = performance.now();
          await (fault === "delay" ? faulted.delay(100) : faulted[fault]());
          await exchange(neighbor);
          if (fault === "cut" || fault === "reset") {
            await Promise.race([closed, sleep(1000).then(() => { throw new Error(`${fault} did not close existing connection`); })]);
            if (fault === "cut") await assert.rejects(connected(faulted.port), /ECONNREFUSED/);
          } else {
            const before = chunks.length;
            socket.write("echo");
            await sleep(50);
            assert.equal(returned, false, `${fault} let bytes through without its configured fault`);
            assert.equal(socket.destroyed, false, `${fault} closed the TCP connection`);
            assert.equal(chunks.length - before, fault === "deafen" ? 1 : 0, `${fault} used the wrong stream direction`);
            if (fault === "delay") {
              await once(socket, "data");
              assert.ok(performance.now() - start >= 180, "latency did not affect both directions");
            }
          }
        } finally { socket.destroy(); }
        await faulted.heal();
        await exchange(faulted);
      }
      await faulted.close();
      await neighbor.close();
      assert.equal(api.connectors.size, 0);
    });
  } finally { await api.stop(); }
});

test("spawn error and unexpected child exit are retained for the next operation", async () => {
  for (const event of ["error", "exit"]) {
    const child = new EventEmitter();
    const api = new Toxiproxy("http://127.0.0.1:1", child);
    if (event === "error") child.emit(event, new Error("spawn failed"));
    else child.emit(event, 2, null);
    await assert.rejects(api.api("GET", "/version"), FixtureFailure);
  }
});

test("a child exit during an HTTP response cannot be a successful controller operation", async () => {
  const child = new EventEmitter();
  const server = createServer((request, response) => {
    response.writeHead(200);
    response.write('{"version":');
    child.emit("exit", 2, null);
    response.end('"2"}');
  });
  server.listen(0, "127.0.0.1");
  await once(server, "listening");
  try {
    const api = new Toxiproxy(`http://127.0.0.1:${server.address().port}`, child);
    await assert.rejects(api.api("GET", "/version"), FixtureFailure);
  } finally {
    server.closeAllConnections();
    await new Promise(resolve => server.close(resolve));
  }
});
