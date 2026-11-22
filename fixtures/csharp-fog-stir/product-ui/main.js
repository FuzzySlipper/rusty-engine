export function mountProductUi(root) {
  const label = document.createElement('output');
  label.textContent = 'Fog stir fixture: a body crosses a mist sump; the Engine parts the mist around it and lets the wake settle.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;color:white;pointer-events:none;font:14px system-ui,sans-serif';
  root.append(label);
  return { dispose: () => label.remove() };
}
