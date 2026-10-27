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

export function frameFailures(sample, options = {}) {
  const max_frame_ms = positiveBudget(options.max_frame_ms, 33, "performance.max_frame_ms");
  const min_fps = positiveBudget(options.min_fps, 60, "performance.min_fps");
  const failures = [];
  if (sample.max_ms > max_frame_ms) failures.push(`image ${sample.max_ms.toFixed(1)} ms > ${max_frame_ms} ms`);
  // rAF timestamps are quantized; half an image/second is the measurement
  // resolution of the short certification sample, not a performance budget.
  if (sample.fps < min_fps - 0.5) failures.push(`${sample.fps.toFixed(1)} images/s < ${min_fps}`);
  return failures;
}

export async function measureFrames(page, durationMs = 3000, painted = null) {
  positiveBudget(durationMs, 3000, "performance.sample_ms");
  const observed = page.evaluate(({ duration, painted }) => new Promise((resolve, reject) => {
    window.__certFrameSample?.stop();
    let start;
    let previous;
    let previousPaint;
    let lastPaint;
    let frames = 0;
    const intervals = [];
    const sample = { active: true, id: null };
    sample.stop = () => { sample.active = false; cancelAnimationFrame(sample.id); };
    window.__certFrameSample = sample;
    function frame(now) {
      if (!sample.active) return;
      try {
        start ??= now;
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
          sample.stop();
          const tail = painted ? now - lastPaint : 0;
          resolve({ intervals, frames, duration_ms: now - start, tail_ms: tail });
        } else sample.id = requestAnimationFrame(frame);
      } catch (error) { sample.stop(); reject(error); }
    }
    sample.id = requestAnimationFrame(frame);
  }), { duration: durationMs, painted });
  let timer;
  try {
    const measured = await Promise.race([observed, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error("le rendu ne termine pas son échantillon de cadence")), durationMs + 3000);
    })]);
    // The terminal gap is still a stall if the renderer stops while the
    // observer's own rAF keeps running. It does not count as another paint.
    const intervals = measured.tail_ms > 0 ? [...measured.intervals, measured.tail_ms] : measured.intervals;
    const summary = summarizeFrames(intervals);
    return { ...summary, frames: measured.frames, duration_ms: measured.duration_ms, fps: measured.frames * 1000 / measured.duration_ms, observer: painted ? "present" : "raf", trailing_gap_ms: measured.tail_ms };
  } finally {
    clearTimeout(timer);
    let cleanupTimer;
    await Promise.race([
      page.evaluate(() => window.__certFrameSample?.stop()).catch(() => {}),
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

export class FixtureGuard {
  constructor() { this.failure = null; }
  invalidate(reason) { this.failure ??= new FixtureFailure(reason); }
  check() { if (this.failure) throw this.failure; }
}
