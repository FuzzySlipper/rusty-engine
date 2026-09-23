import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { unzlibSync } from 'fflate';
import * as THREE from 'three';
import { renderHandle, type RenderFrameDiff } from '@rusty-engine/render-contracts';
import {
  MapAnimatedMeshAssetSource,
  loadAnimatedMeshGlbResource,
} from './animated-mesh.js';
import { RendererOutputExecutor } from './render-output.js';
import { ThreeRenderer, type ThreeRendererIsolatedCaptureScene } from './three-renderer.js';

void test('capture restores WebGL state and returns top-down straight-alpha PNG pixels', async () => {
  const world = new THREE.Scene();
  const viewmodel = new THREE.Scene();
  let target: THREE.WebGLRenderTarget | null = null;
  const viewport = new THREE.Vector4(3, 4, 20, 30);
  const scissor = new THREE.Vector4(5, 6, 7, 8);
  let scissorTest = true;
  let clearColor = new THREE.Color(0.1, 0.2, 0.3);
  let clearAlpha = 0.75;
  const requestedClearColors: THREE.Color[] = [];
  const logicalViewportCallsOnTarget: number[][] = [];
  const events: string[] = [];
  const webgl = {
    capabilities: { maxTextureSize: 64, maxSamples: 0 },
    extensions: { has: () => true },
    toneMapping: THREE.ReinhardToneMapping,
    toneMappingExposure: 1.5,
    outputColorSpace: THREE.LinearSRGBColorSpace,
    localClippingEnabled: true,
    autoClear: true,
    autoClearColor: true,
    autoClearDepth: true,
    autoClearStencil: false,
    xr: { enabled: true },
    getRenderTarget: () => target,
    getActiveCubeFace: () => 0,
    getActiveMipmapLevel: () => 0,
    getViewport: (out: THREE.Vector4) => out.copy(viewport),
    getScissor: (out: THREE.Vector4) => out.copy(scissor),
    getScissorTest: () => scissorTest,
    getClearColor: (out: THREE.Color) => out.copy(clearColor),
    getClearAlpha: () => clearAlpha,
    setRenderTarget: (next: THREE.WebGLRenderTarget | null) => { target = next; },
    setViewport: (next: THREE.Vector4 | number, y?: number, width?: number, height?: number) => {
      if (target !== null && typeof next === 'number') {
        logicalViewportCallsOnTarget.push([next, y ?? 0, width ?? 0, height ?? 0]);
      }
      if (typeof next === 'number') viewport.set(next, y ?? 0, width ?? 0, height ?? 0);
      else viewport.copy(next);
    },
    setScissor: (next: THREE.Vector4 | number, y?: number, width?: number, height?: number) => {
      if (typeof next === 'number') scissor.set(next, y ?? 0, width ?? 0, height ?? 0);
      else scissor.copy(next);
    },
    setScissorTest: (next: boolean) => { scissorTest = next; },
    setClearColor: (color: THREE.Color, alpha: number) => {
      requestedClearColors.push(color.clone());
      clearColor = color.clone();
      clearAlpha = alpha;
    },
    clear: () => events.push('clear'),
    clearDepth: () => events.push('clearDepth'),
    render: (scene: THREE.Scene) => events.push(`render:${scene.name}`),
    readRenderTargetPixels: (
      _target: THREE.WebGLRenderTarget,
      _x: number,
      _y: number,
      _width: number,
      _height: number,
      pixels: Uint8Array,
    ) => {
      // The conversion pass returns bottom-up straight-alpha sRGB bytes.
      pixels.set([0, 255, 0, 128, 255, 255, 255, 255]);
      events.push('readback');
    },
  } as unknown as THREE.WebGLRenderer;
  const isolated = {
    scene: world,
    viewmodelScene: viewmodel,
    objectFor: () => undefined,
    sceneFor: () => undefined,
    setViewportSize: () => undefined,
    prepareSpritesForCamera: () => undefined,
    prepareStaticInstanceBatches: () => undefined,
    dispose: () => events.push('dispose'),
  } satisfies Partial<ThreeRendererIsolatedCaptureScene>;
  const retained = {
    createIsolatedCaptureScene: () => isolated,
  } as unknown as ThreeRenderer;
  const executor = new RendererOutputExecutor(webgl, retained);

  const png = await executor.capture(
    { schemaVersion: 1, ops: [] } satisfies RenderFrameDiff,
    new THREE.PerspectiveCamera(),
    {
      width: 1,
      height: 2,
      background: [0.5, 0.25, 0.125, 0.5],
      toneMapping: THREE.NoToneMapping,
      exposure: 1,
      samples: 0,
    },
  );

  assert.deepEqual([...png.subarray(0, 8)], [137, 80, 78, 71, 13, 10, 26, 10]);
  const decoded = readRgbaPng(png);
  assert.deepEqual([decoded.width, decoded.height], [1, 2]);
  assert.equal(decoded.srgbIntent, 0);
  assert.deepEqual([...decoded.pixels], [255, 255, 255, 255, 0, 255, 0, 128]);
  assert.deepEqual([...viewport.toArray()], [3, 4, 20, 30]);
  assert.deepEqual(logicalViewportCallsOnTarget, [], 'offscreen target dimensions must not be pixel-ratio scaled');
  assert.deepEqual([...scissor.toArray()], [5, 6, 7, 8]);
  assert.equal(scissorTest, true);
  assert.deepEqual(requestedClearColors[0]?.toArray(), new THREE.Color(0.5, 0.25, 0.125).toArray());
  assert.deepEqual(clearColor.toArray(), new THREE.Color(0.1, 0.2, 0.3).toArray());
  assert.equal(clearAlpha, 0.75);
  assert.equal(webgl.toneMapping, THREE.ReinhardToneMapping);
  assert.equal(webgl.toneMappingExposure, 1.5);
  assert.equal(webgl.outputColorSpace, THREE.LinearSRGBColorSpace);
  assert.equal(webgl.xr.enabled, true);
  assert.deepEqual(events, ['clear', 'render:', 'clearDepth', 'render:', 'render:render-output-conversion', 'readback', 'dispose']);
  executor.dispose();
});

