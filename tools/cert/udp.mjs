// A real datagram relay: faulting happens below DTLS/SCTP, not in game code.
import dgram from 'node:dgram';
import { once } from 'node:events';
import { cleanupAll, runWithCleanup } from './cleanup.mjs';
import { FixtureFailure } from './performance.mjs';

/** A rewritten JSON body has new UTF-8 bytes, not the fetched response's
 * compressed/streamed framing. Do not pass `response` back to fulfill: it
 * would copy stale entity headers before applying the new body. */
export function rewrittenJsonFulfillment(response, value) {
  const body = Buffer.from(JSON.stringify(value), 'utf8');
  const framing = new Set(['content-length', 'content-encoding', 'transfer-encoding']);
  const headers = Object.fromEntries(Object.entries(response.headers())
    .filter(([name]) => !framing.has(name.toLowerCase())));
  if (!Object.keys(headers).some(name => name.toLowerCase() === 'content-type')) {
    headers['content-type'] = 'application/json; charset=utf-8';
  }
  headers['content-length'] = String(body.byteLength);
  return { status: response.status(), headers, body };
}

export class UdpProxy {
  static async open(host, port, { listenPort = 0 } = {}) {
    const proxy = new UdpProxy(host, port);
    try {
      proxy.socket.bind(listenPort, host);
      await once(proxy.socket, 'listening');
      proxy.port = proxy.socket.address().port;
      return proxy;
    } catch(error) {
      let cleanupError;
      try {await proxy.close();}catch(failure){cleanupError=failure;}
      const failure=new FixtureFailure(`UDP relay bind failed: ${error.message}${cleanupError ? `; cleanup: ${cleanupError.message}` : ''}`);
      failure.cause=error;
      throw failure;
    }
  }
  constructor(host, port) {
    this.host = host; this.target = port;
    this.socket = dgram.createSocket('udp4');
    this.peers = new Map(); this.timers = new Map(); this.errors = [];
    this.profile = { dropEvery: 0, delayMs: 0, reorderEvery: 0, blackhole: false };
    this.stats = { upstream: 0, downstream: 0, dropped: 0, delayed: 0, reordered: 0, forwarded: 0,
      directions: Object.fromEntries(['upstream','downstream'].map(direction=>[direction,{received:0,dropped:0,delayed:0,reordered:0,forwarded:0}])) };
    this.sequences = {upstream:0,downstream:0}; this.closed = false;
    this.socket.on('error', error => this.errors.push(error.message));
    this.socket.on('message', (bytes, peer) => {
      if(this.closed)return;
      const key = `${peer.address}:${peer.port}`;
      let upstream = this.peers.get(key);
      if (!upstream) {
        if (this.peers.size >= 32) { this.drop('upstream'); return; }
        upstream = dgram.createSocket('udp4');
        upstream.on('error', error => this.errors.push(error.message));
        upstream.on('message', reply => this.forward('downstream', reply, this.socket, peer.port, peer.address));
        upstream.bind(0, host);
        this.peers.set(key, upstream);
      }
      this.forward('upstream', bytes, upstream, this.target, host);
    });
  }
  set(profile = {}) {
    for (const key of ['dropEvery', 'delayMs', 'reorderEvery']) {
      if (profile[key] !== undefined && (!Number.isSafeInteger(profile[key]) || profile[key] < 0)) throw new Error(`invalid UDP ${key}`);
    }
    this.profile = { dropEvery: 0, delayMs: 0, reorderEvery: 0, blackhole: false, ...profile };
    this.sequences = {upstream:0,downstream:0};
    // Pending packets must not escape a newly imposed silence.
    if (this.profile.blackhole) for (const [timer,direction] of this.timers) { clearTimeout(timer); this.timers.delete(timer); this.drop(direction); }
  }
  drop(direction) { this.stats.dropped++; this.stats.directions[direction].dropped++; }
  snapshot() { return structuredClone(this.stats); }
  forward(direction, bytes, socket, port, host) {
    this.stats[direction]++;
    const directional = this.stats.directions[direction];
    directional.received++;
    const n = ++this.sequences[direction], p = this.profile;
    if (this.closed || p.blackhole || (p.dropEvery && n % p.dropEvery === 0)) { this.drop(direction); return; }
    let delay = p.delayMs;
    if (p.reorderEvery && n % p.reorderEvery === 0) { delay += 80; this.stats.reordered++; directional.reordered++; }
    const send = () => {
      if (this.closed || this.profile.blackhole) { this.drop(direction); return; }
      try {socket.send(bytes, port, host, error => { if (error && !this.closed) this.errors.push(error.message); });}
      catch(error){if(!this.closed)this.errors.push(error.message);return;}
      this.stats.forwarded++;
      directional.forwarded++;
    };
    if (delay) {
      if (this.timers.size >= 4096) { this.drop(direction); return; }
      this.stats.delayed++;
      directional.delayed++;
      const timer = setTimeout(() => { this.timers.delete(timer); send(); }, delay);
      this.timers.set(timer,direction);
    } else send();
  }
  rewrite(answer) {
    let candidates = 0;
    const sdp = answer.sdp.replace(/^a=candidate:([^\r\n]+)$/gm, (_, fields) => {
      const parts = fields.trim().split(/\s+/);
      if (parts[2]?.toLowerCase() !== 'udp') throw new Error('non-UDP RTC candidate');
      parts[4] = this.host; parts[5] = String(this.port); candidates++;
      return `a=candidate:${parts.join(' ')}`;
    });
    if (!candidates) throw new Error('SDP has no relayable host candidate');
    return { ...answer, sdp };
  }
  async close() {
    if(this.closing) return this.closing;
    this.closed = true;
    for (const timer of this.timers.keys()) clearTimeout(timer);
    this.timers.clear();
    const sockets=[this.socket,...this.peers.values()];
    this.closing=(async()=>{
      const failures=await cleanupAll(sockets.map((socket,index)=>({name:`UDP socket ${index}`,run:()=>new Promise((resolve,reject)=>{
        try {socket.close(resolve);} catch(error) {if(error.code==='ERR_SOCKET_DGRAM_NOT_RUNNING')resolve();else reject(error);}
      })})));
      this.peers.clear();
      if(failures.length)throw new FixtureFailure(failures.join(' ; '));
    })();
    return this.closing;
  }
}

