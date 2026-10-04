export function mountProductUi(root, context) {
  const help = document.createElement('div');
  help.textContent = 'Time moves when you move. WASD / left stick move · mouse / right stick look (free) · click / RB shoot (buys 0.35 s) · B / LB wait 2 s · G / X crawl floor · T / Y realtime';
  help.style.cssText = 'position:absolute;top:8px;left:8px;color:white;pointer-events:none;max-width:640px;font:14px system-ui,sans-serif';
  const reticle = document.createElement('div');
  reticle.textContent = '+';
  reticle.style.cssText = 'position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);color:white;pointer-events:none';
  const hud = document.createElement('div');
  hud.dataset.fixtureHud = 'time';
  hud.setAttribute('aria-live', 'polite');
  hud.style.cssText = 'position:absolute;right:12px;top:8px;color:white;font:14px ui-monospace,monospace;text-align:right;pointer-events:none';
  const lifecycle = createLifecycleControls(context);
  // Works running or paused: the product applies the same rule either way.
  const clear = document.createElement('button');
  clear.textContent = 'Clear hits';
  clear.dataset.fixtureClear = 'hits';
  clear.addEventListener('click', () => context.intents?.claim('gameplay-time.clear', {
    kind: 'product-payload', contract: 'gameplay-time.clear.v1', data: {},
  }));
  lifecycle.element.append(clear);
  root.append(help, reticle, hud, lifecycle.element);

  const unsubscribe = context.projection?.subscribe((projection) => {
    if (projection?.contract !== 'gameplay-time.hud.v1' || !isHud(projection.value)) return;
    const value = projection.value;
    const time = value.held ? 'HELD' : value.advanceSteps > 0 ? `ACTION ×${value.rate.toFixed(2)}` : `×${value.rate.toFixed(2)}`;
    hud.textContent = [
      `time ${time}`,
      `step ${value.step}`,
      `cooldown ${value.cooldownSteps}`,
      `hits ${value.hits} (cleared while paused ${value.pausedClears ?? 0}×)`,
      `${value.crawl ? 'crawl' : 'stop'} when idle${value.realtime ? ' · realtime' : ''}`,
    ].join('\n');
    hud.style.whiteSpace = 'pre';
  }) ?? (() => {});

  return { dispose() { unsubscribe(); lifecycle.dispose(); help.remove(); reticle.remove(); hud.remove(); } };
}

// Pause and Resume ask the Engine; the label and button show the state the
// Engine reports, not what was last clicked.
function createLifecycleControls(context) {
  const element = document.createElement('div');
  element.style.cssText = 'position:absolute;left:8px;bottom:8px;display:flex;gap:8px;align-items:center;color:white;font:14px system-ui,sans-serif';
  const toggle = document.createElement('button');
  toggle.dataset.fixtureLifecycle = 'toggle';
  const status = document.createElement('span');
  status.dataset.fixtureLifecycle = 'state';
  status.setAttribute('aria-live', 'polite');
  element.append(toggle, status);
  const lifecycle = context.lifecycle;
  if (lifecycle === undefined) {
    toggle.disabled = true;
    toggle.textContent = 'Pause';
    status.textContent = 'No runtime lifecycle';
    return { element, dispose() { element.remove(); } };
  }
  const show = (state) => {
    toggle.textContent = state === 'paused' ? 'Resume' : 'Pause';
    toggle.disabled = state !== 'running' && state !== 'paused';
    status.textContent = `Engine: ${state ?? 'unknown'}`;
  };
  const unsubscribe = lifecycle.subscribe(show);
  show(lifecycle.state());
  toggle.addEventListener('click', async () => {
    const resuming = lifecycle.state() === 'paused';
    if (!resuming) context.ui.setInteractionMode('interface');
    toggle.disabled = true;
    try {
      const result = await (resuming ? lifecycle.resume() : lifecycle.pause());
      if (!result.accepted) status.textContent = `Engine: ${result.state ?? 'unknown'} (${result.code})`;
      if (resuming && result.accepted) {
        context.ui.setInteractionMode('gameplay');
        context.ui.focusGameplay();
      }
    } catch (error) {
      status.textContent = `Engine unavailable: ${error.message}`;
    } finally {
      toggle.disabled = !['running', 'paused'].includes(lifecycle.state());
    }
  });
  return { element, dispose() { unsubscribe(); element.remove(); } };
}

function isHud(value) {
  return typeof value === 'object' && value !== null
    && typeof value.rate === 'number'
    && typeof value.held === 'boolean'
    && typeof value.step === 'number';
}
