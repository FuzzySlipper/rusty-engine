export function mountProductUi(root) {
  const label = document.createElement('p');
  label.textContent = 'Engine water field · WASD swim, Space up, Shift down · water.storm, water.calm, water.dam, water.breach, water.sample x z, water.inspect, water.camera overview|follow';
  label.style.cssText = 'position:absolute;top:12px;left:16px;max-width:700px;color:white;font:16px system-ui;background:#102030cc;padding:12px';
  root.append(label);
  return { dispose() { label.remove(); } };
}
