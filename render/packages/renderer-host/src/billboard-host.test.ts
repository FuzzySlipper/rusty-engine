import assert from 'node:assert/strict';
import test from 'node:test';
import {
  billboardHandle,
  type BillboardDescriptor,
  type BillboardPatch,
  type PresentationFrameDiff,
  type PresentationOp,
} from '@rusty-engine/render-contracts';
import {
  RendererBillboardHost,
  type RendererBillboardElement,
  type RendererBillboardElementStyle,
} from './billboard-host.js';
import { RendererPresentationHostSet } from './presentation-host-set.js';

class FakeElement implements RendererBillboardElement {
  readonly style = {} as RendererBillboardElementStyle;
  textContent: string | null = null;
  readonly attributes = new Map<string, string>();
  removed = false;

  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }

  remove(): void {
    this.removed = true;
  }
}

class FakeContainer {
  readonly elements: FakeElement[] = [];

  appendChild(element: RendererBillboardElement): void {
    this.elements.push(element as FakeElement);
  }
}

function descriptor(entity: number, layer: BillboardDescriptor['layer'] = 'occluded'): BillboardDescriptor {
  return {
    anchor: { kind: 'entityAttached', entity, offset: [0, 2, 0] },
    content: {
      kind: 'value',
      labelKey: 'enemy.health',
      fallbackLabel: 'Enemy health',
      value: '80/100',
      unitKey: null,
      fallbackUnit: null,
    },
    font: { kind: 'system', family: 'sans-serif' },
    heightPixels: 24,
    color: [1, 1, 1, 1],
    background: [0, 0, 0, 0.7],
    maxDistance: 40,
    layer,
    visible: true,
  };
}

function patch(overrides: Partial<BillboardPatch>): BillboardPatch {
  return {
    anchor: null,
    content: null,
    font: null,
    heightPixels: null,
    color: null,
    background: null,
    maxDistance: null,
    layer: null,
    visible: null,
    ...overrides,
  };
}

function operation(
  sequence: number,
  op: Extract<PresentationOp, { readonly domain: 'billboard' }>['op'],
): PresentationOp {
  return {
    domain: 'billboard',
    meta: { sequence },
    op,
  };
}

function presentation(ops: readonly PresentationOp[]): PresentationFrameDiff {
  return { schemaVersion: 1, ops };
}

void test('billboard host creates updates localizes lays out and destroys multiple entity cues', async () => {
  const container = new FakeContainer();
  const positions = new Map([
    [10, [1, 0, 3] as const],
    [20, [-1, 0, 4] as const],
  ]);
  const host = new RendererBillboardHost({
    container,
    createElement: () => new FakeElement(),
    localize: (key, fallback, argumentsByName) => {
      const localized = key === 'enemy.health' ? 'Vitality' : fallback;
      return Object.entries(argumentsByName).reduce(
        (text, [name, value]) => text.replaceAll(`{${name}}`, value),
        localized,
      );
    },
    resolveEntityPosition: (entity) => positions.get(entity) ?? null,
    projectWorld: (position) => ({
      xPixels: 400 + position[0] * 10,
      yPixels: 220 - position[1] * 10,
      depth: position[2] / 10,
      distance: position[2],
      insideViewport: true,
      occluded: false,
    }),
  });
  const first = await host.applyPresentation(presentation([
    operation(0, { op: 'create', handle: billboardHandle(1), descriptor: descriptor(10) }),
    operation(1, { op: 'create', handle: billboardHandle(2), descriptor: descriptor(20) }),
  ]));
  assert.equal(first.applied, 2);
  assert.equal(first.readout.activeBillboards, 2);
  assert.equal(container.elements[0]?.textContent, 'Vitality: 80/100');
  assert.equal(container.elements[0]?.style.left, '410px');

  const second = await host.applyPresentation(presentation([
    operation(0, {
      op: 'update',
      handle: billboardHandle(2),
      patch: patch({
        content: {
          kind: 'text',
          localizationKey: 'enemy.defeated',
          fallbackText: 'Target {state}',
          arguments: [{ name: 'state', value: 'defeated' }],
        },
      }),
    }),
    operation(1, { op: 'destroy', handle: billboardHandle(1) }),
  ]));
  assert.equal(second.applied, 2);
  assert.equal(second.readout.activeBillboards, 1);
  assert.equal(container.elements[1]?.textContent, 'Target defeated');
  assert.equal(container.elements[0]?.removed, true);
});

