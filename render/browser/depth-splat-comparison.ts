import type {
  RenderFrameDiff,
  RenderMaterialDescriptor,
  StaticMeshAsset,
  TextureDescriptor,
} from '@rusty-engine/render-contracts';
import { decodeRenderFrameDiff, renderHandle } from '@rusty-engine/render-contracts';
import {
  mountRendererSurface,
  type RendererMeshResourceManifest,
  type RendererSurface,
  type RendererSurfaceSubmissionSample,
  type RendererTextureResourceManifest,
} from '@rusty-engine/renderer-host';
import {
  DEPTH_SPLAT_MESH_RESOURCE_LOCATIONS,
  DEPTH_SPLAT_TEXTURE_RESOURCE_LOCATIONS,
} from './depth-splat-resource-locations.js';

interface DepthSplatFixture {
  readonly schemaVersion: 1;
  readonly source: {
    readonly project: string;
    readonly task: number;
    readonly run: string;
    readonly subject: string;
    readonly direction: string;
    readonly sourceGlbSha256: Readonly<Record<string, string>>;
  };
  readonly meshResourceManifest: RendererMeshResourceManifest;
  readonly textureResourceManifest: RendererTextureResourceManifest;
  readonly texture: TextureDescriptor;
  readonly materials: readonly RenderMaterialDescriptor[];
  readonly assets: readonly StaticMeshAsset[];
  readonly metrics: {
    readonly sourceGlbBytes: number;
    readonly packedMeshBytes: number;
    readonly encodedTextureBytes: number;
    readonly decodedTextureBytes: number;
    readonly uploadedMeshBytes: number;
    readonly variants: readonly {
      readonly id: string;
      readonly vertices: number;
      readonly triangles: number;
      readonly sourceGlbBytes: number;
      readonly uploadedBytes: number;
    }[];
  };
}

interface DepthSplatReadout {
  readonly route: number;
  readonly visibleVariant: string | null;
  readonly occluderVisible: boolean;
  readonly source: DepthSplatFixture['source'];
  readonly metrics: DepthSplatFixture['metrics'];
  readonly submission: RendererSurfaceSubmissionSample;
  readonly visibility: ReturnType<RendererSurface['visibilityReadout']>;
  readonly pick: ReturnType<RendererSurface['pick']>;
  readonly averageCameraRouteMs: number | null;
  readonly mechanisms: {
    readonly alphaModes: readonly string[];
    readonly allDoubleSided: boolean;
    readonly depictionCount: number;
    readonly retainedInstancesPerDepiction: 1;
    readonly textureFilter: string;
  };
  readonly disposed: boolean;
}

interface DepthSplatComparisonApi {
  readonly ready: true;
  step(route: number): DepthSplatReadout;
  setVisibleVariant(variant: string | null): DepthSplatReadout;
  setOccluder(visible: boolean): DepthSplatReadout;
  transformVariant(variant: string, x: number, scale: number): DepthSplatReadout;
  snapshot(): DepthSplatReadout;
  capture(): string;
  sample(): readonly number[];
  measure(): Promise<DepthSplatReadout>;
  dispose(): { readonly disposed: true };
}

declare global {
  interface Window {
    depthSplatComparison?: DepthSplatComparisonApi;
  }
}

const INSTANCE_HANDLES = [100, 101, 102, 103, 104].map(renderHandle);
const OCCLUDER_HANDLE = renderHandle(201);
const VARIANT_X = [-5.2, -2.6, 0, 2.6, 5.2] as const;
const VARIANT_SCALE = [2.8, 1.8, 1.8, 1.8, 1.8] as const;

void boot();

