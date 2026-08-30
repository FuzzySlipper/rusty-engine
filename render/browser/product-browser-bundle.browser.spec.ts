import { createServer, type IncomingMessage, type Server, type ServerResponse } from 'node:http';
import { createHash } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { expect, test, type Page } from '@playwright/test';
import {
  productBrowserBundleAssets,
  type ProductBrowserBundleAsset,
} from '@rusty-engine/product-browser-host';

const RUNTIME = { instanceId: '8', generation: '1', controlRevision: '1' };
const READOUT = {
  artifact: 'rusty.product.runtime-readout',
  runtime: RUNTIME,
  mode: 'realtime',
  state: 'running',
  admittedSimulationSteps: '0',
  admittedPresentations: '0',
  droppedRealtimeSteps: '0',
  clockRegressions: '0',
  scaledRemainder: 0,
  lastObservedTimeNs: null,
  fault: null,
};

test('relocatable generated bundle starts over plain HTTP without bare package imports', async ({ page }) => {
  const engineHostModule = await readFile(
    fileURLToPath(new URL('../artifacts/product-browser-host/product-browser-host.js', import.meta.url)),
    'utf8',
  );
  const generatedAssets = exposeBundleHost(productBrowserBundleAssets({
    engineHostModule,
    uiModule: './ui/main.js',
    runtimeAdapterModule: './runtime-adapter.js',
    lifecycleMode: 'realtime',
    uiProjection: {
      expectedStream: 'product.local',
      expectedContract: 'product.local.current',
    },
  }));
  const rendererResources = rendererPreloadResources();
  const requests: string[] = [];
  const server = await createBundleServer(generatedAssets, rendererResources, requests);
  const address = server.address();
  if (address === null || typeof address === 'string') {
    throw new Error('plain HTTP Product Bundle server did not expose a TCP address');
  }
  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(error.message));
  try {
    await page.goto(`http://127.0.0.1:${String(address.port)}/index.html`);
    await expect.poll(() => pageErrors).toEqual([]);
    await expect(page.locator('#bundle-state')).toHaveText('projection: product.local.current');
    await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
    await expect.poll(() => requests.some((request) => request === 'GET /__rusty/product/runtime/outputs')).toBe(true);
    await expect.poll(() => requests.some((request) => request === 'POST /__rusty/product/runtime/lifecycle/start')).toBe(true);
    expect(requests.indexOf('GET /__rusty/product/runtime/outputs'))
      .toBeLessThan(requests.indexOf('POST /__rusty/product/runtime/lifecycle/start'));
    await expect(page.locator('body')).toHaveAttribute('data-rusty-product-host-state', 'ready');
    await expect.poll(() => readStartupRendererProof(page)).not.toBeNull();
    const pixels = await readStartupRendererProof(page);
    expect(pixels).not.toBeNull();
    expect(pixels!.composed[2]).toBeGreaterThan(pixels!.composed[0]! + 20);
    expect(pixels!.composed[2]).toBeGreaterThan(pixels!.unoccupied[2]! + 20);
    await expect.poll(() => requests.some((request) => request === 'GET /renderer-preload.json')).toBe(true);
    await expect.poll(() => requests.some((request) => request === 'GET /content/renderer/%C3%A9.png')).toBe(true);
    await expect.poll(() => requests.some((request) => request === 'GET /content/renderer/theme.wav')).toBe(true);
    await expect.poll(() => requests.some((request) => request === 'GET /content/renderer/packed.rmesh')).toBe(true);
    expect(pageErrors).toEqual([]);
  } finally {
    await closeBundleServer(server);
  }
});

test('generated bundle admits valid resources without Web Crypto subtle', async ({ page }) => {
  await removeWebCryptoSubtle(page);
  const engineHostModule = await readFile(
    fileURLToPath(new URL('../artifacts/product-browser-host/product-browser-host.js', import.meta.url)),
    'utf8',
  );
  const generatedAssets = productBrowserBundleAssets({
    engineHostModule,
    uiModule: './ui/main.js',
    runtimeAdapterModule: './runtime-adapter.js',
    lifecycleMode: 'realtime',
    uiProjection: {
      expectedStream: 'product.local',
      expectedContract: 'product.local.current',
    },
  });
  const server = await createBundleServer(generatedAssets, rendererPreloadResources(), []);
  const address = server.address();
  if (address === null || typeof address === 'string') {
    throw new Error('no-Web-Crypto Product Bundle server did not expose a TCP address');
  }
  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(error.message));
  try {
    await page.goto(`http://127.0.0.1:${String(address.port)}/index.html`);
    await expect.poll(() => pageErrors).toEqual([]);
    await expect(page.locator('#bundle-state')).toHaveText('projection: product.local.current');
    await expect(page.locator('canvas[data-rusty-application-renderer="engine-owned"]')).toHaveCount(1);
    await expect.poll(() => page.evaluate(() => globalThis.crypto.subtle)).toBeUndefined();
    expect(pageErrors).toEqual([]);
  } finally {
    await closeBundleServer(server);
  }
});