void test('billboard updates validate the merged structured layout before mutation', async () => {
  const container = new FakeContainer();
  const host = new RendererBillboardHost({
    container,
    createElement: () => new FakeElement(),
    resolveEntityPosition: () => [0, 0, 0],
    projectWorld: () => ({
      xPixels: 100,
      yPixels: 50,
      depth: 0.5,
      distance: 5,
      insideViewport: true,
      occluded: false,
    }),
  });
  const legacy: BillboardDescriptor = {
    ...descriptor(10),
    content: {
      kind: 'text',
      localizationKey: 'indicator.legacy',
      fallbackText: 'Legacy indicator',
      arguments: [],
    },
  };
  const structured = (fallbackText: string): BillboardDescriptor['content'] => ({
    kind: 'structured',
    indicator: {
      label: { localizationKey: 'indicator.name', fallbackText },
      icon: null,
      accessibleLabel: { localizationKey: 'indicator.status', fallbackText: `${fallbackText} status` },
      meters: [],
      statusCues: [],
      widthPixels: 120,
      spacingPixels: 4,
      alignment: 'center',
      style: {
        opacity: 1,
        backing: [0, 0, 0, 0.5],
        border: [0, 0, 0, 1],
        radiusPixels: 4,
      },
    },
  });
  const layout: NonNullable<BillboardDescriptor['layout']> = {
    priority: 10,
    sizing: { kind: 'constantPixels' },
    safeArea: { topPixels: 2, rightPixels: 2, bottomPixels: 2, leftPixels: 2 },
    edgeBehavior: 'clamp',
    overlapBehavior: 'stack',
  };

  const created = await host.applyPresentation(presentation([
    operation(0, { op: 'create', handle: billboardHandle(1), descriptor: legacy }),
  ]));
  const element = container.elements[0];
  assert.equal(created.applied, 1);
  assert.equal(element?.textContent, 'Legacy indicator');

  const rejected = await host.applyPresentation(presentation([
    operation(0, {
      op: 'update',
      handle: billboardHandle(1),
      patch: patch({ content: structured('Structured one') }),
    }),
  ]));
  assert.equal(rejected.applied, 0);
  assert.equal(rejected.diagnostics[0]?.code, 'invalidDescriptor');
  assert.equal(rejected.readout.activeBillboards, 1);
  assert.equal(container.elements[0], element);
  assert.equal(element?.textContent, 'Legacy indicator');
  assert.equal(element?.style.display, 'block');

  const retried = await host.applyPresentation(presentation([
    operation(0, {
      op: 'update',
      handle: billboardHandle(1),
      patch: patch({ content: structured('Structured one'), layout }),
    }),
  ]));
  assert.equal(retried.applied, 1);
  assert.equal(element?.textContent, 'Structured one');
  assert.equal(element?.style.display, 'flex');

  const reused = await host.applyPresentation(presentation([
    operation(0, {
      op: 'update',
      handle: billboardHandle(1),
      patch: patch({ content: structured('Structured two') }),
    }),
  ]));
  assert.equal(reused.applied, 1);
  assert.equal(element?.textContent, 'Structured two');
  assert.equal(element?.style.display, 'flex');
});

void test('billboard layers and distance culling are renderer-owned and do not alter descriptors', async () => {
  const container = new FakeContainer();
  let occluded = true;
  let distance = 12;
  const host = new RendererBillboardHost({
    container,
    createElement: () => new FakeElement(),
    resolveEntityPosition: () => [0, 0, 0],
    projectWorld: () => ({
      xPixels: 100,
      yPixels: 50,
      depth: 0.5,
      distance,
      insideViewport: true,
      occluded,
    }),
  });
  await host.applyPresentation(presentation([
    operation(0, { op: 'create', handle: billboardHandle(1), descriptor: descriptor(10, 'occluded') }),
    operation(1, { op: 'create', handle: billboardHandle(2), descriptor: descriptor(20, 'alwaysOnTop') }),
  ]));
  assert.equal(container.elements[0]?.style.display, 'none');
  assert.equal(container.elements[1]?.style.display, 'block');
  assert.equal(container.elements[1]?.style.zIndex, '30000');
  distance = 50;
  occluded = false;
  host.refreshLayout();
  assert.equal(host.readout().culledBillboards, 2);
});

void test('persistent missing anchors retain a bounded deduplicated diagnostic', async () => {
  const host = new RendererBillboardHost({
    container: new FakeContainer(),
    createElement: () => new FakeElement(),
    resolveEntityPosition: () => null,
    projectWorld: () => ({
      xPixels: 0,
      yPixels: 0,
      depth: 0,
      distance: 0,
      insideViewport: true,
      occluded: false,
    }),
  });
  await host.applyPresentation(presentation([
    operation(0, { op: 'create', handle: billboardHandle(1), descriptor: descriptor(10) }),
  ]));
  for (let refresh = 0; refresh < 300; refresh += 1) host.refreshLayout();
  assert.equal(host.readout().diagnostics.length, 1);
  assert.equal(host.readout().diagnostics[0]?.code, 'anchorMissing');
});

