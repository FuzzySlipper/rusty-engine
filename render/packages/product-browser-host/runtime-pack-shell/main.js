import {
  createProductBrowserLocalHttpAdapter,
  mountProductBrowserHost,
} from './engine/product-browser-host.js';

const root = document.querySelector('#application');
if (root === null) throw new Error('Rusty runtime shell root is missing');

const mountDefaultUi = (uiRoot) => {
  const status = document.createElement('output');
  status.id = 'rusty-runtime-status';
  status.textContent = 'Rusty Engine browser runtime connected.';
  uiRoot.append(status);
};

const bootstrap = await fetch('./product-bootstrap.json').then(async (response) => {
  if (!response.ok) throw new Error(`Product bootstrap failed: HTTP ${response.status}`);
  return response.json();
});
if (bootstrap?.artifact !== 'rusty.product.browser-bootstrap' || bootstrap?.schemaVersion !== 1) {
  throw new Error('Product bootstrap is not a Rusty Product V1 descriptor');
}
if (typeof bootstrap?.product?.title === 'string') document.title = bootstrap.product.title;
if (typeof bootstrap?.ui?.entry !== 'string' || !bootstrap.ui.entry.startsWith('product-ui/')) {
  throw new Error('Product bootstrap has no admitted product UI entry');
}
const uiProjection = bootstrap.uiProjection;
if (uiProjection !== undefined
  && (uiProjection === null
    || typeof uiProjection !== 'object'
    || typeof uiProjection.expectedStream !== 'string'
    || typeof uiProjection.expectedContract !== 'string')) {
  throw new Error('Product bootstrap has an invalid UI projection declaration');
}
const productUi = await import(`./${bootstrap.ui.entry}`);
if (typeof productUi.mountProductUi !== 'function') {
  throw new Error('Product UI entry must export mountProductUi(root)');
}

const transport = createProductBrowserLocalHttpAdapter();
void mountProductBrowserHost({
  root,
  transport,
  lifecycleMode: bootstrap.lifecycle.mode,
  realtimeAdvanceOwner: 'rust-host',
  initialInteractionMode: 'gameplay',
  runtimeInput: {},
  ...(uiProjection === undefined ? {} : { uiProjection }),
  mountUi: (uiRoot, context) => {
    mountDefaultUi(uiRoot);
    return productUi.mountProductUi(uiRoot, context);
  },
}).catch((error) => {
  const detail = document.createElement('pre');
  detail.id = 'rusty-runtime-shell-failure';
  detail.textContent = error instanceof Error ? error.message : String(error);
  document.body.append(detail);
});
