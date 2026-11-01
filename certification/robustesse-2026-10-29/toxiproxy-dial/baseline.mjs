import {spawn} from 'node:child_process';
import net from 'node:net';
import {once} from 'node:events';
import {writeFileSync} from 'node:fs';
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
const listener=net.createServer();listener.listen(0,'127.0.0.1');await once(listener,'listening');const adminPort=listener.address().port;await new Promise(r=>listener.close(r));
const py=spawn('python3',['-u','-c','import socket,time\ns=socket.socket();s.bind(("127.0.0.1",0));s.listen(1);print(s.getsockname()[1],flush=True);time.sleep(120)'],{stdio:['ignore','pipe','pipe']});
let target='';py.stdout.on('data',b=>{target+=b.toString()});while(!target.includes('\n')) await sleep(10);const targetPort=+target.trim();
const sockets=[];let connected=0;for(let i=0;i<32;i++){const s=net.connect(targetPort,'127.0.0.1');s.on('error',()=>{});s.on('connect',()=>connected++);sockets.push(s)}await sleep(250);
const go=spawn('/opt/homebrew/bin/toxiproxy-server',['-host','127.0.0.1','-port',String(adminPort)],{stdio:['ignore','pipe','pipe']});let goLog='';go.stdout.on('data',b=>goLog+=b);go.stderr.on('data',b=>goLog+=b);const base=`http://127.0.0.1:${adminPort}`;
const api=async(method,path,body,timeout=28000)=>{const start=performance.now();const r=await fetch(base+path,{method,headers:{'content-type':'application/json'},body:body&&JSON.stringify(body),signal:AbortSignal.timeout(timeout)});const text=await r.text();return {ms:performance.now()-start,status:r.status,body:text}};
try{for(let i=0;i<50;i++){try{await api('GET','/version',undefined,500);break}catch{await sleep(100)}}
const created=await api('POST','/proxies',{name:'blocked',listen:'127.0.0.1:0',upstream:`127.0.0.1:${targetPort}`,enabled:true});const p=JSON.parse(created.body);const client=net.connect(+p.listen.split(':').pop(),'127.0.0.1');client.on('error',()=>{});sockets.push(client);await once(client,'connect');await sleep(200);
console.log(JSON.stringify({targetPort,backlogConnected:connected,pid:go.pid,version:await api('GET','/version')}));
const begin=performance.now();const disable=api('POST','/proxies/blocked',{enabled:false});await sleep(100);const independent=await api('GET','/version');const result=await disable;console.log(JSON.stringify({disable:result,independent,totalMs:performance.now()-begin}));
go.kill('SIGQUIT');await once(go,'exit');writeFileSync('/private/tmp/toxiproxy-backlog-go.log',goLog);console.log('saved /private/tmp/toxiproxy-backlog-go.log');
}finally{for(const s of sockets)s.destroy();py.kill();go.kill();}
