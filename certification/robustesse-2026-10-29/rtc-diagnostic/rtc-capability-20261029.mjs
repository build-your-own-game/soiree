import {chromium,webkit} from '/Users/axel2/Projects/build-your-own-game/soiree/tools/cert/node_modules/playwright-core/index.mjs';
import {createServer} from 'node:http';
import dgram from 'node:dgram';
import {once} from 'node:events';
import {networkInterfaces} from 'node:os';
import {writeFileSync} from 'node:fs';
const lan=Object.values(networkInterfaces()).flat().find(a=>a?.family==='IPv4'&&!a.internal)?.address;
if(!lan)throw new Error('LAN unavailable');
const server=createServer((req,res)=>{res.writeHead(200,{'content-type':'text/html'});res.end('<!doctype html><title>Owned RTC diagnostic</title>RTC diagnostic');});
server.listen(0,'127.0.0.1');await once(server,'listening');
const receiver=dgram.createSocket('udp4');receiver.bind(0,lan);await once(receiver,'listening');
const packets=[];receiver.on('message',(bytes,peer)=>packets.push({at:Date.now(),size:bytes.length,stun:bytes.length>=20&&bytes.readUInt32BE(4)===0x2112a442,source:peer.address}));
const results=[];
const browsers=[];
try{
 for(const [engine,type] of [['chromium',chromium],['webkit',webkit]]){
  const browser=await type.launch({headless:true,...(engine==='chromium'?{channel:'chrome'}:{})});browsers.push(browser);
  for(const mobile of engine==='webkit'?[false,true]:[false]){
   const context=await browser.newContext(mobile?{viewport:{width:390,height:844},deviceScaleFactor:3,isMobile:true,hasTouch:true,userAgent:'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1'}:{});
   const page=await context.newPage();await page.goto(`http://127.0.0.1:${server.address().port}`);
   const errors=[];page.on('pageerror',error=>errors.push(error.message));
   const first=packets.length;
   const state=await page.evaluate(async({host,port})=>{
    const pc=new RTCPeerConnection({iceServers:[]}),events=[],candidates=[];pc.createDataChannel('owned-probe');
    pc.onicecandidate=event=>{if(event.candidate)candidates.push(event.candidate.candidate);};
    for(const name of ['icegatheringstatechange','iceconnectionstatechange','connectionstatechange'])pc.addEventListener(name,()=>events.push({name,ice:pc.iceConnectionState,gathering:pc.iceGatheringState,connection:pc.connectionState}));
    try{
     const offer=await pc.createOffer();await pc.setLocalDescription(offer);
     let answer=offer.sdp.replace(/a=setup:actpass/g,'a=setup:passive').replace(/a=ice-ufrag:[^\r\n]+/g,'a=ice-ufrag:ownedProbe').replace(/a=ice-pwd:[^\r\n]+/g,'a=ice-pwd:ownedProbePassword1234567890').replace(/^a=candidate:[^\r\n]+\r?\n/gm,'').replace('t=0 0\r\n','t=0 0\r\na=ice-lite\r\n');
     answer+=`a=candidate:1 1 UDP 2130706431 ${host} ${port} typ host\r\na=end-of-candidates\r\n`;
     await pc.setRemoteDescription({type:'answer',sdp:answer});
     await new Promise(resolve=>setTimeout(resolve,2000));
     return {api:true,offer:offer.sdp,candidates,events,ice:pc.iceConnectionState,connection:pc.connectionState,gathering:pc.iceGatheringState};
    }catch(error){return{api:true,error:error.message,candidates,events};}finally{pc.close();}
   },{host:lan,port:receiver.address().port});
   const result={engine,mobile,received:packets.slice(first),state,errors};results.push(result);console.log(JSON.stringify({...result,state:{...state,offer:undefined}}));
   if(engine==='webkit'){
    result.peerPair=await page.evaluate(async()=>{
     const a=new RTCPeerConnection({iceServers:[]}),b=new RTCPeerConnection({iceServers:[]});
     const candidates={a:[],b:[]};a.onicecandidate=e=>{if(e.candidate){candidates.a.push(e.candidate.candidate);b.addIceCandidate(e.candidate).catch(()=>{});}};b.onicecandidate=e=>{if(e.candidate){candidates.b.push(e.candidate.candidate);a.addIceCandidate(e.candidate).catch(()=>{});}};
     let opened=false,received=false;const channel=a.createDataChannel('pair');channel.onopen=()=>{opened=true;channel.send('owned');};b.ondatachannel=e=>{e.channel.onmessage=e=>{received=e.data==='owned';};};
     try{await a.setLocalDescription(await a.createOffer());await b.setRemoteDescription(a.localDescription);await b.setLocalDescription(await b.createAnswer());await a.setRemoteDescription(b.localDescription);await new Promise(resolve=>setTimeout(resolve,2000));return{opened,received,candidates,iceA:a.iceConnectionState,iceB:b.iceConnectionState};}catch(error){return{error:error.message,candidates};}finally{a.close();b.close();}
    });console.log(JSON.stringify({engine,mobile,peerPair:result.peerPair}));
   }
   await context.close();
  }
 }
}finally{await Promise.all(browsers.map(browser=>browser.close()));receiver.close();server.closeAllConnections();await new Promise(resolve=>server.close(resolve));writeFileSync('/tmp/rtc-capability-20261029.json',JSON.stringify({lan,results},null,2));}
