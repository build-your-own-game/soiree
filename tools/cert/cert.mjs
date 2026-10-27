#!/usr/bin/env node
// Stage 2 of CERTIFICATION.md: the black-box certification of one game.
//
//   node tools/cert/cert.mjs games/quiplash                 everything (~5 min)
//   node tools/cert/cert.mjs games/quiplash --only cut,freeze
//   node tools/cert/cert.mjs games/quiplash --soak 30       30 min of chaos
//   node tools/cert/cert.mjs games/quiplash --phones 4 --seed 12   replay a run
//
// It starts the game from its cert.json, opens the TV and the phones in a
// headless Chrome driven by Playwright (browser.mjs), each phone behind its
// own Toxiproxy proxy (toxiproxy.mjs), and runs the scenarios. It only knows the game through cert.json: the same harness
// certifies every game, as web-platform-tests does every browser.
//
// Every game talks through salon (salon/README.md). Toxiproxy only carries
// TCP, and the scenarios count and cut WebSockets: the TV and the phones get
// `?salon=ws`, which keeps salon on its WebSocket. Only the `rtc` scenario
// lets one phone try WebRTC, and reads the host's `SALON_LOG` to see which
// transport carried it.
//
// Oracles that hold in every scenario: no uncaught exception in any page, no
// panic in the host, the host still running, the TV still drawing.

