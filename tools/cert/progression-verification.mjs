/** Observe a bounded journey to a real product terminal, then a new running
 * state. Changing screens, an animated clock or A↔B alone cannot satisfy it.
 * The predicates/readings remain game oracles, outside this helper's policy. */
export async function verifyTerminalJourney({ read, advance, terminal, restart, restarted, maxSteps, timeoutMs }) {
  if (!Number.isSafeInteger(maxSteps) || maxSteps < 1 || !Number.isSafeInteger(timeoutMs) || timeoutMs < 1) throw new Error("terminal journey needs positive explicit bounds");
  for (const operation of [read, advance, terminal, restart, restarted]) if (typeof operation !== "function") throw new Error("terminal journey operation missing");
  const began = performance.now(), states = [];
  let steps = 0, terminalState;
  const remember = state => {
    if (state === null || state === undefined) throw new Error("terminal journey state absent");
    if (JSON.stringify(states.at(-1)) !== JSON.stringify(state)) states.push(state);
    return state;
  };
  const predicate = (check, ...values) => {
    const value = check(...values);
    if (typeof value !== "boolean") throw new Error("terminal journey predicate must return a boolean");
    return value;
  };
  const failure = reason => {
    const error = new Error(`terminal journey: ${reason}`);
    error.diagnostics = { steps, max_steps: maxSteps, timeout_ms: timeoutMs, elapsed_ms: performance.now() - began, terminal_seen: terminalState !== undefined, states };
    return error;
  };
  const bounded = async (operation, label) => {
    const remaining = timeoutMs - (performance.now() - began);
    if (remaining <= 0) throw failure(`deadline before ${label}`);
    let timer;
    try {
      return await Promise.race([Promise.resolve().then(operation), new Promise((_, reject) => { timer = setTimeout(() => reject(failure(`deadline during ${label}`)), remaining); })]);
    } finally { clearTimeout(timer); }
  };
  let state = remember(await bounded(read, "initial reading"));
  if (predicate(terminal, state)) throw failure("initial state was already terminal");
  while (!predicate(terminal, state)) {
    if (steps >= maxSteps) throw failure("step bound reached without a terminal");
    await bounded(() => advance(state), "advance");
    steps++;
    state = remember(await bounded(read, "terminal reading"));
  }
  terminalState = state;
  await bounded(() => restart(terminalState), "restart");
  while (true) {
    state = remember(await bounded(read, "resumed reading"));
    if (!predicate(terminal, state) && predicate(restarted, state, terminalState)) break;
    await bounded(() => new Promise(resolve => setTimeout(resolve, 25)), "resumed state");
  }
  return { steps, terminal: terminalState, resumed: state, states, elapsed_ms: performance.now() - began, max_steps: maxSteps, timeout_ms: timeoutMs };
}
