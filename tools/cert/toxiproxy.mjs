// Each phone's Wi-Fi is a Toxiproxy proxy (Shopify's fault-injecting TCP
// proxy: https://github.com/Shopify/toxiproxy). The phone's page is loaded
// through it, so its WebSocket goes through it too, and the harness can hurt
// that phone alone.
//
//   delay(ms, jitter)  each direction waits ms ± jitter (a slow Wi-Fi)
//   cut()              closes every connection and refuses new ones (the
//                      Wi-Fi drops; both ends see it) until heal()
//   blackhole()        nothing gets through and nothing is closed (an iPhone
//                      that locks, a router that hangs): the host only learns
//                      it from its own silence timeout, the page from its own
//                      heartbeat, if it has one
//   deafen()           nothing gets from the host to the page, and the page
//                      still gets through to the host: the host has no
//                      reason to close anything, so only the page's own
//                      silence timer can notice
//   reset()            closes every connection at once (iOS took the sockets
//                      back while the phone slept) and lets new ones through
//   heal()             back to normal
//
// The harness starts its own toxiproxy-server on a free port and stops it at
// the end; install it with `brew install toxiproxy`.

import { spawn, execFileSync } from "node:child_process";
import net from "node:net";

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

async function freePort() {
  return new Promise((resolve) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

export class Toxiproxy {
  static async start() {
    try {
      execFileSync("which", ["toxiproxy-server"]);
    } catch {
      throw new Error("toxiproxy-server introuvable : brew install toxiproxy");
    }
    const port = await freePort();
    const server = spawn("toxiproxy-server", ["-host", "127.0.0.1", "-port", String(port)], { stdio: "ignore" });
    const toxiproxy = new Toxiproxy(`http://127.0.0.1:${port}`, server);
    for (let tries = 0; ; tries++) {
      try {
        await toxiproxy.api("GET", "/version");
        return toxiproxy;
      } catch (error) {
        if (tries > 50) throw new Error(`toxiproxy-server ne répond pas : ${error.message}`);
        await sleep(100);
      }
    }
  }

  constructor(base, server) {
    this.base = base;
    this.server = server;
    this.count = 0;
  }

  async api(method, path, body) {
    const response = await fetch(`${this.base}${path}`, {
      method,
      headers: body ? { "content-type": "application/json" } : {},
      body: body ? JSON.stringify(body) : undefined,
    });
    if (!response.ok && response.status !== 404) throw new Error(`Toxiproxy ${method} ${path} : ${response.status} ${await response.text()}`);
    return response.status === 204 ? null : response.json().catch(() => null);
  }

  /** A new proxy toward `host:port`, listening on a free local port. */
  async proxy(host, port) {
    const name = `cert-${++this.count}`;
    const created = await this.api("POST", "/proxies", { name, listen: "127.0.0.1:0", upstream: `${host}:${port}`, enabled: true });
    return new Proxy(this, name, Number(created.listen.split(":").pop()));
  }

  stop() {
    this.server.kill();
  }
}

class Proxy {
  constructor(toxiproxy, name, port) {
    this.toxiproxy = toxiproxy;
    this.name = name;
    this.port = port;
    this.toxics = new Set();
  }

  set(enabled) {
    return this.toxiproxy.api("POST", `/proxies/${this.name}`, { enabled });
  }

  async toxic(name, type, stream, attributes) {
    await this.toxiproxy.api("POST", `/proxies/${this.name}/toxics`, { name, type, stream, attributes });
    this.toxics.add(name);
  }

  async clear() {
    for (const name of this.toxics) await this.toxiproxy.api("DELETE", `/proxies/${this.name}/toxics/${name}`);
    this.toxics.clear();
  }

  async delay(ms, jitter = 0) {
    await this.clear();
    if (ms <= 0) return;
    for (const stream of ["upstream", "downstream"]) {
      await this.toxic(`latency-${stream}`, "latency", stream, { latency: ms, jitter });
    }
  }

  cut() {
    return this.set(false);
  }

  async blackhole() {
    // `timeout` with 0: data is dropped and the connection is never closed.
    for (const stream of ["upstream", "downstream"]) {
      await this.toxic(`blackhole-${stream}`, "timeout", stream, { timeout: 0 });
    }
  }

  deafen() {
    return this.toxic("blackhole-downstream", "timeout", "downstream", { timeout: 0 });
  }

  async reset() {
    await this.set(false);
    await this.clear();
    await this.set(true);
  }

  async heal() {
    await this.clear();
    await this.set(true);
  }

  close() {
    return this.toxiproxy.api("DELETE", `/proxies/${this.name}`);
  }
}
