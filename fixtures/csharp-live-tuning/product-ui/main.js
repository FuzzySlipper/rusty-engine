export function mountProductUi(root) {
  const label = document.createElement('output');
  label.textContent = 'Live tuning fixture: edit content/tuning/marker.json and the marker follows without a restart.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;color:white;pointer-events:none;font:14px system-ui,sans-serif';
  root.append(label);
  return { dispose: () => label.remove() };
}
