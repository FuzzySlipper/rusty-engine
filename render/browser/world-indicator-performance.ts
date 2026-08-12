import {
  billboardHandle,
  type BillboardDescriptor,
  type PresentationFrameDiff,
  type PresentationOp,
} from '@rusty-engine/render-contracts';
import { RendererBillboardHost } from '@rusty-engine/renderer-host';

export interface WorldIndicatorPerformanceResult {
  readonly idleRefreshMedianMs: number;
  readonly valueUpdateMedianMs: number;
  readonly submitted500Ms: number;
  readonly visibleAfter100: number;
  readonly visibleAfter500: number;
}

declare global {
  interface Window {
    __rustyWorldIndicatorPerformance?: Promise<WorldIndicatorPerformanceResult>;
    __rustyWorldIndicatorLayout?: {
      readonly setCameraOffset: (pixels: number) => void;
      readonly setOccluded: (value: boolean) => void;
    };
  }
}

const container = requiredIndicatorContainer();

let cameraOffset = 0;
let occluded = false;
const host = new RendererBillboardHost({
  container,
  resolveEntityPosition: () => null,
  projectWorld: ([x, y]) => ({
    xPixels: 28 + x * 76 + cameraOffset,
    yPixels: 44 + y * 48,
    depth: 0.5,
    distance: 10,
    insideViewport: true,
    occluded,
  }),
});

window.__rustyWorldIndicatorPerformance = measure().then(async (result) => {
  await setupLayoutProof();
  return result;
});

async function measure(): Promise<WorldIndicatorPerformanceResult> {
  await host.applyPresentation(frame(
    Array.from({ length: 100 }, (_, index) =>
      operation(index, 'create', index, descriptor(index, 50))),
  ));
  const visibleAfter100 = visibleCount();
  const idleRefreshMedianMs = median(Array.from({ length: 21 }, () => {
    const start = performance.now();
    host.refreshLayout();
    return performance.now() - start;
  }));

  const updates: number[] = [];
  for (let run = 0; run < 21; run += 1) {
    const start = performance.now();
    await host.applyPresentation(frame(
      Array.from({ length: 100 }, (_, index) =>
        operation(index, 'update', index, descriptor(index, 51 + run).content)),
    ));
    updates.push(performance.now() - start);
  }
  const valueUpdateMedianMs = median(updates);

  const start500 = performance.now();
  await host.applyPresentation(frame(
    Array.from({ length: 400 }, (_, offset) => {
      const index = offset + 100;
      return operation(offset, 'create', index, descriptor(index, 72));
    }),
  ));
  const submitted500Ms = performance.now() - start500;
  const visibleAfter500 = visibleCount();

  return {
    idleRefreshMedianMs,
    valueUpdateMedianMs,
    submitted500Ms,
    visibleAfter100,
    visibleAfter500,
  };
}

async function setupLayoutProof(): Promise<void> {
  host.cleanup();
  const edge = {
    ...descriptor(0, 70),
    anchor: { kind: 'world' as const, position: [-4, 2, 0] as const },
    layer: 'alwaysOnTop' as const,
    layout: {
      ...descriptor(0, 70).layout!,
      priority: 100,
      edgeBehavior: 'clamp' as const,
    },
  };
  const depth = {
    ...descriptor(1, 65),
    anchor: { kind: 'world' as const, position: [5, 3, 0] as const },
    layer: 'depthTested' as const,
    layout: { ...descriptor(1, 65).layout!, priority: 90 },
  };
  const occlusion = {
    ...descriptor(2, 60),
    anchor: { kind: 'world' as const, position: [8, 3, 0] as const },
    layer: 'occluded' as const,
    layout: { ...descriptor(2, 60).layout!, priority: 80 },
  };
  const suppressed = {
    ...descriptor(3, 55),
    anchor: depth.anchor,
    layer: 'depthTested' as const,
    layout: { ...descriptor(3, 55).layout!, priority: 1 },
  };
  await host.applyPresentation(frame([
    operation(0, 'create', 0, edge),
    operation(1, 'create', 1, depth),
    operation(2, 'create', 2, occlusion),
    operation(3, 'create', 3, suppressed),
  ]));
  window.__rustyWorldIndicatorLayout = {
    setCameraOffset: (pixels) => {
      cameraOffset = pixels;
      host.refreshLayout();
    },
    setOccluded: (value) => {
      occluded = value;
      host.refreshLayout();
    },
  };
}

function operation(
  sequence: number,
  op: 'create' | 'update',
  index: number,
  value: BillboardDescriptor | BillboardDescriptor['content'],
): PresentationOp {
  return {
    domain: 'billboard',
    meta: { sequence },
    op: op === 'create'
      ? { op, handle: billboardHandle(index + 1), descriptor: value as BillboardDescriptor }
      : {
        op,
        handle: billboardHandle(index + 1),
        patch: {
          anchor: null,
          content: value as BillboardDescriptor['content'],
          font: null,
          heightPixels: null,
          color: null,
          background: null,
          maxDistance: null,
          layer: null,
          visible: null,
        },
      },
  };
}

function descriptor(index: number, current: number): BillboardDescriptor {
  return {
    anchor: { kind: 'world', position: [index % 16, Math.floor(index / 16), 0] },
    content: {
      kind: 'structured',
      indicator: {
        label: { localizationKey: 'actor.name', fallbackText: `Actor ${index + 1}` },
        icon: null,
        accessibleLabel: {
          localizationKey: 'actor.indicator',
          fallbackText: `Actor ${index + 1} status`,
        },
        meters: [{
          id: 'health',
          accessibleLabel: { localizationKey: 'resource.health', fallbackText: 'Health' },
          current,
          min: 0,
          max: 100,
          preview: null,
          fillDirection: 'leftToRight',
          segments: 10,
          fill: [0.2, 0.8, 0.3, 1],
          previewFill: [0.9, 0.7, 0.1, 1],
          back: [0.05, 0.05, 0.05, 0.9],
          border: [0, 0, 0, 1],
        }],
        statusCues: [],
        widthPixels: 68,
        spacingPixels: 2,
        alignment: 'center',
        style: {
          opacity: 1,
          backing: [0, 0, 0, 0.5],
          border: [0, 0, 0, 1],
          radiusPixels: 2,
        },
      },
    },
    font: { kind: 'system', family: 'sans-serif' },
    heightPixels: 10,
    color: [1, 1, 1, 1],
    background: [0, 0, 0, 0],
    maxDistance: 100,
    layer: 'depthTested',
    visible: true,
    layout: {
      priority: 500 - index,
      sizing: { kind: 'constantPixels' },
      safeArea: { topPixels: 2, rightPixels: 2, bottomPixels: 2, leftPixels: 2 },
      edgeBehavior: 'cull',
      overlapBehavior: 'suppress',
    },
  };
}

function frame(ops: readonly PresentationOp[]): PresentationFrameDiff {
  return { schemaVersion: 1, ops };
}

function visibleCount(): number {
  return [...container.children].filter((element) =>
    element instanceof HTMLElement && element.style.display !== 'none').length;
}

function median(values: readonly number[]): number {
  const sorted = [...values].sort((left, right) => left - right);
  return sorted[Math.floor(sorted.length / 2)] ?? 0;
}

function requiredIndicatorContainer(): HTMLElement {
  const value = document.querySelector<HTMLElement>('#indicators');
  if (value === null) throw new Error('indicator performance container is missing');
  return value;
}
