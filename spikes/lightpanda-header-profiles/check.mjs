// Node.js 22+. Tests a patched Lightpanda directly, with no proxy.
import assert from 'node:assert/strict';
import http from 'node:http';
import https from 'node:https';
import { spawn, execFileSync } from 'node:child_process';
import { mkdtempSync, readFileSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { once } from 'node:events';

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const dir = mkdtempSync(join(tmpdir(), 'camopanda-native-headers-'));
const children = [], sockets = [], records = [];
const profiles = ['Mozilla/5.0 CamopandaAlpha/1', 'Mozilla/5.0 CamopandaBeta/2'];
const binary = process.env.LIGHTPANDA_BIN || 'lightpanda';
execFileSync('openssl', ['req', '-x509', '-newkey', 'rsa:2048', '-nodes', '-days', '1',
  '-keyout', join(dir, 'key.pem'), '-out', join(dir, 'cert.pem'), '-subj', '/CN=127.0.0.1',
  '-addext', 'subjectAltName=IP:127.0.0.1'], { stdio: 'ignore' });
function handler(req, res) {
  const u = new URL(req.url, 'http://fixture');
  records.push({ path: u.pathname, id: u.searchParams.get('id'), headers: req.headers });
  if (u.pathname === '/redirect') { res.writeHead(302, { Location: '/page' + u.search }); res.end(); }
  else if (u.pathname === '/page') { res.setHeader('Content-Type', 'text/html'); res.end(`<html><body><script src="/script${u.search}"></script></body></html>`); }
  else if (u.pathname === '/script') { res.setHeader('Content-Type', 'application/javascript'); res.end(`fetch('/fetch${u.search}').then(r=>r.text()).then(t=>window.done=t)`); }
  else res.end('complete');
}
const servers = [http.createServer(handler), https.createServer({ key: readFileSync(join(dir, 'key.pem')), cert: readFileSync(join(dir, 'cert.pem')) }, handler)];
for (const s of servers) await new Promise(resolve => s.listen(0, '127.0.0.1', resolve));
const origins = servers.map((s, i) => `${i ? 'https' : 'http'}://127.0.0.1:${s.address().port}`);

async function start(ua) {
  const reserve = http.createServer();
  await new Promise(resolve => reserve.listen(0, '127.0.0.1', resolve));
  const port = reserve.address().port;
  await new Promise(resolve => reserve.close(resolve));
  const child = spawn(binary, ['serve', '--host', '127.0.0.1', '--port', String(port),
    '--user-agent', ua, '--ca-cert', join(dir, 'cert.pem')], {
    env: { ...process.env, CAMOPANDA_TEST_HEADERS: '1', LIGHTPANDA_DISABLE_TELEMETRY: 'true' },
    stdio: ['ignore', 'ignore', 'pipe'],
  });
  children.push(child);
  let logs = '', spawnError;
  child.stderr.on('data', b => { logs += b; });
  child.on('error', e => { spawnError = e; });
  for (let i = 0; i < 150; i++) {
    if (spawnError) throw spawnError;
    if (child.exitCode !== null) throw new Error(`browser rejected testing profile: ${logs}`);
    try { if ((await fetch(`http://127.0.0.1:${port}/json/version`)).ok) return port; } catch {}
    await delay(100);
  }
  throw new Error('browser startup timed out: ' + logs);
}
async function check(port, ua, index) {
  const socket = new WebSocket(`ws://127.0.0.1:${port}/`);
  sockets.push(socket);
  const pending = new Map(); let sequence = 0;
  const interceptionErrors = [];
  function call(method, params = {}, sessionId) {
    return new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => { pending.delete(id); reject(new Error('CDP timeout: ' + method)); }, 10000);
      pending.set(id, { resolve, reject, timer });
      socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  }
  socket.onmessage = async ({ data }) => {
    const m = JSON.parse(data);
    if (m.id) {
      const p = pending.get(m.id); if (!p) return;
      pending.delete(m.id); clearTimeout(p.timer);
      if (m.error) p.reject(new Error(JSON.stringify(m.error))); else p.resolve(m.result);
    } else if (m.method === 'Fetch.requestPaused') {
      // Try to override the immutable profile and leak client hints/internal markers.
      const headers = Object.entries(m.params.request.headers).filter(([n]) => !/^(user-agent|sec-ch-ua|x-camopanda-)/i.test(n)).map(([name, value]) => ({ name, value: String(value) }));
      headers.push({ name: 'User-Agent', value: 'Forged/1' }, { name: 'Sec-Ch-Ua', value: 'forged' }, { name: 'Sec-Ch-Ua-Platform', value: 'forged' }, { name: 'X-Camopanda-Profile', value: 'internal' });
      try { await call('Fetch.continueRequest', { requestId: m.params.requestId, headers }, m.sessionId); } catch (e) { interceptionErrors.push(e.message); }
    }
  };
  try {
    await Promise.race([once(socket, 'open'), delay(10000).then(() => { throw new Error('WebSocket timeout'); })]);
    const context = (await call('Target.createBrowserContext')).browserContextId;
    // CLI profile, then CDP profile; each creates a fresh target.
    for (const mode of ['cli', 'cdp']) {
      const expected = mode === 'cli' ? ua : ua + ' CDP';
      const { targetId } = await call('Target.createTarget', { url: 'about:blank', browserContextId: context });
      const session = (await call('Target.attachToTarget', { targetId, flatten: true })).sessionId;
      await call('Page.enable', {}, session); await call('Runtime.enable', {}, session);
      if (mode === 'cdp') await call('Emulation.setUserAgentOverride', { userAgent: expected }, session);
      await call('Fetch.enable', { patterns: [{ urlPattern: '*', requestStage: 'Request' }] }, session);
      for (const [scheme, origin] of origins.entries()) {
        const id = `${index}-${mode}-${scheme}`;
        await call('Page.navigate', { url: `${origin}/redirect?id=${id}` }, session);
        let done = false;
        for (let i = 0; i < 100; i++) {
          const r = await call('Runtime.evaluate', { expression: 'window.done', returnByValue: true }, session);
          if (r.result?.value === 'complete' && records.some(row => row.id === id && row.path === '/fetch')) { done = true; break; } await delay(100);
        }
        assert(done, `page did not complete: ${id}`);
        for (const path of ['/redirect', '/page', '/script', '/fetch']) {
          const rows = records.filter(r => r.id === id && r.path === path);
          assert(rows.length, `missing ${id} ${path}`);
          for (const row of rows) {
            assert.equal(row.headers['user-agent'], expected, `${id} ${path} user agent`);
            assert(!Object.keys(row.headers).some(n => /^(sec-ch-ua|x-camopanda-)/i.test(n)), `${id} ${path} leaked reserved headers`);
          }
        }
      }
      await call('Target.closeTarget', { targetId });
    }
    assert.deepEqual(interceptionErrors, []);
    await call('Target.disposeBrowserContext', { browserContextId: context });
  } finally {
    for (const p of pending.values()) { clearTimeout(p.timer); p.reject(new Error('client closed')); }
    socket.close();
  }
}
try {
  const ports = await Promise.all(profiles.map(start));
  await Promise.all(ports.map((p, i) => check(p, profiles[i], i)));
  console.log(`PASS: ${records.length} direct HTTP/HTTPS requests; two processes; CLI and CDP profiles; redirects/scripts/fetch; interception; reserved-header stripping.`);
} catch (e) { console.error(e.message); process.exitCode = 1; }
finally {
  for (const s of sockets) s.close();
  await Promise.all(children.map(async c => {
    if (!c.pid || c.exitCode !== null || c.signalCode !== null) return;
    const exit = once(c, 'exit'); c.kill('SIGTERM'); await Promise.race([exit, delay(2000)]);
    if (c.exitCode === null && c.signalCode === null) { c.kill('SIGKILL'); await exit; }
  }));
  for (const s of servers) { s.closeAllConnections(); await new Promise(resolve => s.close(resolve)); }
  rmSync(dir, { recursive: true, force: true });
}
