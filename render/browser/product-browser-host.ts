/// <reference types="vite/client" />

import {
  mountProductBrowserHost,
  ProductBrowserLocalTransportError,
  type ProductBrowserDiagnosticsReport,
  type ProductBrowserRuntimeAdapter,
  type ProductBrowserHost,
  type ProductBrowserRuntimeOutput,
  type ProductBrowserRuntimeReadout,
  type ProductBrowserRuntimeTerminalFailure,
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
    __rustyProductBrowserInputAttempts?: readonly (readonly RustyApplicationRuntimeInputEnvelope[])[];
    __rustyProductBrowserDiagnosticReports?: readonly ProductBrowserDiagnosticsReport[];
    __rustyProductBrowserAcceptedDiagnosticReports?: readonly ProductBrowserDiagnosticsReport[];
    __rustyProductBrowserMaximumActiveDiagnostics?: number;
    __rustyProductBrowserRealtimeTicks?: readonly string[];
    __rustyProductBrowserOutputs?: readonly ProductBrowserRuntimeOutput[];
    __rustyProductBrowserRafCount?: number;
    __rustyProductBrowserUiContextShape?: {
      readonly keys: readonly string[];
      readonly projectionKeys: readonly string[] | null;
      readonly intentsKeys: readonly string[] | null;
    };
    __rustyProductBrowserHost?: ProductBrowserHost;
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
let terminalFailureListeners = new Set<(failure: ProductBrowserRuntimeTerminalFailure) => void>();
let disposed = false;
let scheduledInputResultIndex = 0;
const inputBatches: (readonly RustyApplicationRuntimeInputEnvelope[])[] = [];
const inputAttempts: (readonly RustyApplicationRuntimeInputEnvelope[])[] = [];
let transientInputFailuresRemaining = new URLSearchParams(window.location.search).has('transientInputFailure') ? 1 : 0;
let transientInputFailureObserved = false;
let repeatedRetryableFailureRemaining = new URLSearchParams(window.location.search).has('repeatRetryableFailure') ? 1 : 0;
const realtimeTicks: string[] = [];
const outputs: ProductBrowserRuntimeOutput[] = [];
const diagnosticReports: ProductBrowserDiagnosticsReport[] = [];
const acceptedDiagnosticReports: ProductBrowserDiagnosticsReport[] = [];
let rejectedRecoveryDiagnosticsRemaining = new URLSearchParams(window.location.search).has('rejectRecoveryDiagnostic') ? 1 : 0;
const delayRecoveryDiagnostic = new URLSearchParams(window.location.search).has('delayRecoveryDiagnostic');
let activeDiagnostics = 0;
window.__rustyProductBrowserMaximumActiveDiagnostics = 0;
window.__rustyProductBrowserInputBatches = inputBatches;
window.__rustyProductBrowserInputAttempts = inputAttempts;
window.__rustyProductBrowserDiagnosticReports = diagnosticReports;
window.__rustyProductBrowserAcceptedDiagnosticReports = acceptedDiagnosticReports;
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
      emit({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' });
      emit({ kind: 'runtime-readout', readout: runtimeReadout('running') });
    }
    return {
      accepted: true,
      code: 'DEV_HOST_ACCEPTED',
      disposition: 'accepted',
      operation: operation.kind,
      binding: RUNTIME,
      nextInputSequence: '1',
      readout: runtimeReadout('running'),
    };
  },
  input: async (batch) => {
    inputAttempts.push(batch);
    if (transientInputFailuresRemaining > 0) {
      transientInputFailuresRemaining -= 1;
      transientInputFailureObserved = true;
      throw new ProductBrowserLocalTransportError(
        'request_failed',
        'fixture same-origin input request was transiently unavailable',
        { retryable: true, route: '/__rusty/product/runtime/input' },
      );
    }
    inputBatches.push(batch);
    return { accepted: true, code: 'DEV_HOST_ACCEPTED', disposition: 'accepted', count: batch.length, binding: RUNTIME, readout: runtimeReadout('running') };
  },
  reportAudioFeedback: async (feedback) => ({
    accepted: true,
    code: 'DEV_HOST_ACCEPTED',
    disposition: 'accepted',
    runtime: feedback.runtime,
    ...(feedback.facts.at(-1) === undefined
      ? {}
      : { acceptedThroughFactId: feedback.facts.at(-1)!.factId }),
  }),
  reportAnimationFeedback: async (feedback) => ({
    accepted: true,
    code: 'DEV_HOST_ACCEPTED',
    disposition: 'accepted',
    runtime: feedback.runtime,
    ...(feedback.facts.at(-1) === undefined
      ? {}
      : { acceptedThroughFactId: feedback.facts.at(-1)!.factId }),
  }),
  reportGhostPlateFeedback: async (feedback) => ({ accepted: true, code: 'DEV_HOST_ACCEPTED', disposition: 'accepted', runtime: feedback.runtime }),
  reportBrowserDiagnostics: async (report) => {
    activeDiagnostics += 1;
    window.__rustyProductBrowserMaximumActiveDiagnostics = Math.max(
      window.__rustyProductBrowserMaximumActiveDiagnostics ?? 0,
      activeDiagnostics,
    );
    diagnosticReports.push(report);
    try {
      if (rejectedRecoveryDiagnosticsRemaining > 0
        && report.recoverableEvent?.code === 'BROWSER_LOCAL_REQUEST_UNAVAILABLE') {
        rejectedRecoveryDiagnosticsRemaining -= 1;
        throw new Error('fixture rejected the first recovery diagnostic');
      }
      if (delayRecoveryDiagnostic
        && report.recoverableEvent?.code === 'BROWSER_LOCAL_REQUEST_UNAVAILABLE') {
        await new Promise<void>((resolve) => { window.setTimeout(resolve, 100); });
      }
      acceptedDiagnosticReports.push(report);
      return { accepted: true, reported: 1 };
    } finally {
      activeDiagnostics -= 1;
    }
  },
  advanceRealtime: async (observedTimeNs) => {
    if (transientInputFailureObserved && repeatedRetryableFailureRemaining > 0) {
      repeatedRetryableFailureRemaining -= 1;
      throw new ProductBrowserLocalTransportError(
        'request_failed',
        'fixture repeated same-origin advance request was transiently unavailable',
        { retryable: true, route: '/__rusty/product/runtime/advance-realtime' },
      );
    }
    realtimeTicks.push(observedTimeNs);
    emit({ kind: 'runtime-readout', readout: runtimeReadout('running') });
    return {
      accepted: true,
      code: 'DEV_HOST_ACCEPTED',
      disposition: 'accepted',
      operation: 'advance-realtime',
      binding: RUNTIME,
      nextInputSequence: '1',
      readout: runtimeReadout('running'),
    };
  },
  subscribeOutputs: (listener) => {
    outputListeners.add(listener);
    // Adversarially emit a binding before application-host mount. Product
    // Browser Host must retain it in its bounded pre-mount buffer and apply it
    // only after the public input/projection ports exist.
    listener({ kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' });
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

function emitUnboundedTerminalFailure(): void {
  const failure = {
    kind: 'runtime-failure',
    diagnostic: 'x'.repeat(513),
  } as never;
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
  const malformed = document.createElement('button');
  malformed.id = 'product-unbounded-terminal-failure';
  malformed.textContent = 'Simulate invalid terminal failure';
  malformed.addEventListener('click', emitUnboundedTerminalFailure);
  const fakeProgress = document.createElement('button');
  fakeProgress.id = 'product-fake-rust-progress';
  fakeProgress.textContent = 'Simulate fake Rust progress';
  fakeProgress.addEventListener('click', () => {
    emit({ kind: 'runtime-progress', owner: 'rust-host' });
  });
  const scheduledInputResult = document.createElement('button');
  scheduledInputResult.id = 'product-runtime-input-result';
  scheduledInputResult.textContent = 'Emit scheduled input result';
  scheduledInputResult.addEventListener('click', () => {
    const results: readonly ProductBrowserRuntimeOutput[] = [
      {
        kind: 'runtime-input-result',
        result: {
          accepted: true,
          code: 'DEV_HOST_ACCEPTED',
          disposition: 'accepted',
          count: 2,
          acceptedCount: 2,
          droppedCount: 0,
          acceptedThrough: '4',
          consumedThrough: '4',
          nextInputSequence: '5',
          binding: RUNTIME,
          readout: runtimeReadout('running'),
        },
      },
      {
        kind: 'runtime-input-result',
        result: {
          accepted: false,
          code: 'CSHARP_INPUT_STALE_DROPPED',
          disposition: 'rejected-recoverable',
          count: 2,
          acceptedCount: 1,
          droppedCount: 1,
          acceptedThrough: '6',
          consumedThrough: '7',
          nextInputSequence: '8',
          binding: RUNTIME,
          readout: runtimeReadout('running'),
          diagnostic: 'dropped one stale input event',
        },
      },
      {
        kind: 'runtime-input-result',
        result: {
          accepted: false,
          code: 'DEV_HOST_INPUT_MAILBOX_FULL',
          disposition: 'resync-required',
          count: 2,
          acceptedCount: 0,
          droppedCount: 2,
          nextInputSequence: '9',
          binding: RUNTIME,
          readout: runtimeReadout('running'),
          diagnostic: 'input mailbox requires a fresh binding',
        },
      },
    ];
    const output = results[Math.min(scheduledInputResultIndex, results.length - 1)];
    scheduledInputResultIndex += 1;
    if (output !== undefined) emit(output);
  });
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
  ui.append(button, projection, state, lag, malformed, fakeProgress, scheduledInputResult, dispose);
  uiRoot.append(ui);
};
void mountProductBrowserHost({
  root,
  transport: adapter,
  lifecycleMode: 'realtime',
  mountUi,
  initialInteractionMode: 'gameplay',
  inputContext: 'gameplay.default',
  runtimeInput: { maximumPointerDelta: 32, maximumWheelDelta: 64 },
  uiProjection: { expectedStream: 'product.ui', expectedContract: 'product.ui.v1' },
}).then((host) => {
  productHost = host;
  window.__rustyProductBrowserHost = host;
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
