export function mountProductUi(root, context) {
  const label = document.createElement('div');
  label.textContent = 'Controller interaction: WASD / left stick move · mouse / right stick look · E / X use · Q / RB cycle. Green = focused, blue = opened. K locks the left chest.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;color:white;pointer-events:none;max-width:600px';
  const reticle = document.createElement('div');
  reticle.textContent = '+';
  reticle.style.cssText = 'position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);color:white;pointer-events:none';
  const panel = document.createElement('section');
  panel.hidden = true;
  panel.setAttribute('aria-live', 'polite');
  panel.style.cssText = 'position:absolute;right:16px;top:16px;max-width:330px;padding:14px 16px;border:1px solid #79b7ff;border-radius:8px;color:#eff8ff;background:#102944ee;font:15px system-ui,sans-serif';
  const title = document.createElement('strong');
  const contents = document.createElement('p');
  const close = document.createElement('p');
  close.textContent = 'Press Escape to close.';
  close.style.margin = '12px 0 0';
  contents.style.margin = '8px 0 0';
  panel.append(title, contents, close);
  const lifecycle = createLifecycleControls(context);
  root.append(label, reticle, panel, lifecycle.element);

  const unsubscribe = context.projection?.subscribe((projection) => {
    if (projection?.contract !== 'controller-interaction.panel.v1' || !isContainerPanel(projection.value)) return;
    panel.hidden = !projection.value.open;
    title.textContent = projection.value.title;
    contents.textContent = `Contents: ${projection.value.contents}`;
  }) ?? (() => {});

  return { dispose() { unsubscribe(); lifecycle.dispose(); panel.remove(); label.remove(); reticle.remove(); } };
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

function isContainerPanel(value) {
  return typeof value === 'object' && value !== null
    && typeof value.open === 'boolean'
    && typeof value.title === 'string'
    && typeof value.contents === 'string';
}
