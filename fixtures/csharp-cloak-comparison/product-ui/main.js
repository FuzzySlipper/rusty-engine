export function mountProductUi(root) {
  const label = document.createElement('p');
  label.textContent = 'Engine cloak comparison · the owner\'s mage on a scripted run, her cloak as simulated cape bones or a soft sheet · cloak.mode bones|soft, cloak.count <n>, cloak.tune <setting> <value>, cloak.probe on|off, cloak.freeze <s>|off, cloak.camera side|back|front|quarter, cloak.restart, cloak.inspect, cloak.report';
  label.style.cssText = 'position:absolute;top:12px;left:16px;max-width:700px;color:white;font:16px system-ui;background:#102030cc;padding:12px';
  root.append(label);
  return { dispose() { label.remove(); } };
}
