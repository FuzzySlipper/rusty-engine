export function mountProductUi(root) {
  const label = document.createElement('p');
  label.textContent = 'Engine movement and voxel streaming proof · cyan: default block · green: variant face · yellow: swept character. Debug: streaming.inspect, movement.start swim/climb/fly.';
  label.style.cssText = 'position:absolute;top:12px;left:16px;max-width:700px;color:white;font:16px system-ui;background:#102030cc;padding:12px';
  root.append(label);
  return { dispose() { label.remove(); } };
}
