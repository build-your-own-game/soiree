// How busy the machine is, measured before the harness starts anything and
// after it has stopped everything, so that its own browsers do not count.
//
// A compile running beside a certification delays clicks and frames without
// any error: the run fails, or passes, with no sign of why. This never changes
// a verdict. It tells the reader which timings not to trust.
//
// Two short windows do not see a load that came and went between them. The load
// average is kept as context only: it still remembers the build `soiree certify`
// has just finished, and would raise this warning on every run.

import { cpus, loadavg } from "node:os";

/** Half the cores busy elsewhere: a parallel compile reaches this, a browser tab does not. */
export const LOADED_CPU_SHARE = 0.5;
const WINDOW_MS = 2000;

const busy = ({ user, nice, sys, irq }) => user + nice + sys + irq;
const total = (times) => busy(times) + times.idle;

/** The busy share of all cores between two readings of `os.cpus()`, or null if they say nothing. */
export function busyShare(before, after) {
  if (!before.length || before.length !== after.length) return null;
  let busyTicks = 0;
  let allTicks = 0;
  before.forEach((cpu, index) => {
    busyTicks += busy(after[index].times) - busy(cpu.times);
    allTicks += total(after[index].times) - total(cpu.times);
  });
  return allTicks > 0 ? busyTicks / allTicks : null;
}

export async function measureLoad({ windowMs = WINDOW_MS, read = cpus, average = loadavg, wait = (ms) => new Promise((resolve) => setTimeout(resolve, ms)) } = {}) {
  const before = read();
  await wait(windowMs);
  const after = read();
  return { cores: after.length, cpu_share: busyShare(before, after), load_1m: average()[0] };
}

export function loadSummary(start, end) {
  const shares = [start.cpu_share, end.cpu_share];
  const state = shares.some((share) => share !== null && share >= LOADED_CPU_SHARE) ? "loaded" : shares.includes(null) ? "unknown" : "calm";
  return { start, end, threshold: LOADED_CPU_SHARE, state };
}

const percent = (share) => (share === null ? "?" : `${Math.round(share * 100)} %`);

/** The sentence for the reader, or null on a calm machine. */
export function loadWarning({ start, end, threshold, state }) {
  if (state === "calm") return null;
  if (state === "unknown") return "⚠ Charge de la machine non mesurée : un délai dépassé peut venir d'elle.";
  return `⚠ Machine chargée pendant ce passage : processeur occupé à ${percent(start.cpu_share)} au début et ${percent(end.cpu_share)} à la fin (seuil ${percent(threshold)}). Les délais et les mesures de ce passage ne sont pas fiables : relance sur une machine au repos avant de conclure.`;
}
