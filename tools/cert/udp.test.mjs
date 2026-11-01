import test from 'node:test';
import assert from 'node:assert/strict';
import dgram from 'node:dgram';
import { once } from 'node:events';
import vm from 'node:vm';
import { UdpProxy, receivedRtcSince, withRtcRelay, rewrittenJsonFulfillment, observeRtc, welcomeDevice } from './udp.mjs';
import { FixtureFailure } from './performance.mjs';
const pause = ms => new Promise(resolve => setTimeout(resolve,ms));

test('RTC diagnostics retain native results and rejections, distinguish stalled stages, and stay bounded', async () => {
 const failure=new Error('native remote SDP rejection');
 class Peer extends EventTarget {
  iceConnectionState='new'; iceGatheringState='new'; connectionState='new'; signalingState='stable';
  createOffer(){return Promise.resolve({type:'offer',sdp:'owned offer'});}
  setLocalDescription(){return Promise.resolve();}
  setRemoteDescription(){return Promise.reject(failure);}
  createDataChannel(){const channel=new EventTarget();channel.send=()=>true;return channel;}
  close(){this.connectionState='closed';}
 }
 class Reply {
  status=200; headers={get:()=> 'not-the-actual-byte-count'};
  body='{"type":"answer","sdp":"é"}'; failure;
  text(){return this.failure?Promise.reject(this.failure):Promise.resolve(this.body);}
 }
 const reply=new Reply();
 let fetchFailure;
 const context=vm.createContext({RTCPeerConnection:Peer,Request,Response:Reply,TextEncoder,URL,location:{href:'http://owned.test/'},performance,Blob,Uint8Array,
  fetch:()=>fetchFailure?Promise.reject(fetchFailure):Promise.resolve(reply)});
 vm.runInContext(`(${observeRtc.toString()})()`,context);
 vm.runInContext(`(${observeRtc.toString()})()`,context);
 const pc=new Peer(),channel=pc.createDataChannel('owned');
 assert.deepEqual(await pc.createOffer(),{type:'offer',sdp:'owned offer'});
 await pc.setLocalDescription();
 await assert.rejects(pc.setRemoteDescription(),error=>error===failure,'the observer must not swallow the native rejection');
 assert.equal(channel.send(new Uint8Array([1])),true);
 channel.dispatchEvent(new Event('open'));
 assert.equal(await context.fetch('/salon/offer'),reply);
 assert.equal(await reply.text(),'{"type":"answer","sdp":"é"}');
 fetchFailure=new Error('native fetch failed');
 await assert.rejects(context.fetch('/salon/offer'),error=>error===fetchFailure);
 const events=context.__certRtc.events;
 assert.ok(events.some(event=>event.kind==='setRemoteDescription:rejected'&&event.error===failure.message));
 assert.ok(events.some(event=>event.kind==='offer:rejected'&&event.error===fetchFailure.message));
 const body=events.find(event=>event.kind==='offer:body');
 assert.equal(body.valid,true);
 assert.equal(body.bytes,Buffer.byteLength('{"type":"answer","sdp":"é"}'));
 reply.body='{"type":"answer","sdp":"truncated';
 assert.equal(await reply.text(),reply.body,'the observer must not repair a malformed body');
 assert.equal(events.at(-1).valid,false);
 reply.failure=new Error('native body read failed');
 await assert.rejects(reply.text(),error=>error===reply.failure);
 assert.equal(events.at(-1).kind,'offer:body:rejected');
 assert.equal(events.at(-1).error,reply.failure.message);
 assert.equal(JSON.stringify(events).includes('owned offer'),false,'SDP credentials and payloads must not be retained by the diagnostic');
 assert.equal(context.__certRtc.sent,1);
 assert.ok(events.every(event=>Number.isFinite(event.at)));
 for(let index=0;index<200;index++)pc.dispatchEvent(new Event('iceconnectionstatechange'));
 assert.equal(events.length,128);
 pc.close();
 assert.equal(events.at(-1).kind,'close');
 assert.equal(pc.connectionState,'closed');
});
test('RTC Welcome identity requires a complete valid prefix and never accepts another control',()=>{
 assert.equal(welcomeDevice([1,2,0,1,1,42]),42);
 assert.equal(welcomeDevice([1,2,1,3,4,1,1,172,2]),300);
 for(const bytes of [[],[1,2,0,1,1],[1,2,2,1,1,42],[1,2,0,1,0,42],[1,2,0,1,2,42],[1,2,0,0,42],[1,2,0,1,1,0],[1,2,0,1,1,...Array(10).fill(255)]])assert.equal(welcomeDevice(bytes),null);
});
test('a fresh ACK cannot make an old live RTC packet fresh', () => {
 const trace = {lastAt: 200, packets: [{at:50,bytes:[1]}, {at:200,bytes:[2]}]};
 const live = bytes => bytes[0] === 1;
 assert.equal(receivedRtcSince(trace,100,live),false);
 trace.packets.push({at:201,bytes:[1]});
 assert.equal(receivedRtcSince(trace,100,live),true);
 assert.equal(receivedRtcSince(undefined,100,live),false);
});
test('UDP relay applies actual loss, delay, reordering, silence and recovery', {timeout:10000}, async () => {
 const server=dgram.createSocket('udp4'), client=dgram.createSocket('udp4');
 server.bind(0,'127.0.0.1'); await once(server,'listening');
 server.on('message',(bytes,peer)=>server.send(bytes,peer.port,peer.address));
 const relay=await UdpProxy.open('127.0.0.1',server.address().port);
 const received=[]; client.on('message',bytes=>received.push(bytes.toString()));
 const send=text=>client.send(Buffer.from(text),relay.port,'127.0.0.1');
 try {
  send('baseline'); await pause(80); assert.deepEqual(received,['baseline']);
  relay.set({blackhole:true}); send('silenced'); await pause(80); assert.equal(received.length,1);
  relay.set({dropEvery:1}); send('lost'); await pause(80); assert.equal(received.length,1);
  relay.set({delayMs:40,reorderEvery:2}); send('delayed'); await pause(20); assert.equal(received.length,1); await pause(250); assert.equal(received.at(-1),'delayed');
  relay.set(); send('recovered'); await pause(80); assert.equal(received.at(-1),'recovered');
  const beforeReorder=received.length;
  relay.set({reorderEvery:2});
  for(let index=0;index<6;index++)send(`order-${index}`);
  await pause(400);
  const reordered=received.slice(beforeReorder);
  assert.equal(reordered.length,6,'reordering lost a packet');
  assert.deepEqual([...reordered].sort(),Array.from({length:6},(_,i)=>`order-${i}`));
  assert.notDeepEqual(reordered,Array.from({length:6},(_,i)=>`order-${i}`),'delay counter did not produce an actual reordering');
  const phaseStart=relay.snapshot();
  relay.set({dropEvery:5,delayMs:10,reorderEvery:3});
  for(let index=0;index<20;index++)send(`impaired-${index}`);
  await pause(500);
  for(const direction of ['upstream','downstream'])for(const key of ['received','dropped','delayed','reordered','forwarded']){
    assert.ok(relay.stats.directions[direction][key]>phaseStart.directions[direction][key],`${direction} ${key} was not independently exercised`);
  }
  assert.ok(relay.stats.dropped>=2); assert.ok(relay.stats.delayed>=2); assert.ok(relay.stats.reordered>0);
  assert.deepEqual(relay.errors,[]);
  assert.throws(()=>relay.rewrite({sdp:'no candidates'}));
  const rewritten=relay.rewrite({sdp:'a=candidate:1 1 UDP 1 192.0.2.1 4000 typ host\r\n'});
  assert.ok(rewritten.sdp.includes(`127.0.0.1 ${relay.port}`));
 } finally { await relay.close(); client.close(); server.close(); }
});

