// Timing belongs to the observer, never to a guessed timeout in a game.
// page clocks share an epoch (timeOrigin + now); raw performance.now values
// from different contexts cannot be subtracted. rAF observes presentation
// cadence, not GPU execution time or a physical display's photon latency.

export function summarizeFrames(intervals) {
  if (!intervals.length || intervals.some((x) => !Number.isFinite(x) || x <= 0)) throw new Error("aucune série d'images valide");
  const sorted = [...intervals].sort((a, b) => a - b);
  const percentile = (p) => sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1)];
  const duration = intervals.reduce((sum, x) => sum + x, 0);
  return { frames: intervals.length, duration_ms: duration, fps: 1000 * intervals.length / duration, p50_ms: percentile(0.5), p95_ms: percentile(0.95), max_ms: sorted.at(-1) };
}

export function positiveBudget(value, fallback, label) {
  const result = value ?? fallback;
  if (typeof result !== "number" || !Number.isFinite(result) || result <= 0) throw new Error(`${label} doit être un nombre positif fini`);
  return result;
}

/** rAF timestamps land on vsyncs: one skipped vsync at 60 Hz is 33.33 ms, which
 * the 33 ms budget names rounded down, and Chrome reports it as 33.3 or 33.4 ms.
 * This 1 ms is that resolution, not extra budget. Adding a whole vsync (16.7 ms)
 * instead would accept a 45 ms image, which is a real slow frame on a 120 Hz grid. */
export const FRAME_RESOLUTION_MS = 1;

export function frameFailures(sample, options = {}) {
  const max_frame_ms = positiveBudget(options.max_frame_ms, 33, "performance.max_frame_ms");
  const min_fps = positiveBudget(options.min_fps, 60, "performance.min_fps");
  const failures = [];
  if (sample.max_ms > max_frame_ms + FRAME_RESOLUTION_MS) failures.push(`image ${sample.max_ms.toFixed(1)} ms > ${max_frame_ms} ms`);
  // rAF timestamps are quantized; half an image/second is the measurement
  // resolution of the short certification sample, not a performance budget.
  if (sample.fps < min_fps - 0.5) failures.push(`${sample.fps.toFixed(1)} images/s < ${min_fps}`);
  return failures;
}

let nextFrameSample = 0;

class FrameSampleTimeout extends Error {
  constructor() { super("le rendu ne termine pas son échantillon de cadence"); }
}

async function boundedFrameReceipt(page, id) {
  let timer;
  try {
    return await Promise.race([
      page.evaluate((id) => {
        const sample = window.__certFrameSample;
        if (!sample || sample.id !== id) return { state: "replaced-or-absent" };
        return {
          state: sample.completed ? "completed" : sample.active ? "pending" : "stopped",
          reason: sample.reason,
          visibility: document.visibilityState,
          callback_count: sample.callbackCount,
          elapsed_ms: performance.now() - sample.installedAt,
          recent_callbacks: sample.callbacks,
          long_frames: sample.longFrames,
          completed: sample.completed,
        };
      }, id).catch((error) => ({ state: "unreachable", error: error.message })),
      new Promise((resolve) => { timer = setTimeout(() => resolve({ state: "unreachable" }), 250); }),
    ]);
  } finally { clearTimeout(timer); }
}

