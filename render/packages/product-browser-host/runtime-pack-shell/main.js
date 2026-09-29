import { startProductBrowserShell } from './engine/product-browser-host.js';

const root = document.querySelector('#application');
if (root === null) throw new Error('Rusty runtime shell root is missing');
startProductBrowserShell(root).catch((error) => {
  const detail = document.createElement('pre');
  detail.id = 'rusty-runtime-shell-failure';
  detail.textContent = error instanceof Error ? error.message : String(error);
  document.body.append(detail);
});
