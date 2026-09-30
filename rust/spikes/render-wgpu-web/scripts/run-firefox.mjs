// The same page in headless Firefox through geckodriver's WebDriver endpoint.
//
//   node run-firefox.mjs <geckodriver url> <origin> <out dir> [label] [--prefs]
//
// --prefs forces WebGPU on (`dom.webgpu.enabled`, `gfx.webgpu.ignore-blocklist`).
import { mkdirSync, writeFileSync } from 'node:fs';

const [driver, origin, out, label = 'firefox'] = process.argv.slice(2).filter((arg) => !arg.startsWith('--'));
mkdirSync(out, { recursive: true });
const call = async (method, path, body) => {
  const response = await fetch(`${driver}${path}`, {
    method,
    headers: { 'content-type': 'application/json' },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await response.json();
  if (json.value?.error) throw new Error(`${path}: ${json.value.error} ${json.value.message}`);
  return json.value;
};
const prefs = process.argv.includes('--prefs') ? { 'dom.webgpu.enabled': true, 'gfx.webgpu.ignore-blocklist': true, 'gfx.webrender.all': true, 'webgl.force-enabled': true } : {};
const session = await call('POST', '/session', {
  capabilities: { alwaysMatch: { 'moz:firefoxOptions': { args: ['-headless', '--width=1280', '--height=900'], prefs } } },
});
const id = session.sessionId;
const script = (source) => call('POST', `/session/${id}/execute/sync`, { script: source, args: [] });
try {
  await call('POST', `/session/${id}/url`, { url: `${origin}/index.html` });
  const deadline = Date.now() + 120000;
  let state;
  do {
    await new Promise((resolve) => setTimeout(resolve, 500));
    state = await script('return window.__spike?.state ?? null;');
  } while (!['done', 'failed'].includes(state) && Date.now() < deadline);
  const result = JSON.parse(await script('return JSON.stringify(window.__spike);'));
  result.userAgent = await script('return navigator.userAgent;');
  result.gpuAdapter = await call('POST', `/session/${id}/execute/async`, {
    script: `const done = arguments[arguments.length - 1];
      (navigator.gpu ? navigator.gpu.requestAdapter() : Promise.resolve(null))
        .then((a) => done(a ? { vendor: a.info?.vendor, architecture: a.info?.architecture } : null), (e) => done(String(e)));`,
    args: [],
  });
  if (result.state === 'done') {
    const element = await call('POST', `/session/${id}/element`, { using: 'css selector', value: '#engine' });
    const png = await call('GET', `/session/${id}/element/${Object.values(element)[0]}/screenshot`);
    writeFileSync(`${out}/${label}.png`, Buffer.from(png, 'base64'));
  }
  writeFileSync(`${out}/${label}.json`, JSON.stringify(result, null, 2));
  console.log(JSON.stringify(result, null, 2));
} finally {
  await call('DELETE', `/session/${id}`);
}