test('failed route registration and unroute always close a real UDP socket',async()=>{
 for(const stage of ['route','unroute']){
  const relay=await UdpProxy.open('127.0.0.1',9);
  const primary=new Error('original RTC product failure');
  const cleaned=[];
  let cleanupFailure;
  const page={
   addInitScript:async()=>{},
   route:async()=>{if(stage==='route')throw primary;},
   unroute:async()=>{cleaned.push('route');if(stage==='unroute')throw new Error('closed page');},
  };
  await assert.rejects(withRtcRelay(relay,page,()=>{},async()=>{throw primary;},async()=>{cleaned.push('restore');},error=>{cleanupFailure=error;}),error=>error===primary);
  assert.deepEqual(cleaned,['route','restore']);
  assert.throws(()=>relay.socket.address(),error=>error.code==='ERR_SOCKET_DGRAM_NOT_RUNNING');
  assert.equal(relay.timers.size,0);
  assert.equal(relay.peers.size,0);
  assert.equal(Boolean(cleanupFailure),stage==='unroute');
  await relay.close();
 }
});

test('failed UDP bind releases the native socket and remains a fixture failure',async()=>{
 const occupied=dgram.createSocket('udp4');
 occupied.bind(0,'127.0.0.1');await once(occupied,'listening');
 const count=()=>process.getActiveResourcesInfo().filter(name=>name.toLowerCase().includes('udp')).length;
 // Earlier tests closed their sockets; let libuv retire those handles first.
 await pause(10);
 const before=count();
 assert.equal(before,1,'the occupied socket must be the only remaining native UDP resource');
 try{
  await assert.rejects(UdpProxy.open('127.0.0.1',9,{listenPort:occupied.address().port}),error=>error instanceof FixtureFailure&&error.cause?.code==='EADDRINUSE');
  // libuv finalizes its closed UDP handle after the JS close callback.
  const until=Date.now()+250;
  while(count()>before&&Date.now()<until)await pause(10);
  assert.equal(count(),before,'failed bind retained an additional native UDP resource');
 }finally{occupied.close();}
});

