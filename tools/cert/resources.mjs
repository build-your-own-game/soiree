// Resource retention is measured at repeated, settled boundaries, separately
// from frame timings. RSS includes Chromium's renderer and GPU processes.
import { execFileSync } from 'node:child_process';

export function observeWasmMemory() {
  const references = new Set(), seen = new WeakSet();
  const remember = memory => { if(memory instanceof WebAssembly.Memory && !seen.has(memory)) {seen.add(memory); references.add(new WeakRef(memory));} };
  const inspect = result => { const instance=result?.instance??result; if(instance instanceof WebAssembly.Instance) for(const value of Object.values(instance.exports)) remember(value); return result; };
  WebAssembly.Memory = new Proxy(WebAssembly.Memory, {construct(target,args,newTarget) {const memory=Reflect.construct(target,args,newTarget);remember(memory);return memory;}});
  for(const key of ['instantiate','instantiateStreaming']) {
    const original=WebAssembly[key];
    if(original) WebAssembly[key]=function(...args){return original.apply(this,args).then(inspect);};
  }
  globalThis.__certWasmBytes=()=>{let bytes=0;for(const reference of references){const memory=reference.deref();if(memory)bytes+=memory.buffer.byteLength;else references.delete(reference);}return bytes;};
}

export function resourceFailures(samples, budgets = {}) {
  if (samples.length < 3) return ['moins de trois mesures de ressources'];
  const failures=[];
  const first=samples[0];
  for (const [key, allowance] of Object.entries({nodes:1000, jsEventListeners:100, usedSize:32*1024*1024, backingStorageSize:32*1024*1024, wasm_bytes:32*1024*1024, chromium_rss_kib:128*1024, ...budgets})) {
    if(!Number.isFinite(allowance)||allowance<0) {failures.push(`${key} : budget invalide`);continue;}
    if (samples.some(sample=>!Number.isFinite(sample[key]) || sample[key]<0)) { failures.push(`${key} non mesuré`); continue; }
    const growth=Math.max(...samples.map(sample=>sample[key]))-first[key];
    if (growth>allowance) failures.push(`${key} : croissance ${growth} > budget ${allowance}`);
  }
  return failures;
}

export async function browserResources(browser, tab) {
  if (tab.engine !== 'chromium') throw new Error('ressources CDP : page Chromium requise');
  const instance=tab.context.browser();
  const browserSession=await instance.newBrowserCDPSession();
  const totals={documents:0,nodes:0,jsEventListeners:0,usedSize:0,totalSize:0,embedderHeapUsedSize:0,backingStorageSize:0,wasm_bytes:0};
  const pages=[];
  const isolates=new Set();
  try {
    for(const context of instance.contexts()) for(const page of context.pages()) {
      const session=await context.newCDPSession(page);
      try {
        await session.send('HeapProfiler.collectGarbage');
        const [dom,heap,{id:isolate}]=await Promise.all([session.send('Memory.getDOMCounters'),session.send('Runtime.getHeapUsage'),session.send('Runtime.getIsolateId')]);
        const wasm_bytes=await page.evaluate(()=>{if(typeof __certWasmBytes!=='function')throw new Error('WASM observer absent');return __certWasmBytes();});
        const values={...dom,...heap,wasm_bytes};
        for(const key of Object.keys(totals)) {
          if(!Number.isFinite(values[key])) throw new Error(`${key} non mesuré`);
          // CDP's heap/DOM counters belong to a renderer isolate, which same-site tabs may share.
          if(key==='wasm_bytes'||!isolates.has(isolate)) totals[key]+=values[key];
        }
        isolates.add(isolate);
        pages.push({url:page.url(),isolate,...values});
      } finally {await session.detach();}
    }
    const processes=await browserSession.send('SystemInfo.getProcessInfo');
    const ids=processes.processInfo.map(process=>process.id);
    const rss=execFileSync('ps',['-o','rss=','-p',ids.join(',')],{encoding:'utf8'}).trim().split(/\s+/).filter(Boolean).map(Number);
    if(!rss.length||rss.some(value=>!Number.isFinite(value)))throw new Error('RSS Chromium indisponible');
    return {at:Date.now(),...totals,pages,chromium_rss_kib:rss.reduce((a,b)=>a+b,0),processes:processes.processInfo.map(({id,type})=>({id,type}))};
  } finally {await browserSession.detach();}
}

/** Real repeated allocation/destruction detects retained subtrees and bitmaps.
 * It also validates that the observer is sensitive to deliberately retained DOM.
 * Product-specific import/export cycles belong to cert.json.ui.checks. */
export async function resourceCycle(page, retain = false) {
  await page.evaluate(async retain => {
    const root=document.createElement('div');
    root.style.cssText='position:fixed;left:-10000px;top:0';
    const images=[];
    try {
      document.body.append(root);
      for(let i=0;i<300;i++) { const node=document.createElement('button'); node.textContent=`resource ${i}`; node.addEventListener('click',()=>i); root.append(node); }
      const canvas=document.createElement('canvas'); canvas.width=256;canvas.height=256;
      canvas.getContext('2d').fillRect(0,0,256,256);
      for(let i=0;i<8;i++) images.push(await createImageBitmap(canvas));
      if(retain) (window.__certRetainedResources??=[]).push(root);
    } finally { for(const image of images) image.close(); if(!retain) root.remove(); }
  },retain);
}
