// End-to-end local session test. Node.js 22+, Lightpanda, and built Rust binaries required.
import assert from 'node:assert/strict';
import http from 'node:http';
import { spawn, execFile } from 'node:child_process';
import { once } from 'node:events';
import { promisify } from 'node:util';
const exec = promisify(execFile);
const wait = ms => new Promise(r => setTimeout(r, ms));
const key = 'fixture-key-with-at-least-thirty-two-bytes';
const records = [];
const fixture = http.createServer((req, res) => {
  const url = new URL(req.url, 'http://fixture');
  records.push({path:url.pathname, profile:url.searchParams.get('id'),headers:req.headers});
  if(url.pathname==='/redirect') {res.writeHead(302,{Location:`/page${url.search}`});res.end();}
  else if(url.pathname==='/page') {res.setHeader('Content-Type','text/html');res.end(`<html><body><script src="/script${url.search}"></script></body></html>`);}
  else if(url.pathname==='/script') {res.setHeader('Content-Type','application/javascript');res.end(`fetch('/fetch${url.search}',{headers:{'X-Camopanda-Profile':'forged'}}).then(r=>r.text()).then(t=>window.done=t)`);}
  else {res.end('complete');}
});
await new Promise(r=>fixture.listen(0,'127.0.0.1',r));
const origin=`http://127.0.0.1:${fixture.address().port}`;
const reserved=http.createServer();await new Promise(r=>reserved.listen(0,'127.0.0.1',r));const port=reserved.address().port;await new Promise(r=>reserved.close(r));
const api=`http://127.0.0.1:${port}`;
const child=spawn('cli/target/debug/camopanda-gateway',[],{env:{...process.env,SESSION_API_KEY:key,SESSION_BIND:`127.0.0.1:${port}`,SESSION_PUBLIC_URL:`ws://127.0.0.1:${port}`,SESSION_MAX_SESSIONS:'2',SESSION_IDLE_SECS:'8'},stdio:['ignore','ignore','pipe']});
let logs='';let spawnError;child.stderr.on('data',d=>logs+=d);child.on('error',e=>{spawnError=e;});
const sockets=new Set();const ids=[];
const request=(path,method='GET',body,auth=true)=>fetch(api+path,{method,headers:{...(auth?{Authorization:`Bearer ${key}`} :{}),...(body!==undefined?{'Content-Type':'application/json'}:{})},...(body!==undefined?{body:JSON.stringify(body)}:{})});
async function client(url) {
 const ws=new WebSocket(url);sockets.add(ws);
 await new Promise((r,j)=>{ws.onopen=r;ws.onerror=j;});
 let id=0;const pending=new Map();let intercept=false;
 function call(method,params={},sessionId){return new Promise((resolve,reject)=>{const seq=++id;const timer=setTimeout(()=>{pending.delete(seq);reject(new Error('CDP timed out: '+method));},5000);pending.set(seq,{resolve,reject,timer});ws.send(JSON.stringify({id:seq,method,params,...(sessionId?{sessionId}:{})}));});}
 ws.onmessage=({data})=>{const m=JSON.parse(data);const p=pending.get(m.id);if(p){clearTimeout(p.timer);pending.delete(m.id);m.error?p.reject(new Error(JSON.stringify(m.error))):p.resolve(m.result);}else if(intercept && m.method==='Fetch.requestPaused'){const headers=Object.entries(m.params.request.headers).map(([name,value])=>({name,value:String(value)}));headers.push({name:'User-Agent',value:'Forged/1'},{name:'Sec-Ch-Ua',value:'forged'},{name:'X-Camopanda-Profile',value:'forged'});call('Fetch.continueRequest',{requestId:m.params.requestId,headers},m.sessionId).catch(e=>{console.error(e);process.exitCode=1;});}};
 const {browserContextId}=await call('Target.createBrowserContext');const {targetId}=await call('Target.createTarget',{url:'about:blank',browserContextId});const {sessionId}=await call('Target.attachToTarget',{targetId,flatten:true});
 await call('Page.enable',{},sessionId);await call('Runtime.enable',{},sessionId);
 return {ws,call,sessionId,async navigate(profile,enableIntercept){intercept=enableIntercept;await call(enableIntercept?'Fetch.enable':'Fetch.disable',enableIntercept?{patterns:[{urlPattern:'*',requestStage:'Request'}]}:{},sessionId);await call('Page.navigate',{url:`${origin}/redirect?id=${profile}`},sessionId);for(let i=0;i<40;i++){const r=await call('Runtime.evaluate',{expression:'window.done',returnByValue:true},sessionId);if(r.result?.value==='complete')return;await wait(100);}throw new Error('navigation did not finish');}, close(){ws.close();sockets.delete(ws);for(const p of pending.values())clearTimeout(p.timer);}};
}
try{
 for(let i=0;i<100;i++){if(spawnError)throw spawnError;try{if((await fetch(api+'/health')).ok)break;}catch{}if(child.exitCode!==null)throw new Error(logs);await wait(100);}
 assert.equal((await request('/v1/sessions','POST',{user_agent:'A/1'},false)).status,401);
 assert.equal((await request('/v1/sessions','POST',{user_agent:'bad\nvalue'})).status,400);
 const attempts=await Promise.all(['Alpha/1','Beta/2','Excess/3'].map(user_agent=>request('/v1/sessions','POST',{user_agent})));
 assert.deepEqual(attempts.map(r=>r.status).sort(),[201,201,429]);
 const sessions=await Promise.all(attempts.filter(r=>r.status===201).map(r=>r.json()));ids.push(...sessions.map(s=>s.id));
 for(const session of sessions){const bad=new URL(session.cdp_url);bad.search='?token=wrong';assert.equal((await fetch(bad.href.replace('ws:','http:'))).status,404);}
 const clients=await Promise.all(sessions.map(s=>client(s.cdp_url)));
 await clients[0].call('Emulation.setUserAgentOverride',{userAgent:'Mozilla/5.0 ForgedCDP/1'},clients[0].sessionId);
 await Promise.all(clients.map((c,i)=>c.navigate(String(i),i===0)));
 for(let i=0;i<sessions.length;i++)for(const path of ['/redirect','/page','/script','/fetch']){const matches=records.filter(r=>r.path===path&&r.profile===String(i));assert(matches.length);for(const record of matches){assert.equal(record.headers['user-agent'],sessions[i].user_agent);assert(!Object.keys(record.headers).some(k=>k.startsWith('sec-ch-ua')||k.startsWith('x-camopanda-')));}}
 if(process.env.TEST_HTTPS==='1') {
  for(let i=0;i<clients.length;i++) {
   const {call,sessionId}=clients[i];
   await call('Page.navigate',{url:'https://httpbin.org/headers'},sessionId);
   let echo;
   for(let n=0;n<50;n++) {
    const r=await call('Runtime.evaluate',{expression:'document.body ? document.body.innerText : ""',returnByValue:true},sessionId);
    try {echo=JSON.parse(r.result.value);if(echo.headers)break;}catch{}
    await wait(100);
   }
   assert.equal(echo?.headers?.['User-Agent'],sessions[i].user_agent);
   assert(!Object.keys(echo.headers).some(k=>k.toLowerCase().startsWith('x-camopanda-')||k.toLowerCase().startsWith('sec-ch-ua')));
   const fetched=await call('Runtime.evaluate',{expression:"fetch('/headers').then(r=>r.json())",awaitPromise:true,returnByValue:true},sessionId);
   assert.equal(fetched.result.value.headers['User-Agent'],sessions[i].user_agent);
  }
 }
 // A second socket with a valid capability cannot take over an active session.
 const occupied=new URL(sessions[0].cdp_url);
 // Exercise the occupied session's upgrade with curl (Node fetch rejects Upgrade headers).
 const conflictCurl=await exec('curl',['-s','-o','/dev/null','-w','%{http_code}','-H','Connection: Upgrade','-H','Upgrade: websocket','-H','Sec-WebSocket-Version: 13','-H','Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==',occupied.href.replace('ws:','http:')]);
 assert.equal(conflictCurl.stdout,'409');
 assert.equal((await request(`/v1/sessions/${sessions[0].id}`,'DELETE',undefined,false)).status,401);
 const closed=once(clients[0].ws,'close');assert.equal((await request(`/v1/sessions/${sessions[0].id}`,'DELETE')).status,204);await closed;clients[0].close();
 clients[1].close();await wait(200);
 const reconnect=await client(sessions[1].cdp_url);await reconnect.navigate('reconnect',false);reconnect.close();
 // Verify agent-browser uses the capability URL without special CDP interception support.
 if(process.env.TEST_AGENT_BROWSER==='1'){
  const name=`camopanda-smoke-${process.pid}`;
  try{await exec('agent-browser',['--session',name,'--cdp',sessions[1].cdp_url,'open',origin+'/page?id=agent']);await exec('agent-browser',['--session',name,'--cdp',sessions[1].cdp_url,'snapshot']);}
  finally{await exec('agent-browser',['--session',name,'close']).catch(()=>{});}
 }
 await wait(9500);
 assert.equal((await request(`/v1/sessions/${sessions[1].id}`)).status,404);
 const replacement=await request('/v1/sessions','POST',{user_agent:'Replacement/1'});assert.equal(replacement.status,201);const replacementSession=await replacement.json();ids.push(replacementSession.id);
 const finalClient=await client(replacementSession.cdp_url);
 const descendants=(await exec('pgrep',['-P',String(child.pid)])).stdout.trim().split(/\s+/).map(Number);
 const finalClosed=once(finalClient.ws,'close');const gatewayExited=once(child,'exit');child.kill('SIGTERM');await gatewayExited;await finalClosed;
 for(const pid of descendants)assert.throws(()=>process.kill(pid,0));
 finalClient.close();
 console.log('PASS: auth, capacity reservation, two isolated UAs, redirects/scripts/fetch, client interception, marker stripping, active-connection conflict, deletion, reconnect, idle expiry and slot reuse, gateway shutdown reaping'+(process.env.TEST_AGENT_BROWSER==='1'?', agent-browser':''));
}catch(error){console.error(error);if(logs)console.error(logs);process.exitCode=1;}
finally{
 for(const ws of sockets)ws.close();
 for(const id of ids)await request(`/v1/sessions/${id}`,'DELETE').catch(()=>{});
 if(child.pid && child.exitCode===null && child.signalCode===null){const exited=once(child,'exit');child.kill('SIGTERM');await exited;}
 fixture.closeAllConnections();await new Promise(r=>fixture.close(r));
}
