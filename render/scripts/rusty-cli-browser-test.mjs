import { chromium } from '@playwright/test';

const origin = process.argv[2];
let loopbackOrigin;
try {
  loopbackOrigin = new URL(origin);
} catch {
  throw new Error('expected one explicit Product Dev Host loopback origin');
}
if (
  loopbackOrigin.protocol !== 'http:'
  || loopbackOrigin.hostname !== '127.0.0.1'
  || loopbackOrigin.port === ''
  || loopbackOrigin.pathname !== '/'
  || loopbackOrigin.search !== ''
  || loopbackOrigin.hash !== ''
) {
  throw new Error('expected one explicit Product Dev Host loopback origin');
}

const HOST_STABILITY_OBSERVATION_MS = 1_000;

const browser = await chromium.launch({ headless: true });
try {
  const page = await browser.newPage();
  const failures = [];
  page.on('pageerror', (error) => failures.push(`pageerror: ${error.message}`));
  page.on('console', (message) => {
    if (message.type() === 'error') failures.push(`console: ${message.text()}`);
  });
  // The Product Browser Host owns a long-lived SSE output stream, so network
  // idle is intentionally not a valid readiness signal.
  const response = await page.goto(origin, { waitUntil: 'domcontentloaded', timeout: 30_000 });
  if (!response?.ok()) throw new Error(`browser root returned ${response?.status() ?? 'no response'}`);
  if (new URL(page.url()).origin !== loopbackOrigin.origin) {
    throw new Error(`browser root redirected outside the Product Dev Host loopback origin: ${page.url()}`);
  }
  await page.waitForSelector('canvas', { state: 'attached', timeout: 30_000 });
  const canvas = page.locator('canvas');
  const canvasCount = await canvas.count();
  if (canvasCount !== 1) throw new Error(`expected exactly one Engine canvas, found ${canvasCount}`);
  const mountedCanvas = await canvas.elementHandle();
  if (mountedCanvas === null) throw new Error('Engine canvas detached before host stability observation');
  await page.waitForTimeout(HOST_STABILITY_OBSERVATION_MS);
  const stableCanvasCount = await canvas.count();
  const originalCanvasIsStillMounted = await mountedCanvas.evaluate((element) => element.isConnected);
  if (stableCanvasCount !== 1 || !originalCanvasIsStillMounted) {
    throw new Error('Engine host did not retain one stable mounted canvas during bounded observation');
  }

  // Any Product Model consumer may use a different UI and intent vocabulary.
  // The counter hooks below belong only to the conformance fixture; when both
  // hooks are present, preserve its stronger UI, physical-input, and lifecycle
  // proof without requiring that product vocabulary from ordinary `rusty test`.
  const counter = page.locator('[data-rusty-test-counter]');
  const increment = page.locator('[data-rusty-test-increment]');
  const counterHookCount = await counter.count();
  const incrementHookCount = await increment.count();
  const hasConformanceHooks = counterHookCount === 1 && incrementHookCount === 1;
  if (!hasConformanceHooks && (counterHookCount !== 0 || incrementHookCount !== 0)) {
    throw new Error('expected either no conformance hooks or exactly one counter and increment hook');
  }
  let conformanceProof = false;
  if (hasConformanceHooks) {
    conformanceProof = true;
    await page.waitForFunction(() => {
      const value = document.querySelector('[data-rusty-test-counter]')?.textContent ?? '';
      return value.includes('observed=1') && value.includes('recurring=1');
    }, undefined, { timeout: 30_000 });
    const counterValue = async () => {
      const text = await counter.textContent();
      const match = /^value=(\d+);/.exec(text ?? '');
      if (match === null) throw new Error(`counter projection has no value: ${text ?? 'null'}`);
      return Number(match[1]);
    };
    const initialValue = await counterValue();
    if (initialValue < 2) throw new Error(`expected released timeline increment before browser intent proof, found ${initialValue}`);
    // Product UI and physical keyboard input converge on the exact same
    // declared `increment` intent. The browser drives normal public controls;
    // it does not receive a test-only mutation route.
    await increment.click();
    await page.waitForFunction((before) => {
      const text = document.querySelector('[data-rusty-test-counter]')?.textContent ?? '';
      const match = /^value=(\d+);/.exec(text);
      return match !== null && Number(match[1]) > before;
    }, initialValue, { timeout: 30_000 });
    const afterUi = await counterValue();
    // The semantic button retains DOM focus after its click. Return focus to
    // the application canvas before proving the physical keyboard lane. The UI
    // overlay intentionally owns its own pointer layer, so use its accessible
    // focus target rather than trying to pierce that presentation boundary.
    await page.locator('canvas').focus();
    await page.keyboard.press('KeyW');
    await page.waitForFunction((before) => {
      const text = document.querySelector('[data-rusty-test-counter]')?.textContent ?? '';
      const match = /^value=(\d+);/.exec(text);
      return match !== null && Number(match[1]) > before;
    }, afterUi, { timeout: 30_000 });
    const afterPhysical = await counterValue();
    const lifecycle = await page.evaluate(async () => {
      const request = async (path, body) => {
        const response = await fetch(`/__rusty/product/runtime/${path}`, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(body),
        });
        if (!response.ok) throw new Error(`${path} returned ${response.status}`);
        return response.json();
      };
      const restarted = await request('lifecycle/restart', {});
      if (restarted.accepted !== true || restarted.binding === undefined) {
        throw new Error(`restart was rejected: ${JSON.stringify(restarted)}`);
      }
      const fresh = restarted.binding;
      const stale = {
        ...fresh,
        generation: (BigInt(fresh.generation) - 1n).toString(),
      };
      const claim = (runtime, sequence) => ({
        batch: [{ runtime, sequence: String(sequence), context: 'gameplay.default', intent: 'increment', value: { kind: 'digital', active: true } }],
      });
      const staleResult = await request('input', claim(stale, 900));
      const paused = await request('lifecycle/pause', {});
      const resumed = await request('lifecycle/resume', {});
      if (paused.accepted !== true || resumed.accepted !== true || resumed.binding === undefined) {
        throw new Error(`pause/resume was rejected: ${JSON.stringify({ paused, resumed })}`);
      }
      const staleRevisionResult = await request('input', claim(fresh, 901));
      // A rebind itself occupies sequence zero. A rejected foreign binding
      // clears only transient ingress, so the valid binding resumes at sequence
      // one and proves it can continue without inheriting stale facts.
      const freshResult = await request('input', claim(resumed.binding, 1));
      // Drive one ordinary realtime step through the public host. The browser
      // cadence may still have an old transport clock after the direct restart,
      // so seed this fresh runtime with a monotonic value beyond the page clock
      // and then admit exactly one fixed step.
      const baselineAdvance = await request('advance-realtime', { observedTimeNs: '1000000000000' });
      const freshAdvance = await request('advance-realtime', { observedTimeNs: '1000016666667' });
      return { restarted, staleResult, staleRevisionResult, freshResult, baselineAdvance, freshAdvance };
    });
    if (lifecycle.staleResult.accepted !== false || lifecycle.staleRevisionResult.accepted !== false || lifecycle.freshResult.accepted !== true || lifecycle.baselineAdvance.accepted !== true || lifecycle.freshAdvance.accepted !== true) {
      throw new Error(`lifecycle did not reject stale generation/revision input and accept fresh input: ${JSON.stringify({
        staleGeneration: lifecycle.staleResult.diagnostic,
        staleControlRevision: lifecycle.staleRevisionResult.diagnostic,
        fresh: lifecycle.freshResult,
        baselineAdvance: lifecycle.baselineAdvance,
        freshAdvance: lifecycle.freshAdvance,
      })}`);
    }
    // Normal browser-owned realtime admission now consumes the fresh direct
    // claim. Timeline increments are deliberately even, so a changed parity is
    // exact evidence of the one accepted `increment`; both rejected stale
    // claims report zero accepted events and cannot contribute a mutation.
    await page.waitForFunction((beforeParity) => {
      const text = document.querySelector('[data-rusty-test-counter]')?.textContent ?? '';
      const match = /^value=(\d+);/.exec(text);
      return match !== null && Number(match[1]) % 2 !== beforeParity;
    }, afterPhysical % 2, { timeout: 30_000 });
  }
  if (failures.length > 0) throw new Error(failures.join('\n'));
  process.stdout.write(`${JSON.stringify(conformanceProof
    ? { status: 'ok', origin, canvasCount, observationMs: HOST_STABILITY_OBSERVATION_MS, inputPaths: ['ui', 'physical-w'], lifecycle: 'restart-and-control-revision-stale-rejected-fresh-accepted' }
    : { status: 'ok', origin, canvasCount, observationMs: HOST_STABILITY_OBSERVATION_MS, proof: 'loopback-root-one-stable-engine-canvas-no-page-errors' })}\n`);
} finally {
  await browser.close();
}
