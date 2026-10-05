export function mountProductUi(root) {
  const text = document.createElement('p');
  text.textContent = 'Engine HTTP download proof. Through live debug: http.index, http.inspect, http.download, http.cancel, http.fail, http.library, http.open, http.remove.';
  root.append(text);
  return { dispose() { text.remove(); } };
}