async function boot(): Promise<void> {
  const fixtureUrl = new URL('../../fixtures/render/depth-splat-comparison-v1.json', import.meta.url);
  const fixture = await parseFixture(await (await fetch(fixtureUrl)).json());
  const frame = comparisonFrame(fixture);
  const canvas = requiredElement<HTMLCanvasElement>('comparison');
  const surface = await mountRendererSurface(canvas, {
    autoStart: false,
    clearColor: 0x283746,
    controls: { enabled: false, initialPosition: [0, 1.1, 7] },
    fog: { color: 0x283746, near: 7, far: 18 },
    frame,
    lighting: {
      schemaVersion: 1,
      defaultLights: { world: 'neutral', viewmodel: 'disabled' },
      shadows: { enabled: false },
    },
    meshResourceManifest: fixture.meshResourceManifest,
    resolveMeshResource: resourceResolver(DEPTH_SPLAT_MESH_RESOURCE_LOCATIONS),
    textureResourceManifest: fixture.textureResourceManifest,
    resolveTextureResource: resourceResolver(DEPTH_SPLAT_TEXTURE_RESOURCE_LOCATIONS),
    pixelRatio: 1,
    projection: { fovYDegrees: 50, near: 0.1, far: 40 },
  });
  const labels = requiredElement<HTMLDivElement>('labels');
  labels.replaceChildren(...fixture.metrics.variants.map((variant, index) => {
    const label = document.createElement('div');
    label.className = 'label';
    label.style.left = `${String(82 + index * 246)}px`;
    label.textContent = `${variant.id} · ${String(variant.triangles)} tris`;
    return label;
  }));

  let route = 0;
  let visibleVariant: string | null = null;
  let occluderVisible = true;
  let disposed = false;
  let averageCameraRouteMs: number | null = null;
  let sourceTime = 0;
  const readout = requiredElement<HTMLPreElement>('readout');

  function render(): RendererSurfaceSubmissionSample {
    if (disposed) throw new Error('depth-splat comparison is disposed');
    sourceTime += 1;
    return surface.renderOnce(sourceTime);
  }

  function snapshot(): DepthSplatReadout {
    if (disposed) throw new Error('depth-splat comparison is disposed');
    const submission = surface.submission();
    const value = Object.freeze({
      route,
      visibleVariant,
      occluderVisible,
      source: fixture.source,
      metrics: fixture.metrics,
      submission,
      visibility: surface.visibilityReadout(),
      pick: surface.pick({ ray: { kind: 'viewport', point: [0, 0] }, maxDistance: 40 }),
      averageCameraRouteMs,
      mechanisms: {
        alphaModes: fixture.materials.map((material) => material.alphaMode?.kind ?? 'opaque'),
        allDoubleSided: fixture.materials.every((material) => material.doubleSided === true),
        depictionCount: fixture.assets.length,
        retainedInstancesPerDepiction: 1 as const,
        textureFilter: fixture.texture.filter,
      },
      disposed,
    });
    readout.textContent = JSON.stringify(value, null, 2);
    return value;
  }

  function step(nextRoute: number): DepthSplatReadout {
    if (!Number.isFinite(nextRoute)) throw new TypeError('route must be finite');
    route = ((nextRoute % 1) + 1) % 1;
    const angle = route * Math.PI * 2;
    surface.setCameraPose({
      position: [Math.sin(angle) * 1.4, 1.1 + Math.sin(angle * 2) * 0.25, 7 + Math.cos(angle) * 2],
      pitchDegrees: Math.sin(angle * 2) * 1.2,
      yawDegrees: Math.sin(angle) * -7,
    });
    render();
    return snapshot();
  }

  function setVisibleVariant(variant: string | null): DepthSplatReadout {
    if (variant !== null && !fixture.metrics.variants.some((candidate) => candidate.id === variant)) {
      throw new RangeError(`unknown variant ${variant}`);
    }
    visibleVariant = variant;
    applyComparisonFrame(surface, decodeRenderFrameDiff({
      schemaVersion: 1,
      ops: fixture.metrics.variants.map((candidate, index) => ({
        op: 'update',
        handle: INSTANCE_HANDLES[index],
        transform: null,
        material: null,
        visible: variant === null || candidate.id === variant,
        metadata: null,
      })),
    }));
    render();
    return snapshot();
  }

  function setOccluder(visible: boolean): DepthSplatReadout {
    occluderVisible = visible;
    applyComparisonFrame(surface, decodeRenderFrameDiff({ schemaVersion: 1, ops: [{
      op: 'update', handle: OCCLUDER_HANDLE, transform: null, material: null,
      visible, metadata: null,
    }] }));
    render();
    return snapshot();
  }

  function transformVariant(variant: string, x: number, scale: number): DepthSplatReadout {
    const index = fixture.metrics.variants.findIndex((candidate) => candidate.id === variant);
    if (index < 0 || !Number.isFinite(x) || !Number.isFinite(scale) || scale <= 0) {
      throw new RangeError('transform needs a known variant, finite x, and positive scale');
    }
    applyComparisonFrame(surface, decodeRenderFrameDiff({ schemaVersion: 1, ops: [{
      op: 'update', handle: INSTANCE_HANDLES[index],
      transform: transform([x, 0, -5], scale), material: null, visible: null, metadata: null,
    }] }));
    render();
    return snapshot();
  }

  render();
  const api: DepthSplatComparisonApi = {
    ready: true,
    step,
    setVisibleVariant,
    setOccluder,
    transformVariant,
    snapshot,
    capture: () => canvas.toDataURL('image/png'),
    sample: () => sampleCanvas(canvas),
    measure: async () => {
      await animationFrames(2);
      const samples = 24;
      const started = performance.now();
      for (let index = 0; index < samples; index += 1) step(index / samples);
      averageCameraRouteMs = (performance.now() - started) / samples;
      step(0);
      return snapshot();
    },
    dispose: () => {
      if (!disposed) {
        surface.dispose();
        disposed = true;
        readout.textContent = JSON.stringify({ disposed: true }, null, 2);
      }
      return { disposed: true };
    },
  };
  window.depthSplatComparison = api;
  snapshot();
}

