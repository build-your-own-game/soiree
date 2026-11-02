// Production SDK negotiation contracts. Every case uses one real page/identity,
// an owned UDP relay and the normal native timers; no retry or engine override.
// A tab remembers WebRTC failing (RtcMemory), so each case starts as a new tab
// would: unless it is about that memory, a case must not read the one before.
import assert from 'node:assert/strict';
import { UdpProxy, observeRtc, rewrittenJsonFulfillment, welcomeDevice } from './udp.mjs';
import { cleanupAll, runWithCleanup } from './cleanup.mjs';
import { FixtureFailure } from './performance.mjs';

const CASES = [
  { name: 'SDP delayed 2 s', delay: 2000, expect: 'rtc' },
  { name: 'SDP delayed 4 s', delay: 4000, expect: 'rtc' },
  { name: 'native SDP rejection', badSdp: true, expect: 'ws' },
  { name: 'native DTLS fingerprint rejection', badCertificate: true, expect: 'ws' },
  { name: 'remembered failure skips the window', remembered: true, expect: 'ws' },
  { name: 'absolute deadline after SDP progress', delay: 4000, blackhole: true, deadline: true, expect: 'ws' },
  { name: 'absolute deadline and late SDP', delay: 16000, deadline: true, late: true, expect: 'ws' },
  { name: 'deadline registration failure releases attempt', armFailure: true, expect: 'ws' },
];
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

/** A new tab under the same identity: sessionStorage goes, localStorage stays. */
export const forgetTab = page => page.evaluate(() => sessionStorage.clear());

function installCaseObserver() {
  if (globalThis.__certNegotiation) return;
  const trace = globalThis.__certNegotiation = { ws: [], packets: [] };
  const Native = globalThis.WebSocket;
  globalThis.WebSocket = new Proxy(Native, {
    construct(target, args, newTarget) {
      const socket = Reflect.construct(target, args, newTarget === globalThis.WebSocket ? target : newTarget);
      const row = { at: performance.timeOrigin + performance.now(), open: false, closed: false };
      if (trace.ws.length < 8) trace.ws.push(row);
      socket.addEventListener('open', () => { row.open = true; });
      socket.addEventListener('close', () => { row.closed = true; });
      socket.addEventListener('message', async event => {
        const value = event.data instanceof Blob ? await event.data.arrayBuffer() : event.data;
        if (typeof value === 'string' || trace.packets.length >= 64) return;
        trace.packets.push(Array.from(new Uint8Array(value).slice(0, 64)));
      });
      return socket;
    },
  });
  const timeout = globalThis.setTimeout;
  globalThis.setTimeout = function (callback, ms, ...args) {
    if (new URL(location.href).searchParams.has('__cert_rtc_arm_failure') && ms > 14000 && ms <= 15000)
      throw new Error('owned RTC timer-registration negative control');
    return timeout.call(this, callback, ms, ...args);
  };
}

async function snapshot(page) {
  return page.evaluate(() => ({ rtc: globalThis.__certRtc, ws: globalThis.__certNegotiation }));
}

/** Caller supplies an existing welcomed phone URL and its identity. Reports are
 * emitted before assertions; a failure retains all preceding receipts. The
 * finally path returns the page to WebSocket and waits for every owned route.
 * onCleanupFailure is independent from the primary case error (H1 vs C1). */
