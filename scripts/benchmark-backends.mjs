// Optional local comparison. Node 22+, ps, patched Lightpanda and both CLI builds.
import assert from 'node:assert/strict';
import http from 'node:http';
import {spawn,execFileSync} from 'node:child_process';
import {once} from 'node:events';
import {mkdtempSync,rmSync} from 'node:fs';
import {tmpdir} from 'node:os';
import {join} from 'node:path';
const legacy=process.env.LEGACY_CAMOPANDA;
const direct=process.env.DIRECT_CAMOPANDA||'cli/target/debug/camopanda';
assert(legacy,'Set LEGACY_CAMOPANDA to the previous proxy-backed CLI');
assert(process.env.LIGHTPANDA_BIN,'Set LIGHTPANDA_BIN to the same patched browser for both backends');
const ua='Mozilla/5.0 CamopandaBenchmark/1';
const wait=ms=>new Promise(r=>setTimeout(r,ms));
const median=values=>[...values].sort((a,b)=>a-b)[Math.floor(values.length/2)];
const fixture=http.createServer((req,res)=>{
 assert.equal(req.headers['user-agent'],ua);
 if(req.url.startsWith('/page')) {res.setHeader('Content-Type','text/html');res.end(`<html><body>${req.url}</body></html>`);}
 else res.end('benchmark');
});
await new Promise(r=>fixture.listen(0,'127.0.0.1',r));
const origin=`http://127.0.0.1:${fixture.address().port}`;
function rss(root) {
 const rows=execFileSync('ps',['-axo','pid=,ppid=,rss='],{encoding:'utf8'}).trim().split('\n').map(r=>r.trim().split(/\s+/).map(Number));
 const pids=new Set([root]);let changed=true;while(changed){changed=false;for(const [pid,parent]of rows)if(pids.has(parent)&&!pids.has(pid)){pids.add(pid);changed=true;}}
 const selected=rows.filter(([pid])=>pids.has(pid));return {kib:selected.reduce((sum,r)=>sum+r[2],0),processes:selected.length};
}
async function sample(binary) {
 const state=mkdtempSync(join(tmpdir(),'camopanda-benchmark-'));
 const reserved=http.createServer();await new Promise(r=>reserved.listen(0,'127.0.0.1',r));const port=reserved.address().port;await new Promise(r=>reserved.close(r));
 const child=spawn(binary,['--state-dir',state,'--user-agent',ua,'serve','--host','127.0.0.1','--port',String(port)],{env:{...process.env,LIGHTPANDA_DISABLE_TELEMETRY:'true'},stdio:['ignore','ignore','pipe']});
 let logs='',error;child.stderr.on('data',d=>logs+=d);child.on('error',e=>error=e);
 let socket;const pending=new Map();
 try{
  let ready=false;for(let i=0;i<100;i++){if(error)throw error;if(child.exitCode!==null)throw new Error(logs);try{if((await fetch(`http://127.0.0.1:${port}/json/version`)).ok){ready=true;break;}}catch{}await wait(100);}assert(ready,'startup');
  const idle=rss(child.pid);
  socket=new WebSocket(`ws://127.0.0.1:${port}/`);await once(socket,'open');let sequence=0;
  function call(method,params={},sessionId){return new Promise((resolve,reject)=>{const id=++sequence;const timer=setTimeout(()=>{pending.delete(id);reject(new Error('CDP timeout '+method));},10000);pending.set(id,{resolve,reject,timer});socket.send(JSON.stringify({id,method,params,...(sessionId?{sessionId}:{})}));});}
  socket.onmessage=({data})=>{const m=JSON.parse(data),p=pending.get(m.id);if(p){pending.delete(m.id);clearTimeout(p.timer);m.error?p.reject(new Error(JSON.stringify(m.error))):p.resolve(m.result);}};
  const {browserContextId}=await call('Target.createBrowserContext');const {targetId}=await call('Target.createTarget',{url:'about:blank',browserContextId});const {sessionId}=await call('Target.attachToTarget',{targetId,flatten:true});await call('Page.enable',{},sessionId);await call('Runtime.enable',{},sessionId);
  const nav=[];
  for(let i=0;i<23;i++){const path='/page?i='+i,start=performance.now();await call('Page.navigate',{url:origin+path},sessionId);let loaded=false;for(let j=0;j<100;j++){const r=await call('Runtime.evaluate',{expression:'document.body && document.body.innerText',returnByValue:true},sessionId);if(r.result?.value===path){loaded=true;break;}await wait(1);}assert(loaded);if(i>=3)nav.push(performance.now()-start);}
  const fetchTimes=[];
  for(let i=0;i<23;i++){const r=await call('Runtime.evaluate',{expression:"(async()=>{const start=performance.now();await fetch('/fetch').then(r=>r.text());return performance.now()-start})()",awaitPromise:true,returnByValue:true},sessionId);assert.equal(typeof r.result.value,'number');if(i>=3)fetchTimes.push(r.result.value);}
  return {idle,loaded:rss(child.pid),navigation_ms:median(nav),fetch_ms:median(fetchTimes)};
 }finally{
  for(const p of pending.values())clearTimeout(p.timer);socket?.close();
  if(child.pid&&child.exitCode===null&&child.signalCode===null){const exit=once(child,'exit');child.kill('SIGTERM');await Promise.race([exit,wait(2000)]);if(child.exitCode===null&&child.signalCode===null){child.kill('SIGKILL');await exit;}}
  rmSync(state,{recursive:true,force:true});
 }
}
try{
 const results={legacy:[],direct:[]};
 for(let i=0;i<3;i++)for(const [name,binary]of(i%2?[['direct',direct],['legacy',legacy]]:[['legacy',legacy],['direct',direct]]))results[name].push(await sample(binary));
 const summary=Object.fromEntries(Object.entries(results).map(([name,rows])=>[name,{idle_rss_kib:median(rows.map(r=>r.idle.kib)),loaded_rss_kib:median(rows.map(r=>r.loaded.kib)),processes:rows[0].loaded.processes,navigation_ms:median(rows.map(r=>r.navigation_ms)),fetch_ms:median(rows.map(r=>r.fetch_ms))}]));
 console.log(JSON.stringify({scope:'Local HTTP only; native wrapper+browser tree RSS; three runs/backend; 20 warmed navigation and fetch samples/run. Same browser executable. No TLS or production workload comparison.',summary,samples:results},null,2));
}finally{fixture.closeAllConnections();await new Promise(r=>fixture.close(r));}
