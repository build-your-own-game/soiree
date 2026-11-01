import { FixtureFailure } from "./performance.mjs";

/** Every owned resource is offered cleanup even when another cleanup fails.
 * A deadline is an infrastructure failure, never implicit successful closure.
 * This helper does not kill processes or infer that a timed-out close worked. */
export async function cleanupAll(actions, timeoutMs = 5000) {
  if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) throw new FixtureFailure("invalid cleanup deadline");
  return (await Promise.all(actions.map(async ({name, run}) => {
    let timer;
    try {
      await Promise.race([
        Promise.resolve().then(run),
        new Promise((_, reject) => { timer = setTimeout(() => reject(new Error(`deadline ${timeoutMs} ms exceeded`)), timeoutMs); }),
      ]);
      return null;
    } catch(error) { return `${name}: ${error.message ?? error}`; }
    finally { clearTimeout(timer); }
  }))).filter(Boolean);
}

/** Keep the primary failure; report cleanup failures independently as H1. */
export async function runWithCleanup(work, actions, {timeoutMs = 5000, onFailure = () => {}} = {}) {
  let result;
  let primary;
  try { result = await work(); }
  catch(error) { primary = error; }
  const failures = await cleanupAll(actions, timeoutMs);
  if (failures.length) {
    const error = new FixtureFailure(`cleanup failed: ${failures.join(" ; ")}`);
    onFailure(error, primary);
    primary ??= error;
  }
  if (primary) throw primary;
  return result;
}
