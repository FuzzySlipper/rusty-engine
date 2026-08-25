import assert from 'node:assert/strict';
import test from 'node:test';
import {
  ProductBrowserHostError,
  PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE,
  createProductBrowserRuntimeTransport,
  productBrowserBundleAssets,
  productBrowserBundleDescriptor,
} from './product-browser-host.js';
import type { ProductBrowserRuntimeAdapter } from './product-browser-host.js';
import type { RustyApplicationRuntimeInputEnvelope } from '@rusty-engine/application-host';
import { createProductBrowserCadence } from './realtime-cadence.js';

const adapter: ProductBrowserRuntimeAdapter = {
  lifecycle: async (operation) => ({
    accepted: true,
    operation: operation.kind,
  }),
  input: async (batch: readonly RustyApplicationRuntimeInputEnvelope[]) => ({ accepted: true, count: batch.length }),
  advanceRealtime: async () => ({ accepted: true, operation: 'advance-realtime' as const }),
  subscribeOutputs: () => () => undefined,
  dispose: () => undefined,
};

test('fixed runtime transport preserves only named operations', async () => {
  const transport = createProductBrowserRuntimeTransport(adapter);
  assert.equal((await transport.lifecycle({ kind: 'start' })).accepted, true);
  assert.equal((await transport.input([])).count, 0);
  assert.equal((await transport.advanceRealtime('1000000')).operation, 'advance-realtime');
  assert.equal('call' in transport, false);
});

test('realtime owner controls advancement without dropping typed cadence input', async () => {
  const input: RustyApplicationRuntimeInputEnvelope = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };

  const run = async (realtimeAdvanceOwner: 'browser' | 'rust-host') => {
    const inputBatches: Array<readonly RustyApplicationRuntimeInputEnvelope[]> = [];
    const observedTimes: string[] = [];
    const failures: unknown[] = [];
    const cadence = createProductBrowserCadence({
      lifecycleMode: 'realtime',
      realtimeAdvanceOwner,
      isReady: () => true,
      enqueueOperation: (operation) => operation(),
      sampleInput: () => [input],
      sendInput: async (batch) => {
        inputBatches.push(batch);
      },
      advanceRealtime: async (observedTimeNs) => {
        observedTimes.push(observedTimeNs);
      },
      onFailure: (cause) => {
        failures.push(cause);
      },
    });
    cadence.enqueue(16.5);
    await cadence.settle();
    cadence.dispose();
    return { inputBatches, observedTimes, failures };
  };

  const browser = await run('browser');
  assert.deepEqual(browser.inputBatches, [[input]]);
  assert.deepEqual(browser.observedTimes, ['16500000']);
  assert.deepEqual(browser.failures, []);

  const rustHost = await run('rust-host');
  assert.deepEqual(rustHost.inputBatches, [[input]]);
  assert.deepEqual(rustHost.observedTimes, []);
  assert.deepEqual(rustHost.failures, []);
});

test('Rust-host output pulse drains typed input without browser advancement', async () => {
  const input: RustyApplicationRuntimeInputEnvelope = {
    runtime: { instanceId: '1', generation: '1', controlRevision: '1' },
    sequence: '1',
    context: 'gameplay.default',
    fact: { kind: 'key', code: 'key-w', edge: 'pressed' },
  };
  const batches: Array<readonly RustyApplicationRuntimeInputEnvelope[]> = [];
  const advances: string[] = [];
  const cadence = createProductBrowserCadence({
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'rust-host',
    isReady: () => true,
    enqueueOperation: (operation) => operation(),
    sampleInput: () => [input],
    sendInput: async (batch) => { batches.push(batch); },
    advanceRealtime: async (time) => { advances.push(time); },
    onFailure: (cause) => { assert.fail(String(cause)); },
  });
  cadence.pulseRustHost();
  await cadence.settle();
  cadence.dispose();
  assert.deepEqual(batches, [[input]]);
  assert.deepEqual(advances, []);
});

test('transport rejects an adapter with an arbitrary or missing operation surface', () => {
  assert.throws(
    () => createProductBrowserRuntimeTransport({
      ...adapter,
      lifecycle: undefined,
    } as never),
    /lifecycle must be a function/u,
  );
  assert.throws(
    () => createProductBrowserRuntimeTransport({
      ...adapter,
      subscribeOutputs: undefined,
    } as never),
    /subscribeOutputs must be a function/u,
  );
  assert.throws(
    () => createProductBrowserRuntimeTransport({
      ...adapter,
      subscribeTerminalFailures: true,
    } as never),
    /subscribeTerminalFailures must be a function/u,
  );
});

