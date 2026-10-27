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

const ALL = ["join", "layout", "cut", "freeze", "silent", "reload", "twin", "hostile", "jitter", "monkey", "restart", "storage"];
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

/** A message's kind, to compare two answers without knowing the protocol. */
function shape(payload, binary) {
  if (binary) return `octets:${payload[0] ?? ""}`;
  try {
    const value = JSON.parse(payload.toString("utf8"));
    if (value && typeof value === "object" && !Array.isArray(value)) {
      for (const key of ["t", "type", "kind", "tag", "msg"]) {
        if (typeof value[key] === "string") return `${key}=${value[key]}`;
      }
      return `{${Object.keys(value).sort().join(",")}}`;
    }
    return typeof value;
  } catch {
    return `texte:${payload.toString("utf8").slice(0, 12)}`;
  }
}

// ---------------------------------------------------------------- the report

/** item → { status: "ok" | "fail" | "partial" | "skip", notes: [] } */
const results = new Map();
function record(item, status, note) {
  // The worst outcome wins; "not applicable" only stands alone.
  const rank = { skip: 0, ok: 1, partial: 2, fail: 3 };
  const entry = results.get(item) ?? { status, notes: [] };
  if (rank[status] > rank[entry.status]) entry.status = status;
  if (note) entry.notes.push(note);
  results.set(item, entry);
  const mark = { ok: "✓", fail: "✗", partial: "~", skip: "–" }[status];
  if (note) log(`    ${mark} ${item} ${note}`);
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
  }

  /** A page path with the captured values filled in. */
  path(template) {
    return template.replace(/\{(\w+)\}/g, (all, name) => this.captured[name] ?? all);
  }

  async start(command, timeoutMs) {
    const fill = (s) => s.replaceAll("{port}", String(this.port)).replaceAll("{tmp}", join(outDir, "data"));
    const env = { ...process.env };
    for (const [k, v] of Object.entries(config.env ?? {})) env[k] = fill(v);
    const [program, ...rest] = command.map(fill);
    this.expectExit = false;
    // Its own process group: `cargo run` and the game it starts die together.
    this.process = spawn(program, rest, { cwd: gameDir, env, stdio: ["ignore", "pipe", "pipe"], detached: true });
    let ready = false;
    const onLine = (line, err) => {
      if (/panicked at|thread '.*' panicked/.test(line)) this.panics.push(line.trim());
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

  /** The host's open TCP connections on its port: one goes when it gives up on a phone. */
  connections() {
    try {
      const out = execFileSync("lsof", ["-nP", "-a", "-p", String(this.pid), "-iTCP", "-sTCP:ESTABLISHED"]).toString();
      return out.split("\n").filter((line) => line.includes(`:${this.port}->`)).length;
    } catch {
      return null;
    }
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
let host;
let browser;
let toxiproxy;
let tv;
const phones = []; // { index, name, tab, proxy, sent: [], context }

async function openTv() {
  // The TV is opened directly, at the host's own address: a game may keep
  // organizer rights for its own page (Sidequest checks the origin).
  const tab = await browser.open(`http://localhost:${host.port}${host.path(config.tv)}`, { width: 1920, height: 1080 });
  tab.label = "TV";
  tv = { tab, sent: tab.wsSent, received: tab.wsReceived };
}

/** The game sent something to this screen within the last `withinMs`. */
function isLive(screen, withinMs) {
  return Date.now() - screen.tab.lastWsFrame < withinMs;
}

/** The phone's address, with the host's current room code. */
function phoneUrl(phone) {
  return `http://127.0.0.1:${phone.proxy.port}${host.path(config.phone)}`;
}

async function openPhone(index) {
  // A phone reaches the game from the network, as a real phone would.
  const proxy = await toxiproxy.proxy(config.phone_upstream === "loopback" || !lan ? "127.0.0.1" : lan, host.port);
  const phone = { index, name: null, proxy };
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

async function waitLive(screen, timeoutMs) {
  const start = Date.now();
  while (Date.now() - start < timeoutMs) {
    if (isLive(screen, 3_000)) return Date.now() - start;
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

async function rejoinAll() {
  for (const phone of phones) {
    if (!(await joined(phone))) {
      // The host may have a new room code since the phone first came.
      phone.url = phoneUrl(phone);
      await phone.tab.goto(phone.url);
      await runJoin(phone, phone.name ?? `Joueur${phone.index}`);
    }
  }
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
    } else skip("R3", `--phones ${args.phones} < maximum ${max ?? "?"}`);
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
    // A browser that refuses storage (private mode, blocked cookies).
    const allowStorage = await a.tab.beforeScripts(BLOCK_STORAGE);
    a.url = phoneUrl(a);
    await a.tab.goto(a.url);
    await sleep(1_000);
    if (!(await joined(a))) await runJoin(a, "SansCookie");
    if ((await waitJoined(a, 10_000)) === null) {
      const text = (await a.tab.evaluate("document.body.innerText").catch(() => "")).replace(/\s+/g, " ").trim();
      fail("C9", `sans stockage, on ne peut pas rejoindre ; la page dit « ${text.slice(0, 100)} »`);
    } else ok("C9", "stockage refusé : on rejoint quand même");
    await allowStorage();
    // Garbage in every key the page saved, then a reload.
    if (!(await joined(b))) {
      b.url = phoneUrl(b);
      await b.tab.goto(b.url);
      await runJoin(b, "Corrompu");
    }
    await waitJoined(b, 10_000);
    const keys = await b.tab.evaluate("Object.keys(localStorage)").catch(() => []);
    await b.tab.evaluate(`for (const k of Object.keys(localStorage)) localStorage.setItem(k, ${JSON.stringify(NASTY[2] + "\u0000{[\"")});`).catch(() => {});
    await b.tab.reload();
    await sleep(1_500);
    const text = (await b.tab.evaluate("document.body.innerText").catch(() => "")).trim();
    if (!text) fail("C9", "après corruption du stockage, la page est vide");
    else ok("C9", `${keys.length} clé(s) du stockage corrompue(s) : la page s'ouvre`);
    await invariants("storage");
    await rejoinAll();
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
    const before = host.connections();
    await phone.proxy.blackhole();
    await phone.tab.freeze();
    const asleep = Date.now();
    // The host gives up when one of its connections goes.
    let noticed = null;
    while (Date.now() - asleep < 30_000) {
      const now = host.connections();
      if (noticed === null && before !== null && now !== null && now < before) noticed = Date.now() - asleep;
      await sleep(250);
    }
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
   * can notice it. */
  async silent() {
    if (phones.length < 5) return skip("C3", "il faut au moins 5 téléphones pour ce scénario");
    const phone = phones[4];
    const sockets = phone.tab.sockets;
    const before = host.connections();
    await phone.proxy.blackhole();
    const start = Date.now();
    // A page that hears nothing for a while opens a new WebSocket. Toxiproxy
    // passes on the host's own close, which a dead Wi-Fi never would: a page
    // that only reacts after the host gave up has no silence timeout of its own.
    let noticed = null;
    let hostGaveUp = null;
    while (Date.now() - start < 25_000 && noticed === null) {
      const now = host.connections();
      if (hostGaveUp === null && before !== null && now !== null && now < before) hostGaveUp = Date.now() - start;
      if (phone.tab.sockets > sockets) noticed = Date.now() - start;
      await sleep(100);
    }
    if (noticed !== null && hostGaveUp !== null && noticed >= hostGaveUp - 200) {
      fail("C3", `Wi-Fi mort en silence : la page n'a réagi qu'après la fermeture par l'hôte (${(hostGaveUp / 1000).toFixed(1)} s), qu'un vrai Wi-Fi mort ne transmet pas. Elle croit son WebSocket ouvert (\`readyState\` reste OPEN) : il lui faut son propre délai de silence`);
    } else if (noticed === null) {
      fail("C3", "Wi-Fi mort en silence pendant 25 s : la page n'a rien tenté. Elle croit son WebSocket ouvert (`readyState` reste OPEN) : il lui faut son propre délai de silence");
    } else if (noticed > 20_000) fail("C3", `Wi-Fi mort en silence : la page s'en est aperçue en ${(noticed / 1000).toFixed(1)} s (> 20 s)`);
    else ok("C3", `Wi-Fi mort en silence : la page s'en est aperçue en ${(noticed / 1000).toFixed(1)} s et a rouvert une connexion`);
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

  /** E1–E4: what a hostile or broken client sends. */
  async hostile() {
    if (!config.socket) return skip("E1", "pas de WebSocket déclaré (`socket`)");
    const url = `ws://${lan ?? "127.0.0.1"}:${host.port}${config.socket}`;
    const real = phones.flatMap((p) => p.tab.wsSent).filter((m) => m.payload.length < 4096);
    const connect = () =>
      new Promise((resolve) => {
        const ws = new WebSocket(url);
        ws.binaryType = "arraybuffer";
        const socket = { ws, received: [], closed: false };
        ws.addEventListener("message", (e) => socket.received.push(e.data));
        ws.addEventListener("close", () => (socket.closed = true));
        ws.addEventListener("error", () => (socket.closed = true));
        ws.addEventListener("open", () => resolve(socket));
        setTimeout(() => resolve(socket), 3_000);
      });
    const mutate = (payload) => {
      const b = Buffer.from(payload);
      const kind = random();
      if (kind < 0.25 && b.length) b[Math.floor(random() * b.length)] ^= 1 << Math.floor(random() * 8);
      else if (kind < 0.45) return b.subarray(0, Math.floor(random() * b.length));
      else if (kind < 0.65) return Buffer.from(b.toString("utf8").replace(/-?\d+(\.\d+)?/g, () => pick(["-1", "4294967296", "1e308", "NaN", "0.5", "-0", "99999999999999999999"])));
      else if (kind < 0.8) return Buffer.from(b.toString("utf8").replace(/"[^"]*"/g, (m) => (random() < 0.3 ? JSON.stringify(pick(NASTY)) : m)));
      else return Buffer.concat([b, b]);
      return b;
    };
    // E1: garbage, then mutated real messages, as text and as binary.
    const garbage = await connect();
    let sentCount = 0;
    for (let i = 0; i < 300 && garbage.ws.readyState === 1; i++) {
      const kind = random();
      let payload;
      if (kind < 0.15) payload = Buffer.from(Array.from({ length: Math.floor(random() * 200) }, () => Math.floor(random() * 256)));
      else if (kind < 0.3 || !real.length) payload = Buffer.from(pick(["{", "}", "[]", "null", "{\"t\":", "\"\"", pick(NASTY), "{\"t\":\"hello\"}"]));
      else payload = mutate(pick(real).payload);
      if (random() < 0.5) garbage.ws.send(payload.toString("utf8"));
      else garbage.ws.send(payload);
      sentCount++;
    }
    await sleep(1_000);
    ok("E1", `${sentCount} messages illisibles ou mutés (sur ${real.length} vrais messages capturés)`);
    garbage.ws.close();
    // E2: one huge message closes that connection only.
    const huge = await connect();
    huge.ws.send("x".repeat(4 * 1024 * 1024));
    await sleep(3_000);
    if (!huge.closed) partial("E2", "un message de 4 Mio n'a pas fermé la connexion (accepté en mémoire ?)");
    else ok("E2", "un message de 4 Mio ferme sa connexion");
    try { huge.ws.close(); } catch {}
    // E3: a flood, while the others keep playing.
    const flood = await connect();
    const sample = real.length ? pick(real).payload : Buffer.from("{}");
    const floodStart = Date.now();
    let flooded = 0;
    while (Date.now() - floodStart < 3_000 && flood.ws.readyState === 1) {
      for (let i = 0; i < 500; i++) flood.ws.send(mutate(sample).toString("utf8"));
      flooded += 500;
      await sleep(10);
    }
    await sleep(500);
    const stale = [];
    for (const p of [tv, ...phones]) {
      if (!isLive(p, 6_000)) stale.push(`${p.tab.label} (rien depuis ${((Date.now() - p.tab.lastWsFrame) / 1000).toFixed(1)} s)`);
    }
    if (stale.length) fail("E3", `après ${flooded} messages en 3 s, plus de nouvelles pour : ${stale.join(", ")}`);
    else ok("E3", `${flooded} messages en 3 s : la TV et les téléphones reçoivent toujours`);
    try { flood.ws.close(); } catch {}
    // E4: a phone replays what the TV says first.
    const tvFirst = tv.sent[0];
    const tvAnswer = tv.received[0];
    if (tvFirst && tvAnswer) {
      const impostor = await connect();
      impostor.ws.send(tvFirst.binary ? tvFirst.payload : tvFirst.payload.toString("utf8"));
      await sleep(1_500);
      const answers = impostor.received.map((d) => (typeof d === "string" ? shape(Buffer.from(d), false) : shape(Buffer.from(d), true)));
      if (answers.includes(shape(tvAnswer.payload, tvAnswer.binary))) {
        if (config.spectators) partial("E4", `un appareil du réseau qui se dit TV la regarde en spectateur (permis par cert.json) : vérifie qu'il n'a aucun droit d'organisateur`);
        else fail("E4", `un téléphone qui répète le premier message de la TV reçoit l'écran de la TV (${answers[0]})`);
      }
      else ok("E4", `un téléphone qui se dit TV reçoit ${answers[0] ?? "rien"}, pas l'écran de la TV`);
      try { impostor.ws.close(); } catch {}
    } else partial("E4", "pas de message de la TV capturé");
    // Tokens: the real phone's first message with every long hex/base64 token changed.
    const hello = phones[0]?.tab.wsSent[0];
    if (hello && /[0-9a-f]{16,}/i.test(hello.payload.toString("utf8"))) {
      const forged = await connect();
      forged.ws.send(hello.payload.toString("utf8").replace(/[0-9a-f]{16,}/gi, (m) => m.split("").map(() => "0123456789abcdef"[Math.floor(random() * 16)]).join("")));
      await sleep(1_000);
      const ownAnswer = phones[0].tab.wsReceived[0];
      const answers = forged.received.map((d) => shape(Buffer.from(d), typeof d !== "string"));
      partial("E4", `jeton inventé : réponses ${answers.join(", ") || "aucune"} (le vrai téléphone a reçu ${ownAnswer ? shape(ownAnswer.payload, ownAnswer.binary) : "?"}) — vérifie qu'il n'a pris la place de personne`);
      try { forged.ws.close(); } catch {}
    }
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
    await sleep(2_000);
    host.exitedUnexpectedly = false;
    await host.start(config.relaunch, 120_000);
    const stale = [];
    for (const p of [tv, ...phones]) {
      if ((await waitLive(p, 15_000)) === null) stale.push(p.tab.label);
    }
    if (stale.length) fail("C7", `15 s après le redémarrage de l'hôte, toujours déconnectés : ${stale.join(", ")}`);
    else ok("C7", "après le redémarrage de l'hôte, la TV et tous les téléphones se sont reconnectés");
    const lost = [];
    for (const p of phones) if (!(await joined(p))) lost.push(p);
    if (lost.length) {
      const text = (await lost[0].tab.evaluate("document.body.innerText").catch(() => "")).replace(/\s+/g, " ").trim();
      partial("C7", `${lost.length} joueur(s) ont perdu leur place (partie neuve) ; ils lisent : « ${text.slice(0, 100)} »`);
    }
    await invariants("restart");
    if (!order.includes("storage")) await rejoinAll();
  },
};

// ---------------------------------------------------------------- the run

async function main() {
  host = new Host(await freePort());
  log(`${gameDir} : lancement (${config.launch.join(" ")}) sur le port ${host.port}, graine ${args.seed}`);
  await host.start(config.launch, 15 * 60_000);
  log(`prêt (pid ${host.pid ?? "?"}) ; captures dans ${outDir}`);
  toxiproxy = await Toxiproxy.start();
  browser = await launchBrowser();
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
  const runOnce = async () => {
    for (const name of order) {
      if (name === "join" && phones.length) continue;
      log(`▶ ${name}`);
      sampleMemory();
      try {
        await scenarios[name]();
      } catch (error) {
        fail("E1", `[${name}] le scénario s'est interrompu : ${error.message}`);
        if (!host.alive()) throw error;
      }
      if (name === "join" && config.start) {
        await press(tv.tab, config.start);
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
    const warm = memory.find((m) => m.t - started > 5 * 60_000) ?? memory[0];
    const lastSample = memory[memory.length - 1];
    if (warm && lastSample) {
      const growth = (lastSample.rss - warm.rss) / warm.rss;
      const line = `mémoire ${Math.round(warm.rss / 1024)} → ${Math.round(lastSample.rss / 1024)} Mio, threads ${warm.threads} → ${lastSample.threads}`;
      if (growth > 0.3 || lastSample.threads > warm.threads + 4) fail("R2", line);
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
  const mark = { ok: "✓", fail: "✗", partial: "~", skip: "–" }[r.status];
  const notes = r.status === "ok" ? r.notes.slice(-1) : r.notes.filter((n) => n).slice(0, 4);
  lines.push(`| ${item} | ${mark} | ${notes.join("<br>").replaceAll("|", "\\|")} |`);
}
lines.push("", "✓ tenu · ✗ cassé · ~ à juger par un humain, ou vérifié en partie · – non applicable", "");
if (host?.stderr.length) lines.push("Dernières lignes d'erreur de l'hôte :", "", "```", ...host.stderr.slice(-10), "```");
writeFileSync(join(outDir, "rapport.md"), lines.join("\n"));
console.log(`\n${lines.join("\n")}`);
if (crashed) console.error(crashed.stack);
process.exit(failures || crashed ? 1 : 0);
