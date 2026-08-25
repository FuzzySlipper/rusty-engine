/// <reference types="vite/client" />

import {
  mountProductBrowserHost,
  type ProductBrowserRuntimeAdapter,
} from '@rusty-engine/product-browser-host';
import type { RustyApplicationUiMount } from '@rusty-engine/application-host';

const root = document.querySelector<HTMLElement>('#application');
if (root === null) throw new Error('Product Browser Host failure root is missing');

const adapter: ProductBrowserRuntimeAdapter = {
  lifecycle: async () => {
    throw new Error('deliberate runtime start failure');
  },
  input: async (batch) => ({ accepted: true, count: batch.length }),
  advanceRealtime: async () => ({ accepted: true, operation: 'advance-realtime' }),
  subscribeOutputs: () => () => undefined,
  dispose: () => { root.dataset['transportDisposed'] = 'true'; },
};

const mountUi: RustyApplicationUiMount = (uiRoot) => {
  uiRoot.textContent = 'failure fixture UI';
};

void mountProductBrowserHost({
  root,
  transport: adapter,
  lifecycleMode: 'realtime',
  mountUi,
}).catch((error: unknown) => {
  root.dataset['failureObserved'] = 'true';
  const failure = document.createElement('pre');
  failure.id = 'product-browser-host-failure';
  failure.textContent = error instanceof Error ? error.message : String(error);
  document.body.append(failure);
});
