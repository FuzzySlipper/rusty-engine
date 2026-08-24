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
  assert.match(first[2]!.content, /\.\/engine\/product-browser-host\.js/u);
  assert.equal(first[3]!.content, options.engineHostModule);
  assert.match(first[2]!.content, /lifecycleMode: "demand"/u);
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
  assert.equal(ProductBrowserHostError.prototype.name, 'Error');
});
