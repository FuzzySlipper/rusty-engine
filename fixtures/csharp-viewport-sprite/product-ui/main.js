export function mountProductUi(root) {
  const label = document.createElement('output');
  label.textContent = 'Viewport sprite fixture: retained atlas playback is active.';
  root.append(label);
  return { dispose: () => label.remove() };
}
