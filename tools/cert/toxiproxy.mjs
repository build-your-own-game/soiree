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

import { spawn, spawnSync } from "node:child_process";
import net from "node:net";
import { FixtureFailure } from "./performance.mjs";

// The harness is POSIX: lsof in cert.mjs, ps and process groups in processes.mjs.
// cert.mjs imports this file, and `soiree certify` shows what its preparation
// prints: the reason reaches a Windows user before anything else fails.
if (process.platform === "win32") {
  console.error("soiree certify n'est pas encore disponible sous Windows : le harnais dépend de lsof, de ps et des groupes de processus POSIX.");
  process.exit(1);
}

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Toxiproxy 2.13 performs an uncancellable net.Dial in its accept loop;
// stopping that proxy waits for the OS's connect timeout. Keep its fault
// engine, but let it dial an owned local acceptor. Only this connector dials
// the real upstream, with an explicit deadline and cancellation on teardown.
// No retries, address substitution or idle timeout: bytes/FIN still travel
// through the official toxics, with Node streams providing backpressure.
const CONNECT_TIMEOUT_MS = 2000;
const MAX_CONNECTIONS = 128;

class UpstreamConnector {
  static async start(owner, host, port) {
    const connector = new UpstreamConnector(owner, host, port);
    try {
      await new Promise((resolve, reject) => {
        connector.server.once("error", reject);
        connector.server.listen({ host: "127.0.0.1", port: 0, backlog: 512 }, () => {
          connector.server.removeListener("error", reject);
          resolve();
        });
      });
    } catch (error) {
      throw new FixtureFailure(`écoute du connecteur Toxiproxy : ${error.message}`);
    }
    connector.address = `127.0.0.1:${connector.server.address().port}`;
    return connector;
  }

  constructor(owner, host, port) {
    this.host = host;
    this.port = port;
    this.sockets = new Set();
    this.closed = null;
    this.server = net.createServer({ allowHalfOpen: true }, client => {
      const upstream = new net.Socket({ allowHalfOpen: true });
      const destroy = () => { client.destroy(); upstream.destroy(); };
      const deadline = setTimeout(destroy, CONNECT_TIMEOUT_MS);
      for (const socket of [client, upstream]) {
        this.sockets.add(socket);
        socket.on("error", destroy);
        socket.once("close", () => {
          clearTimeout(deadline);
          this.sockets.delete(socket);
          destroy();
        });
      }
      client.pause();
      upstream.once("connect", () => {
        clearTimeout(deadline);
        client.pipe(upstream);
        upstream.pipe(client);
      });
      upstream.connect({ host, port });
    });
    this.server.maxConnections = MAX_CONNECTIONS;
    this.server.on("error", error => {
      owner.failure = new FixtureFailure(`connecteur Toxiproxy ${host}:${port} : ${error.message}`);
    });
  }

  close() {
    this.closed ??= (async () => {
      const socketClosures = [...this.sockets].map(socket => new Promise(resolve => socket.once("close", resolve)));
      const serverClosed = new Promise((resolve, reject) => this.server.close(error => error ? reject(error) : resolve()));
      for (const socket of this.sockets) socket.destroy();
      await Promise.all([serverClosed, ...socketClosures]);
    })();
    return this.closed;
  }
}

async function freePort() {
  return new Promise((resolve, reject) => {
    const server = net.createServer();
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const { port } = server.address();
      server.close((error) => error ? reject(error) : resolve(port));
    });
  });
}

export class Toxiproxy {
  static async start() {
    if (spawnSync("toxiproxy-server", ["--version"], { stdio: "ignore" }).error?.code === "ENOENT") {
      throw new FixtureFailure("toxiproxy-server introuvable : brew install toxiproxy");
    }
    let toxiproxy;
    try {
      const port = await freePort();
      const server = spawn("toxiproxy-server", ["-host", "127.0.0.1", "-port", String(port)], { stdio: "ignore" });
      toxiproxy = new Toxiproxy(`http://127.0.0.1:${port}`, server);
      const deadline = Date.now() + 5000;
      let lastError;
      while (Date.now() < deadline) {
        try {
          const version = await toxiproxy.api("GET", "/version", undefined, Math.min(500, Math.max(1, deadline - Date.now())));
          if (typeof version.version !== "string" || !version.version) throw new FixtureFailure("Toxiproxy /version : version absente");
          return toxiproxy;
        } catch (error) {
          lastError = error;
          if (toxiproxy.failure) throw toxiproxy.failure;
          await sleep(100);
        }
      }
      throw new FixtureFailure(`toxiproxy-server ne répond pas après 5 s : ${lastError?.message ?? "aucune réponse"}`);
    } catch (error) {
      await toxiproxy?.stop();
      throw error instanceof FixtureFailure ? error : new FixtureFailure(`démarrage Toxiproxy : ${error.message}`);
    }
  }

