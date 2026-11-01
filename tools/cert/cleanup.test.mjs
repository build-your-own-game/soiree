import test from "node:test";
import assert from "node:assert/strict";
import {cleanupAll,runWithCleanup} from "./cleanup.mjs";
import {FixtureFailure} from "./performance.mjs";

test("failed route registration still closes its UDP relay and keeps the original failure",async()=>{
 const primary=new Error("route registration failed");
 const calls=[];
 await assert.rejects(runWithCleanup(async()=>{throw primary;},[
  {name:"route",run:async()=>{calls.push("unroute");}},
  {name:"UDP",run:async()=>{calls.push("close relay");}},
 ]),error=>error===primary);
 assert.deepEqual(calls,["unroute","close relay"]);
});

test("failed unroute cannot prevent UDP closure or hide the primary product error",async()=>{
 const primary=new Error("product failed");
 let closed=false,health;
 await assert.rejects(runWithCleanup(async()=>{throw primary;},[
  {name:"route",run:async()=>{throw new Error("page closed");}},
  {name:"UDP",run:async()=>{closed=true;}},
 ],{onFailure:(error,original)=>{health={error,original};}}),error=>error===primary);
 assert.equal(closed,true);
 assert.equal(health.original,primary);
 assert.ok(health.error instanceof FixtureFailure);
 assert.match(health.error.message,/page closed/);
});

test("browser close rejection and timeout remain infrastructure failures",async()=>{
 for(const run of [async()=>{throw new Error("close rejected");},()=>new Promise(()=>{})]){
  await assert.rejects(runWithCleanup(async()=>"otherwise passed",[{name:"browser",run}],{timeoutMs:15}),FixtureFailure);
 }
 assert.deepEqual(await cleanupAll([{name:"closed",run:async()=>{}}]),[]);
});
