export function mountProductUi(root) {
  const label = document.createElement('p');
  label.textContent = 'Engine Dynamics soft bodies · a flag in gusting wind against a held ball, a jelly and a blob · soft.inspect, soft.wind <m/s>, soft.drop, soft.clear, soft.squash, soft.jump, soft.expect flutter|touched|squashed|recovered|jumped, soft.cost <columns>';
  label.style.cssText = 'position:absolute;top:12px;left:16px;max-width:700px;color:white;font:16px system-ui;background:#102030cc;padding:12px';
  root.append(label);
  return { dispose() { label.remove(); } };
}
