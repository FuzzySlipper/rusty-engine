export function mountProductUi(root) {
  const text = document.createElement('p');
  text.textContent = 'Release fixture: opens release-data.rpak, which tools/ReleaseImport packs.';
  root.append(text);
  return { dispose() { text.remove(); } };
}
