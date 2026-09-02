export function mountProductUi(root) {
  const marker = document.createElement('output');
  marker.id = 'fixture-product-ui-marker';
  marker.textContent = 'fixture Product UI mounted';
  root.append(marker);
}
