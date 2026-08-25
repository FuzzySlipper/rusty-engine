/// <reference types="vite/client" />

import {
  mountProductBrowserHost,
  type ProductBrowserRuntimeAdapter,
  type ProductBrowserRuntimeOutput,
  type ProductBrowserRuntimeReadout,
} from '@rusty-engine/product-browser-host';
import type {
  RustyApplicationRuntimeIdentity,
  RustyApplicationRuntimeInputEnvelope,
  RustyApplicationUiMount,
} from '@rusty-engine/application-host';

const RUNTIME: RustyApplicationRuntimeIdentity = {
  instanceId: '7',
  generation: '1',
  controlRevision: '1',
};

declare global {
  interface Window {
    __rustyProductBrowserInputBatches?: readonly (readonly RustyApplicationRuntimeInputEnvelope[])[];
    __rustyProductBrowserRealtimeTicks?: readonly string[];
    __rustyProductBrowserOutputs?: readonly ProductBrowserRuntimeOutput[];
    __rustyProductBrowserRafCount?: number;
    __rustyProductBrowserUiContextShape?: {
      readonly keys: readonly string[];
      readonly projectionKeys: readonly string[] | null;
      readonly intentsKeys: readonly string[] | null;
    };
  }
}

const root = document.querySelector<HTMLElement>('#application');
if (root === null) throw new Error('Product Browser Host acceptance root is missing');

const nativeRequestAnimationFrame = window.requestAnimationFrame.bind(window);
window.__rustyProductBrowserRafCount = 0;
window.requestAnimationFrame = (callback: FrameRequestCallback): number => {
  window.__rustyProductBrowserRafCount = (window.__rustyProductBrowserRafCount ?? 0) + 1;
  return nativeRequestAnimationFrame(callback);
};

let outputListeners = new Set<(output: ProductBrowserRuntimeOutput) => void>();
let terminalFailureListeners = new Set<(failure: { readonly kind: 'output-lag'; readonly diagnostic: string }) => void>();
let disposed = false;
const inputBatches: (readonly RustyApplicationRuntimeInputEnvelope[])[] = [];
const realtimeTicks: string[] = [];
const outputs: ProductBrowserRuntimeOutput[] = [];
window.__rustyProductBrowserInputBatches = inputBatches;
window.__rustyProductBrowserRealtimeTicks = realtimeTicks;
window.__rustyProductBrowserOutputs = outputs;

function emit(output: ProductBrowserRuntimeOutput): void {
  outputs.push(output);
  for (const listener of outputListeners) listener(output);
}

const runtimeReadout = (state: ProductBrowserRuntimeReadout['state']): ProductBrowserRuntimeReadout => ({
  artifact: 'rusty.product.runtime-readout',
  runtime: RUNTIME,
  mode: 'realtime',
  state,
  admittedSimulationSteps: String(realtimeTicks.length),
  admittedPresentations: '0',
  droppedRealtimeSteps: '0',
  clockRegressions: '0',
  scaledRemainder: 0,
  lastObservedTimeNs: realtimeTicks.at(-1) ?? null,
  fault: null,
});

const adapter: ProductBrowserRuntimeAdapter = {
  lifecycle: async (operation) => {
    if (operation.kind === 'start') {
      emit({ kind: 'binding', runtime: RUNTIME });
      emit({ kind: 'runtime-readout', readout: runtimeReadout('running') });
    }
    return { accepted: true, operation: operation.kind, binding: RUNTIME, readout: runtimeReadout('running') };
  },
  input: async (batch) => {
    inputBatches.push(batch);
    return { accepted: true, count: batch.length, binding: RUNTIME, readout: runtimeReadout('running') };
  },
  advanceRealtime: async (observedTimeNs) => {
    realtimeTicks.push(observedTimeNs);
    emit({ kind: 'runtime-readout', readout: runtimeReadout('running') });
    return { accepted: true, operation: 'advance-realtime', binding: RUNTIME, readout: runtimeReadout('running') };
  },
  subscribeOutputs: (listener) => {
    outputListeners.add(listener);
    // Adversarially emit a binding before application-host mount. Product
    // Browser Host must retain it in its bounded pre-mount buffer and apply it
    // only after the public input/projection ports exist.
    listener({ kind: 'binding', runtime: RUNTIME });
    return () => { outputListeners.delete(listener); };
  },
  subscribeTerminalFailures: (listener) => {
    terminalFailureListeners.add(listener);
    return () => { terminalFailureListeners.delete(listener); };
  },
  dispose: () => {
    disposed = true;
    root.dataset['transportDisposed'] = 'true';
    outputListeners = new Set();
    terminalFailureListeners = new Set();
  },
};