export async function measureFrames(page, durationMs = 3000, painted = null) {
  positiveBudget(durationMs, 3000, "performance.sample_ms");
  const id = ++nextFrameSample;
  const controllerStarted = Date.now();
  const observed = page.evaluate(({ duration, painted, id }) => new Promise((resolve, reject) => {
    window.__certFrameSample?.stop("superseded");
    let start;
    let previous;
    let previousPaint;
    let lastPaint;
    let leadingGap = 0;
    let frames = 0;
    const intervals = [];
    const sample = { id, active: true, idRaf: null, installedAt: performance.now(), callbackCount: 0, callbacks: [], longFrames: [], completed: null, reason: null };
    let longObserver;
    if (typeof PerformanceObserver !== "undefined" && (PerformanceObserver.supportedEntryTypes ?? []).includes("long-animation-frame")) {
      longObserver = new PerformanceObserver((list) => {
        for (const entry of list.getEntries()) {
          sample.longFrames.push({ start_ms: entry.startTime, duration_ms: entry.duration, blocking_ms: entry.blockingDuration, scripts: entry.scripts?.map((script) => ({ duration_ms: script.duration, invoker: script.invoker, source: script.sourceURL })) });
        }
        sample.longFrames = sample.longFrames.slice(-16);
      });
      longObserver.observe({ type: "long-animation-frame", buffered: false });
    }
    sample.stop = (reason = "cancelled", error = null) => {
      if (!sample.active) return;
      sample.active = false;
      sample.reason = reason;
      cancelAnimationFrame(sample.idRaf);
      longObserver?.disconnect();
      if (!sample.completed) reject(error ?? new Error(reason === "superseded" ? "échantillon de cadence remplacé" : "échantillon de cadence annulé"));
    };
    window.__certFrameSample = sample;
    function frame(now) {
      if (!sample.active) return;
      try {
        const arrived = performance.now();
        sample.callbackCount++;
        sample.callbacks.push({ frame_ms: now, callback_ms: arrived, visibility: document.visibilityState });
        if (sample.callbacks.length > 16) sample.callbacks.shift();
        if (start === undefined) { start = now; leadingGap = arrived - sample.installedAt; }
        if (painted) {
          const current = (0, eval)(painted);
          if (typeof current !== "number" || !Number.isFinite(current)) throw new Error("le compteur de présentations GPU est absent ou invalide");
          if (previousPaint === undefined) { previousPaint = current; lastPaint = now; }
          else if (current !== previousPaint) {
            intervals.push(now - lastPaint);
            frames++;
            previousPaint = current;
            lastPaint = now;
          }
        } else if (previous !== undefined) { intervals.push(now - previous); frames++; }
        previous = now;
        if (now - start >= duration) {
          const tail = painted ? now - lastPaint : 0;
          // The browser owns this receipt and closes it before resolving the
          // protocol response. A delayed Node event loop cannot turn an already
          // completed, timely measurement into a renderer timeout.
          sample.completed = { intervals, frames, duration_ms: now - start, tail_ms: tail, leading_gap_ms: leadingGap, browser_elapsed_ms: performance.now() - sample.installedAt, long_frames: [...sample.longFrames], recent_callbacks: [...sample.callbacks] };
          sample.stop("completed");
          resolve(sample.completed);
        } else sample.idRaf = requestAnimationFrame(frame);
      } catch (error) { sample.stop("error", error); }
    }
    sample.idRaf = requestAnimationFrame(frame);
  }), { duration: durationMs, painted, id });
  let timer;
  let recovered = false;
  try {
    let measured;
    try {
      measured = await Promise.race([observed, new Promise((_, reject) => {
        timer = setTimeout(() => reject(new FrameSampleTimeout()), durationMs + 3000);
      })]);
    } catch (error) {
      if (!(error instanceof FrameSampleTimeout)) throw error;
      const receipt = await boundedFrameReceipt(page, id);
      // This recovers the SAME immutable browser sample, never a retry or a
      // longer rendering budget. A late/absent browser result remains failed.
      if (receipt.completed?.browser_elapsed_ms <= durationMs + 3000) {
        measured = receipt.completed;
        recovered = true;
      } else {
        error.diagnostics = receipt;
        throw error;
      }
    }
    const intervals = [...measured.intervals];
    if (measured.leading_gap_ms > 0) intervals.push(measured.leading_gap_ms);
    // The terminal gap is still a stall if the renderer stops while the
    // observer's own rAF keeps running. It does not count as another paint.
    if (measured.tail_ms > 0) intervals.push(measured.tail_ms);
    const summary = summarizeFrames(intervals);
    return { ...summary, frames: measured.frames, duration_ms: measured.duration_ms, fps: measured.frames * 1000 / measured.duration_ms, observer: painted ? "present" : "raf", trailing_gap_ms: measured.tail_ms, leading_gap_ms: measured.leading_gap_ms, browser_elapsed_ms: measured.browser_elapsed_ms, controller_elapsed_ms: Date.now() - controllerStarted, recovered_controller_delay: recovered, long_frames: measured.long_frames, recent_callbacks: measured.recent_callbacks };
  } finally {
    clearTimeout(timer);
    let cleanupTimer;
    await Promise.race([
      page.evaluate((id) => { if (window.__certFrameSample?.id === id) window.__certFrameSample.stop(); }, id).catch(() => {}),
      new Promise((resolve) => { cleanupTimer = setTimeout(resolve, 500); }),
    ]);
    clearTimeout(cleanupTimer);
  }
}