void test('capture rejects an unsupported exact RGBA16F multisample count', async () => {
  const webgl = {
    capabilities: { maxTextureSize: 64, maxSamples: 8, isWebGL2: true },
    getContext: () => ({
      RENDERBUFFER: 1,
      RGBA16F: 2,
      SAMPLES: 3,
      getInternalformatParameter: () => new Int32Array([8, 4, 2]),
    }),
  } as unknown as THREE.WebGLRenderer;
  const executor = new RendererOutputExecutor(
    webgl,
    {} as ThreeRenderer,
  );
  await assert.rejects(
    executor.capture(
      { schemaVersion: 1, ops: [] } satisfies RenderFrameDiff,
      new THREE.PerspectiveCamera(),
      {
        width: 1,
        height: 1,
        background: [0, 0, 0, 0],
        samples: 3,
      },
    ),
    /requested 3 samples, but the RGBA16F render target supports only 8, 4, 2/,
  );
  executor.dispose();
});

void test('GLB export includes embedded clips when the selected animated handle is the subtree root', async () => {
  const testGlobal = globalThis as unknown as {
    self: unknown;
    createImageBitmap?: (blob: Blob, options?: ImageBitmapOptions) => Promise<ImageBitmap>;
    FileReader?: unknown;
  };
  const priorSelf = testGlobal.self;
  const priorCreateImageBitmap = testGlobal.createImageBitmap;
  const priorFileReader = testGlobal.FileReader;
  testGlobal.self = globalThis;
  testGlobal.createImageBitmap = async () => ({ width: 1, height: 1, close() {} }) as ImageBitmap;
  testGlobal.FileReader = class {
    result: ArrayBuffer | null = null;
    onloadend: (() => void) | null = null;

    readAsArrayBuffer(blob: Blob): void {
      void blob.arrayBuffer().then((result) => {
        this.result = result;
        this.onloadend?.();
      });
    }
  };
  try {
    const sourceBytes = readFileSync(
      resolve(import.meta.dirname, '../../../../fixtures/render/assets/kenney-retro-character/character-medium.glb'),
    );
    const sourceData = sourceBytes.buffer.slice(
      sourceBytes.byteOffset,
      sourceBytes.byteOffset + sourceBytes.byteLength,
    );
    const embeddedMaterialSlots = [{ slot: 0, sourceMaterialSlot: 0 }];
    const resource = await loadAnimatedMeshGlbResource(
      'mesh-animation/kenney-retro-character-medium',
      sourceData,
      undefined,
      embeddedMaterialSlots,
    );
    // GLTFExporter requires a browser canvas to encode embedded images. This
    // test verifies skin and animation track export; the live packaged fixture
    // exercises the texture-backed path in the browser.
    resource.scene.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) return;
      const materials = Array.isArray(object.material) ? object.material : [object.material];
      for (const material of materials) {
        if ('map' in material) (material as THREE.MeshStandardMaterial).map = null;
      }
    });

    const asset = {
      asset: resource.asset,
      runtimeFormat: 'glb',
      contentHash: null,
      clips: resource.clips.map((clip) => ({
        id: clip.name,
        name: clip.name,
        durationSeconds: clip.duration,
      })),
      defaultClip: resource.clips[0]?.name ?? null,
      materialSlots: [],
      embeddedMaterialSlots,
      bounds: { min: [-0.5, 0, -0.5], max: [0.5, 1.8, 0.5] },
    } as const;
    const handle = renderHandle(64321);
    const instance = {
      asset: asset.asset,
      transform: {
        translation: [0, 0, 0] as const,
        rotation: [0, 0, 0, 1] as const,
        scale: [1, 1, 1] as const,
      },
      visible: true,
      materialOverrides: [],
      playback: null,
      metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'animated-root' },
    };
    const frame = {
      schemaVersion: 1,
      ops: [
        { op: 'defineAnimatedMesh' as const, asset },
        { op: 'createAnimatedMeshInstance' as const, handle, parent: null, instance },
      ],
    } satisfies RenderFrameDiff;
    const retained = new ThreeRenderer({
      animatedMeshSource: new MapAnimatedMeshAssetSource([resource]),
    });
    const executor = new RendererOutputExecutor({} as THREE.WebGLRenderer, retained);
    try {
      const isolated = retained.createIsolatedCaptureScene(frame);
      try {
        assert.deepEqual(
          isolated.animationClipSourcesInSubtree?.(handle).map((source) => source.clips.map((clip) => clip.name)),
          [resource.clips.map((clip) => clip.name)],
        );
      } finally {
        isolated.dispose();
      }
      const glb = await executor.exportGlb(frame, handle, true);
      const view = new DataView(glb.buffer, glb.byteOffset, glb.byteLength);
      const jsonLength = view.getUint32(12, true);
      const json = JSON.parse(new TextDecoder().decode(glb.subarray(20, 20 + jsonLength)).trim()) as {
        readonly animations?: readonly { readonly name: string; readonly channels: readonly unknown[] }[];
        readonly skins?: readonly unknown[];
      };
      assert.equal(json.skins?.length, 1);
      assert.deepEqual(json.animations?.map((clip) => clip.name).sort(), ['idle', 'jump', 'run']);
      assert.ok(json.animations?.every((clip) => clip.channels.length > 0));
    } finally {
      executor.dispose();
      retained.dispose();
    }
  } finally {
    testGlobal.self = priorSelf;
    if (priorCreateImageBitmap === undefined) delete testGlobal.createImageBitmap;
    else testGlobal.createImageBitmap = priorCreateImageBitmap;
    if (priorFileReader === undefined) delete testGlobal.FileReader;
    else testGlobal.FileReader = priorFileReader;
  }
});

