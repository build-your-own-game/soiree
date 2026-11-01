import test from 'node:test';
import assert from 'node:assert/strict';
import { AdvanceCycle, advanceOnce } from './progression.mjs';

test('a finite deck reverses at both boundaries and keeps its direction between them',()=>{
 const cycle=new AdvanceCycle();
 let cursor=3;
 const visited=[cursor];
 for(let step=0;step<9;step++){
  const direction=cycle.choose(cursor<3,cursor>0);
  cursor+=direction==='forward'?1:-1;
  visited.push(cursor);
 }
 assert.deepEqual(visited,[3,2,1,0,1,2,3,2,1,0]);
 assert.throws(()=>cycle.choose(false,false),/no declared/);
 assert.throws(()=>cycle.choose(undefined,true),/boolean/);
});

test('a failed native action propagates instead of becoming progress',async()=>{
 const tab={page:{evaluate:async()=>({before:'"question"',forward:true,backward:false})}};
 await assert.rejects(advanceOnce(tab,{key:'ArrowRight',progress:'heading'},new AdvanceCycle(),async()=>{throw new Error('native input failed');}),/native input failed/);
});
