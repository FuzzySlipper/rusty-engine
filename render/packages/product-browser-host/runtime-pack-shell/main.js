import {
  createProductBrowserLocalHttpAdapter,
  mountProductBrowserHost,
} from './engine/product-browser-host.js';

const root = document.querySelector('#application');
if (root === null) throw new Error('Rusty runtime shell root is missing');

const mountUi = (uiRoot) => {
  const status = document.createElement('output');
  status.id = 'rusty-runtime-status';
  status.textContent = 'Rusty Engine browser runtime connected.';
  uiRoot.append(status);
};

const transport = createProductBrowserLocalHttpAdapter();
void mountProductBrowserHost({
  root,
  transport,
  lifecycleMode: 'realtime',
  realtimeAdvanceOwner: 'rust-host',
  mountUi,
}).catch((error) => {
  const detail = document.createElement('pre');
  detail.id = 'rusty-runtime-shell-failure';
  detail.textContent = error instanceof Error ? error.message : String(error);
  document.body.append(detail);
});
