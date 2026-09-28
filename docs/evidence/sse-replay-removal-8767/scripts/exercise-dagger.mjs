// #8767 visible exercise: rusty-dagger with no SSE replay (every connection
// gets a fresh baseline). Covers reload, a transient disconnect, a stalled
// subscriber next to a live page, and a runtime replacement by restage.
import { createRequire } from 'node:module';
import { appendFileSync, writeFileSync } from 'node:fs';
import net from 'node:net';
const require = createRequire('/home/agent/dev/rusty-engine/render/package.json');
const { chromium } = require('@playwright/test');

const [hostOrigin, out, touchFile] = process.argv.slice(2);
const { hostname, port } = new URL(hostOrigin);
// The page talks to the host through a local TCP proxy so a transient
// disconnect can be made real: the proxy drops every live connection. Both
// ports are five digits, so rewriting Host/Origin keeps byte lengths.
const proxyPort = Number(port) + 1;
const sockets = new Set();
const proxy = net.createServer((client) => {
  const upstream = net.connect(Number(port), hostname);
  sockets.add(client); sockets.add(upstream);
  client.on('data', (chunk) => upstream.write(Buffer.from(
    chunk.toString('latin1').replaceAll(`${hostname}:${proxyPort}`, `${hostname}:${port}`), 'latin1')));
  upstream.on('data', (chunk) => client.write(chunk));
  const close = () => { client.destroy(); upstream.destroy(); sockets.delete(client); sockets.delete(upstream); };
  client.on('close', close); upstream.on('close', close);
  client.on('error', close); upstream.on('error', close);
});
await new Promise((resolve) => proxy.listen(proxyPort, hostname, resolve));
const origin = `http://${hostname}:${proxyPort}/`;
const browser = await chromium.launch({
  args: ['--autoplay-policy=no-user-gesture-required', '--use-gl=angle', '--use-angle=swiftshader'],
});
const context = await browser.newContext({ viewport: { width: 1280, height: 720 } });
const page = await context.newPage();
await page.addInitScript(() => {
  const seen = { messages: 0, baselines: 0, binding: null, errors: 0 };
  window.__sse = seen;
  const Base = window.EventSource;
  window.EventSource = class extends Base {
    constructor(...args) {
      super(...args);
      this.addEventListener('rusty-output-baseline', (event) => {
        seen.baselines += 1;
        const r = JSON.parse(event.data).binding;
        if (r) seen.binding = `${r.instanceId}/${r.generation}/${r.controlRevision}`;
      });
      this.addEventListener('message', () => { seen.messages += 1; });
      this.addEventListener('error', () => { seen.errors += 1; });
    }
  };
});
const network = { fresh: 0, status503: 0, resource404: 0, resourceFetches: 0 };
page.on('request', (request) => {
  if (request.url().includes('/runtime/outputs/fresh')) network.fresh += 1;
  if (request.url().includes('/runtime/resource')) network.resourceFetches += 1;
});
page.on('response', (response) => {
  if (response.status() === 503) network.status503 += 1;
  if (response.url().includes('/runtime/resource') && response.status() === 404) network.resource404 += 1;
});
const errors = [];
page.on('pageerror', (error) => errors.push(String(error).slice(0, 200)));
page.on('console', (message) => {
  if (message.type() === 'error') errors.push(message.text().slice(0, 200));
});

const view = () => page.evaluate(() => ({
  hostState: document.body.dataset.rustyProductHostState ?? null,
  ...window.__sse,
  contextAlive: !(document.querySelector('canvas')?.getContext('webgl2')?.isContextLost() ?? true),
}));
const debug = (command) => page.evaluate(async (text) => JSON.parse(await (await fetch('/__rusty/product/runtime/debug/execute', {
  method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
})).text()), command);
const result = { checkpoints: [] };
const snapshot = async (label, extra = {}) => {
  const entry = { label, ...(await view()), step: Number((await debug('engine.time')).simulationStep),
    network: { ...network }, ...extra };
  result.checkpoints.push(entry);
  await page.screenshot({ path: `${out}/${String(result.checkpoints.length).padStart(2, '0')}-${label}.png` });
  return entry;
};
const waitReady = async (predicate, timeoutMs) => {
  const started = Date.now();
  while (Date.now() - started < timeoutMs) {
    const current = await view();
    if (current.hostState === 'ready' && predicate(current)) return (Date.now() - started) / 1000;
    await page.waitForTimeout(200);
  }
  return null;
};
const messagesOver = async (ms) => {
  const before = (await view()).messages;
  await page.waitForTimeout(ms);
  return (await view()).messages - before;
};

await page.goto(origin);
await page.waitForTimeout(6000);
const attached = await snapshot('attached', { messagesPerSecond: await messagesOver(1000) });

// Reload: a fresh attachment to the same incarnation.
await page.reload();
const reloadSeconds = await waitReady((v) => v.baselines >= 1, 30000);
await page.waitForTimeout(1500);
await snapshot('reloaded', { seconds: reloadSeconds, messagesPerSecond: await messagesOver(1000) });

// Transient disconnect: every connection through the proxy is cut.
const beforeDisconnect = await view();
const dropped = sockets.size;
for (const socket of [...sockets]) socket.destroy();
const disconnectSeconds = await waitReady((v) => v.baselines > beforeDisconnect.baselines, 30000);
await page.waitForTimeout(1500);
await snapshot('reconnected', { droppedSockets: dropped, seconds: disconnectSeconds,
  streamErrors: (await view()).errors - beforeDisconnect.errors, messagesPerSecond: await messagesOver(1000) });

// Stalled subscriber: a raw SSE client stops reading for 10 s beside the page.
const stalled = await new Promise((resolve) => {
  const socket = net.connect(Number(port), hostname, () => {
    socket.write(`GET /__rusty/product/runtime/outputs/fresh HTTP/1.1\r\nHost: ${hostname}:${port}\r\nAccept: text/event-stream\r\n\r\n`);
  });
  let received = 0;
  let closedAt = null;
  const opened = Date.now();
  socket.on('data', (chunk) => {
    received += chunk.length;
    if (received > 0 && !socket.isPaused() && socket.stalledOnce !== true) {
      socket.stalledOnce = true;
      socket.pause();
      setTimeout(() => socket.resume(), 10000);
    }
  });
  socket.on('close', () => { closedAt = (Date.now() - opened) / 1000; });
  setTimeout(() => { resolve({ received, closedAfterSeconds: closedAt }); socket.destroy(); }, 14000);
});
const duringStall = await snapshot('stalled-subscriber', { stalled, messagesPerSecond: await messagesOver(1000) });

// Runtime replacement: rusty dev restages after a source edit.
const beforeBinding = (await view()).binding;
const edited = Date.now();
appendFileSync(touchFile, `\n// #8767 restage ${edited}\n`);
const replaceSeconds = await waitReady((v) => v.binding !== beforeBinding && v.binding !== null, 120000);
await page.waitForTimeout(3000);
await snapshot('replaced', { beforeBinding, secondsFromEdit: replaceSeconds, messagesPerSecond: await messagesOver(1000) });

result.errors = errors;
writeFileSync(`${out}/result.json`, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
await browser.close();
proxy.close();
for (const socket of sockets) socket.destroy();
void attached; void duringStall;