/** Route registration and removal share the relay's owned lifetime. */
export async function withRtcRelay(relay, page, route, work, restore, onFailure) {
  return runWithCleanup(async()=>{
    await page.addInitScript(observeRtc);
    await page.route('**/salon/offer',route);
    return work();
  },[
    {name:'RTC route',run:()=>page.unroute('**/salon/offer',route)},
    {name:'UDP relay',run:()=>relay.close()},
    {name:'phone restore',run:restore},
  ],{onFailure});
}

// Observe the actual channel before WASM creates it. No test API in the product.
export function observeRtc() {
  if (globalThis.__certRtc?.observerVersion === 2) return;
  const create = RTCPeerConnection.prototype.createDataChannel;
  const trace = globalThis.__certRtc = { observerVersion: 2, received: 0, sent: 0, lastAt: 0, packets: [], events: [] };
  const peers = new WeakMap(), responses = new WeakSet();
  let sequence = 0;
  const record = (kind, details = {}) => {
    if (trace.events.length === 128) trace.events.shift();
    trace.events.push({ at: performance.timeOrigin + performance.now(), kind, ...details });
  };
  const state = pc => ({ ice: pc.iceConnectionState, gathering: pc.iceGatheringState, connection: pc.connectionState, signaling: pc.signalingState });
  const description = value => {
    const sdp = value?.sdp;
    return typeof sdp === 'string' ? {
      descriptionType: value.type, candidates: (sdp.match(/^a=candidate:/gm) ?? []).length,
      iceLite: /^a=ice-lite\r?$/m.test(sdp), endOfCandidates: /^a=end-of-candidates\r?$/m.test(sdp),
    } : {};
  };
  const peer = pc => {
    if (peers.has(pc)) return peers.get(pc);
    const id = ++sequence;
    peers.set(pc, id);
    record('peer', { id, ...state(pc) });
    for (const kind of ['icegatheringstatechange', 'iceconnectionstatechange', 'connectionstatechange', 'signalingstatechange'])
      pc.addEventListener(kind, () => record(kind, { id, ...state(pc) }));
    pc.addEventListener('icecandidate', event => record('icecandidate', { id, present: Boolean(event.candidate) }));
    pc.addEventListener('icecandidateerror', event => record('icecandidateerror', { id, code: event.errorCode, error: event.errorText }));
    return id;
  };
  // Return the observed promise: its rejection must remain visible to the app
  // (or to unhandledrejection), rather than being swallowed by a side handler.
  for (const kind of ['createOffer', 'setLocalDescription', 'setRemoteDescription']) {
    const native = RTCPeerConnection.prototype[kind];
    RTCPeerConnection.prototype[kind] = function (...args) {
      const id = peer(this);
      record(kind, { id, ...description(args[0]) });
      const result = native.apply(this, args);
      return result.then(value => {
        record(`${kind}:fulfilled`, { id, ...state(this), ...description(value) });
        return value;
      }, error => {
        record(`${kind}:rejected`, { id, error: error.message });
        throw error;
      });
    };
  }
  const close = RTCPeerConnection.prototype.close;
  RTCPeerConnection.prototype.close = function (...args) {
    record('close', { id: peer(this), ...state(this) });
    return close.apply(this, args);
  };
  const fetch = globalThis.fetch;
  globalThis.fetch = function (...args) {
    const value = args[0] instanceof Request ? args[0].url : String(args[0]);
    let observed = false;
    try { observed = new URL(value, location.href).pathname === '/salon/offer'; } catch { /* Native fetch owns invalid URLs. */ }
    if (observed) record('offer:fetch');
    const result = fetch.apply(this, args);
    if (!observed) return result;
    return result.then(response => {
      responses.add(response);
      record('offer:response', { status: response.status, length: response.headers.get('content-length') });
      return response;
    }, error => {
      record('offer:rejected', { error: error.message });
      throw error;
    });
  };
  const text = Response.prototype.text;
  Response.prototype.text = function (...args) {
    const result = text.apply(this, args);
    if (!responses.has(this)) return result;
    return result.then(body => {
      let valid = false;
      try { const value = JSON.parse(body); valid = value.type === 'answer' && typeof value.sdp === 'string'; } catch { /* recorded below */ }
      record('offer:body', { bytes: new TextEncoder().encode(body).byteLength, valid });
      return body;
    }, error => {
      record('offer:body:rejected', { error: error.message });
      throw error;
    });
  };
  RTCPeerConnection.prototype.createDataChannel = function (...args) {
    const id = peer(this);
    const channel = create.apply(this, args);
    record('channel', { id });
    for (const kind of ['open', 'close', 'error']) channel.addEventListener(kind, () => record(`channel:${kind}`, { id }));
    const send = channel.send.bind(channel);
    channel.send = (...values) => { trace.sent++; return send(...values); };
    channel.addEventListener('message', async event => {
      const at = performance.timeOrigin + performance.now();
      const bytes = event.data instanceof Blob ? new Uint8Array(await event.data.arrayBuffer()) : new Uint8Array(event.data);
      trace.received++; trace.lastAt = at;
      if (trace.packets.length === 64) trace.packets.shift();
      trace.packets.push({ at, bytes: Array.from(bytes) });
    });
    return channel;
  };
}