test('generated bundle preserves renderer hash mismatch without Web Crypto subtle', async ({ page }) => {
  await removeWebCryptoSubtle(page);
  const engineHostModule = await readFile(
    fileURLToPath(new URL('../artifacts/product-browser-host/product-browser-host.js', import.meta.url)),
    'utf8',
  );
  const generatedAssets = productBrowserBundleAssets({
    engineHostModule,
    uiModule: './ui/main.js',
    runtimeAdapterModule: './runtime-adapter.js',
    lifecycleMode: 'realtime',
  });
  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(error.message));
  const server = await createBundleServer(
    generatedAssets,
    rendererPreloadResources({ tamperedTextureBytes: true }),
    [],
  );
  const address = server.address();
  if (address === null || typeof address === 'string') {
    throw new Error('tampered no-Web-Crypto Product Bundle server did not expose a TCP address');
  }
  try {
    await page.goto(`http://127.0.0.1:${String(address.port)}/index.html`);
    await expect.poll(() => pageErrors.some((error) => error.includes('hash mismatch'))).toBe(true);
    await expect.poll(() => page.evaluate(() => globalThis.crypto.subtle)).toBeUndefined();
  } finally {
    await closeBundleServer(server);
  }
});

test('generated bundle rejects a malformed packed mesh header before initial content admission', async ({ page }) => {
  const engineHostModule = await readFile(
    fileURLToPath(new URL('../artifacts/product-browser-host/product-browser-host.js', import.meta.url)),
    'utf8',
  );
  const generatedAssets = productBrowserBundleAssets({
    engineHostModule,
    uiModule: './ui/main.js',
    runtimeAdapterModule: './runtime-adapter.js',
    lifecycleMode: 'realtime',
  });
  const pageErrors: string[] = [];
  page.on('pageerror', (error) => pageErrors.push(error.message));
  const server = await createBundleServer(
    generatedAssets,
    rendererPreloadResources({ invalidMeshHeader: true }),
    [],
  );
  const address = server.address();
  if (address === null || typeof address === 'string') {
    throw new Error('malformed-mesh Product Bundle server did not expose a TCP address');
  }
  try {
    await page.goto(`http://127.0.0.1:${String(address.port)}/index.html`);
    await expect.poll(() => pageErrors.some((error) => error.includes('media mismatch'))).toBe(true);
  } finally {
    await closeBundleServer(server);
  }
});

async function createBundleServer(
  generatedAssets: readonly ProductBrowserBundleAsset[],
  rendererResources: readonly RendererPreloadResource[],
  requests: string[],
): Promise<Server> {
  const runtimeStream: { response: ServerResponse | null } = { response: null };
  const assetMap = new Map<string, { readonly body: string | Uint8Array; readonly contentType: string }>();
  for (const asset of generatedAssets) {
    assetMap.set(`/${asset.name}`, { body: asset.content, contentType: contentTypeFor(asset.name) });
  }
  assetMap.set('/runtime-adapter.js', {
    body: 'export const PRODUCT_RUNTIME_HTTP_BASE_PATH = "/__rusty/product/runtime/";\n',
    contentType: 'text/javascript; charset=utf-8',
  });
  assetMap.set('/renderer-preload.json', {
    body: JSON.stringify({
      artifact: 'rusty.product.renderer-preload.v1',
      resources: rendererResources.map(({ bytes: _bytes, ...resource }) => resource),
    }),
    contentType: 'application/json; charset=utf-8',
  });
  for (const resource of rendererResources) {
    assetMap.set(`/${resource.path}`, { body: resource.bytes, contentType: resource.mediaType });
  }
  assetMap.set('/ui/main.js', {
    body: [
      'export function mountProductUi(root, context) {',
      '  const state = document.createElement("output");',
      '  state.id = "bundle-state";',
      '  state.textContent = "projection: waiting";',
      '  context.projection?.subscribe((value) => {',
      '    state.textContent = `projection: ${value?.contract ?? "empty"}`;',
      '  });',
      '  root.append(state);',
      '}',
      '',
    ].join('\n'),
    contentType: 'text/javascript; charset=utf-8',
  });

  const server = createServer((request, response) => {
    void handleRequest(request, response, assetMap, requests, runtimeStream);
  });
  await new Promise<void>((resolve, reject) => {
    const onError = (error: Error): void => reject(error);
    server.once('error', onError);
    server.listen(0, '127.0.0.1', () => {
      server.off('error', onError);
      resolve();
    });
  });
  return server;
}

