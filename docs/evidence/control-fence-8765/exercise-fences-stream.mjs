// #8765 on the runtime-rendered browser shell (after #8792): gameplay input,
// held input across a menu and a control fence, remapping, and pause with the
// menu and an observer camera, on a live Dagger world served by `rusty dev`.
//
// Run through the warning capture so its window spans every fence:
//   EXERCISE_OUT=<dir> node scripts/capture-playtest-warning-delta.mjs \
//     --url http://127.0.0.1:4394/ --exercise-id 8765-fences \
//     --exercise docs/evidence/control-fence-8765/exercise-fences-stream.mjs \
//     --engine-origin http://127.0.0.1:4394 --output <dir>/warning-capture.json
import { writeFileSync } from 'node:fs';

const TITLE_TIMEOUT_MS = 360_000;
const HOLD_MS = 1_200;
const SETTLE_MS = 600;

export async function exercise({ page, url }) {
  const out = process.env.EXERCISE_OUT;
  if (!out) throw new Error('EXERCISE_OUT names the result directory');

  // The page's own output stream and input posts, observed without changing them.
  await page.addInitScript(() => {
    const ex = { streams: 0, bindings: [], projections: 0, projectionRevisions: [], readouts: 0 };
    window.__ex = ex;
    const Base = window.EventSource;
    window.EventSource = class extends Base {
      constructor(source, init) {
        super(source, init);
        ex.streams += 1;
        this.addEventListener('message', (event) => {
          for (const output of JSON.parse(event.data)) {
            if (output.kind === 'binding') ex.bindings.push(output.runtime.controlRevision);
            if (output.kind === 'ui-projection') {
              ex.projections += 1;
              const revision = output.envelope.runtime.controlRevision;
              if (ex.projectionRevisions.at(-1) !== revision) ex.projectionRevisions.push(revision);
            }
            if (output.kind === 'runtime-readout') ex.readouts += 1;
          }
        });
      }
    };
  });
  const input = { keyFacts: 0, clears: [] };
  let binding = null;
  page.on('request', (request) => {
    if (!request.url().includes('/runtime/input')) return;
    for (const entry of JSON.parse(request.postData() ?? '{}').batch ?? []) {
      binding = entry.runtime;
      if (entry.fact?.kind === 'key') input.keyFacts += 1;
      if (entry.fact?.kind === 'clear') input.clears.push(`${entry.runtime.controlRevision}:${entry.fact.reason}`);
    }
  });

  const debug = async (command) => JSON.parse(await page.evaluate(async (text) => (await fetch('/__rusty/product/runtime/debug/execute', {
    method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: text,
  })).text(), command));
  const world = async () => {
    const observed = await debug('playtest.observe');
    return {
      mode: observed.mode,
      step: Number((await debug('engine.time')).simulationStep),
      position: observed.player.position,
      yawDegrees: observed.player.yawDegrees,
      forward: observed.controls['move.forward'],
    };
  };
  const post = async (route, body) => page.evaluate(async ([path, payload]) => JSON.parse(await (await fetch(`/__rusty/product/runtime/${path}`, {
    method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(payload),
  })).text()), [route, body]);
  const fence = async (route) => {
    const result = await post(route, { runtime: binding });
    return { route, accepted: result.accepted, code: result.code, controlRevision: result.binding?.controlRevision };
  };
  const shell = async () => page.evaluate(() => {
    const canvas = document.querySelector('canvas[data-rusty-application-renderer="engine-owned"]');
    window.__canvas ??= canvas;
    return {
      ...window.__ex,
      sameCanvas: window.__canvas === canvas,
      frameSequence: Number(canvas?.dataset.rustyFrameSequence ?? 0),
      interaction: document.activeElement?.tagName ?? null,
      menuOpen: document.querySelector('dialog.dagger-menu')?.open ?? false,
    };
  });
  const renderer = async () => (await debug('engine.renderer.presentation')).presentation?.surfaceId ?? null;
  const checkpoints = [];
  const checkpoint = async (label) => {
    const entry = { label, ...(await shell()), ...(await world()), controlRevision: binding?.controlRevision ?? null, renderer: await renderer() };
    await page.screenshot({ path: `${out}/${label}.png` });
    checkpoints.push(entry);
    return entry;
  };
  const distance = (a, b) => Math.round(Math.hypot(b.x - a.x, b.z - a.z) * 100) / 100;
  const focusGame = async () => { await page.mouse.click(640, 400); await page.waitForTimeout(SETTLE_MS); };
  const walk = async (code) => {
    await focusGame();
    const before = (await world()).position;
    await page.keyboard.down(code);
    await page.waitForTimeout(HOLD_MS);
    await page.keyboard.up(code);
    await page.waitForTimeout(SETTLE_MS);
    return distance(before, (await world()).position);
  };
  // Holds `code`, runs `interrupt` while it is held, and measures the movement
  // while the key stays down afterwards: a cleared hold moves nothing.
  const heldAcross = async (code, interrupt, restore) => {
    await focusGame();
    const start = (await world()).position;
    await page.keyboard.down(code);
    await page.waitForTimeout(HOLD_MS);
    const moving = (await world()).position;
    const interruption = await interrupt();
    await page.waitForTimeout(SETTLE_MS);
    const during = (await world()).position;
    await page.waitForTimeout(HOLD_MS);
    const duringEnd = (await world()).position;
    if (restore !== undefined) await restore();
    await page.waitForTimeout(SETTLE_MS);
    const after = (await world()).position;
    await page.waitForTimeout(HOLD_MS);
    const afterEnd = (await world()).position;
    await page.keyboard.up(code);
    return {
      interruption,
      movedBeforeInterrupt: distance(start, moving),
      movedWhileInterrupted: distance(during, duringEnd),
      movedAfterRestoreStillHeld: distance(after, afterEnd),
    };
  };
  const rebindForward = async (code) => {
    await page.keyboard.press('Escape');
    await page.waitForTimeout(SETTLE_MS);
    await page.getByRole('button', { name: 'Control settings' }).click();
    await page.waitForTimeout(SETTLE_MS);
    await page.getByRole('button', { name: 'Rebind' }).first().click();
    await page.waitForTimeout(400);
    await page.keyboard.press(code);
    await page.waitForTimeout(SETTLE_MS);
    await page.keyboard.press('Escape');
    await page.waitForTimeout(400);
    await page.getByRole('button', { name: 'Return to game' }).click();
    await page.waitForTimeout(SETTLE_MS);
  };

  await page.goto(url);
  await page.waitForTimeout(6_000);
  await checkpoint('attached');
  if ((await world()).mode !== 'Playing') {
    // The ordinary title screen: Begin, then the opening cinematic plays out.
    await page.getByRole('button', { name: 'Begin' }).click();
    const started = Date.now();
    while ((await world()).mode !== 'Playing' && Date.now() - started < TITLE_TIMEOUT_MS) await page.waitForTimeout(2_000);
  }
  await focusGame();
  await checkpoint('playing');

  const result = {};
  // 1. One working gameplay action.
  result.walkW = await walk('KeyW');
  await checkpoint('walked-w');

  // 2. Held input across the menu: opening it takes input to the interface.
  result.heldAcrossMenu = await heldAcross('KeyW',
    async () => { await page.keyboard.press('Escape'); return 'menu opened'; },
    async () => { await page.keyboard.press('Escape'); await focusGame(); });
  result.walkAfterMenu = await walk('KeyW');
  await checkpoint('after-menu');

  // 3. Held input across a control fence.
  result.heldAcrossControlReplace = await heldAcross('KeyW', () => fence('control/replace'));
  result.walkAfterControlReplace = await walk('KeyW');
  await checkpoint('after-control-replace');

  // 4. Remap move.forward W -> K through the Controls UI.
  await rebindForward('KeyK');
  await checkpoint('rebound-k');
  result.walkOldW = await walk('KeyW');
  result.walkNewK = await walk('KeyK');

  // 5. Pause: the world holds; the menu and an observer camera still work.
  result.pause = await fence('lifecycle/pause');
  await page.waitForTimeout(SETTLE_MS);
  const heldStart = await world();
  await page.keyboard.press('Escape');
  await page.waitForTimeout(SETTLE_MS);
  const menuWhileHeld = await shell();
  await page.keyboard.press('Escape');
  await page.waitForTimeout(SETTLE_MS);
  const eye = (await debug('playtest.observe')).player.viewpoint;
  const observer = await debug(`engine.renderer.camera ${eye.x} ${eye.y + 4} ${eye.z} 0 -35`);
  await page.waitForTimeout(1_000);
  await checkpoint('paused-observer');
  const observerCleared = await debug('engine.renderer.camera none');
  await page.waitForTimeout(1_000);
  const heldEnd = await world();
  result.paused = {
    mode: heldStart.mode,
    stepsAdvanced: heldEnd.step - heldStart.step,
    menuOpenedWhileHeld: menuWhileHeld.menuOpen,
    observer: { drawing: observer.drawing, held: observer.held, observer: observer.observer, frame: observer.frame },
    observerCleared: { observer: observerCleared.observer, frame: observerCleared.frame },
  };

  // 6. Resume, and the new key still works.
  result.resume = await fence('lifecycle/resume');
  await page.waitForTimeout(SETTLE_MS);
  result.walkAfterResumeK = await walk('KeyK');
  await checkpoint('resumed-walked-k');

  // 7. Restore the product's mapping.
  await rebindForward('KeyW');
  result.walkRestoredW = await walk('KeyW');
  await checkpoint('restored-w');

  result.input = input;
  result.checkpoints = checkpoints;
  writeFileSync(`${out}/fences-result.json`, JSON.stringify(result, null, 2));
}

export default exercise;
