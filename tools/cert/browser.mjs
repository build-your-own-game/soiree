// The TV and the phones, as pages of the Chrome installed on this computer,
// driven by Playwright (`playwright-core`: no browser to download). Each phone
// gets its own browser context, so its storage (token, saved state) is its
// own, as on a separate device. What Playwright does not expose (touch moves,
// freezing a page as iOS does) goes through the page's DevTools session.

import { chromium } from "playwright-core";

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
const FRAME_COUNTER = "window.__certFrames = 0; (function f() { window.__certFrames++; requestAnimationFrame(f); })();";

export async function launchBrowser() {
  const browser = await chromium.launch({
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
  return {
    /** A new page, in a fresh context unless `context` is given (a second tab of the same phone). */
    async open(url, { width = 1280, height = 720, mobile = false, context = undefined } = {}) {
      const ctx = context ?? (await browser.newContext(mobile ? IPHONE : { viewport: { width, height } }));
      if (!context) await ctx.addInitScript(FRAME_COUNTER);
      const tab = new Tab(ctx, await ctx.newPage(), mobile);
      await tab.attach();
      if (url) await tab.goto(url);
      return tab;
    },
    close: () => browser.close(),
  };
}

/** One page, with what it logged and what its WebSockets carried. */
export class Tab {
  constructor(context, page, mobile) {
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
    /** WebSockets the page opened: a page that keeps reopening is fighting. */
    this.sockets = 0;
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
      ws.on("framereceived", ({ payload: data }) => {
        this.lastWsFrame = Date.now();
        if (this.wsReceived.length < 50) this.wsReceived.push(payload(data));
      });
      ws.on("framesent", ({ payload: data }) => {
        if (this.wsSent.length < 300) this.wsSent.push(payload(data));
      });
    });
    this.cdp = await this.context.newCDPSession(page);
  }

  /** A raw DevTools call, for what Playwright does not expose. */
  call(method, params = {}) {
    return this.cdp.send(method, params);
  }

  async goto(url) {
    await this.page.goto(url, { waitUntil: "load", timeout: 15_000 }).catch(() => {});
  }

  async reload() {
    await this.page.reload({ waitUntil: "load", timeout: 15_000 }).catch(() => {});
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
    return this.page.fill(selector, text, { timeout: 10_000 });
  }

  key(key) {
    return this.page.keyboard.press(key === " " ? "Space" : key);
  }

  type(text) {
    return this.page.keyboard.insertText(text);
  }

  /** A finger on a phone, the mouse elsewhere. */
  tap(x, y) {
    return this.mobile ? this.page.touchscreen.tap(x, y) : this.page.mouse.click(x, y);
  }

  click(x, y) {
    return this.page.mouse.click(x, y);
  }

  /** A mouse drag from one point to another (drag and drop, sliders). */
  async drag(from, to) {
    await this.page.mouse.move(from.x, from.y);
    await this.page.mouse.down();
    await this.page.mouse.move(to.x, to.y, { steps: 8 });
    await this.page.mouse.up();
  }

  /** A finger that slides (sticks, canvases, pads). */
  async swipe(points) {
    await this.call("Input.dispatchTouchEvent", { type: "touchStart", touchPoints: [points[0]] });
    for (const point of points.slice(1)) await this.call("Input.dispatchTouchEvent", { type: "touchMove", touchPoints: [point] });
    await this.call("Input.dispatchTouchEvent", { type: "touchEnd", touchPoints: [] });
  }

  resize(width, height) {
    return this.page.setViewportSize({ width, height });
  }

  /** Freezes the page as iOS does when the phone locks: no JavaScript runs. */
  freeze() {
    return this.call("Page.setWebLifecycleState", { state: "frozen" });
  }

  unfreeze() {
    return this.call("Page.setWebLifecycleState", { state: "active" });
  }

  /** Runs `source` before the page's own scripts, until the returned function is called. */
  async beforeScripts(source) {
    const { identifier } = await this.call("Page.addScriptToEvaluateOnNewDocument", { source });
    return () => this.call("Page.removeScriptToEvaluateOnNewDocument", { identifier });
  }

  screenshot() {
    return this.page.screenshot({ timeout: 20_000 });
  }

  close() {
    return this.page.close().catch(() => {});
  }
}
