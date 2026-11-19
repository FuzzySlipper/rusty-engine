export function mountProductUi(root) {
  const label = document.createElement('output');
  label.textContent = 'Persistence queue fixture: an 8 KB save every update; persistence.report has the timings.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;color:white;pointer-events:none;font:14px system-ui,sans-serif';
  root.append(label);
  return { dispose: () => label.remove() };
}
