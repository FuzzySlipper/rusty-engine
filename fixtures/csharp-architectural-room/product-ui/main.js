export function mountProductUi(root) {
  const panel = document.createElement('output');
  panel.textContent = 'Architectural room: WASD / left stick move · Space / Ctrl fly · mouse / right stick look · E / X raises or lowers the passage door.';
  panel.style.cssText = [
    'position:absolute', 'left:16px', 'top:16px', 'max-width:560px',
    'padding:10px 12px', 'border-radius:6px', 'color:#eefaff',
    'background:#10222ad9', 'font:14px system-ui,sans-serif', 'pointer-events:none',
  ].join(';');
  root.append(panel);
  return { dispose() { panel.remove(); } };
}