export async function verifyRtcNegotiation({ page, url, upstream, port, expectedDevice, onCase = () => {}, onCleanupFailure = () => {} }) {
  if (!Number.isSafeInteger(expectedDevice) || expectedDevice <= 0) throw new FixtureFailure('RTC negotiation requires a welcomed device identity');
  const reports = [];
  const rtcUrl = new URL(url); rtcUrl.searchParams.delete('salon');
  const wsUrl = new URL(rtcUrl); wsUrl.searchParams.set('salon', 'ws');
  await page.addInitScript(observeRtc);
  await page.addInitScript(installCaseObserver);
  const receipts = () => ({ cases: reports, remaining: CASES.slice(reports.length).map(row => row.name) });
  return runWithCleanup(async () => {
    try {
      for (const definition of CASES) {
        const relay = await UdpProxy.open(upstream, port);
        if (definition.blackhole) relay.set({ blackhole: true });
        const controller = new AbortController(), active = new Set(), routeErrors = [];
        const wait = ms => new Promise(resolve => {
          const abort = () => { clearTimeout(timer); resolve(false); };
          const timer = setTimeout(() => { controller.signal.removeEventListener('abort', abort); resolve(true); }, ms);
          controller.signal.addEventListener('abort', abort, { once: true });
          if (controller.signal.aborted) abort();
        });
        const route = request => {
          const task = (async () => {
            try {
              const response = await request.fetch({ timeout: 5000, maxRetries: 0 });
              if (!response.ok()) throw new FixtureFailure(`RTC signaling HTTP ${response.status()}`);
              let answer = relay.rewrite(await response.json());
              if (definition.badSdp) answer = { ...answer, sdp: 'invalid owned SDP negative control' };
              if (definition.badCertificate) answer = { ...answer, sdp: answer.sdp.replace(/(a=fingerprint:sha-256 )([0-9A-Fa-f]{2})/, (_, prefix, byte) => prefix + (byte === '00' ? '01' : '00')) };
              if (!(await wait(definition.delay ?? 0))) { await request.abort(); return; }
              await request.fulfill(rewrittenJsonFulfillment(response, answer));
            } catch (error) {
              routeErrors.push(error.message);
              try { await request.abort(); } catch (abortError) { routeErrors.push(`route abort: ${abortError.message}`); }
            }
          })();
          active.add(task);
          return task.finally(() => active.delete(task));
        };
        let result;
        await runWithCleanup(async () => {
          await page.route('**/salon/offer', route);
          if (!definition.remembered) await forgetTab(page);
          const target = new URL(rtcUrl);
          if (definition.armFailure) target.searchParams.set('__cert_rtc_arm_failure', '1');
          const started = Date.now();
          await page.goto(target.href, { waitUntil: 'load', timeout: 15000 });
          let state;
          const until = started + 28000; // Existing C1 joining budget is 30 s.
          while (Date.now() < until) {
            state = await snapshot(page);
            const rtcDevices = (state.rtc?.packets ?? []).map(packet => welcomeDevice(packet.bytes)).filter(device => device !== null);
            const wsDevices = (state.ws?.packets ?? []).map(welcomeDevice).filter(device => device !== null);
            if (definition.expect === 'rtc' && (rtcDevices.length || state.ws?.ws.length)) break;
            if (definition.expect === 'ws' && wsDevices.length) break;
            if (routeErrors.length) break;
            await sleep(50);
          }
          if (definition.late) {
            // Let the actual delayed response settle after WebSocket is live.
            while (active.size && Date.now() < until) await sleep(50);
            await sleep(200);
            state = await snapshot(page);
          }
          result = { name: definition.name, expected: definition.expect, elapsed_ms: Date.now() - started,
            state, relay: relay.snapshot(), relay_errors: [...relay.errors], route_errors: [...routeErrors] };
          reports.push(result); onCase(result);
          if (routeErrors.length || relay.errors.length) throw new FixtureFailure([...routeErrors, ...relay.errors].join(' ; '));
          const rtcDevices = (state.rtc?.packets ?? []).map(packet => welcomeDevice(packet.bytes)).filter(device => device !== null);
          const wsDevices = (state.ws?.packets ?? []).map(welcomeDevice).filter(device => device !== null);
          const events = state.rtc?.events ?? [], sockets = state.ws?.ws ?? [];
          if (definition.expect === 'rtc') {
            assert.ok(rtcDevices.includes(expectedDevice), `${definition.name}: no real RTC Welcome for device ${expectedDevice}`);
            assert.equal(sockets.length, 0, `${definition.name}: viable negotiation fell back to TCP`);
          } else {
            assert.ok(wsDevices.includes(expectedDevice), `${definition.name}: fallback lost the welcomed identity`);
            assert.equal(sockets.length, 1, `${definition.name}: late work replaced/reopened WebSocket`);
            assert.equal(sockets[0].closed, false, `${definition.name}: fallback WebSocket was evicted`);
            assert.equal(rtcDevices.length, 0, `${definition.name}: negative control unexpectedly opened RTC`);
          }
          if (definition.badSdp) assert.ok(events.some(event => event.kind === 'setRemoteDescription:rejected'), 'SDP rejection was not native');
          if (definition.badCertificate) assert.ok(events.some(event => event.connection === 'failed'), 'DTLS failure was not native');
          if (definition.badSdp || definition.badCertificate || definition.armFailure)
            assert.ok(sockets[0].at - events.find(event => event.kind === 'peer').at < 3000, `${definition.name}: native failure did not fall back immediately`);
          if (definition.remembered) assert.ok(!events.some(event => event.kind === 'peer'), 'a reload paid the WebRTC window again after a failure');
          if (definition.deadline) {
            const peer = events.find(event => event.kind === 'peer'), closed = events.find(event => event.kind === 'close');
            assert.ok(peer, 'the page never tried WebRTC, so nothing tests its deadline');
            assert.ok(closed, 'absolute deadline did not close its peer');
            result.closed_ms = closed.at - peer.at;
            assert.ok(result.closed_ms >= 14900 && result.closed_ms <= 15500, `absolute 15 s deadline was renewed or not enforced: ${result.closed_ms} ms`);
          }
          if (definition.blackhole) {
            assert.ok(events.some(event => event.kind === 'setRemoteDescription:fulfilled'), 'SDP must make real progress before the deadline');
            assert.ok(result.relay.dropped > 0, 'the permanent UDP failure was not injected');
          }
          if (definition.late) assert.ok(events.some(event => event.kind === 'setRemoteDescription:rejected'), 'late completion was not actually delivered to the closed peer');
          if (definition.armFailure) {
            assert.ok(events.some(event => event.kind === 'close'), 'unbounded attempt was not released');
            assert.ok(!events.some(event => event.kind === 'createOffer'), 'async negotiation started without a deadline');
          }
          result.passed = true;
        }, [
          { name: 'negotiation route', run: () => page.unroute('**/salon/offer', route) },
          { name: 'negotiation UDP relay', run: () => relay.close() },
          { name: 'negotiation route completion', run: async () => {
            controller.abort();
            await Promise.all([...active]);
            if (routeErrors.length) throw new FixtureFailure(routeErrors.join(' ; '));
          } },
        ], { onFailure: onCleanupFailure });
      }
      return receipts();
    } catch (error) { error.rtcNegotiation = receipts(); throw error; }
  }, [{ name: 'negotiation phone restore', run: async () => { await page.goto(wsUrl.href, { waitUntil: 'load', timeout: 15000 }); await forgetTab(page); } }],
  { timeoutMs: 16000, onFailure: onCleanupFailure });
}
