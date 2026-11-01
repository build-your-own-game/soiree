import test from 'node:test';
import assert from 'node:assert/strict';
import { resourceFailures, browserResources, resourceCycle } from './resources.mjs';
import { launchBrowser } from './browser.mjs';
const sample={nodes:10,jsEventListeners:2,usedSize:100,backingStorageSize:100,wasm_bytes:0,chromium_rss_kib:100};
test('resource evaluation rejects missing data and retained growth',()=>{
 assert.deepEqual(resourceFailures([sample,sample,sample]),[]);
 assert.ok(resourceFailures([sample,sample,{...sample,nodes:5000}]).some(x=>x.includes('nodes')));
 assert.ok(resourceFailures([sample,sample,{...sample,backingStorageSize:undefined}]).some(x=>x.includes('non mesuré')));
 assert.ok(resourceFailures([]).length);
 assert.ok(resourceFailures([sample,sample,sample],{nodes:NaN}).some(x=>x.includes('budget invalide')));
 assert.ok(resourceFailures([sample,{...sample,nodes:5000},sample]).some(x=>x.includes('nodes')),'intermediate retained peak hidden');
});
test('actual browser resource observer detects retained DOM and WASM backing stores', {timeout:30000}, async()=>{
 const browser=await launchBrowser({webkitPhones:false});
 try {
  const tab=await browser.open('about:blank');
  const baseline=await browserResources(browser,tab);
  await resourceCycle(tab.page);
  const cleaned=await browserResources(browser,tab);
  assert.ok(cleaned.nodes-baseline.nodes<30);
  for(let i=0;i<5;i++) await resourceCycle(tab.page,true);
  await tab.page.evaluate(()=>window.retainedWasm=new WebAssembly.Memory({initial:768}));
  const leaked=await browserResources(browser,tab);
  assert.ok(resourceFailures([baseline,cleaned,leaked]).some(x=>x.includes('nodes')));
  assert.ok(leaked.wasm_bytes-baseline.wasm_bytes>32*1024*1024,'WASM memory not observed');
  assert.ok(resourceFailures([baseline,cleaned,leaked]).some(x=>x.includes('wasm_bytes')));
 }finally{await browser.close();}
});
