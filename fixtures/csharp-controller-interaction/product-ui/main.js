export function mountProductUi(root) {
  const label = document.createElement('div');
  label.textContent = 'Controller interaction: WASD / left stick move · mouse / right stick look · E / X use · Q / RB cycle. Green = focused, blue = opened. K locks the left chest.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;color:white;pointer-events:none;max-width:600px';
  const reticle = document.createElement('div');
  reticle.textContent = '+';
  reticle.style.cssText = 'position:absolute;left:50%;top:50%;transform:translate(-50%,-50%);color:white;pointer-events:none';
  root.append(label, reticle);
  return { dispose() { label.remove(); reticle.remove(); } };
}