let productHost: Awaited<ReturnType<typeof mountProductBrowserHost>> | null = null;
let productStateElement: HTMLOutputElement | null = null;

function emitOutputLag(): void {
  const failure = {
    kind: 'output-lag' as const,
    diagnostic: 'fixture output lag requires a fresh snapshot',
  };
  for (const listener of [...terminalFailureListeners]) listener(failure);
  setTimeout(() => {
    if (productStateElement !== null && productHost !== null) {
      productStateElement.textContent = `state: ${productHost.readout().state}`;
    }
  }, 0);
}

const mountUi: RustyApplicationUiMount = (uiRoot, context) => {
  window.__rustyProductBrowserUiContextShape = {
    keys: Object.keys(context).sort(),
    projectionKeys: context.projection === undefined ? null : Object.keys(context.projection).sort(),
    intentsKeys: context.intents === undefined ? null : Object.keys(context.intents).sort(),
  };
  const ui = document.createElement('div');
  ui.id = 'product-ui';
  const button = document.createElement('button');
  button.id = 'product-intent';
  button.textContent = 'Claim product intent';
  button.addEventListener('click', () => {
    context.intents?.claim('product.jump', { kind: 'digital', active: true });
  });
  const projection = document.createElement('output');
  projection.id = 'projection';
  projection.textContent = 'projection: waiting';
  context.projection?.subscribe((value) => {
    projection.textContent = value === null ? 'projection: empty' : `projection: ${value.contract}`;
  });
  const state = document.createElement('output');
  state.id = 'product-state';
  state.textContent = 'state: starting';
  productStateElement = state;
  const lag = document.createElement('button');
  lag.id = 'product-output-lag';
  lag.textContent = 'Simulate output lag';
  lag.addEventListener('click', emitOutputLag);
  const dispose = document.createElement('button');
  dispose.id = 'product-dispose';
  dispose.textContent = 'Dispose product host';
  dispose.addEventListener('click', () => {
    void productHost?.dispose().then(() => {
      root.dataset['productBrowserState'] = 'disposed';
      const disposedState = document.createElement('output');
      disposedState.id = 'product-disposed-state';
      disposedState.textContent = 'state: disposed';
      root.append(disposedState);
    });
  });
  ui.append(button, projection, state, lag, dispose);
  uiRoot.append(ui);
};
void mountProductBrowserHost({
  root,
  transport: adapter,
  lifecycleMode: 'realtime',
  mountUi,
  inputContext: 'gameplay.default',
  runtimeInput: { maximumPointerDelta: 32, maximumWheelDelta: 64 },
  uiProjection: { expectedStream: 'product.ui', expectedContract: 'product.ui.v1' },
}).then((host) => {
  productHost = host;
  root.dataset['productBrowserState'] = host.readout().state;
  const state = document.querySelector<HTMLOutputElement>('#product-state');
  if (state !== null) state.textContent = `state: ${host.readout().state}`;
  emit({
    kind: 'ui-projection',
    envelope: {
      artifact: 'rusty.product.ui-projection',
      runtime: RUNTIME,
      sequence: '0',
      stream: 'product.ui',
      contract: 'product.ui.v1',
      value: { status: 'ready' },
    },
  });
}).catch((error: unknown) => {
  if (!disposed) {
    const detail = document.createElement('pre');
    detail.id = 'product-browser-host-failure';
    detail.textContent = error instanceof Error ? error.message : String(error);
    document.body.append(detail);
  }
});
