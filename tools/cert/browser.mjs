// The TV uses the installed Chrome; phones alternate WebKit and Chromium.
// Each phone owns its storage, like a separate device. CDP is confined to
// Chromium: WebKit uses browser APIs for gestures and init scripts. The true
// JavaScript freeze scenario deliberately uses a Chromium phone.

import { chromium, webkit } from "playwright-core";

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

const IPHONE = {
  viewport: { width: 390, height: 844 },
  deviceScaleFactor: 3,
  isMobile: true,
  hasTouch: true,
  userAgent:
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
};

// Frames drawn since the page loaded: a TV that stops drawing is frozen.
const FRAME_COUNTER = "window.__certRendered = null; window.__certPaints = 0; window.addEventListener('soiree:rendered', (event) => { window.__certRendered = event.detail; window.__certPaints++; }); window.__certFrames = 0; (function f() { window.__certFrames++; requestAnimationFrame(f); })();";

/** Observe native WebSocket API calls before app scripts. Playwright's
 * WebKit websocket event is emitted only after a blocked upgrade eventually
 * completes/fails; it cannot date the page's connection attempts. */
export function observeWebSockets() {
  const Native = globalThis.WebSocket;
  const close = Native.prototype.close;
  const ids = new WeakMap();
  const trace = { next: 1, sequence: 0, events: [] };
  Object.defineProperty(globalThis, "__certSocketTrace", { value: trace, configurable: true });
  const stamp = () => performance.timeOrigin + performance.now();
  const emit = (event) => {
    const order = trace.sequence++;
    if (trace.events.length < 4096) trace.events.push({ ...event, order });
  };
  Object.defineProperty(Native.prototype, "close", {
    ...Object.getOwnPropertyDescriptor(Native.prototype, "close"),
    value: function (...args) {
      emit({ kind: "close-call", id: ids.get(this) ?? null, at: stamp(), ready: this.readyState });
      return Reflect.apply(close, this, args);
    },
  });
  const Wrapped = new Proxy(Native, {
    construct(target, args, newTarget) {
      const at = stamp();
      const socket = Reflect.construct(target, args, newTarget === Wrapped ? target : newTarget);
      const id = trace.next++;
      ids.set(socket, id);
      emit({ kind: "construct", id, at, ready: socket.readyState, url: String(args[0]) });
      for (const kind of ["open", "error", "close"]) {
        socket.addEventListener(kind, (event) => emit({ kind: `${kind}-event`, id, at: stamp(), ready: socket.readyState, code: event.code ?? null, clean: event.wasClean ?? null }));
      }
      return socket;
    },
  });
  globalThis.WebSocket = Wrapped;
}

/** C3 proves an independent page reaction, never a reaction to a server
 * close: OPEN at the explicit close() call, before any close/error event,
 * followed by a constructor attempt to the same endpoint within budget. */
export function silenceReaction(events, originalIds, started, budget = 20_000) {
  for (const id of originalIds) {
    const created = events.find((event) => event.kind === "construct" && event.id === id);
    if (!created) continue;
    const own = events.find((event) => event.kind === "close-call" && event.id === id && event.ready === 1 && event.at >= started && event.at - started <= budget);
    if (!own) continue;
    if (events.some((event) => event.id === id && (event.kind === "close-event" || event.kind === "error-event") && event.order <= own.order)) continue;
    const retry = events.find((event) => event.kind === "construct" && event.id !== id && event.url === created.url && event.order > own.order && event.at - started <= budget);
    if (retry) return { independent: true, close: own, retry };
  }
  return { independent: false };
}

/** `live(payload)` says whether a received frame shows the game talking to
 * this page (salon's Welcome, or a packet of the game), rather than a refusal
 * or a goodbye: a page only refused is not connected. `remember(payload)`
 * picks the sent frames the harness needs later even after the first 300
 * (salon's Hello, which carries the device's token): the last one is kept. */