async function handleRequest(
  request: IncomingMessage,
  response: ServerResponse,
  assets: ReadonlyMap<string, { readonly body: string | Uint8Array; readonly contentType: string }>,
  requests: string[],
  runtimeStream: { response: ServerResponse | null },
): Promise<void> {
  const pathname = new URL(request.url ?? '/', 'http://127.0.0.1').pathname;
  requests.push(`${request.method ?? 'GET'} ${pathname}`);
  if (pathname === '/__rusty/product/runtime/outputs' && request.method === 'GET') {
    response.writeHead(200, {
      'cache-control': 'no-cache',
      connection: 'keep-alive',
      'content-type': 'text/event-stream; charset=utf-8',
    });
    response.flushHeaders();
    runtimeStream.response = response;
    response.once('close', () => {
      if (runtimeStream.response === response) runtimeStream.response = null;
    });
    return;
  }
  if (pathname.startsWith('/__rusty/product/runtime/') && request.method === 'POST') {
    let body = '';
    request.setEncoding('utf8');
    request.on('data', (chunk: string) => { body += chunk; });
    request.on('end', () => {
      let inputCount = 0;
      try {
        const decoded = JSON.parse(body) as { readonly batch?: readonly unknown[] };
        inputCount = Array.isArray(decoded.batch) ? decoded.batch.length : 0;
      } catch {
        // The generated adapter performs the strict request encoding; this
        // server only records a deterministic Rust-shaped response.
      }
      const operation = pathname.slice('/__rusty/product/runtime/'.length);
      if (operation === 'lifecycle/start') {
        runtimeStream.response?.write(initialRuntimeOutputs()
          .map((value) => `data: ${JSON.stringify(value)}\n\n`).join(''));
      }
      const result = operation === 'input'
        ? { accepted: true, count: inputCount, binding: RUNTIME, readout: READOUT }
        : operation === 'audio-feedback' || operation === 'animation-feedback'
          ? { accepted: true, runtime: RUNTIME }
          : {
            accepted: true,
            operation: operation === 'lifecycle/start' ? 'start' : 'advance-realtime',
            binding: RUNTIME,
            nextInputSequence: '1',
            readout: READOUT,
          };
      sendJson(response, result);
    });
    return;
  }
  const asset = assets.get(decodeURIComponent(pathname));
  if (asset === undefined) {
    response.writeHead(404, { 'content-type': 'text/plain; charset=utf-8' });
    response.end('not found');
    return;
  }
  response.writeHead(200, { 'content-type': asset.contentType });
  response.end(asset.body);
}

function initialRuntimeOutputs(): readonly unknown[] {
  return [
    { kind: 'binding', runtime: RUNTIME, nextInputSequence: '1' },
    { kind: 'runtime-readout', readout: READOUT },
    {
      kind: 'frame',
      frame: {
        schemaVersion: 1,
        ops: [{
          op: 'create',
          handle: 1,
          parent: null,
          node: {
            geometry: { kind: 'cube' },
            material: { color: [0.15, 0.55, 0.95, 1], wireframe: false },
            transform: {
              translation: [0, 0, -4],
              rotation: [0, 0, 0, 1],
              scale: [2, 2, 2],
            },
            visible: true,
            layer: 'scene',
            metadata: {
              sourceEntity: null,
              sourceSceneNode: null,
              tags: [],
              label: 'startup-readiness-proof',
            },
          },
        }],
      },
    },
    {
      kind: 'view-composition',
      composition: {
        schemaVersion: 1,
        cameras: [{
          id: 'camera.startup',
          pose: { position: [0, 0, 0], pitchDegrees: 0, yawDegrees: 0 },
          projection: { kind: 'perspective', fovYDegrees: 60, near: 0.1, far: 100 },
        }],
        targets: [],
        views: [{
          id: 'view.startup',
          cameraId: 'camera.startup',
          order: 0,
          target: { kind: 'primary' },
          viewport: { x: 0.5, y: 0, width: 0.5, height: 1 },
        }],
        presentations: [],
      },
    },
    {
      kind: 'ui-projection',
      envelope: {
        artifact: 'rusty.product.ui-projection',
        runtime: RUNTIME,
        sequence: '0',
        stream: 'product.local',
        contract: 'product.local.current',
        value: { status: 'ready' },
      },
    },
  ];
}

function exposeBundleHost(
  assets: readonly ProductBrowserBundleAsset[],
): readonly ProductBrowserBundleAsset[] {
  return assets.map((asset) => {
    if (asset.name !== 'main.js') return asset;
    const content = asset.content.replace(
      'void host;',
      'globalThis.__rustyBundleHost = host;',
    );
    if (content === asset.content) {
      throw new Error('generated Product Bundle host exposure marker is missing');
    }
    return Object.freeze({ ...asset, content });
  });
}