  constructor(base, server, { timeoutMs = 5000 } = {}) {
    if (!Number.isInteger(timeoutMs) || timeoutMs <= 0) throw new FixtureFailure("délai Toxiproxy invalide");
    this.base = base;
    this.server = server;
    this.timeoutMs = timeoutMs;
    this.count = 0;
    this.failure = null;
    this.stopping = false;
    this.connectors = new Set();
    server?.once("error", error => { this.failure = new FixtureFailure(`toxiproxy-server : ${error.message}`); });
    server?.once("exit", (code, signal) => {
      if (!this.stopping) this.failure = new FixtureFailure(`toxiproxy-server arrêté (${signal ?? code})`);
    });
  }

  async api(method, path, body, timeoutMs = this.timeoutMs) {
    if (this.failure) throw this.failure;
    try {
      const response = await fetch(`${this.base}${path}`, {
        method,
        headers: body ? { "content-type": "application/json" } : {},
        body: body ? JSON.stringify(body) : undefined,
        signal: AbortSignal.timeout(timeoutMs),
      });
      if (this.failure) throw this.failure;
      // Only an idempotent deletion may legitimately find nothing to delete.
      if (method === "DELETE" && (response.status === 404 || response.status === 204)) {
        await response.body?.cancel();
        return null;
      }
      const text = await response.text();
      if (this.failure) throw this.failure;
      if (!response.ok) throw new FixtureFailure(`Toxiproxy ${method} ${path} : HTTP ${response.status} ${text.slice(0, 300)}`);
      let result;
      try { result = JSON.parse(text); }
      catch { throw new FixtureFailure(`Toxiproxy ${method} ${path} : réponse JSON invalide (${text.slice(0, 120)})`); }
      if (!result || typeof result !== "object" || Array.isArray(result)) throw new FixtureFailure(`Toxiproxy ${method} ${path} : objet JSON attendu`);
      return result;
    } catch (error) {
      throw error instanceof FixtureFailure ? error : new FixtureFailure(`Toxiproxy ${method} ${path} : ${error.message}`);
    }
  }

  /** A new proxy toward `host:port`, listening on a free local port. */
  async proxy(host, port) {
    if (typeof host !== "string" || !host || !Number.isInteger(port) || port < 1 || port > 65535) throw new FixtureFailure("destination Toxiproxy invalide");
    const name = `cert-${++this.count}`;
    const connector = await UpstreamConnector.start(this, host, port);
    this.connectors.add(connector);
    try {
      const created = await this.api("POST", "/proxies", { name, listen: "127.0.0.1:0", upstream: connector.address, enabled: true });
      const listen = created.listen;
      const actualPort = typeof listen === "string" ? Number(listen.split(":").pop()) : NaN;
      if (created.name !== name || created.upstream !== connector.address || created.enabled !== true || !Number.isInteger(actualPort) || actualPort < 1 || actualPort > 65535) {
        throw new FixtureFailure("Toxiproxy POST /proxies : création non confirmée par le contrôleur");
      }
      return new Proxy(this, name, actualPort, connector);
    } catch (error) {
      await connector.close();
      this.connectors.delete(connector);
      throw error;
    }
  }

  async stop() {
    this.stopping = true;
    this.server?.kill();
    await Promise.all([...this.connectors].map(connector => connector.close()));
    this.connectors.clear();
  }
}

class Proxy {
  constructor(toxiproxy, name, port, connector) {
    this.toxiproxy = toxiproxy;
    this.name = name;
    this.port = port;
    this.toxics = new Set();
    this.connector = connector;
  }

  async set(enabled) {
    const result = await this.toxiproxy.api("POST", `/proxies/${this.name}`, { enabled });
    if (result.name !== this.name || result.enabled !== enabled) throw new FixtureFailure(`Toxiproxy ${this.name} : changement d’état non confirmé`);
    return result;
  }

  async toxic(name, type, stream, attributes) {
    const result = await this.toxiproxy.api("POST", `/proxies/${this.name}/toxics`, { name, type, stream, attributes });
    if (result.name !== name || result.type !== type || result.stream !== stream) throw new FixtureFailure(`Toxiproxy ${this.name} : panne ${name} non confirmée`);
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

  async close() {
    try {
      return await this.toxiproxy.api("DELETE", `/proxies/${this.name}`);
    } finally {
      await this.connector.close();
      this.toxiproxy.connectors.delete(this.connector);
    }
  }
}
