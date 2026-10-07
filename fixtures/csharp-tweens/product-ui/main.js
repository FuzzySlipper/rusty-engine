export function mountProductUi(root) {
  const label = document.createElement('output');
  label.textContent = 'Tweens fixture: the Engine plays every hop, squash, breath and flash; the product publishes once per move.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;color:white;pointer-events:none;font:14px system-ui,sans-serif';
  root.append(label);
  return { dispose: () => label.remove() };
}