import { spawn, execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import net from "node:net";
import { networkInterfaces, tmpdir } from "node:os";
import { join, resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

import { launchBrowser, sleep } from "./browser.mjs";
import { Toxiproxy } from "./toxiproxy.mjs";

const HERE = dirname(fileURLToPath(import.meta.url));
const NASTY = JSON.parse(readFileSync(join(HERE, "nasty.json"), "utf8")).texts;
NASTY.push("é".repeat(500));

const ALL = ["join", "rtc", "layout", "cut", "freeze", "silent", "reload", "twin", "hostile", "jitter", "monkey", "restart", "storage"];
// Scenarios that run in the lobby, before the TV presses `start`: a new
// player may still join there.
const LOBBY = ["join", "rtc"];
let order = ALL;

// ---------------------------------------------------------------- arguments

function parseArgs(argv) {
  const args = { dir: null, only: ALL, phones: 8, soak: 0, seed: Date.now() % 100000 };
  for (let i = 0; i < argv.length; i++) {
    const a = argv[i];
    if (a === "--only") args.only = argv[++i].split(",");
    else if (a === "--phones") args.phones = Number(argv[++i]);
    else if (a === "--soak") args.soak = Number(argv[++i]);
    else if (a === "--seed") args.seed = Number(argv[++i]);
    else if (a === "-h" || a === "--help") args.help = true;
    else args.dir = a;
  }
  return args;
}

const args = parseArgs(process.argv.slice(2));
if (args.help || !args.dir) {
  console.log(readFileSync(fileURLToPath(import.meta.url), "utf8").split("\n").slice(1, 9).map((l) => l.slice(3)).join("\n"));
  process.exit(args.help ? 0 : 2);
}
const gameDir = resolve(args.dir);
const config = JSON.parse(readFileSync(join(gameDir, "cert.json"), "utf8"));
const outDir = join(tmpdir(), `cert-${gameDir.split("/").pop()}-${new Date().toISOString().replace(/[:.]/g, "-")}`);
mkdirSync(outDir, { recursive: true });

// A seeded generator, so a run that broke something can be replayed (--seed).
let rngState = args.seed || 1;
function random() {
  rngState ^= rngState << 13;
  rngState ^= rngState >>> 17;
  rngState ^= rngState << 5;
  return ((rngState >>> 0) % 1_000_000) / 1_000_000;
}
const pick = (list) => list[Math.floor(random() * list.length)];

/** `path` with `salon=ws` in its query: salon then stays on its WebSocket,
 * which Toxiproxy carries and the scenarios can count and cut. */
function withSalonWs(path) {
  const [base, hash] = path.split("#");
  if (/[?&]salon=/.test(base)) return path;
  return `${base}${base.includes("?") ? "&" : "?"}salon=ws${hash === undefined ? "" : `#${hash}`}`;
}

// Salon's packets (salon/crates/core/src/wire.rs), read only as far as the
// hostile scenario needs: postcard, with varints for integers, a 0/1 byte for
// an Option, a varint for an enum's variant, 16 raw bytes for a token.
const CONTROLS = ["Hello", "Welcome", "Refused", "Replaced", "Bye", "Closing"];
const KINDS = ["Tv", "Phone", "App", "Gamepad", "Keyboard"];
const REFUSALS = ["Version", "Full", "Closed", "NotAllowed", "Removed"];

function readPacket(data) {
  const b = Buffer.from(data);
  let at = 0;
  const byte = () => {
    if (at >= b.length) throw new Error("trop court");
    return b[at++];
  };
  const varint = () => {
    let value = 0;
    for (let i = 0, scale = 1; i < 10; i++, scale *= 128) {
      const x = byte();
      value += (x & 0x7f) * scale;
      if (!(x & 0x80)) return value;
    }
    throw new Error("varint");
  };
  const option = () => {
    const tag = byte();
    if (tag > 1) throw new Error("option");
    return tag === 1;
  };
  const packet = { size: b.length };
  try {
    packet.epoch = varint();
    varint(); // sent
    if (option()) (varint(), varint()); // echo
    if (option()) {
      const variant = varint();
      packet.control = CONTROLS[variant] ?? `?${variant}`;
      if (packet.control === "Hello") {
        varint(); // protocol
        varint(); // game
        packet.kind = KINDS[varint()];
        packet.tokenAt = at;
        if (at + 16 > b.length) throw new Error("jeton coupé");
      } else if (packet.control === "Welcome") packet.device = varint();
      else if (packet.control === "Refused") packet.refusal = REFUSALS[varint()] ?? "?";
    }
  } catch {
    packet.unreadable = true;
  }
  return packet;
}

/** A packet in a few words, for the report. */
function describe(packet) {
  if (packet.unreadable) return `${packet.size} octets illisibles`;
  if (packet.control === "Welcome") return `Welcome (appareil ${packet.device})`;
  if (packet.control === "Refused") return `Refused(${packet.refusal})`;
  if (packet.control) return packet.control;
  return `paquet de ${packet.size} octets`;
}

/** A frame that shows the host talking to this page: its Welcome, or any
 * readable packet that is not a control (state, events, heartbeats, which
 * salon sends only to a welcomed device). A Refused, a Replaced or a Bye is
 * an answer, not a connection: a TV refused after a restart is not back. */
function isLivePacket(payload) {
  const packet = readPacket(payload);
  return !packet.unreadable && (!packet.control || packet.control === "Welcome");
}

/** The first Hello in a list of captured frames. */
function firstHello(frames, kind) {
  for (const frame of frames) {
    const packet = readPacket(frame.payload);
    if (packet.control === "Hello" && (!kind || packet.kind === kind)) return { bytes: Buffer.from(frame.payload), packet };
  }
  return null;
}

/** `hello` with another, invented token: a device nobody knows. */
function forgeToken(hello) {
  const bytes = Buffer.from(hello.bytes);
  for (let i = 0; i < 16; i++) bytes[hello.packet.tokenAt + i] = Math.floor(random() * 256);
  return bytes;
}

/** The device number the host gave this page, from its first Welcome. */
function deviceOf(tab) {
  for (const frame of tab.wsReceived) {
    const packet = readPacket(frame.payload);
    if (packet.control === "Welcome") return packet.device;
  }
  return null;
}

// ---------------------------------------------------------------- the report

/** item → { status: "ok" | "fail" | "partial" | "skip", notes: [{ status, text, count }] }:
 * every note an item received, each with its own outcome, the same note
 * once with how many times it came (a soak repeats them). */
const results = new Map();
const RANK = { skip: 0, ok: 1, partial: 2, fail: 3 };
const MARK = { ok: "✓", fail: "✗", partial: "~", skip: "–" };
function record(item, status, note) {
  // The worst outcome wins; "not applicable" only stands alone.
  const entry = results.get(item) ?? { status, notes: [] };
  if (RANK[status] > RANK[entry.status]) entry.status = status;
  if (note) {
    const same = entry.notes.find((n) => n.status === status && n.text === note);
    if (same) same.count++;
    else entry.notes.push({ status, text: note, count: 1 });
  }
  results.set(item, entry);
  if (note) log(`    ${MARK[status]} ${item} ${note}`);
}
const ok = (item, note) => record(item, "ok", note);
const fail = (item, note) => record(item, "fail", note);
const partial = (item, note) => record(item, "partial", note);
const skip = (item, note) => record(item, "skip", note);

const started = Date.now();
function log(line) {
  const t = ((Date.now() - started) / 1000).toFixed(0).padStart(4);
  console.log(`${t}s ${line}`);
}

// ---------------------------------------------------------------- the host

function lanAddress() {
  for (const list of Object.values(networkInterfaces())) {
    for (const a of list ?? []) if (a.family === "IPv4" && !a.internal) return a.address;
  }
  return null;
}

async function freePort() {
  return new Promise((resolve) => {
    const server = net.createServer();
    server.listen(0, () => {
      const { port } = server.address();
      server.close(() => resolve(port));
    });
  });
}

class Host {
  constructor(port) {
    this.port = port;
    this.panics = [];
    this.stderr = [];
    this.process = null;
    this.exitedUnexpectedly = false;
    /** Values read from the host's output (`capture` in cert.json), such as a room code. */
    this.captured = {};
    /** Salon's own lines (`SALON_LOG=1`): each link opened or closed, with its transport. */
    this.links = [];
  }

  /** A page path with the captured values filled in. */
  path(template) {
    return template.replace(/\{(\w+)\}/g, (all, name) => this.captured[name] ?? all);
  }

  async start(command, timeoutMs) {
    const fill = (s) => s.replaceAll("{port}", String(this.port)).replaceAll("{tmp}", join(outDir, "data"));
    // Salon prints each link with its transport: the `rtc` scenario reads it.
    const env = { ...process.env, SALON_LOG: "1" };
    for (const [k, v] of Object.entries(config.env ?? {})) env[k] = fill(v);
    const [program, ...rest] = command.map(fill);
    this.expectExit = false;
    // Its own process group: `cargo run` and the game it starts die together.
    this.process = spawn(program, rest, { cwd: gameDir, env, stdio: ["ignore", "pipe", "pipe"], detached: true });
    let ready = false;
    const onLine = (line, err) => {
      if (/panicked at|thread '.*' panicked/.test(line)) this.panics.push(line.trim());
      if (/^salon: link \d+ (opened|closed)/.test(line.trim())) {
        this.links.push({ at: Date.now(), line: line.trim() });
        return;
      }
      // Cargo's own progress lines are not the game's.
      if (err && line.trim() && !/^\s*(Compiling|Finished|Running|Blocking|Updating|Downloading|Downloaded|Locking|Adding)\b/.test(line)) {
        this.stderr.push(line.trim());
      }
      if (line.includes(config.ready)) ready = true;
      for (const [name, pattern] of Object.entries(config.capture ?? {})) {
        const found = line.match(new RegExp(pattern));
        if (found) this.captured[name] = found[1];
      }
    };
    for (const [stream, err] of [[this.process.stdout, false], [this.process.stderr, true]]) {
      let rest = "";
      stream.on("data", (chunk) => {
        rest += chunk.toString();
        const lines = rest.split("\n");
        rest = lines.pop();
        for (const line of lines) onLine(line, err);
      });
    }
    this.process.on("exit", (code) => {
      if (!this.expectExit) {
        this.exitedUnexpectedly = true;
        this.exitCode = code;
      }
    });
    const start = Date.now();
    while (!ready) {
      if (this.exitedUnexpectedly) throw new Error(`le jeu s'est arrêté au lancement (code ${this.exitCode}) :\n${this.stderr.slice(-15).join("\n")}`);
      if (Date.now() - start > timeoutMs) throw new Error(`« ${config.ready} » n'est pas apparu en ${timeoutMs / 1000} s`);
      await sleep(100);
    }
    // Some games announce themselves before they listen.
    for (let tries = 0; tries < 50 && !(this.pid = this.listeningPid()); tries++) await sleep(100);
  }

  /** The process that listens on the port: `cargo run` is only its parent. */
  listeningPid() {
    try {
      return Number(execFileSync("lsof", ["-t", `-iTCP:${this.port}`, "-sTCP:LISTEN"]).toString().trim().split("\n")[0]);
    } catch {
      return null;
    }
  }

  alive() {
    if (!this.pid) return !this.exitedUnexpectedly;
    try {
      process.kill(this.pid, 0);
      return true;
    } catch {
      return false;
    }
  }

  /** The host's open TCP connections on its port, by peer address: one goes
   * when the host gives up on a phone. */
  connections() {
    try {
      const out = execFileSync("lsof", ["-nP", "-a", "-p", String(this.pid), "-iTCP", "-sTCP:ESTABLISHED"]).toString();
      return new Set(out.split("\n").flatMap((line) => line.match(new RegExp(`:${this.port}->(\\S+)`))?.slice(1) ?? []));
    } catch {
      return null;
    }
  }

  /** Whether a connection open in `before` is gone now. */
  gaveUp(before) {
    const now = this.connections();
    return before !== null && now !== null && [...before].some((peer) => !now.has(peer));
  }

  /** Resident memory in KiB and thread count, for the soak test. */
  usage() {
    try {
      const rss = Number(execFileSync("ps", ["-o", "rss=", "-p", String(this.pid)]).toString().trim());
      const threads = execFileSync("ps", ["-M", "-p", String(this.pid)]).toString().trim().split("\n").length - 1;
      return { rss, threads };
    } catch {
      return null;
    }
  }

  async kill() {
    this.expectExit = true;
    const exited = new Promise((resolve) => this.process.once("exit", resolve));
    for (const target of [-this.process.pid, this.pid ?? this.listeningPid()]) {
      try {
        if (target) process.kill(target, "SIGKILL");
      } catch {}
    }
    await Promise.race([exited, sleep(5_000)]);
  }
}

// ---------------------------------------------------------------- the room

const lan = lanAddress();

/** When the computer's network address is no longer the one the phones and
 * `hostile` were given (Wi-Fi roaming, a new DHCP lease), a note that says so:
 * what failed on the network then says nothing about the game. */
function networkMoved() {
  const now = lanAddress();
  return lan && now !== lan ? ` (l'adresse réseau de l'ordinateur a changé pendant le passage, ${lan} → ${now ?? "aucune"} : les téléphones et ce scénario passent par l'ancienne, l'échec peut venir de là)` : "";
}
let host;
let browser;
let toxiproxy;
let tv;
const phones = []; // { index, name, tab, proxy, sent: [], context }
/** The TV pressed `start` (cert.json): a new player may be refused from then on. */
let gameStarted = false;

async function openTv() {
  // The TV is opened directly, at the host's own address: a game may keep
  // organizer rights for its own page (Sidequest checks the origin).
  const tab = await browser.open(`http://localhost:${host.port}${withSalonWs(host.path(config.tv))}`, { width: 1920, height: 1080 });
  tab.label = "TV";
  tv = { tab, sent: tab.wsSent, received: tab.wsReceived };
}

/** The host sent this screen a Welcome or a packet of the game within the
 * last `withinMs` (`isLivePacket`): a refusal alone does not count. */
function isLive(screen, withinMs) {
  return Date.now() - screen.tab.lastLiveFrame < withinMs;
}

/** What this screen last received, for the report. */
function lastHeard(screen) {
  const frame = screen.tab.lastReceived;
  if (!frame) return "rien reçu";
  return `dernier paquet : ${describe(readPacket(frame.payload))}, il y a ${((Date.now() - screen.tab.lastWsFrame) / 1000).toFixed(1)} s`;
}

/** The phone's address, with the host's current room code; on salon's
 * WebSocket unless the phone is there to try WebRTC (`rtc`). */
function phoneUrl(phone) {
  const path = host.path(config.phone);
  return `http://127.0.0.1:${phone.proxy.port}${phone.webrtc ? path : withSalonWs(path)}`;
}

async function openPhone(index, { webrtc = false } = {}) {
  // A phone reaches the game from the network, as a real phone would.
  const proxy = await toxiproxy.proxy(config.phone_upstream === "loopback" || !lan ? "127.0.0.1" : lan, host.port);
  const phone = { index, name: null, proxy, webrtc };
  phone.url = phoneUrl(phone);
  phone.tab = await browser.open(phone.url, { width: 390, height: 844, mobile: true });
  phone.tab.label = `téléphone ${index}`;
  return phone;
}

async function joined(phone) {
  try {
    return Boolean(await phone.tab.evaluate(config.joined, 3_000));
  } catch {
    return false;
  }
}

/** Whether the page offers a way in: the element of the first join step, or
 *  no step at all (the game seats whoever connects). */
async function joinOffered(phone) {
  const step = (config.join ?? []).find((s) => s.fill || s.click || s.wait);
  const selector = step?.fill ?? step?.click ?? step?.wait;
  if (!selector) return true;
  return phone.tab.evaluate(`document.querySelector(${JSON.stringify(selector)}) !== null`).catch(() => false);
}

async function runJoin(phone, name) {
  phone.name = name;
  for (const step of config.join ?? []) {
    if (step.wait) await phone.tab.waitFor(`document.querySelector(${JSON.stringify(step.wait)})`, 10_000);
    if (step.fill) {
      try {
        await phone.tab.fill(step.fill, step.text.replaceAll("{name}", name));
      } catch {
        return false;
      }
    }
    if (step.click) {
      const box = await center(phone.tab, step.click);
      if (!box) return false;
      await phone.tab.click(box.x, box.y);
    }
    if (step.press) await phone.tab.key(step.press);
    await sleep(step.pause ?? 150);
  }
  return true;
}

async function center(tab, selector) {
  return tab.evaluate(`(() => {
    const e = document.querySelector(${JSON.stringify(selector)});
    if (!e) return null;
    const r = e.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  })()`);
}

/** Every page's new errors since the last call, attributed to `scenario`. */
const seenErrors = new Map();
function newErrors(scenario) {
  const found = [];
  for (const tab of [tv?.tab, ...phones.map((p) => p.tab), ...extraTabs].filter(Boolean)) {
    const seen = seenErrors.get(tab) ?? 0;
    for (const error of tab.errors.slice(seen)) found.push(`${tab.label}: ${error}`);
    seenErrors.set(tab, tab.errors.length);
  }
  return found;
}
const extraTabs = [];

let tvFrames = 0;
async function tvDrawing() {
  try {
    const frames = await tv.tab.evaluate("window.__certFrames", 3_000);
    // A reload starts the count again: any change means the page draws.
    const moving = frames !== tvFrames;
    tvFrames = frames;
    return moving;
  } catch {
    return false;
  }
}

/** The oracles every scenario must keep. */
async function invariants(scenario) {
  for (const error of newErrors(scenario)) {
    const item = /panicked|RuntimeError|unreachable/.test(error) ? "E1" : "E6";
    if (/^.*: console\./.test(error) && !/panicked/.test(error)) partial(item, `[${scenario}] ${error}`);
    else fail(item, `[${scenario}] ${error}`);
  }
  while (host.panics.length) fail("E1", `[${scenario}] l'hôte a paniqué : ${host.panics.shift()}`);
  if (!host.alive()) {
    fail("E1", `[${scenario}] l'hôte ne tourne plus`);
    throw new Error("hôte arrêté");
  }
  if (!(await tvDrawing())) fail("E6", `[${scenario}] la TV ne dessine plus`);
}

/** Waits for a Welcome or a packet of the game that arrives after `since`
 * (by default, now); the time it took from the call, or null. */
async function waitLive(screen, timeoutMs, since = Date.now()) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (screen.tab.lastLiveFrame >= since) return Date.now() - start;
    await sleep(100);
  }
  return null;
}