function readRgbaPng(bytes: Uint8Array): {
  readonly width: number;
  readonly height: number;
  readonly srgbIntent: number | null;
  readonly pixels: Uint8Array;
} {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  let offset = 8;
  let width = 0;
  let height = 0;
  let srgbIntent: number | null = null;
  const data: Uint8Array[] = [];
  while (offset < bytes.byteLength) {
    const length = view.getUint32(offset, false);
    const typeBytes = bytes.subarray(offset + 4, offset + 8);
    const type = String.fromCharCode(...typeBytes);
    const payload = bytes.subarray(offset + 8, offset + 8 + length);
    assert.equal(
      view.getUint32(offset + 8 + length, false),
      pngCrc32(typeBytes, payload),
      `${type} chunk CRC is valid`,
    );
    if (type === 'IHDR') {
      width = view.getUint32(offset + 8, false);
      height = view.getUint32(offset + 12, false);
    } else if (type === 'sRGB') srgbIntent = payload[0] ?? null;
    else if (type === 'IDAT') data.push(payload);
    offset += length + 12;
  }
  const compressed = new Uint8Array(data.reduce((sum, part) => sum + part.byteLength, 0));
  let cursor = 0;
  for (const part of data) {
    compressed.set(part, cursor);
    cursor += part.byteLength;
  }
  const filtered = unzlibSync(compressed);
  const pixels = new Uint8Array(width * height * 4);
  for (let row = 0; row < height; row++) {
    const sourceOffset = row * (width * 4 + 1);
    assert.equal(filtered[sourceOffset], 0, 'encoder writes unfiltered scanlines');
    pixels.set(filtered.subarray(sourceOffset + 1, sourceOffset + 1 + width * 4), row * width * 4);
  }
  return { width, height, srgbIntent, pixels };
}

function pngCrc32(type: Uint8Array, data: Uint8Array): number {
  let crc = 0xffff_ffff;
  for (const byte of [...type, ...data]) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      crc = (crc & 1) === 1 ? 0xedb8_8320 ^ (crc >>> 1) : crc >>> 1;
    }
  }
  return (crc ^ 0xffff_ffff) >>> 0;
}
