import { ChildProcess, execFile } from "node:child_process";
import { promisify } from "node:util";
import { setTimeout as delay } from "node:timers/promises";
import { FixtureFailure } from "./performance.mjs";

const execute = promisify(execFile);

/** POSIX ps fields shared by macOS and Linux. A zombie has already exited;
 * keeping its process-table entry until its parent reaps it is not live work. */
export function parseProcessTable(output) {
  return output.split(/\r?\n/).filter(line => line.trim()).map(line => {
    const match = line.trim().match(/^(\d+)\s+(\d+)\s+(\S+)$/);
    if (!match) throw new FixtureFailure(`process table cannot be interpreted: ${line}`);
    return { pid: Number(match[1]), pgid: Number(match[2]), state: match[3], live: !/^[ZX]/.test(match[3]) };
  });
}

async function processTable(timeoutMs) {
  const { stdout } = await execute("ps", ["-A", "-o", "pid=", "-o", "pgid=", "-o", "stat="], {
    timeout: Math.max(1, Math.ceil(timeoutMs)), maxBuffer: 8 * 1024 * 1024,
    env: { ...process.env, LC_ALL: "C" },
  });
  return parseProcessTable(stdout);
}

/** The injectable boundary lets negative tests exercise the production
 * deadline and ownership policy without pretending that a real signal failed. */
export function createOwnedGroupKiller({ signal = process.kill.bind(process), inspect = processTable } = {}) {
  const cleanups = new WeakMap();
  return function killOwnedGroup(child, { timeoutMs = 4000 } = {}) {
    if (!child || !Number.isSafeInteger(child.pid) || child.pid <= 1 || child.pid === process.pid) {
      return Promise.reject(new FixtureFailure("cleanup needs the owned detached ChildProcess and its original pid"));
    }
    if (!Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) return Promise.reject(new FixtureFailure("invalid process cleanup deadline"));
    if (cleanups.has(child)) return cleanups.get(child);
    const cleanup = terminate(child, timeoutMs, signal, inspect);
    cleanups.set(child, cleanup);
    return cleanup;
  };
}

async function terminate(child, timeoutMs, signal, inspect) {
  if (process.platform === "win32") throw new FixtureFailure("owned POSIX process groups are unavailable on Windows");
  const pgid = child.pid; // Never discover a kill target from a listening port.
  const started = performance.now();
  let signalled = false;
  let membersBeforeSignal = [];
  let last = [];
  const exited = () => child.exitCode !== null || child.signalCode !== null;
  const expired = () => new FixtureFailure(`owned process group ${pgid} did not terminate in ${timeoutMs} ms: ${JSON.stringify({ child_exited: exited(), members: last })}`);
  while (true) {
    const remaining = timeoutMs - (performance.now() - started);
    if (remaining <= 0) throw expired();
    let timer;
    try {
      const hadExited = exited();
      const table = await Promise.race([
        inspect(remaining),
        new Promise((_, reject) => { timer = setTimeout(() => reject(expired()), remaining); }),
      ]);
      last = table.filter(entry => entry.pgid === pgid);
      const leader = table.find(entry => entry.pid === pgid);
      if (leader && leader.pgid !== pgid) throw new FixtureFailure(`owned child ${pgid} was not launched in its own detached group`);
      // An exited ChildProcess cannot own a later process reusing its pid.
      if (leader?.live && hadExited) throw new FixtureFailure(`refusing to signal reused process-group leader ${pgid}`);
      const live = last.filter(entry => entry.live);
      if (exited() && !live.length) return { pgid, child_exited: true, live: [], zombies: last.filter(entry => !entry.live), members_before_signal: membersBeforeSignal, elapsed_ms: performance.now() - started };
      if (live.length && !signalled) {
        membersBeforeSignal = live;
        try { signal(-pgid, "SIGKILL"); }
        catch (error) {
          if (error.code !== "ESRCH") {
            const failure = new FixtureFailure(`cannot terminate owned process group ${pgid}: ${error.code ?? error.message}`);
            failure.cause = error;
            throw failure;
          }
        }
        signalled = true;
      }
    } catch (error) {
      if (error instanceof FixtureFailure) throw error;
      throw new FixtureFailure(`cannot inspect owned process group ${pgid}: ${error.code ?? error.message}`);
    } finally { clearTimeout(timer); }
    await delay(Math.min(20, Math.max(1, timeoutMs - (performance.now() - started))));
  }
}

const terminateOwned = createOwnedGroupKiller();
export function killOwnedGroup(child, options) {
  if (!(child instanceof ChildProcess)) return Promise.reject(new FixtureFailure("cleanup requires the owned ChildProcess object, never an arbitrary pid"));
  return terminateOwned(child, options);
}