async function readStartupRendererProof(page: Page): Promise<{
  readonly composed: readonly number[];
  readonly unoccupied: readonly number[];
} | null> {
  return page.evaluate(() => {
    const testWindow = globalThis as typeof globalThis & {
      readonly __rustyBundleHost?: {
        readonly application: {
          readonly renderer: { readonly renderOnce: (timeMs?: number) => void };
        };
      };
    };
    const host = testWindow.__rustyBundleHost;
    const canvas = document.querySelector<HTMLCanvasElement>(
      'canvas[data-rusty-application-renderer="engine-owned"]',
    );
    if (host === undefined || canvas === null) return null;
    host.application.renderer.renderOnce(1);
    const context = canvas.getContext('webgl2') ?? canvas.getContext('webgl');
    if (context === null) return null;
    const read = (x: number): readonly number[] => {
      const pixel = new Uint8Array(4);
      context.readPixels(
        Math.floor(context.drawingBufferWidth * x),
        Math.floor(context.drawingBufferHeight * 0.5),
        1,
        1,
        context.RGBA,
        context.UNSIGNED_BYTE,
        pixel,
      );
      return [...pixel];
    };
    return { composed: read(0.75), unoccupied: read(0.25) };
  });
}

interface RendererPreloadResource {
  readonly identity: string;
  readonly contentHash: string;
  readonly mediaType: 'image/png' | 'audio/wav' | 'application/octet-stream';
  readonly path: string;
  readonly byteLength: number;
  readonly bytes: Uint8Array;
}

function rendererPreloadResources(
  options: { readonly invalidMeshHeader?: boolean; readonly tamperedTextureBytes?: boolean } = {},
): readonly RendererPreloadResource[] {
  const png = Uint8Array.from(Buffer.from(
    'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVQIHWP4z8DwHwAFgAI/ScL95wAAAABJRU5ErkJggg==',
    'base64',
  ));
  const wav = new Uint8Array(44);
  wav.set([82, 73, 70, 70], 0);
  wav.set([87, 65, 86, 69], 8);
  const mesh = packedMeshResourceBytes();
  if (options.invalidMeshHeader === true) mesh[12] = 0;
  const texture = preloadResource('texture', 'image/png', 'content/renderer/é.png', png);
  if (options.tamperedTextureBytes === true) png[8] = png[8]! ^ 1;
  return Object.freeze([
    texture,
    preloadResource('audio', 'audio/wav', 'content/renderer/theme.wav', wav),
    preloadResource('mesh', 'application/octet-stream', 'content/renderer/packed.rmesh', mesh),
  ]);
}

async function removeWebCryptoSubtle(page: Page): Promise<void> {
  await page.addInitScript(() => {
    Object.defineProperty(globalThis.crypto, 'subtle', {
      configurable: true,
      value: undefined,
    });
  });
}

function preloadResource(
  kind: 'texture' | 'audio' | 'mesh',
  mediaType: 'image/png' | 'audio/wav' | 'application/octet-stream',
  path: string,
  bytes: Uint8Array,
): RendererPreloadResource {
  const hash = createHash('sha256').update(bytes).digest('hex');
  return Object.freeze({
    identity: `${kind}-resource/${hash}`,
    contentHash: `sha256:${hash}`,
    mediaType,
    path,
    byteLength: bytes.byteLength,
    bytes,
  });
}

function packedMeshResourceBytes(): Uint8Array {
  const bytes = new Uint8Array(16);
  bytes.set(Buffer.from('RMSHLE01', 'ascii'), 0);
  new DataView(bytes.buffer).setUint32(8, bytes.byteLength, true);
  new DataView(bytes.buffer).setUint32(12, 1, true);
  return bytes;
}

function sendJson(response: ServerResponse, value: unknown): void {
  const body = JSON.stringify(value);
  response.writeHead(200, { 'content-type': 'application/json; charset=utf-8' });
  response.end(body);
}

function contentTypeFor(path: string): string {
  return path.endsWith('.html')
    ? 'text/html; charset=utf-8'
    : path.endsWith('.json')
      ? 'application/json; charset=utf-8'
      : path.endsWith('.png')
        ? 'image/png'
        : path.endsWith('.wav')
          ? 'audio/wav'
          : path.endsWith('.rmesh')
            ? 'application/octet-stream'
            : 'text/javascript; charset=utf-8';
}

async function closeBundleServer(server: Server): Promise<void> {
  // EventSource intentionally stays connected for the lifetime of the host;
  // close the test-only HTTP connections before awaiting server.close().
  server.closeAllConnections();
  await new Promise<void>((resolve, reject) => {
    server.close((error) => error === undefined ? resolve() : reject(error));
  });
}
