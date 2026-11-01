import {chromium,webkit} from '/Users/axel2/Projects/build-your-own-game/soiree/tools/cert/node_modules/playwright-core/index.mjs';
import {UdpProxy,observeRtc} from '/Users/axel2/Projects/build-your-own-game/soiree/tools/cert/udp.mjs';
import {spawn} from 'node:child_process';
import {createServer} from 'node:net';
import {once} from 'node:events';
import {networkInterfaces} from 'node:os';
import {writeFileSync,mkdtempSync} from 'node:fs';
const lan=Object.values(networkInterfaces()).flat().find(a=>a?.family==='IPv4'&&!a.internal)?.address;
const reserve=createServer();reserve.listen(0);await once(reserve,'listening');const port=reserve.address().port;await new Promise(r=>reserve.close(r));
const cwd='/Users/axel2/Projects/build-your-own-game/games/punchline';
const host=spawn(`${cwd}/target/release/punch-host`,['--no-build'],{cwd,env:{...process.env,PUNCH_PORT:String(port),PUNCH_DATA:mkdtempSync('/tmp/rtc-owned-host-'),SALON_LOG:'1'},stdio:['ignore','pipe','pipe']});
let output='';host.stdout.on('data',b=>output+=b);host.stderr.on('data',b=>output+=b);
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const results=[];const browsers=[];let relay;
try{
 const ready=Date.now()+5000;while(!output.includes('Punchline ready')&&Date.now()<ready){if(host.exitCode!==null)throw new Error(output);await wait(50);}if(!output.includes('Punchline ready'))throw new Error('host not ready');
 for(const [engine,type] of [['webkit',webkit]]){
  const browser=await type.launch({headless:true,...(engine==='chromium'?{channel:'chrome'}:{})});browsers.push(browser);
  for(const mode of ['background','foreground']){
   const rewritten=mode!=='direct';
   const context=await browser.newContext(engine==='webkit'?{viewport:{width:390,height:844},deviceScaleFactor:3,isMobile:true,hasTouch:true,userAgent:'Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1'}:{});
   const page=await context.newPage();await page.addInitScript(observeRtc);const errors=[],http=[];page.on('pageerror',e=>errors.push(e.message));
   await page.route('**/__rtc_probe',route=>route.fulfill({status:200,contentType:'text/html',body:'<!doctype html>Owned RTC diagnostic'}));
   if(rewritten){relay=await UdpProxy.open(lan,port);await page.route('**/salon/offer',async route=>{try{const response=await route.fetch();const answer=await response.json();const modified=relay.rewrite(answer);const headers=response.headers();http.push({status:response.status(),headers,actualOriginalBytes:Buffer.byteLength(JSON.stringify(answer)),rewrittenBytes:Buffer.byteLength(JSON.stringify(modified)),offer:route.request().postData(),answer,modified});if(mode==='fixed'){delete headers['content-length'];delete headers['content-encoding'];await route.fulfill({response,headers,json:modified});}else await route.fulfill({response,json:modified});}catch(error){http.push({error:error.message});await route.abort();}});}
   await page.goto(`http://${lan}:${port}/__rtc_probe`);
   const extras=[];for(let i=0;i<2;i++){const ctx=await browser.newContext();const other=await ctx.newPage();await other.route('**/__rtc_probe',route=>route.fulfill({status:200,contentType:'text/html',body:'<!doctype html>Owned competing page'}));await other.goto(`http://${lan}:${port}/__rtc_probe`);extras.push(ctx);}if(mode==='foreground')await page.bringToFront();
   const state=await page.evaluate(async()=>{
    const pc=new RTCPeerConnection(),events=[],candidates=[];
    const dc=pc.createDataChannel('salon',{ordered:false,maxRetransmits:0});dc.binaryType='arraybuffer';
    let opened=false,received=0;dc.onopen=()=>{opened=true;};dc.onmessage=()=>received++;
    pc.onicecandidate=e=>{if(e.candidate)candidates.push(e.candidate.candidate);};
    for(const name of ['icegatheringstatechange','iceconnectionstatechange','connectionstatechange'])pc.addEventListener(name,()=>events.push({at:performance.now(),name,ice:pc.iceConnectionState,gathering:pc.iceGatheringState,connection:pc.connectionState}));
    let offer,answer,error,stats;
    try{offer=await pc.createOffer();await pc.setLocalDescription(offer);const response=await fetch('/salon/offer',{method:'POST',body:JSON.stringify({type:'offer',sdp:offer.sdp})});answer=await response.json();await pc.setRemoteDescription(answer);await new Promise(r=>setTimeout(r,2500));stats=Array.from((await pc.getStats()).values());}
    catch(e){error=e.message;}
    const result={visibility:document.visibilityState,focused:document.hasFocus(),url:location.href,secure:isSecureContext,offer,answer,error,opened,received,candidates,events,ice:pc.iceConnectionState,connection:pc.connectionState,gathering:pc.iceGatheringState,stats};pc.close();return result;
   });
   const result={engine,mode,rewritten,state,http,errors,relay:relay?{stats:relay.snapshot(),errors:relay.errors}:null};results.push(result);console.log(JSON.stringify({engine,mode,rewritten,opened:state.opened,error:state.error,ice:state.ice,connection:state.connection,relay:result.relay,events:state.events}));
   await Promise.all(extras.map(ctx=>ctx.close()));await context.close();if(relay){await relay.close();relay=undefined;}
  }
 }
}finally{
 if(relay)await relay.close();await Promise.all(browsers.map(b=>b.close()));host.kill('SIGTERM');await once(host,'exit');writeFileSync('/tmp/rtc-real-relay-background-20261029.json',JSON.stringify({lan,port,results,output},null,2));
}
