/// <reference types="vite/client" />

import {
  mountProductBrowserHost,
  type ProductBrowserRuntimeAdapter,
} from '@rusty-engine/product-browser-host';
import type {
  RustyApplicationRuntimeIdentity,
  RustyApplicationUiMount,
} from '@rusty-engine/application-host';

const root = document.querySelector<HTMLElement>('#application');
if (root === null) throw new Error('cadence acceptance root is missing');

declare global {
  interface Window {
    __rustyProductCadenceRafCount?: number;
    __rustyProductCadenceLatestFrameMs?: number;
    __rustyProductCadenceAdvanceCalls?: number;
    __rustyProductCadenceActiveRequests?: number;
    __rustyProductCadenceMaximumActiveRequests?: number;
    __rustyProductCadenceObservedTimes?: readonly string[];
    __rustyProductCadenceState?: string;
  }
}

const nativeSetTimeout = window.setTimeout.bind(window);
const nativeRequestAnimationFrame = window.requestAnimationFrame.bind(window);
void nativeRequestAnimationFrame;
const MAXIMUM_FRAMES = 1_000;
const observedTimes: string[] = [];
window.__rustyProductCadenceRafCount = 0;
window.__rustyProductCadenceAdvanceCalls = 0;
window.__rustyProductCadenceActiveRequests = 0;
window.__rustyProductCadenceMaximumActiveRequests = 0;
window.__rustyProductCadenceObservedTimes = observedTimes;
window.requestAnimationFrame = (callback: FrameRequestCallback): number => {
  const count = (window.__rustyProductCadenceRafCount ?? 0) + 1;
  window.__rustyProductCadenceRafCount = count;
  if (count > MAXIMUM_FRAMES) return 0;
  return nativeSetTimeout(() => {
    const timeMs = performance.now();
    window.__rustyProductCadenceLatestFrameMs = timeMs;
    callback(timeMs);
  }, 0) as unknown as number;
};

const runtime: RustyApplicationRuntimeIdentity = {
  instanceId: '9',
  generation: '1',
  controlRevision: '1',
};
const mountUi: RustyApplicationUiMount = (uiRoot) => {
  const output = document.createElement('output');
  output.id = 'product-cadence-state';
  output.textContent = 'state: starting';
  uiRoot.append(output);
};

const adapter: ProductBrowserRuntimeAdapter = {
  lifecycle: async (operation) => ({ accepted: true, operation: operation.kind, binding: runtime }),
  input: async (batch) => ({ accepted: true, count: batch.length, binding: runtime }),
  advanceRealtime: async (observedTimeNs) => {
    window.__rustyProductCadenceAdvanceCalls = (window.__rustyProductCadenceAdvanceCalls ?? 0) + 1;
    const active = (window.__rustyProductCadenceActiveRequests ?? 0) + 1;
    window.__rustyProductCadenceActiveRequests = active;
    window.__rustyProductCadenceMaximumActiveRequests = Math.max(
      window.__rustyProductCadenceMaximumActiveRequests ?? 0,
      active,
    );
    return new Promise((resolve) => {
      nativeSetTimeout(() => {
        window.__rustyProductCadenceActiveRequests = Math.max(
          0,
          (window.__rustyProductCadenceActiveRequests ?? 1) - 1,
        );
        observedTimes.push(observedTimeNs);
        resolve({ accepted: true, operation: 'advance-realtime', binding: runtime });
      }, 50);
    });
  },
  subscribeOutputs: (listener) => {
    listener({ kind: 'binding', runtime });
    return () => undefined;
  },
  dispose: () => undefined,
};

void mountProductBrowserHost({
  root,
  transport: adapter,
  lifecycleMode: 'realtime',
  mountUi,
}).then((host) => {
  window.__rustyProductCadenceState = host.readout().state;
  const state = document.querySelector<HTMLOutputElement>('#product-cadence-state');
  if (state !== null) state.textContent = `state: ${host.readout().state}`;
}).catch((error: unknown) => {
  window.__rustyProductCadenceState = `failed: ${error instanceof Error ? error.message : String(error)}`;
});
