// The entry screen: what the product reports, Import and Cancel. The product
// owns every step; the page only shows its projection and claims intents.
export function mountProductUi(root, context) {
  const panel = document.createElement('div');
  panel.style.cssText = 'position:absolute;inset:0;display:flex;flex-direction:column;gap:12px;padding:32px;color:white;background:#1b1d22;font:16px system-ui,sans-serif';
  const title = document.createElement('h1');
  title.textContent = 'Release Fixture';
  title.style.cssText = 'margin:0;font-size:24px';
  const message = document.createElement('p');
  message.dataset.fixtureMessage = '';
  message.setAttribute('aria-live', 'polite');
  message.style.margin = '0';
  const buttons = document.createElement('div');
  buttons.style.cssText = 'display:flex;gap:8px';
  const importButton = document.createElement('button');
  importButton.textContent = 'Import…';
  importButton.dataset.fixtureImport = '';
  importButton.addEventListener('click', () => claim('release.import'));
  const cancelButton = document.createElement('button');
  cancelButton.textContent = 'Cancel';
  cancelButton.dataset.fixtureCancel = '';
  cancelButton.hidden = true;
  cancelButton.addEventListener('click', () => claim('release.cancel'));
  buttons.append(importButton, cancelButton);
  const output = document.createElement('pre');
  output.dataset.fixtureOutput = '';
  output.style.cssText = 'margin:0;flex:1;overflow:auto;padding:8px;background:#0e0f12;font:13px ui-monospace,monospace;white-space:pre-wrap';
  panel.append(title, message, buttons, output);
  root.append(panel);

  function claim(intent) {
    context.intents?.claim(intent, { kind: 'product-payload', contract: `${intent}.v1`, data: {} });
  }

  const unsubscribe = context.projection?.subscribe((projection) => {
    if (projection?.contract !== 'release.screen.v1') return;
    const value = projection.value ?? {};
    const phase = value.phase;
    message.textContent = phase === 'opened' ? `${value.message} ${value.greeting}` : value.message ?? '';
    importButton.hidden = phase === 'opened';
    importButton.disabled = phase === 'choosing' || phase === 'importing';
    cancelButton.hidden = phase !== 'importing';
    output.textContent = value.output ?? '';
    output.scrollTop = output.scrollHeight;
  }) ?? (() => {});

  return { dispose() { unsubscribe(); panel.remove(); } };
}
