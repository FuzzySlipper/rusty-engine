// #8736 exercise: a product exception after successful mutations, on the real
// supervised host with a browser attached. Usage:
//   node exception-exercise.mjs <rusty-product-host> <Product dir> <out.json> <label> <throwAt>
import { spawn } from 'node:child_process';
import { createRequire } from 'node:module';
import { writeFileSync } from 'node:fs';
const require = createRequire(new URL("../../../../render/package.json", import.meta.url));
const { chromium } = require('@playwright/test');
const [host, product, out, label, throwAt] = process.argv.slice(2);
const origin = 'http://127.0.0.1:40841';
const supervised = process.argv[7] === 'supervised';
const child = spawn(host, ['--product', product, '--loader', 'coreclr', ...(supervised ? ['--supervised'] : [])], {
  env: { ...process.env, BENCH_THROW_AT: throwAt }, stdio: ['pipe', 'pipe', 'pipe'], detached: true,
});
let log = '';
child.stdout.on('data', (d) => { log += d; });
child.stderr.on('data', (d) => { log += d; });
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const until = async (f, ms) => { const end = Date.now() + ms; while (Date.now() < end) { if (await f()) return true; await sleep(100); } return false; };
await until(async () => { try { return (await fetch(origin + '/')).ok; } catch { return false; } }, 60000);
const post = async (path, body = '{}', type = 'application/json') => {
  try {
    const response = await fetch(origin + path, { method: 'POST', headers: { 'Content-Type': type }, body });
    return { status: response.status, body: await response.text() };
  } catch (error) {
    return { status: null, body: JSON.stringify({ unreachable: String(error.cause?.code ?? error) }) };
  }
};
const json = (text) => { try { return JSON.parse(text); } catch { return { text: text.slice(0, 200) }; } };
const browser = await chromium.launch({ args: ['--use-gl=angle', '--use-angle=swiftshader'] });
const page = await browser.newPage({ viewport: { width: 960, height: 540 } });
await page.addInitScript(() => {
  const seen = { readouts: [], baselines: 0 };
  window.__seen = seen;
  const Base = window.EventSource;
  window.EventSource = class extends Base {
    constructor(...args) {
      super(...args);
      const record = (event) => {
        const batch = JSON.parse(event.data);
        for (const output of batch.outputs ?? []) {
          if (output.kind === 'runtime-readout') {
            seen.readouts.push({ at: Date.now(), state: output.readout.state, instance: output.readout.runtime.instanceId, generation: output.readout.runtime.generation, runtime: output.readout.runtime });
          }
        }
      };
      this.addEventListener('message', record);
      this.addEventListener('rusty-output-baseline', (event) => { seen.baselines += 1; record(event); });
    }
  };
});
const pageErrors = [];
page.on('pageerror', (e) => pageErrors.push(String(e).slice(0, 200)));
const statuses = [];
page.on('response', (r) => { if (r.status() >= 500) statuses.push({ url: new URL(r.url()).pathname, status: r.status() }); });
await page.goto(origin + '/');
const seen = () => page.evaluate(() => window.__seen);
const hostState = () => page.evaluate(() => document.body.dataset.rustyProductHostState ?? null);
await until(async () => (await hostState()) === 'ready', 30000);
const readyAt = Date.now();
const faulted = await until(async () => (await seen()).readouts.some((r) => r.state === 'faulted'), 20000);
await sleep(3000);
const afterFault = await seen();
const stateAfterFault = await hostState();
const diagnostics = json((await post('/__rusty/product/runtime/diagnostics/read')).body);
const exceptionEvent = (diagnostics.events ?? []).find((event) => String(event.message ?? '').includes('bench exception'));
const stepBefore = json((await post('/__rusty/product/runtime/debug/execute', 'engine.time', 'text/plain; charset=utf-8')).body);
const current = afterFault.readouts.at(-1)?.runtime;
const resume = await post('/__rusty/product/runtime/lifecycle/resume', JSON.stringify({ runtime: current }));
await sleep(3000);
const stepAfter = json((await post('/__rusty/product/runtime/debug/execute', 'engine.time', 'text/plain; charset=utf-8')).body);
const afterResume = await seen();
const result = {
  label,
  launch: supervised ? '--supervised' : 'direct',
  throwAt: Number(throwAt),
  faultedObservedInBrowser: faulted,
  secondsFromReadyToFault: afterFault.readouts.find((r) => r.state === 'faulted') ? (afterFault.readouts.find((r) => r.state === 'faulted').at - readyAt) / 1000 : null,
  readoutStates: afterResume.readouts.map((r) => `${r.state}@${r.instance}/${r.generation}`),
  runtimeInstances: [...new Set(afterResume.readouts.map((r) => r.instance))],
  baselines: afterResume.baselines,
  pageHostStateAfterFault: stateAfterFault,
  pageHostStateAfterResume: await hostState(),
  exceptionLogged: exceptionEvent ? { code: exceptionEvent.code, hasStackTrace: String(exceptionEvent.message).includes('at MovingWorkload.Product.Update'), firstLine: String(exceptionEvent.message).split('\n')[0] } : null,
  resumeStatus: resume.status,
  resumeAccepted: json(resume.body).accepted ?? null,
  resumeCode: json(resume.body).code ?? json(resume.body).unreachable ?? null,
  simulationStepBeforeResume: stepBefore.simulationStep ?? stepBefore,
  simulationStepAfterResume: stepAfter.simulationStep ?? stepAfter,
  serverErrors: statuses.slice(0, 10),
  supervisorRestarts: (log.match(/automatic restart|restarting the runtime|DEV_HOST_RUNTIME_EXIT/g) ?? []).length,
  pageErrors,
  logTail: log.split('\n').filter((line) => /BENCH|RUNTIME_EXIT|restart|pause/i.test(line)).slice(0, 12),
};
await page.screenshot({ path: out.replace(/\.json$/, '.png') });
writeFileSync(out, JSON.stringify(result, null, 2));
console.log(JSON.stringify(result, null, 2));
await browser.close();
try { child.stdin.end(); } catch {}
await sleep(2000);
try { process.kill(-child.pid, 'SIGKILL'); } catch {}