async function waitJoined(phone, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (await joined(phone)) return Date.now() - start;
    await sleep(200);
  }
  return null;
}

/** Puts a phone back in the game after a scenario that broke it. */
async function restore(phone) {
  await phone.proxy.reset();
  phone.url = phoneUrl(phone);
  await phone.tab.goto(phone.url);
  if ((await waitJoined(phone, 5_000)) === null) {
    await runJoin(phone, phone.name ?? `Joueur${phone.index}`);
    await waitJoined(phone, 10_000);
  }
}

/** Seats every phone that lost its place; the phones still without one
 * 10 s later, for the caller to report. */
async function rejoinAll() {
  for (const phone of phones) {
    if (!(await joined(phone))) {
      // The host may have a new room code since the phone first came.
      phone.url = phoneUrl(phone);
      await phone.tab.goto(phone.url);
      await runJoin(phone, phone.name ?? `Joueur${phone.index}`);
    }
  }
  const missing = [];
  for (const phone of phones) if ((await waitJoined(phone, 10_000)) === null) missing.push(phone);
  log(`  ${phones.length - missing.length}/${phones.length} téléphones à leur place`);
  return missing;
}

/** What a page shows, on one line. */
async function pageText(tab) {
  return (await tab.evaluate("document.body.innerText").catch(() => "")).replace(/\s+/g, " ").trim();
}

/** The phones `rejoinAll` could not seat, as a note. */
async function describeMissing(missing) {
  return `${missing.map((p) => p.tab.label).join(", ")} sans place à la fin du scénario ; ${missing[0].tab.label} lit « ${(await pageText(missing[0].tab)).slice(0, 100)} »`;
}

/** The token this phone's salon uses now, from the last Hello it sent, in
 * hex as salon-web stores it (`salon/crates/web/src/lib.rs`, `token`). */
function tokenInUse(tab) {
  const frame = tab.remembered;
  if (!frame) return null;
  const packet = readPacket(frame.payload);
  if (packet.control !== "Hello" || packet.unreadable) return null;
  return Buffer.from(frame.payload).subarray(packet.tokenAt, packet.tokenAt + 16).toString("hex");
}

// ---------------------------------------------------------------- the monkey

const TARGETS = `(() => {
  const out = [];
  for (const e of document.querySelectorAll("input, textarea, button, select, a[href], [role=button], [onclick], [tabindex], canvas, label, li, [class*=btn], [class*=button], [class*=card], [class*=choice]")) {
    const r = e.getBoundingClientRect();
    if (r.width < 4 || r.height < 4 || r.bottom < 0 || r.right < 0 || r.top > innerHeight || r.left > innerWidth) continue;
    const x = Math.min(innerWidth - 1, Math.max(0, r.x + r.width / 2)), y = Math.min(innerHeight - 1, Math.max(0, r.y + r.height / 2));
    const top = document.elementFromPoint(x, y);
    if (!top || !(e === top || e.contains(top) || top.contains(e))) continue;
    const typing = e.matches("input:not([type=button]):not([type=submit]):not([type=checkbox]):not([type=radio]), textarea, [contenteditable]");
    out.push({ x, y, typing, w: r.width, h: r.height });
  }
  return out;
})()`;

/** One random gesture on a phone, like a player who has not read anything. */
async function gesture(phone) {
  const tab = phone.tab;
  let targets = [];
  try {
    targets = await tab.evaluate(TARGETS, 3_000);
  } catch {}
  const roll = random();
  if (targets.length > 1 && roll < 0.15) {
    // A drag from one thing to another, with the mouse (drag and drop, sliders).
    await tab.drag(pick(targets), pick(targets));
  } else if (roll < 0.2) {
    await tab.key(pick(["Enter", "Escape", "Backspace", "Tab", "ArrowLeft", "ArrowRight", " "]));
  } else if (targets.length && roll < 0.75) {
    const t = pick(targets);
    if (random() < 0.5) await tab.tap(t.x, t.y);
    else await tab.click(t.x, t.y);
    if (t.typing) {
      await tab.type(random() < 0.6 ? pick(NASTY) : `réponse ${Math.floor(random() * 1000)}`);
      if (random() < 0.5) await tab.key("Enter");
    }
  } else if (roll < 0.9) {
    // A swipe, for sticks, canvases and pads.
    const { w, h } = await tab.evaluate("({ w: innerWidth, h: innerHeight })", 3_000).catch(() => ({ w: 390, h: 844 }));
    const x = random() * w, y = random() * h;
    const points = Array.from({ length: 6 }, (_, i) => ({ x: x + i * (random() * 30 - 15), y: y + i * (random() * 30 - 15) }));
    if (tab.mobile) await tab.swipe(points);
    else await tab.drag(points[0], points[5]);
  } else {
    await tab.tap(random() * 390, random() * 600);
  }
}

/** Runs gestures on every phone (except those in `busy`) until `stop()`. */
function monkey(busy = new Set()) {
  let running = true;
  // A game without phones is played on its only page.
  const players = phones.length ? phones : [tv];
  const loops = players.map(async (phone) => {
    while (running) {
      if (!busy.has(phone)) {
        try {
          await gesture(phone);
        } catch {}
      }
      await sleep(150 + random() * 600);
    }
  });
  return async () => {
    running = false;
    await Promise.all(loops);
  };
}

/** Presses the TV's advance key every `everyMs`, to go through the game fast. */
function advancer(everyMs) {
  let running = true;
  const loop = (async () => {
    while (running && config.advance) {
      await sleep(everyMs);
      if (running) await press(tv.tab, config.advance).catch(() => {});
    }
  })();
  return async () => {
    running = false;
    await loop;
  };
}

/** `{ key }` presses a key; `{ text }` clicks the button that says it. */
async function press(tab, action) {
  if (action.key) return tab.key(action.key);
  const at = await tab.evaluate(`(() => {
    const e = [...document.querySelectorAll("button, a, [role=button]")].find((e) => e.innerText.trim() === ${JSON.stringify(action.text)});
    if (!e) return null;
    const r = e.getBoundingClientRect();
    return { x: r.x + r.width / 2, y: r.y + r.height / 2 };
  })()`);
  if (at) await tab.click(at.x, at.y);
}

async function tvHash() {
  return createHash("sha1").update(await tv.tab.screenshot()).digest("hex");
}

// ---------------------------------------------------------------- scenarios

/** A browser that refuses storage (private mode, blocked cookies). */
const BLOCK_STORAGE = `for (const name of ["localStorage", "sessionStorage"]) Object.defineProperty(window, name, { get() { throw new DOMException("refusé", "SecurityError"); } });`;

/** C9 for a game without phones: its page with damaged or refused storage. */
async function storageOfPage(page) {
  const keys = await page.tab.evaluate("Object.keys(localStorage)").catch(() => []);
  for (const garbage of [NASTY[2] + "\u0000{[\"", "{}", "[]", "null", JSON.stringify({ version: 1e308, rows: "x" })]) {
    await page.tab.evaluate(`for (const k of Object.keys(localStorage)) localStorage.setItem(k, ${JSON.stringify(garbage)});`).catch(() => {});
    await page.tab.reload();
    await sleep(800);
    const text = (await page.tab.evaluate("document.body.innerText").catch(() => "")).trim();
    if (!text) fail("C9", `stockage remplacé par ${JSON.stringify(garbage).slice(0, 30)} : la page est vide`);
  }
  ok("C9", `${keys.length} clé(s) remplacée(s) par 5 contenus abîmés, page rechargée à chaque fois`);
  const allowStorage = await page.tab.beforeScripts(BLOCK_STORAGE);
  await page.tab.reload();
  await sleep(800);
  const text = (await page.tab.evaluate("document.body.innerText").catch(() => "")).trim();
  if (!text) fail("C9", "stockage refusé : la page est vide");
  else ok("C9", "stockage refusé : la page s'ouvre");
  await allowStorage();
  await page.tab.reload();
  await invariants("storage");
}