test('bundle assets are fixed JS composition roots and descriptor bytes are reproducible', () => {
  const options = {
    engineHostModule: 'export const engineHost = true;\n',
    uiModule: './ui/main.js',
    runtimeAdapterModule: './runtime-adapter.js',
    lifecycleMode: 'demand' as const,
    uiProjection: {
      expectedStream: 'product.hud',
      expectedContract: 'product.hud.current',
    },
  };
  const first = productBrowserBundleAssets(options);
  const second = productBrowserBundleAssets(options);
  assert.deepEqual(first, second);
  assert.deepEqual(first.map((asset) => asset.name), [
    'index.html',
    'main.js',
    'bridge.js',
    PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE,
  ]);
  assert.equal(first.some((asset) => asset.name.endsWith('.ts')), false);
  assert.equal(first.some((asset) => asset.content.includes('globalThis.__rustyProductBrowserHost')), false);
  assert.equal(first.some((asset) => asset.content.includes('product.ui.v1')), false);
  assert.match(first[0]!.content, /main\.js/u);
  assert.match(first[1]!.content, /\.\/engine\/product-browser-host\.js/u);
  assert.match(first[1]!.content, /initialInteractionMode: 'gameplay'/u);
  assert.match(first[1]!.content, /renderer-preload\.json/u);
  assert.match(first[1]!.content, /renderer: \{ initialContent: rendererInitialContent \}/u);
  assert.match(first[1]!.content, /crypto\.subtle\.digest\('SHA-256'/u);
  assert.match(first[1]!.content, /PRODUCT_RENDERER_PRELOAD_TEXTURE_MAX_COUNT/u);
  assert.match(first[1]!.content, /PRODUCT_RENDERER_PRELOAD_AUDIO_MAX_TOTAL_BYTES/u);
  assert.match(first[1]!.content, /new TextEncoder\(\)\.encode\(path\)\.byteLength <= 512/u);
  assert.match(first[1]!.content, /realtimeAdvanceOwner: bridge\.realtimeAdvanceOwner/u);
  assert.match(first[2]!.content, /\.\/engine\/product-browser-host\.js/u);
  assert.equal(first[3]!.content, options.engineHostModule);
  assert.match(first[2]!.content, /lifecycleMode: "demand"/u);
  assert.match(first[2]!.content, /realtimeAdvanceOwner: "browser"/u);
  assert.match(first[2]!.content, /createProductBrowserLocalHttpAdapter/u);
  assert.match(first[2]!.content, /PRODUCT_RUNTIME_HTTP_BASE_PATH/u);
  const descriptor = productBrowserBundleDescriptor(options);
  assert.equal(descriptor.artifact, 'rusty.product.bundle');
  assert.deepEqual(
    descriptor.files.map((file) => [file.name, file.utf8Bytes]),
    first.map((file) => [file.name, new TextEncoder().encode(file.content).byteLength]),
  );
});

test('bundle path and identity admission is fail-closed', () => {
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: 'export const engineHost = true;\n',
      uiModule: '../ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'realtime',
    }),
    /must not escape/u,
  );
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: 'export const engineHost = true;\n',
      uiModule: './ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'realtime',
      uiProjection: { expectedStream: 'product hud', expectedContract: 'product.hud' },
    }),
    /bounded product identity/u,
  );
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: "export { mountProductBrowserHost } from '@rusty-engine/product-browser-host';\n",
      uiModule: './ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'realtime',
    }),
    /bare Engine package imports/u,
  );
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: 'export const engineHost = true;\n',
      uiModule: './ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'demand',
      realtimeAdvanceOwner: 'rust-host',
    }),
    /requires realtime lifecycle mode/u,
  );
  assert.throws(
    () => productBrowserBundleAssets({
      engineHostModule: 'export const engineHost = true;\n',
      uiModule: './ui/main.js',
      runtimeAdapterModule: './runtime-adapter.js',
      lifecycleMode: 'realtime',
      realtimeAdvanceOwner: 'unknown' as never,
    }),
    /realtimeAdvanceOwner must be browser or rust-host/u,
  );
  assert.equal(ProductBrowserHostError.prototype.name, 'Error');
});

test('packaged Rust-host realtime ownership propagates through the generated bridge', () => {
  const assets = productBrowserBundleAssets({
    engineHostModule: 'export const engineHost = true;\n',
    uiModule: './ui/main.js',
    runtimeAdapterModule: './runtime-adapter.js',
    lifecycleMode: 'realtime',
    realtimeAdvanceOwner: 'rust-host',
  });
  assert.match(assets[1]!.content, /realtimeAdvanceOwner: bridge\.realtimeAdvanceOwner/u);
  assert.match(assets[2]!.content, /realtimeAdvanceOwner: "rust-host"/u);
});
