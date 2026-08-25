/// <reference types="vite/client" />

import {
  createProductBrowserLocalHttpAdapter,
  mountProductBrowserHost,
  type ProductBrowserRuntimeReadout,
} from '@rusty-engine/product-browser-host';
import type {
  RustyApplicationRuntimeIdentity,
  RustyApplicationUiMount,
} from '@rusty-engine/application-host';

const RUNTIME: RustyApplicationRuntimeIdentity = {
  instanceId: '8',
  generation: '1',
  controlRevision: '1',
};

declare global {
  interface Window {
    __rustyProductLocalTransportState?: string;
    __rustyProductLocalTransportProjection?: string;
  }
}

const root = document.querySelector<HTMLElement>('#application');
if (root === null) throw new Error('Product Browser local transport root is missing');

const mountUi: RustyApplicationUiMount = (uiRoot, context) => {
  const ui = document.createElement('div');
  ui.id = 'product-ui';
  const state = document.createElement('output');
  state.id = 'product-state';
  state.textContent = 'state: starting';
  const projection = document.createElement('output');
  projection.id = 'product-projection';
  projection.textContent = 'projection: waiting';
  context.projection?.subscribe((value) => {
    const contract = value?.contract ?? 'empty';
    projection.textContent = `projection: ${contract}`;
    window.__rustyProductLocalTransportProjection = contract;
  });
  ui.append(state, projection);
  uiRoot.append(ui);
};

const runtimeReadout = (state: ProductBrowserRuntimeReadout['state']): ProductBrowserRuntimeReadout => ({
  artifact: 'rusty.product.runtime-readout',
  runtime: RUNTIME,
  mode: 'realtime',
  state,
  admittedSimulationSteps: '0',
  admittedPresentations: '0',
  droppedRealtimeSteps: '0',
  clockRegressions: '0',
  scaledRemainder: 0,
  lastObservedTimeNs: null,
  fault: null,
});

const adapter = createProductBrowserLocalHttpAdapter();
void mountProductBrowserHost({
  root,
  transport: adapter,
  lifecycleMode: 'realtime',
  mountUi,
  runtimeInput: { maximumPointerDelta: 32, maximumWheelDelta: 64 },
  uiProjection: { expectedStream: 'product.local', expectedContract: 'product.local.current' },
}).then((host) => {
  window.__rustyProductLocalTransportState = host.readout().state;
  const state = document.querySelector<HTMLOutputElement>('#product-state');
  if (state !== null) state.textContent = `state: ${host.readout().state}`;
}).catch((error: unknown) => {
  window.__rustyProductLocalTransportState = error instanceof Error ? `failed: ${error.message}` : `failed: ${String(error)}`;
  const detail = document.createElement('pre');
  detail.id = 'product-browser-local-transport-failure';
  detail.textContent = window.__rustyProductLocalTransportState;
  document.body.append(detail);
});
