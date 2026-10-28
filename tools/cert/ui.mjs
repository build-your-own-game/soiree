// Declarative UI continuity checks. Assertions are independent observations
// supplied by cert.json; the runner knows no game's state or DOM vocabulary.
import { performAction } from "./performance.mjs";
import { spawn } from "node:child_process";

export const UI_CATEGORIES = ["editing", "effects", "lifecycle", "async"];

const WAIT_TIMEOUT_MS = 5000;
const WAIT_POLL_MS = 20;

function commandTimeout(check) {
  const timeout = check.timeout_ms === undefined ? 120_000 : check.timeout_ms;
  if (!Number.isSafeInteger(timeout) || timeout <= 0 || timeout > 2_147_483_647) throw new Error("ui.checks[].timeout_ms doit être un entier positif fini compatible avec un délai Node");
  return timeout;
}

async function waitForObservation(page, expression, name) {
  const expiresAt = performance.now() + WAIT_TIMEOUT_MS;
  const timeoutError = () => new Error(`ui « ${name} » : attente non satisfaite après ${WAIT_TIMEOUT_MS} ms : ${expression}`);
  let active = true;
  let deadline;
  let pollTimer;
  const timeout = new Promise((_, reject) => {
    deadline = setTimeout(() => {
      active = false;
      reject(timeoutError());
    }, WAIT_TIMEOUT_MS);
  });
  const poll = async () => {
    while (active) {
      if (performance.now() >= expiresAt) throw timeoutError();
      // page.evaluate resolves the predicate's Promise before returning its
      // boolean. A Promise object itself must never satisfy the observation.
      const passed = await page.evaluate(async (source) => Boolean(await (0, eval)(source)), expression);
      if (!active) return;
      if (performance.now() >= expiresAt) throw timeoutError();
      if (passed === true) return;
      await new Promise((resolve) => { pollTimer = setTimeout(resolve, WAIT_POLL_MS); });
    }
  };
  try {
    await Promise.race([poll(), timeout]);
  } finally {
    active = false;
    clearTimeout(deadline);
    clearTimeout(pollTimer);
  }
}

function successPattern(check) {
  if (check.success === undefined) return null;
  if (typeof check.success !== "string" || !check.success.trim()) throw new Error("ui.checks[].success doit être une expression régulière non vide");
  try { return new RegExp(check.success); }
  catch (error) { throw new Error(`ui.checks[].success invalide : ${error.message}`); }
}

export function validateUi(ui) {
  if (!ui || typeof ui !== "object") throw new Error("cert.json doit décrire ui.scenarios et ui.not_applicable");
  const covered = new Set();
  for (const scenario of ui.scenarios ?? []) {
    if (!scenario.name || !["tv", "phone"].includes(scenario.screen)) throw new Error("ui.scenarios : name et screen requis");
    if (!Array.isArray(scenario.covers) || !scenario.covers.length) throw new Error("ui.scenarios[].covers absent");
    for (const category of scenario.covers) {
      if (!UI_CATEGORIES.includes(category)) throw new Error(`ui : catégorie inconnue ${category}`);
      covered.add(category);
    }
    if (!Array.isArray(scenario.steps) || !scenario.steps.some((step) => step.assert || step.unchanged)) throw new Error("ui.scenarios[].steps doit observer une assertion");
  }
  for (const check of ui.checks ?? []) {
    successPattern(check);
    commandTimeout(check);
    if (!check.name || !Array.isArray(check.command) || !check.command.length || check.command.some((arg) => typeof arg !== "string" || !arg)) throw new Error("ui.checks : name et command requis");
    if (!Array.isArray(check.covers) || !check.covers.length) throw new Error("ui.checks[].covers absent");
    for (const category of check.covers) {
      if (!UI_CATEGORIES.includes(category)) throw new Error(`ui : catégorie inconnue ${category}`);
      covered.add(category);
    }
  }
  for (const category of UI_CATEGORIES) {
    const reason = ui.not_applicable?.[category];
    if (covered.has(category) && reason) throw new Error(`ui : ${category} est couvert et déclaré non applicable`);
    if (!covered.has(category) && (typeof reason !== "string" || !reason.trim())) throw new Error(`ui : ${category} doit être exercé ou explicitement non applicable`);
  }
}

/** Run actual game regression suites against the host, keeping the outcome
 * and terminating their entire process group, including on timeout. */