/** Fresh transport traffic is insufficient: the live packet itself is fresh. */
export function receivedRtcSince(trace, since, accept) {
  return trace?.packets?.some(({ at, bytes }) => Number.isFinite(at) && at >= since && accept(bytes)) ?? false;
}

/** Welcome prefix of salon's postcard Packet. Never infer identity from ACKs. */
export function welcomeDevice(bytes) {
  let offset = 0;
  const variable = () => {
    let value = 0, scale = 1;
    for (let index = 0; index < 10; index++) {
      const byte = bytes[offset++];
      if (!Number.isInteger(byte) || byte < 0 || byte > 255) throw new Error('truncated varint');
      value += (byte & 127) * scale;
      if (!Number.isSafeInteger(value)) throw new Error('unsafe varint');
      if (!(byte & 128)) return value;
      scale *= 128;
    }
    throw new Error('long varint');
  };
  const option = () => {
    const tag = bytes[offset++];
    if (tag !== 0 && tag !== 1) throw new Error('invalid option');
    return tag === 1;
  };
  try {
    variable(); variable(); // epoch, sent
    if (option()) { variable(); variable(); }
    if (!option() || variable() !== 1) return null;
    const device = variable();
    return device > 0 && device <= 0xffff_ffff ? device : null;
  } catch { return null; }
}
