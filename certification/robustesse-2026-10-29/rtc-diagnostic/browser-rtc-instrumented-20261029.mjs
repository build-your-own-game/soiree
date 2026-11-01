export * from '/Users/axel2/Projects/build-your-own-game/soiree/tools/cert/browser.mjs';
import {launchBrowser as original} from '/Users/axel2/Projects/build-your-own-game/soiree/tools/cert/browser.mjs';
export async function launchBrowser(options){
 const browser=await original(options);const open=browser.open.bind(browser);
 browser.open=async function(url,opts){const tab=await open(null,opts);await tab.page.addInitScript(()=>{
  const entries=globalThis.__rtcDiagnostic=[];const log=(type,data)=>{entries.push({at:Date.now(),type,...data});};
  const watched=new WeakSet();
  const watch=pc=>{if(watched.has(pc))return;watched.add(pc);for(const name of ['icecandidate','icecandidateerror','icegatheringstatechange','iceconnectionstatechange','connectionstatechange','signalingstatechange'])pc.addEventListener(name,event=>log(name,{candidate:event.candidate?.candidate,errorCode:event.errorCode,errorText:event.errorText,ice:pc.iceConnectionState,connection:pc.connectionState,gathering:pc.iceGatheringState,signaling:pc.signalingState}));};
  for(const name of ['createOffer','setLocalDescription','setRemoteDescription']){const native=RTCPeerConnection.prototype[name];RTCPeerConnection.prototype[name]=function(...args){watch(this);log(name,{args});return native.apply(this,args).then(result=>{log(`${name}:ok`,{result});return result;},error=>{log(`${name}:error`,{error:error.message});throw error;});};}
  for(const name of ['createDataChannel','close']){const native=RTCPeerConnection.prototype[name];RTCPeerConnection.prototype[name]=function(...args){watch(this);log(name,{args});return native.apply(this,args);};}
  const fetch=globalThis.fetch;globalThis.fetch=function(...args){const relevant=String(args[0]).includes('/salon/offer');if(relevant)log('fetch',{args});return fetch.apply(this,args).then(response=>{if(relevant){log('fetch:response',{status:response.status,headers:Array.from(response.headers)});response.clone().text().then(body=>log('fetch:body',{body}),error=>log('fetch:body:error',{error:error.message}));}return response;},error=>{if(relevant)log('fetch:error',{error:error.message});throw error;});};
 });if(url)await tab.goto(url);return tab;};return browser;
}