export async function runUiCommand(check, { cwd, url, tmp }) {
  const success = successPattern(check);
  const timeout = commandTimeout(check);
  const command = check.command.map((argument) => argument.replaceAll("{url}", url));
  const child = spawn(command[0], command.slice(1), {
    cwd, detached: process.platform !== "win32", stdio: ["ignore", "pipe", "pipe"],
    env: { ...process.env, SOIREE_CERT_BROWSER_MODULE: new URL("./browser.mjs", import.meta.url).href, SOIREE_CERT_URL: url, ...(tmp ? { SOIREE_CERT_TMP: tmp } : {}) },
  });
  let output = "";
  for (const stream of [child.stdout, child.stderr]) stream.on("data", (bytes) => { output = (output + bytes.toString()).slice(-64_000); });
  const kill = (signal) => {
    try {
      if (process.platform === "win32") child.kill(signal);
      else if (child.pid) process.kill(-child.pid, signal);
    } catch (error) { if (error.code !== "ESRCH") throw error; }
  };
  let expired = false;
  let hardStop;
  const timer = setTimeout(() => {
    expired = true;
    kill("SIGTERM");
    hardStop = setTimeout(() => kill("SIGKILL"), 1000);
  }, timeout);
  try {
    const code = await new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("close", resolve);
    });
    if (expired || code !== 0) throw new Error(`ui « ${check.name} » : ${expired ? "délai dépassé" : `sortie ${code}`}\n${output}`);
    if (success && !success.test(output)) throw new Error(`ui « ${check.name} » : sortie réussie sans la preuve attendue (${check.success})\n${output}`);
    return output.trim();
  } finally {
    clearTimeout(timer);
    clearTimeout(hardStop);
    kill("SIGKILL");
  }
}

export async function runUiScenario(tab, scenario) {
  const { page } = tab;
  await page.evaluate(() => { window.__certUiRemembered = new Map(); });
  try {
    for (const step of scenario.steps) {
      if (step.wait) {
        await waitForObservation(page, step.wait, scenario.name);
      } else if (step.focus) {
        await page.locator(step.focus).focus({ timeout: 3000 });
      } else if (step.select) {
        await page.locator(step.select).evaluate((element, [start, end]) => {
          if (typeof element.setSelectionRange === "function") element.setSelectionRange(start, end);
          else {
            if (!element.firstChild) element.appendChild(document.createTextNode(""));
            window.getSelection().setBaseAndExtent(element.firstChild, start, element.firstChild, end);
          }
        }, [step.start, step.end]);
      } else if (step.composition) {
        await page.locator(step.composition).evaluate((element, detail) => {
          const type = { start: "compositionstart", update: "compositionupdate", end: "compositionend" }[detail.phase];
          if (!type) throw new Error("phase de composition inconnue");
          if (detail.phase !== "start") {
            if (element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) element.value = detail.text ?? "";
            else element.textContent = detail.text ?? "";
          }
          element.dispatchEvent(new CompositionEvent(type, { bubbles: true, data: detail.text ?? "" }));
          if (detail.phase !== "start") {
            element.dispatchEvent(new InputEvent("input", { bubbles: true, data: detail.text ?? "", inputType: detail.phase === "end" ? "insertText" : "insertCompositionText", isComposing: detail.phase !== "end" }));
          }
        }, step);
      } else if (step.composing_key) {
        await page.locator(step.composing_key).dispatchEvent("keydown", { key: step.key, isComposing: true, keyCode: 229, bubbles: true, cancelable: true });
      } else if (step.dispatch) {
        if (!["click", "input", "change", "blur"].includes(step.event)) throw new Error(`événement ui non admis : ${step.event}`);
        await page.evaluate(({ selectors, event }) => {
          for (const selector of selectors) {
            const element = document.querySelector(selector);
            if (!element) throw new Error(`cible ui absente : ${selector}`);
            element.dispatchEvent(new Event(event, { bubbles: true }));
          }
        }, { selectors: Array.isArray(step.dispatch) ? step.dispatch : [step.dispatch], event: step.event });
      } else if (step.remember) {
        await page.evaluate(({ key, expression }) => {
          const value = (0, eval)(expression);
          if (value === undefined) throw new Error(`observation absente : ${key}`);
          window.__certUiRemembered.set(key, value);
        }, { key: step.remember, expression: step.read });
      } else if (step.unchanged) {
        const same = await page.evaluate(({ key, expression }) => {
          if (!window.__certUiRemembered.has(key)) throw new Error(`observation inconnue : ${key}`);
          const before = window.__certUiRemembered.get(key);
          const after = (0, eval)(expression);
          return Object.is(before, after);
        }, { key: step.unchanged, expression: step.read });
        if (!same) throw new Error(`ui « ${scenario.name} » : ${step.unchanged} a changé`);
      } else if (step.assert) {
        if (!await page.evaluate(async (expression) => Boolean(await (0, eval)(expression)), step.assert)) throw new Error(`ui « ${scenario.name} » : assertion fausse : ${step.assert}`);
      } else {
        await performAction(tab, step);
      }
    }
  } finally {
    // A stopped page must not turn an observation deadline into an unbounded
    // wait while cleaning up the observer's own bookkeeping.
    let cleanupTimer;
    try {
      await Promise.race([
        page.evaluate(() => { delete window.__certUiRemembered; }).catch(() => {}),
        new Promise((resolve) => { cleanupTimer = setTimeout(resolve, 500); }),
      ]);
    } finally { clearTimeout(cleanupTimer); }
  }
}