test('rewritten JSON fulfillment replaces entity framing with the exact UTF-8 body', () => {
  const originalHeaders = {
    'Content-Length': '764',
    'CONTENT-ENCODING': 'gzip',
    'Transfer-Encoding': 'chunked',
    'Content-Type': 'application/json',
    'Cache-Control': 'no-store',
    'Access-Control-Allow-Origin': 'http://localhost',
    'X-Trace': 'preserved',
  };
  const value = { type: 'answer', sdp: 'été 🦆\r\na=candidate:127.0.0.1' };
  const result = rewrittenJsonFulfillment({ status: () => 201, headers: () => originalHeaders }, value);
  assert.equal(result.status, 201);
  assert.ok(Buffer.isBuffer(result.body));
  assert.deepEqual(JSON.parse(result.body.toString('utf8')), value);
  assert.notEqual(result.body.byteLength, JSON.stringify(value).length, 'Unicode fixture must distinguish bytes from JS code units');
  assert.equal(result.headers['content-length'], String(Buffer.byteLength(JSON.stringify(value), 'utf8')));
  assert.deepEqual(Object.keys(result.headers).filter(name => ['content-length', 'content-encoding', 'transfer-encoding'].includes(name.toLowerCase())), ['content-length']);
  for (const name of ['Content-Type', 'Cache-Control', 'Access-Control-Allow-Origin', 'X-Trace']) {
    assert.equal(result.headers[name], originalHeaders[name]);
  }
  assert.equal(originalHeaders['Content-Length'], '764', 'the fetched response headers were mutated');
  assert.equal(originalHeaders['CONTENT-ENCODING'], 'gzip');
  assert.equal('response' in result, false, 'fulfill must not inherit the fetched response a second time');
  const noType = rewrittenJsonFulfillment({ status: () => 200, headers: () => ({}) }, value);
  assert.equal(noType.headers['content-type'], 'application/json; charset=utf-8');
});
