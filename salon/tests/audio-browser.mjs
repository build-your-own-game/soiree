// Real production Web Audio graph, trusted gestures, bounded voices and lifecycle.
// From common root: node salon/tests/audio-browser.mjs
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { readFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { fileURLToPath } from 'node:url';
import { launchBrowser } from '../../tools/cert/browser.mjs';
const workspace=fileURLToPath(new URL('../',import.meta.url));
const output=new URL('../target/audio-browser/',import.meta.url);
function run(command,args){const result=spawnSync(command,args,{cwd:workspace,stdio:'inherit'});if(result.error)throw result.error;assert.equal(result.status,0,`${command} failed`)}
run('cargo',['build','-p','salon-web','--example','audio_probe','--features','salon-web/audio-test','--release','--target','wasm32-unknown-unknown']);
run('wasm-bindgen',['target/wasm32-unknown-unknown/release/examples/audio_probe.wasm','--target','web','--out-dir','target/audio-browser']);
const server=createServer(async(request,response)=>{
 const path=new URL(request.url,'http://localhost').pathname;
 if(path==='/')return response.writeHead(200,{'Content-Type':'text/html'}).end('<button id="gesture">Jouer</button><script type="module">import init,* as audio from "./audio_probe.js";await init();window.audio=audio;</script>');
 if(!['/audio_probe.js','/audio_probe_bg.wasm'].includes(path))return response.writeHead(404).end();
 try{response.writeHead(200,{'Content-Type':path.endsWith('.wasm')?'application/wasm':'text/javascript'}).end(await readFile(new URL(path.slice(1),output)))}catch{response.writeHead(500).end()}
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
let browser;
try{
 browser=await launchBrowser();
 for(const engine of ['chromium','webkit']){
  const tab=await browser.open(null,{engine});
  try{
   const errors=[];tab.page.on('pageerror',error=>errors.push(String(error)));
   await tab.page.addInitScript(()=>{
    window.audioTrace={contexts:[],started:0,disconnected:0};const Native=window.AudioContext;
    window.AudioContext=new Proxy(Native,{construct(target,args){
     const context=Reflect.construct(target,args);audioTrace.contexts.push(context);
     const gain=context.createGain.bind(context);
     context.createGain=()=>{if(window.failGain){window.failGain=false;throw new Error('injected gain failure')}return gain()};
     const create=context.createOscillator.bind(context);
     context.createOscillator=()=>{const oscillator=create(),start=oscillator.start.bind(oscillator),disconnect=oscillator.disconnect.bind(oscillator);oscillator.start=(...args)=>{audioTrace.started++;return start(...args)};oscillator.disconnect=(...args)=>{audioTrace.disconnected++;return disconnect(...args)};return oscillator};return context;
    }});
   });
   await tab.page.goto(`http://127.0.0.1:${server.address().port}/`);await tab.page.waitForFunction(()=>!!window.audio);
   const signal=await tab.page.evaluate(async()=>{const buffer=await audio.rendered(),data=buffer.getChannelData(0);return{peak:Math.max(...data.map(Math.abs)),energy:data.reduce((sum,n)=>sum+n*n,0),tail:Math.max(...data.slice(12000).map(Math.abs)),finite:data.every(Number.isFinite)}});
   assert.ok(signal.finite&&signal.peak>0.05&&signal.peak<=0.121&&signal.energy>1&&signal.tail<0.00001,`${engine}: invalid production waveform ${JSON.stringify(signal)}`);
   await tab.page.evaluate(()=>{audio.mount();audio.cue(0,0,0);audio.cue(1,0,0);document.getElementById('gesture').dispatchEvent(new PointerEvent('pointerdown',{bubbles:true}))});
   assert.equal(await tab.page.evaluate(()=>audioTrace.contexts.length),0,`${engine}: startup/synthetic gesture opened audio`);
   await tab.page.locator('#gesture').click();await tab.page.waitForFunction(()=>audioTrace.contexts.at(-1)?.state==='running');
   await tab.page.evaluate(()=>{audio.cue(1,0,0);audio.cue(2,0,1000)});assert.equal(await tab.page.evaluate(()=>audioTrace.started),0);
   await tab.page.evaluate(()=>{for(let id=3;id<100;id++)audio.cue(id,1000,1000)});assert.equal(await tab.page.evaluate(()=>audioTrace.started),4,`${engine}: voice budget exceeded`);
   await tab.page.waitForFunction(()=>audio.active()===0);assert.equal(await tab.page.evaluate(()=>audioTrace.disconnected),4,`${engine}: finished voices retained`);
   await tab.page.getByRole('button',{name:'Son activé · couper',exact:true}).click();await tab.page.evaluate(()=>audio.cue(100,1000,1000));
   await tab.page.getByRole('button',{name:'Son coupé · activer',exact:true}).click();await tab.page.evaluate(()=>audio.cue(100,1000,1000));
   assert.equal(await tab.page.evaluate(()=>audioTrace.started),4,`${engine}: mute accumulated backlog`);
   await tab.page.evaluate(()=>audio.cue(101,1000,1000));assert.equal(await tab.page.evaluate(()=>audioTrace.started),5);
   await tab.page.evaluate(async()=>{await audioTrace.contexts.at(-1).suspend();audio.cue(102,1000,1000)});
   await tab.page.locator('#gesture').click();await tab.page.waitForFunction(()=>audioTrace.contexts.at(-1).state==='running');
   await tab.page.evaluate(()=>{audio.cue(102,1000,1000);audio.discontinuity();audio.cue(0,1000,1000)});assert.equal(await tab.page.evaluate(()=>audioTrace.started),5,`${engine}: interruption/reconnect replayed cue`);
   await tab.page.evaluate(()=>audio.cue(1,1000,1000));assert.equal(await tab.page.evaluate(()=>audioTrace.started),6);
   await tab.page.evaluate(()=>audio.dispose());await tab.page.waitForFunction(()=>audioTrace.contexts.every(context=>context.state==='closed'));
   assert.equal(await tab.page.getByRole('group',{name:'Son du jeu'}).count(),0);assert.equal(await tab.page.evaluate(()=>audioTrace.disconnected),6);
   for(let n=0;n<5;n++){await tab.page.evaluate(()=>audio.mount());await tab.page.locator('#gesture').click();await tab.page.waitForFunction(()=>audioTrace.contexts.at(-1).state==='running');await tab.page.evaluate(()=>audio.dispose());await tab.page.waitForFunction(()=>audioTrace.contexts.every(context=>context.state==='closed'))}
   // A partially constructed graph must release its hardware and allow retry.
   await tab.page.evaluate(()=>{audio.mount();window.failGain=true});
   await tab.page.locator('#gesture').click();
   await tab.page.waitForFunction(()=>audioTrace.contexts.every(context=>context.state==='closed'));
   assert.equal(await tab.page.getByRole('button',{name:'Son indisponible · réessayer',exact:true}).count(),1);
   await tab.page.getByRole('button',{name:'Son indisponible · réessayer',exact:true}).click();
   await tab.page.waitForFunction(()=>audioTrace.contexts.at(-1).state==='running');
   await tab.page.evaluate(()=>{window.leakedGameKeys=0;document.addEventListener('keydown',()=>leakedGameKeys++)});
   await tab.page.getByRole('button',{name:'Son activé · couper',exact:true}).focus();
   await tab.page.keyboard.press('Enter');
   assert.equal(await tab.page.evaluate(()=>leakedGameKeys),0,'audio button key advanced a game shortcut');
   await tab.page.getByRole('slider',{name:'Volume du jeu'}).fill('30');
   await tab.page.evaluate(()=>{audio.dispose();audio.mount()});
   assert.equal(await tab.page.getByRole('button',{name:'Son coupé · activer',exact:true}).count(),1,'mute choice did not survive a new owner');
   assert.equal(await tab.page.getByRole('slider',{name:'Volume du jeu'}).inputValue(),'30');
   await tab.page.evaluate(()=>audio.dispose());
   await tab.page.waitForFunction(()=>audioTrace.contexts.every(context=>context.state==='closed'));
   assert.deepEqual(errors,[]);console.log(`${engine}: waveform, gestures, expiry/dedup, bounded voices, mute/interruption and cleanup passed`);
  }finally{await tab.close()}
 }
}finally{await browser?.close();await new Promise(resolve=>server.close(resolve))}
