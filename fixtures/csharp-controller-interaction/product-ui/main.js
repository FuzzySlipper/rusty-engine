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
  root.append(label, reticle, panel);

  const unsubscribe = context.projection?.subscribe((projection) => {
    if (projection?.contract !== 'controller-interaction.panel.v1' || !isContainerPanel(projection.value)) return;
    panel.hidden = !projection.value.open;
    title.textContent = projection.value.title;
    contents.textContent = `Contents: ${projection.value.contents}`;
  }) ?? (() => {});

  return { dispose() { unsubscribe(); panel.remove(); label.remove(); reticle.remove(); } };
}

function isContainerPanel(value) {
  return typeof value === 'object' && value !== null
    && typeof value.open === 'boolean'
    && typeof value.title === 'string'
    && typeof value.contents === 'string';
}
