// Reproducible CDP interception experiment. Requires Node.js 22+ and Lightpanda.
// This is a test harness, not a production profile-selection API.
import assert from 'node:assert/strict';
import http from 'node:http';
import { spawn } from 'node:child_process';
import { once } from 'node:events';

const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const profiles = ['alpha', 'beta'];
const records = [];
const events = [];
const clients = new Set();
const origin = http.createServer((request, response) => {
  const url = new URL(request.url, 'http://fixture');
  records.push({ path: url.pathname, profile: url.searchParams.get('id'),
    marker: request.headers['x-camopanda-profile'], userAgent: request.headers['user-agent'] });
  switch (url.pathname) {
    case '/redirect':
      response.writeHead(302, { Location: `/page${url.search}` });
      response.end();
      break;
    case '/page':
      response.setHeader('Content-Type', 'text/html');
      response.end(`<html><body><script src="/script${url.search}"></script></body></html>`);
      break;
    case '/script':
      response.setHeader('Content-Type', 'application/javascript');
      response.end(`fetch('/fetch${url.search}').then(r=>r.text()).then(t=>window.done=t)`);
      break;
    default:
      response.end('complete');
  }
});
await new Promise(resolve => origin.listen(0, '127.0.0.1', resolve));
const base = `http://127.0.0.1:${origin.address().port}`;
const reservation = http.createServer();
await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
const port = reservation.address().port;
await new Promise(resolve => reservation.close(resolve));
const proxyMode = process.env.USE_PROXY === '1';
const binary = proxyMode ? 'proxy/target/debug/camopanda' : (process.env.LIGHTPANDA_BIN || 'lightpanda');
const child = spawn(binary, ['serve', '--host', '127.0.0.1', '--port', String(port)], {
  env: { ...process.env, LIGHTPANDA_DISABLE_TELEMETRY: 'true' },
  stdio: ['ignore', 'ignore', 'pipe'],
});
let spawnError;
let logs = '';
child.on('error', error => { spawnError = error; });
child.stderr.on('data', data => { logs += data; });

async function runClient(profile) {
  const socket = new WebSocket(`ws://127.0.0.1:${port}/`);
  clients.add(socket);
  const pending = new Map();
  let sequence = 0;
  let context;
  let session;
  const interceptionErrors = [];
  function call(method, params = {}, sessionId) {
    return new Promise((resolve, reject) => {
      const id = ++sequence;
      const timer = setTimeout(() => {
        pending.delete(id);
        reject(new Error(`CDP timeout: ${method}`));
      }, 8000);
      pending.set(id, { resolve, reject, timer });
      socket.send(JSON.stringify({ id, method, params, ...(sessionId ? { sessionId } : {}) }));
    });
  }
  socket.onmessage = async ({ data }) => {
    const message = JSON.parse(data);
    if (message.id) {
      const waiter = pending.get(message.id);
      if (waiter) {
        clearTimeout(waiter.timer);
        pending.delete(message.id);
        if (message.error) waiter.reject(new Error(JSON.stringify(message.error)));
        else waiter.resolve(message.result);
      }
    } else if (message.method === 'Fetch.requestPaused') {
      events.push({ profile, type: message.params.resourceType, url: message.params.request.url });
      const headers = Object.entries(message.params.request.headers)
        .filter(([name]) => name.toLowerCase() !== 'x-camopanda-profile')
        .map(([name, value]) => ({ name, value: String(value) }));
      headers.push({ name: 'X-Camopanda-Profile', value: profile });
      try {
        await call('Fetch.continueRequest', { requestId: message.params.requestId, headers }, message.sessionId);
      } catch (error) { interceptionErrors.push(error.message); }
    }
  };
  try {
    await Promise.race([new Promise((resolve, reject) => {
      socket.onopen = resolve;
      socket.onerror = reject;
    }), delay(8000).then(() => { throw new Error('WebSocket connection timed out'); })]);
    context = (await call('Target.createBrowserContext')).browserContextId;
    const { targetId } = await call('Target.createTarget', { url: 'about:blank', browserContextId: context });
    session = (await call('Target.attachToTarget', { targetId, flatten: true })).sessionId;
    await call('Page.enable', {}, session);
    await call('Runtime.enable', {}, session);
    await call('Fetch.enable', { patterns: [{ urlPattern: '*', requestStage: 'Request' }] }, session);
    await call('Page.navigate', { url: `${base}/redirect?id=${profile}` }, session);
    let complete = false;
    for (let i = 0; i < 50; i++) {
      const result = await call('Runtime.evaluate', { expression: 'window.done', returnByValue: true }, session);
      if (result.result?.value === 'complete') { complete = true; break; }
      await delay(100);
    }
    assert(complete, `page did not complete for ${profile}`);
    assert.deepEqual(interceptionErrors, []);
  } finally {
    if (context && socket.readyState === WebSocket.OPEN) {
      await call('Target.disposeBrowserContext', { browserContextId: context }).catch(() => {});
    }
    for (const waiter of pending.values()) clearTimeout(waiter.timer);
    socket.close();
    clients.delete(socket);
  }
}

try {
  let version;
  for (let i = 0; i < 100; i++) {
    if (spawnError) throw spawnError;
    if (child.exitCode !== null) throw new Error(`browser exited: ${logs}`);
    try {
      const response = await fetch(`http://127.0.0.1:${port}/json/version`);
      if (response.ok) { version = await response.json(); break; }
    } catch {}
    await delay(100);
  }
  assert(version, 'browser startup timed out');
  const results = await Promise.allSettled(profiles.map(runClient));
  for (const result of results) if (result.status === 'rejected') throw result.reason;
  for (const profile of profiles) {
    for (const path of ['/redirect', '/page', '/script', '/fetch']) {
      const matches = records.filter(record => record.profile === profile && record.path === path);
      assert(matches.length > 0, `missing ${path} for ${profile}`);
      for (const record of matches) assert.equal(record.marker, profile);
    }
  }
  console.log(JSON.stringify({ mode: proxyMode ? 'proxy' : 'direct', version, records, events, passed: true }, null, 2));
} catch (error) {
  console.error(JSON.stringify({ error: error.message, records, events, logs }, null, 2));
  process.exitCode = 1;
} finally {
  for (const socket of clients) socket.close();
  if (child.pid && child.exitCode === null && child.signalCode === null) {
    const exited = once(child, 'exit');
    child.kill('SIGTERM');
    await Promise.race([exited, delay(2000)]);
    if (child.exitCode === null && child.signalCode === null) {
      child.kill('SIGKILL');
      await exited;
    }
  }
  origin.closeAllConnections();
  await new Promise(resolve => origin.close(resolve));
}