/** One real user action, shared by the performance probe and its setup. */
export async function performAction(tab, action) {
  if (action.key !== undefined) return tab.key(action.key);
  if (action.click !== undefined) return tab.page.locator(action.click).click({ timeout: 3000 });
  if (action.fill !== undefined) return tab.fill(action.fill, action.text ?? "");
  if (action.tap !== undefined) return tab.tap(...action.tap);
  throw new Error("action de mesure absente (key, click, fill ou tap)");
}

/** A visible state projection which changes only as a result of this action.
 * DOM games can read a specific element. Canvas games publish their projected
 * value after drawing. A clock, rAF counter, or arbitrary mutation is not a
 * causal render oracle and must never be supplied as `rendered`. */
export async function measureProbe(tv, target, probe, timeoutMs = 3000) {
  const control = { cancelled: false };
  let timer;
  try {
    return await Promise.race([
      observeProbe(tv, target, probe, timeoutMs, control),
      new Promise((_, reject) => {
        timer = setTimeout(() => {
          control.cancelled = true;
          reject(new Error("la sonde entrée → rendu dépasse son délai global"));
        }, timeoutMs + 3000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
    control.cancelled = true;
    let cleanupTimer;
    await Promise.race([
      Promise.allSettled([
        target.page.evaluate(() => window.__certInputProbe?.stop()),
        tv.page.evaluate(() => window.__certRenderProbe?.stop()),
      ]),
      new Promise((resolve) => { cleanupTimer = setTimeout(resolve, 500); }),
    ]);
    clearTimeout(cleanupTimer);
  }
}

async function observeProbe(tv, target, probe, timeoutMs, control) {
  const active = () => { if (control.cancelled) throw new Error("sonde annulée"); };
  if (typeof probe.rendered !== "string" || !probe.rendered.trim()) throw new Error("performance.probes[].rendered absent");
  for (const action of probe.prepare ?? []) { active(); await performAction(target, action); }
  // Two turns settle the setup before observing the action being timed.
  await tv.page.evaluate(() => new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve))));
  active();
  await target.page.evaluate(() => {
    window.__certInputProbe?.stop();
    const events = ["keydown", "pointerdown", "touchstart", "mousedown", "click", "input", "change"];
    const probe = { at: null };
    const on = (event) => {
      if (event.isTrusted && probe.at === null) probe.at = performance.timeOrigin + performance.now();
    };
    probe.stop = () => events.forEach((type) => window.removeEventListener(type, on, true));
    window.__certInputProbe = probe;
    events.forEach((type) => window.addEventListener(type, on, true));
  });
  active();
  await tv.page.evaluate(({ expression, timeout }) => {
    window.__certRenderProbe?.stop();
    const read = () => JSON.stringify((0, eval)(expression));
    const baseline = read();
    if (baseline === undefined) throw new Error("le signal de rendu n'existe pas");
    const probe = { changes: [], baseline, active: true, error: null };
    probe.stop = () => { probe.active = false; clearTimeout(probe.timer); };
    probe.timer = setTimeout(probe.stop, timeout);
    function frame() {
      if (!probe.active) return;
      try {
        const token = read();
        if (token !== baseline && probe.changes.length < 32) {
          // A second animation turn means the previously observed DOM/canvas
          // state has crossed a rendering opportunity, rather than merely
          // having changed in the simulation before it was drawn.
          requestAnimationFrame(() => {
            if (probe.active) probe.changes.push({ at: performance.timeOrigin + performance.now(), token });
          });
        }
      } catch (error) { probe.error = error.message; }
      requestAnimationFrame(frame);
    }
    window.__certRenderProbe = probe;
    requestAnimationFrame(frame);
  }, { expression: probe.rendered, timeout: timeoutMs + 1000 });
  active();
  {
    await performAction(target, probe.action ?? {});
    active();
    const input = await target.page.evaluate(() => window.__certInputProbe.at);
    if (!Number.isFinite(input)) throw new Error("aucune entrée utilisateur réelle observée");
    const result = await tv.page.waitForFunction((at) => {
      const observed = window.__certRenderProbe;
      if (observed.error) throw new Error(observed.error);
      return observed.changes.find((change) => change.at >= at) ?? false;
    }, input, { timeout: timeoutMs, polling: "raf" });
    const changed = await result.jsonValue();
    await result.dispose();
    return { latency_ms: changed.at - input, screen: probe.screen ?? "phone", baseline: await tv.page.evaluate(() => window.__certRenderProbe.baseline), rendered: changed.token };
  }
}

/** Waits for an action's effect to end: the value came back, as after a jump,
 * or it stopped moving, as a score does. A value that never rests is a failure. */
export async function renderSettled(tv, expression, baseline, { quietMs = 1500, timeoutMs = 3000 } = {}) {
  await tv.page.evaluate(() => { delete window.__certSettle; });
  const settled = await tv.page.waitForFunction(({ expression, baseline, quietMs }) => {
    const value = JSON.stringify((0, eval)(expression));
    const now = performance.now();
    const state = window.__certSettle ??= { value, since: now };
    if (value !== state.value) Object.assign(state, { value, since: now });
    return value === baseline || now - state.since >= quietMs;
  }, { expression, baseline, quietMs }, { timeout: timeoutMs, polling: "raf" });
  await settled.dispose();
}

/** Match a durable panic report to one deliberate target injection. */
export function panicMatches(report, nonce, page) {
  return report?.page === page
    && Number.isInteger(report.attempt) && report.attempt > 0
    && typeof report.message === "string"
    && report.message.split(/\r?\n/).includes(`salon certification panic ${nonce}`);
}

/** Stop background actors even if a browser command never answers. */
export async function stopTasks(tasks, abort, graceMs = 5000) {
  let timer;
  const stopped = await Promise.race([
    Promise.allSettled(tasks).then((results) => results.every((result) => result.status === "fulfilled")),
    new Promise((resolve) => { timer = setTimeout(() => resolve(false), graceMs); }),
  ]);
  clearTimeout(timer);
  if (!stopped) {
    let abortTimer;
    await Promise.race([
      Promise.resolve().then(abort).catch(() => {}),
      new Promise((resolve) => { abortTimer = setTimeout(resolve, 500); }),
    ]);
    clearTimeout(abortTimer);
  }
  return stopped;
}

/** Only a nonce-correlated injected WASM panic may consume one trap. */
export function injectedPanicRemainder(errors, nonce, recorded) {
  if (!recorded) return [...errors];
  let trap = 1;
  let notice = 1;
  return errors.filter((error) => {
    if (trap && /^exception : (?:RuntimeError: )?(?:unreachable\b|unreachable code should not be executed\b)/i.test(error)) {
      trap--;
      return false;
    }
    if (notice && error.startsWith("console.error : ") && error.split(/\r?\n| \| /).some((part) => part.trim().endsWith(`salon certification panic ${nonce}`))) {
      notice--;
      return false;
    }
    return true;
  });
}

/** A phone's on-screen Backspace exists only while editing text. Desktop
 * keyboards keep Backspace, including games which bind it as an action. */
export function gestureKeys({ mobile, textFocus }) {
  const keys = ["Enter", "Escape", "Backspace", "Tab", "ArrowLeft", "ArrowRight", " "];
  return mobile && !textFocus ? keys.filter((key) => key !== "Backspace") : keys;
}

/** Once the harness destroys part of its fixture, later observations cannot
 * certify the product. Keep earlier evidence and terminate with that cause. */
export class FixtureFailure extends Error {
  constructor(reason) {
    super(reason);
    this.name = "FixtureFailure";
  }
}

/** A failed transport exercise belongs to its declared C1 contract, even
 * when it throws before recording a result. Controller failures stay H1. */
export function scenarioFailureItem(scenario, error) {
  if (error instanceof FixtureFailure) return "H1";
  if (scenario === "rtc") return "C1";
  if (scenario === "capacity") return "R3";
  if (scenario === "text") return "T2";
  if (scenario === "layout") return "T3";
  if (scenario === "performance") return "P1";
  if (scenario === "jitter") return "R1";
  return scenario === "ui" ? "U1" : "E1";
}

export class FixtureGuard {
  constructor() { this.failure = null; }
  invalidate(reason) { this.failure ??= new FixtureFailure(reason); }
  check() { if (this.failure) throw this.failure; }
}

/** Primary-interface ordering may change without losing the tested address. */
export function networkAddressPresent(address, interfaces) {
  return Object.values(interfaces).flat().some(entry => entry && !entry.internal && entry.address === address);
}