const scenarios = {
  /** C1, C8, R3: everyone joins, the extra player is refused clearly. */
  async join() {
    if (!config.phone) return skip("C1", "pas de téléphones");
    // The first players have the names that break things (T2).
    const names = [NASTY[0], NASTY[2], NASTY[3], NASTY[1], NASTY[5], NASTY[8]];
    for (let i = 0; i < args.phones; i++) {
      const phone = await openPhone(i + 1);
      phones.push(phone);
      const name = names[i] ?? `Joueur${i + 1}`;
      const start = Date.now();
      await runJoin(phone, name);
      let took = await waitJoined(phone, 10_000);
      if (took === null) {
        // A nasty name may be refused, as long as the player understands why
        // and a plain name then works.
        const text = (await phone.tab.evaluate("document.body.innerText").catch(() => "")).replace(/\s+/g, " ").trim();
        partial("T2", `téléphone ${i + 1} : le prénom ${JSON.stringify(name).slice(0, 40)} n'entre pas ; il lit « ${text.slice(0, 100)} » (clair ?)`);
        await runJoin(phone, `Joueur${i + 1}`);
        took = await waitJoined(phone, 20_000);
        if (took !== null) took = Date.now() - start;
      }
      if (took === null) fail("C1", `téléphone ${i + 1} n'a pas rejoint en 30 s`);
      else ok("C1", `téléphone ${i + 1} a rejoint en ${(took / 1000).toFixed(1)} s`);
    }
    const max = config.players?.max;
    if (max && args.phones >= max) {
      const extra = await openPhone(args.phones + 1);
      extraTabs.push(extra.tab);
      await runJoin(extra, "EnTrop");
      await sleep(2_000);
      if (await joined(extra)) fail("R3", `un ${max + 1}ᵉ joueur a été accepté (maximum ${max})`);
      else {
        const text = (await extra.tab.evaluate("document.body.innerText").catch(() => "")).replace(/\s+/g, " ").trim();
        partial("C8", `le ${max + 1}ᵉ joueur est refusé ; ce qu'il lit : « ${text.slice(0, 120)} » (clair ?)`);
        ok("R3", `salle pleine à ${max}`);
      }
      await extra.proxy.close();
      await extra.tab.close();
    } else if (!max) skip("R3", "cert.json ne déclare pas de maximum (`players.max`) : la salle pleine n'est pas essayée");
    else skip("R3", `--phones ${args.phones} < maximum ${max} : la salle pleine n'est pas essayée`);
    await invariants("join");
  },

  /** T2, T3, T4: no text leaves its frame or the TV's safe area. */
  async layout() {
    if (config.layout === false) return skip("T3", config.layout_reason ?? "désactivé dans cert.json");
    const SIZES = [[1920, 1080], [1280, 720], [3840, 2160], [1280, 1024], [2560, 1080], [900, 1200]];
    const check = `(() => {
      const problems = [];
      const W = innerWidth, H = innerHeight, mx = W * 0.05, my = H * 0.05;
      const walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      for (let n = walker.nextNode(); n; n = walker.nextNode()) {
        if (!n.textContent.trim()) continue;
        const e = n.parentElement, s = getComputedStyle(e);
        if (s.visibility === "hidden" || s.display === "none" || Number(s.opacity) === 0) continue;
        const range = document.createRange(); range.selectNodeContents(n);
        for (const r of range.getClientRects()) {
          if (r.width === 0 || r.height === 0) continue;
          if (r.left < mx - 1 || r.top < my - 1 || r.right > W - mx + 1 || r.bottom > H - my + 1) {
            problems.push("hors zone sûre : « " + n.textContent.trim().slice(0, 40) + " » (" + Math.round(r.left) + "," + Math.round(r.top) + " → " + Math.round(r.right) + "," + Math.round(r.bottom) + ")");
            break;
          }
        }
        for (let p = e; p && p !== document.body; p = p.parentElement) {
          const ps = getComputedStyle(p);
          if (/(hidden|clip)/.test(ps.overflow + ps.overflowX + ps.overflowY) && (p.scrollWidth > p.clientWidth + 2 || p.scrollHeight > p.clientHeight + 2)) {
            problems.push("texte coupé : « " + n.textContent.trim().slice(0, 40) + " » dans <" + p.tagName.toLowerCase() + (p.className ? "." + String(p.className).split(" ")[0] : "") + ">");
            break;
          }
        }
      }
      return [...new Set(problems)].slice(0, 8);
    })()`;
    for (const [width, height] of SIZES) {
      await tv.tab.resize(width, height);
      await sleep(600);
      const problems = await tv.tab.evaluate(check);
      if (problems.length) {
        writeFileSync(join(outDir, `layout-tv-${width}x${height}.png`), await tv.tab.screenshot());
        for (const p of problems) fail(/coupé/.test(p) ? "T2" : "T3", `TV ${width}×${height} : ${p}`);
      } else ok("T3", `TV ${width}×${height} : tout le texte tient`);
    }
    await tv.tab.resize(1920, 1080);
    if (phones.length) {
      const small = await phones[0].tab.evaluate(`[...document.querySelectorAll("input, textarea, select")].filter((e) => parseFloat(getComputedStyle(e).fontSize) < 16).map((e) => e.outerHTML.slice(0, 60))`).catch(() => []);
      if (small.length) fail("T4", `champ en moins de 16 px (iOS zoomera) : ${small[0]}`);
      else ok("T4", "aucun champ visible sous 16 px");
    }
    await invariants("layout");
  },

  /** C9: corrupted or forbidden storage does not stop the page. Runs in a
   * lobby (after `restart`), where joining is allowed. */
  async storage() {
    if (!config.phone) return storageOfPage(tv);
    if (phones.length < 2) return skip("C9", "il faut au moins 2 téléphones");
    const [a, b] = phones;
    // A phone whose storage is refused or corrupted arrives as a new device.
    // If it had a seat, the game keeps that seat for a player who will never
    // come back (Punchline, for 90 s in the lobby): seating the new device
    // too would put N + 1 players in N seats, and the last phone would find
    // the room full. So a phone joins as a new device only when it had no
    // seat (after `restart`, a new game); one that had a seat gets its own
    // storage back afterwards, and its seat with it.

    // A browser that refuses storage (private mode, blocked cookies).
    const aSeated = await joined(a);
    const allowStorage = await a.tab.beforeScripts(BLOCK_STORAGE);
    a.url = phoneUrl(a);
    await a.tab.goto(a.url);
    await sleep(1_000);
    if (!(await pageText(a.tab))) fail("C9", "stockage refusé : la page est vide");
    else if (aSeated) {
      // A game where being connected is being in (the demo) seats it anyway.
      const inNow = (await waitJoined(a, 3_000)) !== null;
      const why = `(${a.tab.label} avait une place : le harnais ne refait pas les étapes pour rejoindre, l'ancienne place resterait prise ; puis son stockage revient, et sa place avec)`;
      if (inNow) ok("C9", `stockage refusé : la page s'ouvre et le joueur est dans la partie, en appareil neuf ${why}`);
      else if (await joinOffered(a)) ok("C9", `stockage refusé : la page s'ouvre et propose de rejoindre ${why}`);
      else partial("C9", `stockage refusé : la page s'ouvre mais ne propose pas de rejoindre ; elle dit « ${(await pageText(a.tab)).slice(0, 100)} » ${why}`);
    }
    else {
      if (!(await joined(a))) await runJoin(a, "SansCookie");
      if ((await waitJoined(a, 10_000)) === null) fail("C9", `sans stockage, on ne peut pas rejoindre ; la page dit « ${(await pageText(a.tab)).slice(0, 100)} »`);
      else ok("C9", "stockage refusé : on rejoint quand même, en appareil neuf");
    }
    await allowStorage();
    const aNew = !aSeated && (await joined(a));
    if (aNew) {
      // The storage comes back: the phone stays the device it joined as,
      // whose token only lived in the page (or a reload would make it a
      // third device, its seat a ghost).
      const token = tokenInUse(a.tab);
      if (token) {
        await a.tab.evaluate(`(() => {
          const keys = Object.keys(localStorage).filter((k) => /^salon\\.token\\.Phone(\\.|$)/.test(k));
          localStorage.setItem(keys.length === 1 ? keys[0] : "salon.token.Phone", ${JSON.stringify(token)});
        })()`).catch(() => {});
      }
    }
    // Back on its storage: a seated phone returns to its seat, the other
    // keeps the one it just took.
    if (aSeated || aNew) await a.tab.reload();
    if ((aSeated || aNew) && (await waitJoined(a, 10_000)) === null) {
      partial("C9", `stockage revenu, ${a.tab.label} rechargé ne retrouve pas sa place (le harnais n'a pas su lui rendre son identité) ; il lit « ${(await pageText(a.tab)).slice(0, 100)} »`);
    }

    // Garbage in every key the page saved, then a reload.
    const bSeated = await joined(b);
    const saved = await b.tab.evaluate("JSON.stringify(Object.entries(localStorage))").catch(() => "[]");
    const keys = JSON.parse(saved).map(([key]) => key);
    await b.tab.evaluate(`for (const k of Object.keys(localStorage)) localStorage.setItem(k, ${JSON.stringify(NASTY[2] + "\u0000{[\"")});`).catch(() => {});
    await b.tab.reload();
    await sleep(1_500);
    if (!(await pageText(b.tab))) fail("C9", "après corruption du stockage, la page est vide");
    else if (bSeated) {
      const inNow = (await waitJoined(b, 3_000)) !== null;
      const why = `(${b.tab.label} avait une place : le harnais ne refait pas les étapes pour rejoindre, l'ancienne place resterait prise ; puis son stockage est remis, et sa place avec)`;
      if (inNow) ok("C9", `${keys.length} clé(s) du stockage corrompue(s) : la page s'ouvre et le joueur est dans la partie, en appareil neuf ${why}`);
      else if (await joinOffered(b)) ok("C9", `${keys.length} clé(s) du stockage corrompue(s) : la page s'ouvre et propose de rejoindre ${why}`);
      else partial("C9", `${keys.length} clé(s) du stockage corrompue(s) : la page s'ouvre mais ne propose pas de rejoindre ; elle dit « ${(await pageText(b.tab)).slice(0, 100)} » ${why}`);
    } else {
      if (!(await joined(b))) await runJoin(b, "Corrompu");
      if ((await waitJoined(b, 10_000)) === null) fail("C9", `après corruption du stockage (${keys.length} clé(s)), la page s'ouvre mais on ne peut pas rejoindre ; elle dit « ${(await pageText(b.tab)).slice(0, 100)} »`);
      else ok("C9", `${keys.length} clé(s) du stockage corrompue(s) : la page s'ouvre et on rejoint en appareil neuf`);
    }
    if (bSeated) {
      await b.tab.evaluate(`localStorage.clear(); for (const [k, v] of ${saved}) localStorage.setItem(k, v);`).catch(() => {});
      await b.tab.reload();
      if ((await waitJoined(b, 10_000)) === null) partial("C9", `stockage remis, ${b.tab.label} ne retrouve pas sa place ; il lit « ${(await pageText(b.tab)).slice(0, 100)} »`);
    }
    await invariants("storage");
    const missing = await rejoinAll();
    if (missing.length) partial("C9", await describeMissing(missing));
  },

  /** C2: a 3 s Wi-Fi drop. */
  async cut() {
    if (!phones.length) return skip("C2", "pas de téléphones");
    const phone = phones[0];
    await phone.proxy.cut();
    await sleep(3_000);
    await phone.proxy.heal();
    const back = await waitLive(phone, 10_000);
    const seat = await waitJoined(phone, 3_000);
    if (back === null) fail("C2", "pas reconnecté 10 s après une coupure de 3 s");
    else if (seat === null) fail("C2", `reconnecté en ${(back / 1000).toFixed(1)} s, mais plus à sa place`);
    else ok("C2", `reconnecté à sa place ${(back / 1000).toFixed(1)} s après une coupure de 3 s`);
    await invariants("cut");
  },

  /** C3: the iPhone locks for 30 s: frozen page, swallowed connection. */
  async freeze() {
    if (!phones.length) return skip("C3", "pas de téléphones");
    const phone = phones[Math.min(1, phones.length - 1)];
    // Connections of earlier scenarios settle first: one closing now would
    // look like the host giving up.
    await sleep(3_000);
    const before = host.connections();
    await phone.proxy.blackhole();
    await phone.tab.freeze();
    const asleep = Date.now();
    // The host gives up when one of its connections goes.
    let noticed = null;
    const sockets = phone.tab.sockets;
    while (Date.now() - asleep < 30_000) {
      if (noticed === null && host.gaveUp(before)) noticed = Date.now() - asleep;
      await sleep(250);
    }
    if (phone.tab.sockets > sockets) fail("C3", "le téléphone « gelé » a ouvert une connexion : le gel du harnais ne tient pas");
    if (noticed === null) fail("C3", "en 30 s, l'hôte n'a pas remarqué le téléphone endormi (aucun délai de silence)");
    else if (noticed > 20_000) fail("C3", `l'hôte a remarqué le téléphone endormi en ${(noticed / 1000).toFixed(1)} s (> 20 s)`);
    else ok("C3", `l'hôte a remarqué le téléphone endormi en ${(noticed / 1000).toFixed(1)} s`);
    // Waking up: iOS took the sockets back while the phone slept.
    await phone.proxy.reset();
    await phone.tab.unfreeze();
    // Waking up, as iOS does: the page becomes visible again.
    await phone.tab.evaluate(`document.dispatchEvent(new Event("visibilitychange")); window.dispatchEvent(new Event("focus")); window.dispatchEvent(new Event("pageshow"));`).catch(() => {});
    const back = await waitLive(phone, 10_000);
    const seat = back === null ? null : await waitJoined(phone, 3_000);
    if (back === null) {
      fail("C3", "au réveil, pas reconnecté en 10 s alors que sa connexion a été fermée");
      await restore(phone);
    } else if (seat === null) fail("C3", `reconnecté au réveil en ${(back / 1000).toFixed(1)} s, mais plus à sa place`);
    else ok("C3", `au réveil, reconnecté à sa place en ${(back / 1000).toFixed(1)} s`);
    await invariants("freeze");
  },

  /** C3: the Wi-Fi dies silently while the page keeps running (out of range,
   * a router that hangs): no `onclose`, ever. Only a heartbeat in the page
   * can notice it. Toxiproxy passes on the host's own close, which a dead
   * Wi-Fi never would, and salon's host gives up after the same silence as
   * the page: so only what the host sends is swallowed. The host keeps
   * hearing the page and closes nothing; a page without its own silence
   * timer would wait forever. */
  async silent() {
    if (phones.length < 5) return skip("C3", "il faut au moins 5 téléphones pour ce scénario");
    const phone = phones[4];
    const sockets = phone.tab.sockets;
    const open = phone.tab.socketTimes.filter((s) => s.closedAt === null);
    await phone.proxy.deafen();
    const start = Date.now();
    let noticed = null;
    while (Date.now() - start < 25_000 && noticed === null) {
      if (phone.tab.sockets > sockets) noticed = Date.now() - start;
      await sleep(100);
    }
    // A page that reacted well after its old socket closed only reacted to that.
    const reopened = phone.tab.socketTimes[sockets]?.openedAt ?? Infinity;
    const closedFirst = open.find((s) => s.closedAt !== null && s.closedAt < reopened - 500);
    if (noticed !== null && closedFirst) {
      fail("C3", `Wi-Fi mort en silence : la page n'a réagi qu'à la fermeture de son WebSocket (${((closedFirst.closedAt - start) / 1000).toFixed(1)} s), qu'un vrai Wi-Fi mort ne transmet pas : il lui faut son propre délai de silence`);
    } else if (noticed === null) {
      fail("C3", "Wi-Fi mort en silence pendant 25 s (plus rien n'arrive de l'hôte) : la page n'a rien tenté. Elle croit son WebSocket ouvert (`readyState` reste OPEN) : il lui faut son propre délai de silence");
    } else if (noticed > 20_000) fail("C3", `Wi-Fi mort en silence : la page s'en est aperçue en ${(noticed / 1000).toFixed(1)} s (> 20 s)`);
    else ok("C3", `Wi-Fi mort en silence : la page s'en est aperçue d'elle-même en ${(noticed / 1000).toFixed(1)} s et a rouvert une connexion`);
    await sleep(Math.max(0, 25_000 - (Date.now() - start)));
    await phone.proxy.reset();
    if ((await waitLive(phone, 10_000)) === null || !(await joined(phone))) await restore(phone);
    await invariants("silent");
  },

  /** C4: reloading the page mid-round. */
  async reload() {
    if (!phones.length) return skip("C4", "pas de téléphones");
    const phone = phones[Math.min(2, phones.length - 1)];
    await phone.tab.reload();
    const seat = await waitJoined(phone, 10_000);
    if (seat !== null) ok("C4", `après un rechargement, de retour à sa place en ${(seat / 1000).toFixed(1)} s`);
    else if ((await runJoin(phone, phone.name)) && (await waitJoined(phone, 10_000)) !== null) {
      partial("C4", "après un rechargement, il faut repasser par l'écran pour rejoindre : vérifie que le joueur y retrouve son personnage et son score");
    } else fail("C4", "après un rechargement, le joueur a perdu sa place");
    await invariants("reload");
  },

  /** C5: the same player in two tabs. */
  async twin() {
    if (!phones.length) return skip("C5", "pas de téléphones");
    const phone = phones[Math.min(3, phones.length - 1)];
    const before = phone.tab.sockets;
    const twin = await browser.open(phone.url, { width: 390, height: 844, mobile: true, context: phone.tab.context });
    twin.label = `${phone.tab.label} (2ᵉ onglet)`;
    extraTabs.push(twin);
    let twinSeat = await waitJoined({ tab: twin }, 10_000);
    if (twinSeat === null && (await runJoin({ tab: twin }, phone.name))) {
      twinSeat = await waitJoined({ tab: twin }, 10_000);
      if (twinSeat !== null) partial("C5", "le nouvel onglet doit repasser par l'écran pour rejoindre");
    }
    await sleep(10_000);
    const connections = phone.tab.sockets - before + twin.sockets;
    if (twinSeat === null) fail("C5", "le nouvel onglet n'a pas pris la place du joueur");
    if (connections > 3) fail("C5", `${connections} WebSockets ouverts en 10 s : les deux onglets se reprennent la place en boucle`);
    else ok("C5", `le dernier onglet gagne, ${connections} WebSocket(s) ouvert(s) en 10 s`);
    await twin.close();
    await restore(phone);
    if (!(await joined(phone))) fail("C5", "l'onglet d'origine n'a pas retrouvé sa place");
    await invariants("twin");
  },

  /** C1: a phone on salon's usual path, WebRTC, which Toxiproxy cannot
   * carry (UDP): the host's `SALON_LOG` says which transport opened. It runs
   * in the lobby, before `start`. With a seat free, the phone must join; with
   * the room full (or the game already started, under `--only`), only the
   * transport is judged: a game may refuse a player it has no room for. */
  async rtc() {
    if (!config.phone) return skip("C1", "pas de téléphones");
    const max = config.players?.max;
    const seatFree = !gameStarted && (!max || phones.length < max);
    const since = host.links.length;
    const phone = await openPhone(args.phones + 2, { webrtc: true });
    phone.tab.label = "téléphone WebRTC";
    extraTabs.push(phone.tab);
    const start = Date.now();
    await runJoin(phone, "Datagramme");
    const took = seatFree ? await waitJoined(phone, 20_000) : null;
    if (!seatFree) await sleep(5_000);
    const opened = host.links.slice(since).map((l) => l.line).filter((l) => / opened \(/.test(l));
    const webrtc = opened.filter((l) => / opened \(WebRTC/.test(l));
    const seconds = ((Date.now() - start) / 1000).toFixed(1);
    const text = (await phone.tab.evaluate("document.body.innerText").catch(() => "")).replace(/\s+/g, " ").trim();
    const why = gameStarted ? "la partie est lancée" : `la salle est pleine (${phones.length}/${max})`;
    if (webrtc.length && !seatFree) {
      // A refused page may try again on WebSocket: the transport was judged
      // on the first link, the one WebRTC opened.
      ok("C1", `[rtc] un téléphone sans \`?salon=ws\` ouvre son lien par WebRTC (« ${webrtc[0]} ») ; ${why}, son entrée n'est pas jugée (il lit « ${text.slice(0, 80)} »${phone.tab.sockets ? `, puis ouvre ${phone.tab.sockets} WebSocket(s)` : ""})`);
    } else if (webrtc.length && phone.tab.sockets === 0) {
      if (!seatFree) ok("C1", `[rtc] un téléphone sans \`?salon=ws\` ouvre son lien par WebRTC (« ${webrtc[0]} ») ; ${why}, son entrée n'est pas jugée (il lit « ${text.slice(0, 80)} »)`);
      else if (took !== null) ok("C1", `[rtc] un téléphone sans \`?salon=ws\` a rejoint par WebRTC en ${seconds} s (« ${webrtc[0]} »)`);
      else fail("C1", `[rtc] un téléphone sans \`?salon=ws\` ouvre son lien par WebRTC (« ${webrtc[0]} ») mais n'a pas rejoint en 20 s, une place libre ; il lit « ${text.slice(0, 100)} »`);
    } else if (phone.tab.sockets > 0) {
      partial("C1", `[rtc] un téléphone sans \`?salon=ws\` ${took !== null ? `a rejoint en ${seconds} s` : seatFree ? "n'a pas rejoint en 20 s" : "s'est connecté"}, mais par WebSocket (${opened.length ? `« ${opened[opened.length - 1]} »` : "aucune ligne `SALON_LOG` de l'hôte"}) : WebRTC n'a pas ouvert. Sur macOS, le pare-feu bloque l'UDP de chaque nouveau binaire tant qu'on ne l'a pas autorisé ; ailleurs, c'est à creuser`);
      if (seatFree && took === null) fail("C1", `[rtc] le téléphone repli sur WebSocket n'a pas rejoint en 20 s, une place libre ; il lit « ${text.slice(0, 100)} »`);
    } else if (opened.length === 0 && took === null) {
      fail("C1", `[rtc] un téléphone sans \`?salon=ws\` n'a ouvert aucun lien en ${seconds} s (ni WebRTC ni WebSocket) ; il lit « ${text.slice(0, 100)} »`);
    } else {
      partial("C1", `[rtc] un téléphone sans \`?salon=ws\` ${took !== null ? "a rejoint" : "s'est connecté"} sans WebSocket, mais l'hôte n'a écrit aucune ligne « salon: link N opened (WebRTC » (SALON_LOG)`);
    }
    await phone.tab.close();
    await phone.proxy.close();
    await invariants("rtc");
  },

  /** E1–E4: what a hostile or broken client sends, on salon's WebSocket,
   * from the network (the LAN address, never loopback: only the TV is). */
  async hostile() {
    if (!config.socket) return skip("E1", "pas de WebSocket déclaré (`socket`)");
    // Run alone (--only hostile), it would start while the phones are still
    // connecting, and count their first connection as a reconnection: wait
    // until every phone is in, and its link has settled.
    for (const p of phones) await waitJoined(p, 10_000);
    await sleep(1_000);
    const url = `ws://${lan ?? "127.0.0.1"}:${host.port}${config.socket}`;
    // Real packets to mutate. A real Hello carries a phone's token, and
    // whoever has the token is that phone: the harness invents another.
    const real = [];
    for (const p of phones) {
      for (const frame of p.tab.wsSent) {
        if (frame.payload.length >= 4096) continue;
        const packet = readPacket(frame.payload);
        if (packet.control === "Hello") real.push(packet.unreadable ? null : forgeToken({ bytes: Buffer.from(frame.payload), packet }));
        else real.push(Buffer.from(frame.payload));
      }
    }
    const samples = real.filter(Boolean);
    const connect = () =>
      new Promise((resolve) => {
        const ws = new WebSocket(url);
        ws.binaryType = "arraybuffer";
        const socket = { ws, received: [], closed: false, closedAt: null, code: null };
        ws.addEventListener("message", (e) => socket.received.push(typeof e.data === "string" ? { text: e.data } : readPacket(e.data)));
        const closed = (e) => {
          socket.closed = true;
          socket.closedAt ??= Date.now();
          if (e?.code) socket.code ??= e.code;
        };
        ws.addEventListener("close", closed);
        ws.addEventListener("error", closed);
        ws.addEventListener("open", () => resolve(socket));
        setTimeout(() => resolve(socket), 3_000);
      });
    /** A connection that is open now, or null after three tries: what is
     * measured on it is then the host's answer, never a close that came
     * before (a socket that did not open, or one the host already shut). */
    const connectOpen = async () => {
      for (let tries = 0; tries < 3; tries++) {
        const socket = await connect();
        if (!socket.closed && socket.ws.readyState === 1) return socket;
        try { socket.ws.close(); } catch {}
        await sleep(500);
      }
      return null;
    };
    /** Whether `bytes` left on an open socket. */
    const send = (socket, bytes) => {
      try {
        if (socket.closed || socket.ws.readyState !== 1) return false;
        socket.ws.send(bytes);
        return true;
      } catch {
        return false;
      }
    };
    const mutate = (payload) => {
      const b = Buffer.from(payload);
      const kind = random();
      if (kind < 0.3 && b.length) {
        for (let i = 0; i < 1 + Math.floor(random() * 3); i++) b[Math.floor(random() * b.length)] ^= 1 << Math.floor(random() * 8);
      } else if (kind < 0.5) return b.subarray(0, Math.floor(random() * b.length));
      else if (kind < 0.65 && b.length) {
        // A varint that never ends, or a huge length, somewhere inside.
        const at = Math.floor(random() * b.length);
        return Buffer.concat([b.subarray(0, at), Buffer.alloc(1 + Math.floor(random() * 12), 0xff), b.subarray(at)]);
      } else if (kind < 0.8) return Buffer.concat([b, b]);
      else return Buffer.concat([b, Buffer.from(Array.from({ length: Math.floor(random() * 40) }, () => Math.floor(random() * 256)))]);
      return b;
    };
    // Nobody else may notice: every screen keeps receiving, no phone loses
    // its connection (one that was replaced would not come back on its own).
    const screens = [tv, ...phones];
    const socketsBefore = new Map(screens.map((p) => [p, p.tab.sockets]));
    const others = async (item, what) => {
      const hurt = [];
      for (const p of screens) {
        if (!isLive(p, 3_000)) hurt.push(`${p.tab.label} ne reçoit plus rien du jeu depuis ${((Date.now() - p.tab.lastLiveFrame) / 1000).toFixed(1)} s (${lastHeard(p)})`);
        else if (p.tab.sockets !== socketsBefore.get(p)) hurt.push(`${p.tab.label} a dû rouvrir sa connexion`);
        else if (p !== tv && !(await joined(p))) hurt.push(`${p.tab.label} n'est plus dans la partie`);
      }
      if (hurt.length) fail(item, `après ${what}, les autres en pâtissent : ${hurt.slice(0, 3).join(" ; ")}${networkMoved()}`);
      return hurt.length === 0;
    };
    // E1: garbage, then mutated real packets, as binary and as text frames.
    let garbage = await connectOpen();
    let sentCount = 0;
    let closures = 0;
    const kinds = { octets: 0, texte: 0, mutés: 0 };
    for (let i = 0; i < 300 && garbage; i++) {
      // A mutated Hello may be refused, which closes that link: another one.
      if (garbage.closed || garbage.ws.readyState !== 1) {
        closures++;
        garbage = await connectOpen();
        if (!garbage) break;
      }
      const kind = random();
      let payload;
      let kindName;
      if (kind < 0.2 || !samples.length) (payload = Buffer.from(Array.from({ length: Math.floor(random() * 200) }, () => Math.floor(random() * 256)))), (kindName = "octets");
      else if (kind < 0.35) (payload = Buffer.from(pick(["{", "{\"t\":\"hello\"}", "null", pick(NASTY)]))), (kindName = "texte");
      else (payload = mutate(pick(samples))), (kindName = "mutés");
      // Only what left on an open socket counts.
      if (send(garbage, kindName === "texte" ? payload.toString("utf8") : payload)) {
        sentCount++;
        kinds[kindName]++;
      }
    }
    await sleep(1_500);
    if (!garbage) fail("E1", `après ${sentCount} messages illisibles, impossible d'ouvrir une nouvelle connexion à ${url} en 3 essais${networkMoved()}`);
    if (await others("E1", `${sentCount} messages illisibles`)) {
      ok("E1", `${sentCount} messages illisibles (${kinds.octets} en octets au hasard, ${kinds.texte} en frames texte, ${kinds.mutés} paquets réels mutés, sur ${samples.length} capturés) : ignorés, les autres continuent${closures ? ` (${closures} connexion(s) fautive(s) fermée(s) par l'hôte)` : ""}`);
    }
    try { garbage?.ws.close(); } catch {}
    // E2: one frame over salon's 64 KiB closes that connection only.
    // The connection must be open when the frame leaves: a close measured
    // otherwise would be an earlier one, and its duration negative.
    for (const size of [64 * 1024 + 1, 4 * 1024 * 1024]) {
      const label = size > 1024 * 1024 ? `${size / 1024 / 1024} Mio` : `${(size / 1024).toFixed(0)} Kio + 1 octet`;
      let huge = null;
      let sentAt = null;
      for (let tries = 0; tries < 3 && sentAt === null; tries++) {
        try { huge?.ws.close(); } catch {}
        huge = await connectOpen();
        if (!huge) break;
        const before = Date.now();
        if (send(huge, Buffer.alloc(size, 0x01))) sentAt = before;
      }
      if (sentAt === null) {
        fail("E2", `impossible d'envoyer la frame de ${label} : aucune connexion à ${url} n'est restée ouverte jusqu'à l'envoi (3 essais) ; rien n'est mesuré${networkMoved()}`);
        try { huge?.ws.close(); } catch {}
        continue;
      }
      for (let waited = 0; waited < 3_000 && !huge.closed; waited += 100) await sleep(100);
      const code = huge.code ? `, code ${huge.code}` : "";
      if (!huge.closed) fail("E2", `une frame de ${label} n'a pas fermé sa connexion en 3 s`);
      else if (huge.closedAt < sentAt) fail("E2", `la connexion de la frame de ${label} s'est fermée ${sentAt - huge.closedAt} ms avant l'envoi : rien n'est mesuré${networkMoved()}`);
      else ok("E2", `une frame de ${label} ferme sa connexion en ${((huge.closedAt - sentAt) / 1000).toFixed(2)} s${code}, et elle seule`);
      try { huge.ws.close(); } catch {}
    }
    await others("E2", "des frames trop grosses");
    // E3: a device the host welcomed floods it, while the others keep playing.
    const flood = await connectOpen();
    const hello = firstHello(phones[0]?.tab.wsSent ?? []);
    if (flood && hello && !hello.packet.unreadable) send(flood, forgeToken(hello));
    await sleep(300);
    const plain = samples.filter((b) => !readPacket(b).control);
    const sample = plain.length ? plain : samples.length ? samples : [Buffer.from([0])];
    const floodStart = Date.now();
    let flooded = 0;
    let worstGap = 0;
    while (flood && Date.now() - floodStart < 3_000 && !flood.closed) {
      for (let i = 0; i < 500; i++) if (send(flood, random() < 0.5 ? pick(sample) : mutate(pick(sample)))) flooded++;
      await sleep(10);
      for (const p of screens) worstGap = Math.max(worstGap, Date.now() - p.tab.lastLiveFrame);
    }
    await sleep(500);
    if (!flood) fail("E3", `impossible d'ouvrir une connexion à ${url} pour l'inondation (3 essais) : rien n'est mesuré${networkMoved()}`);
    else if (await others("E3", `${flooded} messages en 3 s`)) {
      const cut = flood.closed ? `, puis l'hôte a fermé sa connexion au bout de ${((flood.closedAt - floodStart) / 1000).toFixed(1)} s` : "";
      ok("E3", `${flooded} messages en 3 s d'un appareil accueilli${cut} : la TV et les téléphones reçoivent toujours (plus long silence vu : ${(worstGap / 1000).toFixed(1)} s)`);
    }
    try { flood?.ws.close(); } catch {}
    // E4: a device of the network replays the TV's first packet (its key
    // with it). Only a link from this computer may be the TV.
    const tvHello = firstHello(tv.sent, "Tv");
    if (!lan) partial("E4", "pas d'adresse réseau : impossible de se présenter en TV depuis le réseau");
    else if (!tvHello) partial("E4", "pas de Hello de la TV capturé");
    else {
      const impostor = await connectOpen();
      const sentAt = Date.now();
      if (!impostor || !send(impostor, tvHello.bytes)) {
        fail("E4", `impossible d'ouvrir une connexion à ${url} pour rejouer le paquet de la TV : rien n'est mesuré${networkMoved()}`);
        try { impostor?.ws.close(); } catch {}
      } else {
      for (let waited = 0; waited < 4_000 && !impostor.closed; waited += 100) await sleep(100);
      const answers = impostor.received.map((p) => (p.text ? `texte « ${p.text.slice(0, 20)} »` : describe(p)));
      const biggest = Math.max(0, ...impostor.received.map((p) => p.size ?? p.text.length));
      const refused = impostor.received.some((p) => p.control === "Refused");
      const welcomed = impostor.received.some((p) => p.control === "Welcome");
      const took = impostor.closed ? ((impostor.closedAt - sentAt) / 1000).toFixed(1) : null;
      if (welcomed || biggest > 400) fail("E4", `un appareil du réseau qui rejoue le premier paquet de la TV reçoit ${answers.slice(0, 3).join(", ")} (jusqu'à ${biggest} octets) : il voit l'écran de la TV`);
      else if (!refused) fail("E4", `un appareil du réseau qui rejoue le premier paquet de la TV ne reçoit pas de refus (${answers.join(", ") || "rien"})`);
      else if (took === null || took > 3.5) fail("E4", `un appareil du réseau qui se dit TV est refusé (${answers[0]}), mais sa connexion reste ouverte ${took === null ? "plus de 4 s" : `${took} s`}`);
      else ok("E4", `un appareil du réseau qui rejoue le premier paquet de la TV reçoit ${answers[0]} (${biggest} octets au plus) et sa connexion se ferme en ${took} s`);
      try { impostor.ws.close(); } catch {}
      }
    }
    // E4: the real phone's Hello with an invented token: a new device, nobody's place.
    const phone = phones[0];
    const own = phone ? deviceOf(phone.tab) : null;
    const usable = hello && !hello.packet.unreadable;
    const forged = usable ? await connectOpen() : null;
    if (usable && !(forged && send(forged, forgeToken(hello)))) {
      fail("E4", `impossible d'ouvrir une connexion à ${url} pour le jeton inventé : rien n'est mesuré${networkMoved()}`);
      try { forged?.ws.close(); } catch {}
    } else if (usable) {
      await sleep(1_500);
      const answers = forged.received.map((p) => (p.text ? "texte" : describe(p)));
      const welcome = forged.received.find((p) => p.control === "Welcome");
      const refusal = forged.received.find((p) => p.control === "Refused");
      try { forged.ws.close(); } catch {}
      await sleep(500);
      const stillThere = isLive(phone, 3_000) && phone.tab.sockets === socketsBefore.get(phone) && (await joined(phone));
      if (!stillThere) fail("E4", `après un Hello au jeton inventé, ${phone.tab.label} a perdu sa place ou sa connexion`);
      else if (welcome && own !== null && welcome.device === own) fail("E4", `un jeton inventé reçoit l'appareil ${own}, celui de ${phone.tab.label}`);
      else if (welcome) ok("E4", `un jeton inventé devient un appareil neuf (${describe(welcome)}${own !== null ? `, le vrai téléphone est l'appareil ${own}` : ""}) et ne prend la place de personne`);
      else if (refusal) ok("E4", `un jeton inventé est refusé (${describe(refusal)}) et ne prend la place de personne`);
      else partial("E4", `un jeton inventé ne reçoit ni Welcome ni refus (${answers.join(", ") || "rien"})`);
    } else partial("E4", "pas de Hello de téléphone capturé : le jeton inventé n'est pas essayé");
    await sleep(500);
    await invariants("hostile");
  },

  /** R1: 200 ± 100 ms on every phone, during play. */
  async jitter() {
    if (!phones.length) return skip("R1", "pas de téléphones");
    for (const p of phones) await p.proxy.delay(200, 100);
    const stop = monkey();
    const stopAdvance = advancer(8_000);
    await sleep(45_000);
    await stop();
    await stopAdvance();
    const stale = phones.filter((p) => !isLive(p, 8_000)).map((p) => p.tab.label);
    for (const p of phones) await p.proxy.delay(0, 0);
    if (stale.length) fail("R1", `avec 200 ± 100 ms, plus de nouvelles pour : ${stale.join(", ")}`);
    else partial("R1", "45 s à 200 ± 100 ms sans perte de connexion (« jouable » se juge au salon)");
    await invariants("jitter");
  },

  /** E6, D2, D5, T2: a whole game of random gestures, the TV advancing. */
  async monkey() {
    const duration = args.soak ? 60_000 : 120_000;
    const stop = monkey();
    const stopAdvance = advancer(6_000);
    const hashes = [];
    const start = Date.now();
    let stuckSince = Date.now();
    let last = null;
    while (Date.now() - start < duration) {
      await sleep(5_000);
      const h = await tvHash().catch(() => null);
      if (h !== last) stuckSince = Date.now();
      last = h;
      hashes.push(h);
      if (config.advance && Date.now() - stuckSince > 40_000) {
        fail("D2", "la TV n'a pas changé depuis 40 s alors que la partie avance (touche « advance ») : bloquée ?");
        writeFileSync(join(outDir, "monkey-tv-stuck.png"), await tv.tab.screenshot());
        stuckSince = Date.now();
      }
      await invariants("monkey");
    }
    await stop();
    await stopAdvance();
    const distinct = new Set(hashes).size;
    ok("E6", `${(duration / 1000).toFixed(0)} s de gestes au hasard sur ${phones.length ? `${phones.length} téléphones` : "la page"}`);
    if (!config.advance) skip("D2", "pas de touche « advance » : la progression se juge au salon");
    else if (distinct > 3) ok("D2", `${distinct} écrans TV différents vus`);
    else fail("D2", `seulement ${distinct} écrans TV différents en ${duration / 1000} s`);
    if (config.layout !== false) await scenarios.layout();
    writeFileSync(join(outDir, "monkey-tv-end.png"), await tv.tab.screenshot());
  },

  /** C7: the host restarts mid-game. */
  async restart() {
    if (!config.relaunch) return skip("C7", "pas de commande `relaunch`");
    if (!config.socket) return skip("C7", "pas de connexion permanente avec l'hôte");
    await host.kill();
    // Only the new host can send what arrives from now on: a screen is back
    // when it gets a Welcome or a packet of the game, never for a refusal.
    const killedAt = Date.now();
    await sleep(2_000);
    host.exitedUnexpectedly = false;
    await host.start(config.relaunch, 120_000);
    // Every screen gets 15 s from now, together.
    const waits = await Promise.all([tv, ...phones].map(async (p) => ({ p, back: await waitLive(p, 15_000, killedAt) })));
    const frozen = [];
    const told = [];
    for (const { p, back } of waits) {
      if (back !== null) continue;
      // Not back, but no longer showing the game: it may be telling the player what to do.
      if (p !== tv && !(await joined(p))) told.push(p);
      else frozen.push(p);
    }
    if (frozen.length) {
      const text = (await frozen[0].tab.evaluate("document.body.innerText").catch(() => "")).replace(/\s+/g, " ").trim();
      fail("C7", `15 s après le redémarrage de l'hôte, ni Welcome ni paquet du jeu pour : ${frozen.map((p) => `${p.tab.label} (${lastHeard(p)})`).join(", ")} ; ${frozen[0].tab.label} lit « ${text.slice(0, 100)} »`);
    }
    if (told.length) {
      const text = (await told[0].tab.evaluate("document.body.innerText").catch(() => "")).replace(/\s+/g, " ").trim();
      partial("C7", `${told.length} téléphone(s) ne se reconnectent pas mais ont quitté l'écran de jeu ; ils lisent : « ${text.slice(0, 120)} » (clair ?)`);
    }
    if (!frozen.length && !told.length) ok("C7", "après le redémarrage de l'hôte, la TV et tous les téléphones se sont reconnectés");
    const lost = [];
    for (const p of phones) if (!(await joined(p))) lost.push(p);
    if (lost.length) {
      const text = (await lost[0].tab.evaluate("document.body.innerText").catch(() => "")).replace(/\s+/g, " ").trim();
      partial("C7", `${lost.length} joueur(s) ont perdu leur place (partie neuve) ; ils lisent : « ${text.slice(0, 100)} »`);
    }
    await invariants("restart");
    if (!order.includes("storage")) {
      const missing = await rejoinAll();
      if (missing.length) partial("C7", await describeMissing(missing));
    }
  },
};

// ---------------------------------------------------------------- the run

async function main() {
  host = new Host(await freePort());
  log(`${gameDir} : lancement (${config.launch.join(" ")}) sur le port ${host.port}, graine ${args.seed}`);
  await host.start(config.launch, 15 * 60_000);
  log(`prêt (pid ${host.pid ?? "?"}) ; captures dans ${outDir}`);
  toxiproxy = await Toxiproxy.start();
  browser = await launchBrowser({ live: isLivePacket, remember: (payload) => readPacket(payload).control === "Hello" });
  await openTv();
  await sleep(1_500);
  order = ALL.filter((s) => args.only.includes(s));
  // Phones are needed by almost everything: join first, quietly, if not asked.
  if (!order.includes("join") && config.phone) {
    for (let i = 0; i < args.phones; i++) {
      const phone = await openPhone(i + 1);
      phones.push(phone);
      await runJoin(phone, `Joueur${i + 1}`);
    }
  }
  const memory = [];
  const sampleMemory = () => {
    const u = host.usage();
    if (u) memory.push({ t: Date.now(), ...u });
  };
  // The TV presses `start` after the lobby's scenarios, if `join` ran.
  const lastLobby = order.includes("join") ? Math.max(...LOBBY.map((name) => order.indexOf(name))) : -1;
  const runOnce = async () => {
    for (const [index, name] of order.entries()) {
      if (name === "join" && phones.length) continue;
      log(`▶ ${name}`);
      sampleMemory();
      try {
        await scenarios[name]();
      } catch (error) {
        fail("E1", `[${name}] le scénario s'est interrompu : ${error.message}`);
        if (!host.alive()) throw error;
      }
      if (index === lastLobby && config.start && !gameStarted) {
        await press(tv.tab, config.start);
        gameStarted = true;
        log("  partie lancée depuis la TV");
        await sleep(2_000);
      }
    }
  };
  await runOnce();
  if (args.soak) {
    const end = Date.now() + args.soak * 60_000;
    let round = 1;
    while (Date.now() < end) {
      round++;
      log(`— endurance, tour ${round} (${Math.round((end - Date.now()) / 60_000)} min restantes)`);
      for (const name of ["cut", "freeze", "reload", "hostile", "monkey"]) {
        if (!order.includes(name)) continue;
        sampleMemory();
        await scenarios[name]().catch((e) => fail("E1", `[${name}] ${e.message}`));
      }
    }
    // What matters is the slope: projected over a 6 h evening, the host's
    // memory must stay well within a laptop's. A ratio means nothing for a
    // host of a few MiB, where the allocator alone moves it by half.
    const warm = memory.find((m) => m.t - started > 5 * 60_000) ?? memory[0];
    const lastSample = memory[memory.length - 1];
    if (warm && lastSample && lastSample.t > warm.t) {
      const perHour = ((lastSample.rss - warm.rss) / 1024) / ((lastSample.t - warm.t) / 3_600_000);
      const evening = Math.max(0, perHour * 6);
      const series = memory.map((m) => Math.round(m.rss / 1024)).join(" ");
      const line = `mémoire ${Math.round(warm.rss / 1024)} → ${Math.round(lastSample.rss / 1024)} Mio, soit +${evening.toFixed(0)} Mio sur une soirée de 6 h ; threads ${warm.threads} → ${lastSample.threads} (série en Mio : ${series})`;
      if (evening > 256 || lastSample.threads > warm.threads + 4) fail("R2", line);
      else ok("R2", line);
    }
  } else skip("R2", "lance --soak 30 pour l'endurance");
}

/** Stops what the harness started: nothing may outlive it, even on Ctrl-C. */
let cleaning = null;
function cleanup() {
  cleaning ??= (async () => {
    await host?.kill().catch(() => {});
    toxiproxy?.stop();
    await Promise.race([browser?.close().catch(() => {}), sleep(5_000)]);
  })();
  return cleaning;
}
for (const signal of ["SIGINT", "SIGTERM"]) {
  process.once(signal, () => {
    console.error(`\n${signal} : arrêt du harnais, nettoyage…`);
    cleanup().finally(() => process.exit(130));
  });
}

let crashed = null;
try {
  await main();
} catch (error) {
  crashed = error;
  fail("E1", `arrêt : ${error.message}`);
} finally {
  await cleanup();
}

// ---------------------------------------------------------------- summary

const ITEMS = ["C1", "C2", "C3", "C4", "C5", "C7", "C8", "C9", "E1", "E2", "E3", "E4", "E6", "T2", "T3", "T4", "D2", "R1", "R2", "R3"];
const lines = [`# Certification de ${gameDir.split("/").slice(-2).join("/")}`, "", `Graine ${args.seed}, ${((Date.now() - started) / 60_000).toFixed(1)} min. Captures : ${outDir}`, "", "| Item | État | Détail |", "|---|---|---|"];
let failures = 0;
for (const item of ITEMS) {
  const r = results.get(item);
  if (!r) continue;
  if (r.status === "fail") failures++;
  // Every note, the worst first, in the order they came within an outcome.
  // When an item mixes outcomes, each line says its own.
  const notes = [...r.notes].sort((a, b) => RANK[b.status] - RANK[a.status]);
  const mixed = new Set(notes.map((n) => n.status)).size > 1;
  const text = notes.map((n) => `${mixed ? `${MARK[n.status]} ` : ""}${n.text.replace(/\s*\n\s*/g, " ")}${n.count > 1 ? ` (×${n.count})` : ""}`);
  lines.push(`| ${item} | ${MARK[r.status]} | ${text.join("<br>").replaceAll("|", "\\|")} |`);
}
lines.push("", "✓ tenu · ✗ cassé · ~ à juger par un humain, ou vérifié en partie · – non applicable", "");
if (host?.stderr.length) lines.push("Dernières lignes d'erreur de l'hôte :", "", "```", ...host.stderr.slice(-10), "```");
writeFileSync(join(outDir, "rapport.md"), lines.join("\n"));
console.log(`\n${lines.join("\n")}`);
if (crashed) console.error(crashed.stack);
process.exit(failures || crashed ? 1 : 0);