void test('pending async resource admission cannot resurrect a billboard after cleanup', async () => {
  const container = new FakeContainer();
  const fontBytes = new Uint8Array([1, 2, 3]).buffer;
  const fontHash = await sha256(fontBytes);
  let release!: () => void;
  const resourceGate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const host = new RendererBillboardHost({
    container,
    createElement: () => new FakeElement(),
    loadFont: async () => undefined,
    resolveEntityPosition: () => [0, 0, 0],
    projectWorld: () => ({
      xPixels: 0,
      yPixels: 0,
      depth: 0,
      distance: 0,
      insideViewport: true,
      occluded: false,
    }),
    resolveResource: async () => {
      await resourceGate;
      return { bytes: fontBytes };
    },
  });
  const pending = host.applyPresentation(presentation([
    operation(0, {
      op: 'create',
      handle: billboardHandle(1),
      descriptor: {
        ...descriptor(10),
        font: {
          kind: 'asset',
          asset: 'font/delayed',
          contentHash: fontHash,
          family: 'Delayed',
        },
      },
    }),
  ]));
  host.cleanup();
  release();
  const receipt = await pending;
  assert.equal(receipt.applied, 0);
  assert.equal(receipt.diagnostics[0]?.code, 'hostFailure');
  assert.equal(host.readout().activeBillboards, 0);
  assert.equal(container.elements.length, 0);
});

void test('font and icon resources are SHA-256 validated cached and fail with typed diagnostics', async () => {
  const fontBytes = new Uint8Array([1, 2, 3]).buffer;
  const iconBytes = new Uint8Array([4, 5, 6]).buffer;
  const fontHash = await sha256(fontBytes);
  const iconHash = await sha256(iconBytes);
  let fontLoads = 0;
  const host = new RendererBillboardHost({
    container: new FakeContainer(),
    createElement: () => new FakeElement(),
    loadFont: async () => { fontLoads += 1; },
    resolveEntityPosition: () => [0, 0, 0],
    projectWorld: () => ({ xPixels: 0, yPixels: 0, depth: 0, distance: 0, insideViewport: true, occluded: false }),
    resolveResource: async (asset) => asset.startsWith('font/')
      ? { bytes: fontBytes }
      : { bytes: iconBytes, url: '/fixture-icon.png' },
  });
  const assetDescriptor: BillboardDescriptor = {
    ...descriptor(10),
    font: { kind: 'asset', asset: 'font/ui-sans', contentHash: fontHash, family: 'Renderer UI' },
    content: {
      kind: 'icon',
      texture: { asset: 'texture/alert', contentHash: iconHash },
      altKey: 'alert',
      fallbackAlt: 'Alert',
    },
  };
  const receipt = await host.applyPresentation(presentation([
    operation(0, { op: 'create', handle: billboardHandle(1), descriptor: assetDescriptor }),
    operation(1, { op: 'create', handle: billboardHandle(2), descriptor: { ...assetDescriptor, anchor: { kind: 'world', position: [0, 1, 0] } } }),
  ]));
  assert.equal(receipt.diagnostics.length, 0);
  assert.equal(receipt.readout.loadedFonts, 1);
  assert.equal(receipt.readout.loadedIcons, 1);
  assert.equal(fontLoads, 1);

  const bad = await host.applyPresentation(presentation([
    operation(0, {
      op: 'create',
      handle: billboardHandle(3),
      descriptor: {
        ...assetDescriptor,
        font: { kind: 'asset', asset: 'font/ui-sans', contentHash: '00', family: 'Renderer UI' },
      },
    }),
  ]));
  assert.equal(bad.applied, 0);
  assert.equal(bad.diagnostics[0]?.code, 'contentHashMismatch');

  const missingFontHost = new RendererBillboardHost({
    container: new FakeContainer(),
    createElement: () => new FakeElement(),
    loadFont: async () => undefined,
    resolveEntityPosition: () => [0, 0, 0],
    projectWorld: () => ({ xPixels: 0, yPixels: 0, depth: 0, distance: 0, insideViewport: true, occluded: false }),
    resolveResource: async () => null,
  });
  const missingFont = await missingFontHost.applyPresentation(presentation([
    operation(0, {
      op: 'create',
      handle: billboardHandle(4),
      descriptor: assetDescriptor,
    }),
  ]));
  assert.equal(missingFont.applied, 0);
  assert.equal(missingFont.diagnostics[0]?.code, 'fontLoadFailed');
  assert.equal(missingFont.diagnostics[0]?.sequence, 0);
  assert.equal(missingFont.readout.activeBillboards, 0);
});

void test('a missing billboard host is isolated with an explicit domain receipt', async () => {
  const receipt = await new RendererPresentationHostSet({}).apply(presentation([
    operation(0, { op: 'create', handle: billboardHandle(1), descriptor: descriptor(10) }),
  ]));
  const billboard = receipt.domains.find((domain) => domain.domain === 'billboard');
  assert.equal(billboard?.applied, 0);
  assert.equal(billboard?.configured, false);
  assert.equal(billboard?.diagnostics[0]?.code, 'unavailableHost');
});

async function sha256(bytes: ArrayBuffer): Promise<string> {
  const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
}