export async function launchBrowser({ live = () => true, remember = () => false, identify = () => null, webkitPhones = true } = {}) {
  const chrome = await chromium.launch({
    channel: process.env.CHROME ? undefined : "chrome",
    executablePath: process.env.CHROME,
    headless: true,
    // The harness stops the browser itself, after the game (cert.mjs, cleanup).
    handleSIGINT: false,
    handleSIGTERM: false,
    handleSIGHUP: false,
    // Every phone is "in the foreground": background tabs keep their timers.
    args: ["--disable-background-timer-throttling", "--disable-renderer-backgrounding", "--disable-backgrounding-occluded-windows", "--mute-audio"],
  });
  let safari;
  try {
    if (webkitPhones) safari = await webkit.launch({ headless: true, handleSIGINT: false, handleSIGTERM: false, handleSIGHUP: false });
  } catch (error) {
    await chrome.close();
    throw new Error(`WebKit absent ou inutilisable : soiree doctor (installer avec npx playwright-core install webkit). ${error.message}`);
  }
  return {
    /** A new page, in a fresh context unless `context` is given (a second tab of the same phone). */
    async open(url, { width = 1280, height = 720, mobile = false, context = undefined, engine = "chromium" } = {}) {
      const selected = engine === "webkit" ? safari : chrome;
      if (!context && !selected) throw new Error(`moteur ${engine} non lancé`);
      const ctx = context ?? (await selected.newContext(mobile ? IPHONE : { viewport: { width, height } }));
      if (!context) {
        await ctx.addInitScript(FRAME_COUNTER);
        await ctx.addInitScript(observeWebSockets);
      }
      const actualEngine = ctx.browser().browserType().name();
      const tab = new Tab(ctx, await ctx.newPage(), mobile, live, remember, actualEngine, identify);
      await tab.attach();
      if (url) await tab.goto(url);
      return tab;
    },
    close: async () => { await Promise.all([chrome.close(), safari?.close()]); },
  };
}

/** One page, with what it logged and what its WebSockets carried. */
export class Tab {
  constructor(context, page, mobile, live = () => true, remember = () => false, engine = "chromium", identify = () => null) {
    this.identify = identify;
    this.device = null;
    this.welcomes = 0;
    this.engine = engine;
    this.live = live;
    this.remember = remember;
    /** The last sent frame that `remember` picked. */
    this.remembered = null;
    this.context = context;
    this.page = page;
    this.mobile = mobile;
    this.label = "";
    /** Uncaught exceptions and console.error calls, in order. */
    this.errors = [];
    /** WebSocket messages the page sent and received, and when the last one came. */
    this.wsSent = [];
    this.wsReceived = [];
    this.lastWsFrame = 0;
    /** When the last frame that `live` accepted came, and the last frame received. */
    this.lastLiveFrame = 0;
    this.lastReceived = null;
    /** WebSockets the page opened: a page that keeps reopening is fighting. */
    this.sockets = 0;
    /** Each WebSocket's opening and closing times, in order. */
    this.socketTimes = [];
  }

  /** Native call/event timeline of this document, including connection
   * attempts whose upgrade has not reached a Playwright websocket event. */
  socketTrace() {
    return this.evaluate("globalThis.__certSocketTrace?.events ?? []", 3_000);
  }

  async attach() {
    const { page } = this;
    page.on("pageerror", (error) => this.errors.push(`exception : ${String(error.stack ?? error).split("\n").slice(0, 4).join(" | ")}`));
    page.on("console", (message) => {
      if (message.type() !== "error" && message.type() !== "assert") return;
      // The browser's own network reports: expected while the harness cuts the Wi-Fi.
      if (/^Failed to load resource|^WebSocket connection to .* failed/.test(message.text())) return;
      this.errors.push(`console.${message.type()} : ${message.text().slice(0, 300)}`);
    });
    const payload = (data) => ({ payload: Buffer.isBuffer(data) ? data : Buffer.from(data), binary: Buffer.isBuffer(data) });
    page.on("websocket", (ws) => {
      this.sockets++;
      const times = { openedAt: Date.now(), closedAt: null };
      this.socketTimes.push(times);
      ws.on("close", () => (times.closedAt = Date.now()));
      ws.on("framereceived", ({ payload: data }) => {
        this.lastWsFrame = Date.now();
        const frame = payload(data);
        this.lastReceived = frame;
        const device = this.identify(frame.payload);
        if (Number.isInteger(device)) { this.device = device; this.welcomes++; }
        if (this.live(frame.payload)) this.lastLiveFrame = this.lastWsFrame;
        if (this.wsReceived.length < 50) this.wsReceived.push(frame);
      });
      ws.on("framesent", ({ payload: data }) => {
        const frame = payload(data);
        if (this.remember(frame.payload)) this.remembered = frame;
        if (this.wsSent.length < 300) this.wsSent.push(frame);
      });
    });
    this.cdp = this.engine === "chromium" ? await this.context.newCDPSession(page) : null;
  }

  /** A raw DevTools call, for what Playwright does not expose. */
  call(method, params = {}) {
    if (!this.cdp) throw new Error(`${method} nécessite Chromium (page ${this.engine})`);
    return this.cdp.send(method, params);
  }

  async goto(url) {
    await this.page.goto(url, { waitUntil: "load", timeout: 15_000 }).catch(() => {});
  }

  async reload() {
    await this.page.reload({ waitUntil: "load", timeout: 15_000 }).catch(() => {});
  }

  /** Restore fixture storage in the next document, before its application
   * scripts read it. Removing an init script cannot undo getters which the
   * old document already installed (for example, forbidden storage). */
  async reloadWithStorage(entries) {
    const remove = await this.beforeScripts(`for (const [key, value] of ${JSON.stringify(entries)}) localStorage.setItem(key, value);`);
    try {
      await this.page.reload({ waitUntil: "load", timeout: 15_000 });
    } finally {
      await remove();
    }
  }

