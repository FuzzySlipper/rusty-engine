export function mountProductUi(root) {
  const label = document.createElement('output');
  label.textContent = 'Parts rigs fixture: the Engine draws the view of the townsperson each camera sees; live debug rigs.orbit, rigs.spin, rigs.play, rigs.turn.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;color:white;pointer-events:none;font:14px system-ui,sans-serif';
  root.append(label);
  return { dispose: () => label.remove() };
}