function comparisonFrame(fixture: DepthSplatFixture): RenderFrameDiff {
  const ops: unknown[] = [
    { op: 'defineTexture', texture: fixture.texture },
    ...fixture.materials.map((material) => ({ op: 'defineMaterial', material })),
    ...fixture.assets.map((asset) => ({ op: 'defineStaticMesh', asset })),
    {
      op: 'create', handle: renderHandle(200), parent: null,
      node: primitiveNode('comparison-floor', [0, -0.2, -5], [15, 0.18, 8], [0.17, 0.22, 0.24, 1]),
    },
    {
      op: 'create', handle: OCCLUDER_HANDLE, parent: null,
      node: primitiveNode('depth-occluder', [0, 1, -2.7], [1.35, 2.35, 0.35], [0.08, 0.12, 0.16, 1]),
    },
    ...fixture.assets.map((asset, index) => ({
      op: 'createStaticMeshInstance',
      handle: INSTANCE_HANDLES[index],
      parent: null,
      instance: {
        asset: asset.asset,
        transform: transform([VARIANT_X[index]!, 0, -5], VARIANT_SCALE[index]!),
        visible: true,
        materialOverrides: [],
        metadata: {
          sourceEntity: 697800 + index,
          sourceSceneNode: null,
          tags: ['depth-splat', fixture.metrics.variants[index]!.id].sort(),
          label: `depth-splat-${fixture.metrics.variants[index]!.id}`,
        },
      },
    })),
  ];
  return decodeRenderFrameDiff({ schemaVersion: 1, ops });
}

function primitiveNode(
  label: string,
  translation: readonly [number, number, number],
  scale: readonly [number, number, number],
  color: readonly [number, number, number, number],
) {
  return {
    geometry: { kind: 'cube' },
    material: { color, wireframe: false },
    transform: { translation, rotation: [0, 0, 0, 1], scale },
    visible: true,
    layer: 'scene',
    metadata: { sourceEntity: null, sourceSceneNode: null, tags: ['comparison'], label },
  };
}

function transform(translation: readonly [number, number, number], scale: number) {
  return { translation, rotation: [0, 0, 0, 1], scale: [scale, scale, scale] };
}

function resourceResolver(locations: Readonly<Record<string, string>>) {
  return async (descriptor: { readonly resource: string }): Promise<ArrayBuffer> => {
    const location = locations[descriptor.resource];
    if (location === undefined) throw new Error(`fixture resource location missing for ${descriptor.resource}`);
    const response = await fetch(location);
    if (!response.ok) throw new Error(`fixture resource ${descriptor.resource} returned ${String(response.status)}`);
    return response.arrayBuffer();
  };
}

function applyComparisonFrame(surface: RendererSurface, frame: RenderFrameDiff): void {
  const receipt = surface.applyFrame(frame);
  if (!receipt.applied) {
    throw new Error(receipt.diagnostics.map((diagnostic) => diagnostic.message).join('; '));
  }
}

async function parseFixture(input: unknown): Promise<DepthSplatFixture> {
  if (typeof input !== 'object' || input === null || (input as { schemaVersion?: unknown }).schemaVersion !== 1) {
    throw new TypeError('depth-splat fixture schemaVersion must be 1');
  }
  return input as DepthSplatFixture;
}

function sampleCanvas(canvas: HTMLCanvasElement): readonly number[] {
  const sample = document.createElement('canvas');
  sample.width = 64;
  sample.height = 36;
  const context = sample.getContext('2d', { willReadFrequently: true });
  if (context === null) throw new Error('2D sample canvas unavailable');
  context.drawImage(canvas, 0, 0, sample.width, sample.height);
  const pixels = context.getImageData(0, 0, sample.width, sample.height).data;
  const values: number[] = [];
  for (let index = 0; index < pixels.length; index += 4) {
    values.push(Math.round(
      pixels[index]! * 0.2126 + pixels[index + 1]! * 0.7152 + pixels[index + 2]! * 0.0722,
    ));
  }
  return values;
}

function requiredElement<T extends HTMLElement>(id: string): T {
  const value = document.getElementById(id);
  if (value === null) throw new Error(`missing #${id}`);
  return value as T;
}

async function animationFrames(count: number): Promise<void> {
  for (let index = 0; index < count; index += 1) {
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  }
}