  /** Evaluates `expression` in the page; exceptions come back as errors. */
  evaluate(expression, timeoutMs = 10_000) {
    return Promise.race([
      this.page.evaluate(expression),
      sleep(timeoutMs).then(() => {
        throw new Error(`évaluation de plus de ${timeoutMs} ms`);
      }),
    ]);
  }

  /** Waits until `expression` is truthy in the page; the time it took, or null. */
  async waitFor(expression, timeoutMs = 10_000) {
    const start = Date.now();
    while (Date.now() - start < timeoutMs) {
      try {
        if (await this.evaluate(`Boolean(${expression})`, 2_000)) return Date.now() - start;
      } catch {}
      await sleep(100);
    }
    return null;
  }

  /** Replaces the text of the field `selector`, as typing would. */
  fill(selector, text) {
    return this.input("fill", () => this.page.fill(selector, text, { timeout: 10_000 }));
  }

  /** Native-driver stage, retained when a protocol command stops answering. */
  async input(operation, task) {
    const pending = { operation, started: Date.now(), url: this.page.url() };
    this.pendingInput = pending;
    try {
      const result = await task();
      this.lastInput = { ...pending, finished: Date.now(), status: "completed" };
      this.inputHistory ??= [];
      this.inputHistory.push(this.lastInput);
      if (this.inputHistory.length > 32) this.inputHistory.shift();
      return result;
    } catch (error) {
      this.lastInput = { ...pending, finished: Date.now(), status: "failed", error: error.message };
      this.inputHistory ??= [];
      this.inputHistory.push(this.lastInput);
      if (this.inputHistory.length > 32) this.inputHistory.shift();
      throw error;
    } finally {
      if (this.pendingInput === pending) this.pendingInput = null;
    }
  }

  key(key) {
    return this.input(`keyboard.press ${key}`, () => this.page.keyboard.press(key === " " ? "Space" : key));
  }

  type(text) {
    return this.input("keyboard.insertText", () => this.page.keyboard.insertText(text));
  }

  /** A finger on a phone, the mouse elsewhere. */
  tap(x, y) {
    return this.mobile
      ? this.input("touchscreen.tap", () => this.page.touchscreen.tap(x, y))
      : this.input("mouse.click", () => this.page.mouse.click(x, y));
  }

  click(x, y) {
    return this.input("mouse.click", () => this.page.mouse.click(x, y));
  }

  /** A mouse drag from one point to another (drag and drop, sliders). */
  async drag(from, to) {
    try {
      await this.input("drag.mouse.move.start", () => this.page.mouse.move(from.x, from.y));
      await this.input("drag.mouse.down", () => this.page.mouse.down());
      await this.input("drag.mouse.move.steps", () => this.page.mouse.move(to.x, to.y, { steps: 8 }));
    } finally {
      // The native driver remembers pressed buttons independently of page JS.
      // A rejected move must not leave the next gesture dragging already.
      await this.input("drag.mouse.up", () => this.page.mouse.up());
    }
  }

  /** A finger that slides (sticks, canvases, pads). */
  async swipe(points) {
    if (!points.length) return;
    if (!this.cdp) {
      // WebKit has no public CDP touch dispatcher. A real touch start/end
      // followed by mouse dragging still exercises pointer-driven controls;
      // this is explicitly not an iOS lifecycle simulation.
      await this.input("swipe.touchscreen.tap", () => this.page.touchscreen.tap(points[0].x, points[0].y));
      await this.drag(points[0], points.at(-1));
      return;
    }
    try {
      await this.input("swipe.cdp.touchStart", () => this.call("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [points[0]] }));
      for (const point of points.slice(1)) await this.input("swipe.cdp.touchMove", () => this.call("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [point] }));
    } finally {
      await this.input("swipe.cdp.touchEnd", () => this.call("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] }));
    }
  }

  resize(width, height) {
    return this.page.setViewportSize({ width, height });
  }

  /** Freezes the page as iOS does when the phone locks: no JavaScript runs,
   * not even timers. The debugger pauses it; `Page.setWebLifecycleState`
   * leaves a visible page's timers running. */
  async freeze() {
    await this.call("Debugger.enable");
    await this.call("Debugger.pause");
  }

  async unfreeze() {
    await this.call("Debugger.resume").catch(() => {});
    await this.call("Debugger.disable").catch(() => {});
  }

  /** Runs `source` before the page's own scripts, until the returned function is called. */
  async beforeScripts(source) {
    const installed = await this.page.addInitScript(source);
    return () => installed.dispose();
  }

  screenshot() {
    return this.page.screenshot({ timeout: 20_000 });
  }

  close() {
    return this.page.close().catch(() => {});
  }
}
