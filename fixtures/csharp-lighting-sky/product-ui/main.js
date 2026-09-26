export function mountProductUi(root) {
  const label = document.createElement('p');
  label.textContent = 'Engine lighting and sky fixture · lighting.torch true/false · lighting.sky 0…1 · lighting.room · lighting.inspect';
  label.style.cssText = 'position:absolute;top:12px;left:16px;color:white;font:16px system-ui;background:#102030cc;padding:12px';
  root.append(label);
  return { dispose() { label.remove(); } };
}
