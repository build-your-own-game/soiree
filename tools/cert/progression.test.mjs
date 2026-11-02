import test from 'node:test';
import assert from 'node:assert/strict';
import { AdvanceCycle, advanceOnce, advanceUntil } from './progression.mjs';

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

test('a running game is driven to the declared screen, and stops there',async()=>{
 let screen='vote', steps=0;
 const order={vote:'reveal',reveal:'end',end:'lobby',lobby:'starting'};
 const reached=await advanceUntil(async()=>screen==='lobby',async()=>{steps++;screen=order[screen];},{maxSteps:10});
 assert.equal(reached,3);
 assert.equal(screen,'lobby','one more step would have left the screen it was looking for');
 assert.equal(steps,3);
});

test('a screen already there costs no step',async()=>{
 assert.equal(await advanceUntil(async()=>true,async()=>{throw new Error('no step needed');},{maxSteps:10}),0);
});

test('a screen the game never reaches fails instead of measuring another one',async()=>{
 let steps=0;
 await assert.rejects(advanceUntil(async()=>false,async()=>{steps++;},{maxSteps:4}),/4 étapes/);
 assert.equal(steps,4);
 await assert.rejects(advanceUntil(async()=>false,undefined,{maxSteps:4}),/aucun moyen d'avancer/);
});

test('a step that fails is the failure',async()=>{
 await assert.rejects(advanceUntil(async()=>false,async()=>{throw new Error('touche refusée');},{maxSteps:4}),/touche refusée/);
});
