import * as THREE from 'three';

import type {
  SpriteAlphaMode,
  SpriteInstanceDescriptor,
  SpriteLightingMode,
} from '@rusty-engine/render-contracts';
import { createSpriteMaterial } from '@rusty-engine/renderer-three';

interface FixtureRecipe {
  readonly id: string;
  readonly kind: 'cutout' | 'soft' | 'flipbook' | 'foliage' | 'character';
  readonly alpha: 'mask' | 'blend';
  readonly description: string;
}

interface ComparisonFixture {
  readonly schemaVersion: 1;
  readonly pixelSize: number;
  readonly flipbookFrames: number;
  readonly fixtures: readonly FixtureRecipe[];
}

interface ComparisonReadout {
  readonly phase: number;
  readonly flipbookFrame: number;
  readonly fixtureCount: number;
  readonly modes: readonly SpriteLightingMode[];
  readonly meshCount: number;
  readonly materialCount: number;
  readonly textureCount: number;
  readonly shaderProgramCount: number;
  readonly drawCalls: number;
  readonly averageRouteRenderMs: number | null;
  readonly disposed: boolean;
}

interface LitSpriteComparisonApi {
  readonly ready: true;
  step(phase: number): ComparisonReadout;
  snapshot(): ComparisonReadout;
  capture(): string;
  sample(): readonly number[];
  measure(): Promise<ComparisonReadout>;
  dispose(): ComparisonReadout;
}

declare global {
  interface Window {
    litSpriteComparison?: LitSpriteComparisonApi;
  }
}

const MODES: readonly SpriteLightingMode[] = [
  'unlit', 'authoredNormal', 'authoredDepth', 'derivedGradient', 'synthetic',
];

void boot();

async function boot(): Promise<void> {
  const fixtureUrl = new URL('../../fixtures/render/lit-sprite-comparison-v1.json', import.meta.url);
  const fixture = parseFixture(await (await fetch(fixtureUrl)).json());
  const canvas = requiredElement<HTMLCanvasElement>('comparison');
  const labels = requiredElement<HTMLDivElement>('labels');
  const readoutNode = requiredElement<HTMLDivElement>('readout');
  const renderer = new THREE.WebGLRenderer({
    canvas,
    antialias: false,
    alpha: false,
    preserveDrawingBuffer: true,
  });
  renderer.setPixelRatio(1);
  renderer.setSize(canvas.width, canvas.height, false);
  renderer.outputColorSpace = THREE.SRGBColorSpace;
  renderer.shadowMap.enabled = true;
  renderer.shadowMap.type = THREE.PCFShadowMap;

  const scene = new THREE.Scene();
  scene.background = new THREE.Color(0x182431);
  scene.fog = new THREE.Fog(0x182431, 15, 26);
  const camera = new THREE.PerspectiveCamera(42, canvas.width / canvas.height, 0.1, 60);
  const ambient = new THREE.AmbientLight(0x7799bb, 0.55);
  const directional = new THREE.DirectionalLight(0xffe1b0, 2.2);
  directional.position.set(6, 7, 8);
  directional.castShadow = true;
  directional.shadow.mapSize.set(1024, 1024);
  directional.shadow.camera.left = -10;
  directional.shadow.camera.right = 10;
  directional.shadow.camera.top = 7;
  directional.shadow.camera.bottom = -7;
  const point = new THREE.PointLight(0x66aaff, 18, 15, 2);
  point.position.set(-5, 1, 5);
  scene.add(ambient, directional, directional.target, point);

  const backdrop = new THREE.Mesh(
    new THREE.PlaneGeometry(19, 12),
    new THREE.MeshStandardMaterial({ color: 0x263546, roughness: 1, metalness: 0 }),
  );
  backdrop.position.set(0, 0, -1.1);
  backdrop.receiveShadow = true;
  scene.add(backdrop);

  const sharedGeometry = new THREE.PlaneGeometry(1.45, 1.45);
  const meshes: THREE.Mesh[] = [];
  const materials = new Set<THREE.Material>();
  const textures = new Set<THREE.Texture>();
  const animatedTextures: THREE.Texture[] = [];
  const baseTextures: THREE.Texture[] = [];

  labels.replaceChildren(...MODES.map((mode, index) => {
    const label = document.createElement('div');
    label.className = 'column';
    label.style.left = `${String(80 + index * 205)}px`;
    label.textContent = mode;
    return label;
  }));

  fixture.fixtures.forEach((recipe, row) => {
    const generated = generateFixtureTextures(recipe, fixture.pixelSize, fixture.flipbookFrames);
    baseTextures.push(generated.color, generated.normal, generated.depth);
    MODES.forEach((mode, column) => {
      const color = cloneAtlasTexture(generated.color, 'srgb', fixture.flipbookFrames);
      const normal = cloneAtlasTexture(generated.normal, 'linear', fixture.flipbookFrames);
      const depth = cloneAtlasTexture(generated.depth, 'linear', fixture.flipbookFrames);
      textures.add(color);
      textures.add(normal);
      textures.add(depth);
      animatedTextures.push(color, normal, depth);
      const alpha: SpriteAlphaMode = recipe.alpha === 'mask'
        ? { kind: 'mask', cutoff: 0.38 }
        : { kind: 'blend' };
      const sprite: SpriteInstanceDescriptor = {
        asset: `sprite/${recipe.id}`,
        frame: 0,
        pivot: [0.5, 0.5],
        size: [1.45, 1.45],
        sizeMode: 'world',
        billboard: row === fixture.fixtures.length - 1 ? 'cylindrical' : 'spherical',
        tint: [1, 1, 1, 1],
        renderOrder: recipe.alpha === 'blend' ? row + 1 : 0,
        depth: 'default',
        shading: 'unlit',
        material: {
          lighting: mode,
          normalTexture: mode === 'authoredNormal' ? `texture/${recipe.id}-normal` : null,
          depthTexture: mode === 'authoredDepth' ? `texture/${recipe.id}-depth` : null,
          normalStrength: mode === 'synthetic' ? 0.72 : 1.25,
          normalBias: mode === 'synthetic' ? 0.18 : 0,
          alpha,
          shadow: mode === 'unlit' ? 'none' : 'castAndReceive',
        },
        visible: true,
        transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
        attachment: { sourceEntity: null, sourceSceneNode: null, attachmentPoint: null },
        metadata: { sourceEntity: null, sourceSceneNode: null, tags: ['comparison'], label: recipe.id },
      };
      const realized = createSpriteMaterial(sprite, {
        color,
        normal: mode === 'authoredNormal' ? normal : null,
        depth: mode === 'authoredDepth' ? depth : null,
      });
      materials.add(realized.material);
      const mesh = new THREE.Mesh(sharedGeometry, realized.material);
      mesh.position.set(-4.15 + column * 2.08, 3.45 - row * 1.72, 0);
      mesh.castShadow = realized.castShadow;
      mesh.receiveShadow = realized.receiveShadow;
      mesh.renderOrder = sprite.renderOrder;
      mesh.userData['billboard'] = sprite.billboard;
      scene.add(mesh);
      meshes.push(mesh);
      if (recipe.kind === 'soft') {
        const overlap = new THREE.Mesh(sharedGeometry, realized.material);
        overlap.position.copy(mesh.position).add(new THREE.Vector3(0.28, -0.18, -0.16));
        overlap.scale.setScalar(0.82);
        overlap.renderOrder = sprite.renderOrder;
        overlap.castShadow = realized.castShadow;
        overlap.receiveShadow = realized.receiveShadow;
        overlap.userData['billboard'] = sprite.billboard;
        scene.add(overlap);
        meshes.push(overlap);
      }
    });
  });

  let phase = 0;
  let flipbookFrame = 0;
  let averageRouteRenderMs: number | null = null;
  let disposed = false;

  function step(nextPhase: number): ComparisonReadout {
    if (disposed) throw new Error('lit sprite comparison is disposed');
    if (!Number.isFinite(nextPhase)) throw new TypeError('phase must be finite');
    phase = ((nextPhase % 1) + 1) % 1;
    const angle = phase * Math.PI * 2;
    camera.position.set(Math.sin(angle) * 2.4, Math.sin(angle * 2) * 0.35, 15.5 + Math.cos(angle) * 0.8);
    camera.lookAt(0, 0, 0);
    camera.updateMatrixWorld(true);
    point.position.set(Math.sin(angle) * 7, Math.cos(angle * 1.5) * 3.5, 4.5 + Math.cos(angle) * 2);
    directional.position.set(6 * Math.cos(angle), 7, 7 * Math.sin(angle));
    flipbookFrame = Math.floor(phase * fixture.flipbookFrames) % fixture.flipbookFrames;
    for (const texture of animatedTextures) texture.offset.x = flipbookFrame / fixture.flipbookFrames;
    for (const mesh of meshes) {
      if (mesh.userData['billboard'] === 'cylindrical') {
        const direction = camera.position.clone().sub(mesh.getWorldPosition(new THREE.Vector3()));
        direction.y = 0;
        if (direction.lengthSq() > 1e-8) mesh.quaternion.setFromUnitVectors(new THREE.Vector3(0, 0, 1), direction.normalize());
      } else {
        mesh.quaternion.copy(camera.quaternion);
      }
    }
    renderer.render(scene, camera);
    const value = snapshot();
    readoutNode.textContent = JSON.stringify(value, null, 2);
    return value;
  }

  function snapshot(): ComparisonReadout {
    return Object.freeze({
      phase,
      flipbookFrame,
      fixtureCount: fixture.fixtures.length,
      modes: MODES,
      meshCount: disposed ? 0 : meshes.length,
      materialCount: disposed ? 0 : materials.size,
      textureCount: disposed ? 0 : textures.size,
      shaderProgramCount: disposed ? 0 : (renderer.info.programs?.length ?? 0),
      drawCalls: disposed ? 0 : renderer.info.render.calls,
      averageRouteRenderMs,
      disposed,
    });
  }

  async function measure(): Promise<ComparisonReadout> {
    await animationFrames(2);
    const samples = 24;
    const started = performance.now();
    for (let index = 0; index < samples; index += 1) step(index / samples);
    averageRouteRenderMs = (performance.now() - started) / samples;
    return step(0);
  }

  function dispose(): ComparisonReadout {
    if (!disposed) {
      for (const mesh of meshes) scene.remove(mesh);
      for (const material of materials) material.dispose();
      for (const texture of textures) texture.dispose();
      for (const texture of baseTextures) texture.dispose();
      sharedGeometry.dispose();
      backdrop.geometry.dispose();
      (backdrop.material as THREE.Material).dispose();
      renderer.dispose();
      disposed = true;
    }
    const value = snapshot();
    readoutNode.textContent = JSON.stringify(value, null, 2);
    return value;
  }

  const api: LitSpriteComparisonApi = {
    ready: true,
    step,
    snapshot,
    capture: () => canvas.toDataURL('image/png'),
    sample: () => sampleCanvas(canvas),
    measure,
    dispose,
  };
  window.litSpriteComparison = api;
  step(0);
}

function sampleCanvas(canvas: HTMLCanvasElement): readonly number[] {
  const sample = document.createElement('canvas');
  sample.width = 56;
  sample.height = 35;
  const context = sample.getContext('2d', { willReadFrequently: true });
  if (context === null) throw new Error('2D sample canvas unavailable');
  context.drawImage(canvas, 0, 0, sample.width, sample.height);
  const pixels = context.getImageData(0, 0, sample.width, sample.height).data;
  const values: number[] = [];
  for (let offset = 0; offset < pixels.length; offset += 4) {
    values.push(Math.round(
      (pixels[offset] as number) * 0.2126
      + (pixels[offset + 1] as number) * 0.7152
      + (pixels[offset + 2] as number) * 0.0722,
    ));
  }
  return values;
}

function parseFixture(input: unknown): ComparisonFixture {
  if (typeof input !== 'object' || input === null) throw new TypeError('fixture must be an object');
  const value = input as Record<string, unknown>;
  if (value['schemaVersion'] !== 1) throw new TypeError('fixture schemaVersion must be 1');
  const pixelSize = value['pixelSize'];
  const flipbookFrames = value['flipbookFrames'];
  const rawFixtures = value['fixtures'];
  if (!Number.isSafeInteger(pixelSize) || (pixelSize as number) < 8 || (pixelSize as number) > 64) {
    throw new TypeError('fixture pixelSize must be an integer in 8..=64');
  }
  if (!Number.isSafeInteger(flipbookFrames) || (flipbookFrames as number) < 1 || (flipbookFrames as number) > 8) {
    throw new TypeError('fixture flipbookFrames must be an integer in 1..=8');
  }
  if (!Array.isArray(rawFixtures) || rawFixtures.length !== 5) throw new TypeError('fixture must contain five recipes');
  const fixtures = rawFixtures.map((item): FixtureRecipe => {
    if (typeof item !== 'object' || item === null) throw new TypeError('fixture recipe must be an object');
    const recipe = item as Record<string, unknown>;
    const kind = recipe['kind'];
    const alpha = recipe['alpha'];
    if (typeof recipe['id'] !== 'string' || typeof recipe['description'] !== 'string') throw new TypeError('fixture text is invalid');
    if (!['cutout', 'soft', 'flipbook', 'foliage', 'character'].includes(String(kind))) throw new TypeError('fixture kind is invalid');
    if (alpha !== 'mask' && alpha !== 'blend') throw new TypeError('fixture alpha is invalid');
    return { id: recipe['id'], kind: kind as FixtureRecipe['kind'], alpha, description: recipe['description'] };
  });
  return { schemaVersion: 1, pixelSize: pixelSize as number, flipbookFrames: flipbookFrames as number, fixtures };
}

function generateFixtureTextures(
  recipe: FixtureRecipe,
  size: number,
  frames: number,
): { readonly color: THREE.CanvasTexture; readonly normal: THREE.CanvasTexture; readonly depth: THREE.CanvasTexture } {
  const width = size * frames;
  const colorCanvas = document.createElement('canvas');
  const normalCanvas = document.createElement('canvas');
  const depthCanvas = document.createElement('canvas');
  for (const canvas of [colorCanvas, normalCanvas, depthCanvas]) {
    canvas.width = width;
    canvas.height = size;
  }
  const colorPixels = new Uint8ClampedArray(width * size * 4);
  const depthValues = new Float32Array(width * size);
  for (let frame = 0; frame < frames; frame += 1) {
    for (let y = 0; y < size; y += 1) {
      for (let x = 0; x < size; x += 1) {
        const nx = (x + 0.5) / size * 2 - 1;
        const ny = 1 - (y + 0.5) / size * 2;
        const sample = fixtureSample(recipe.kind, nx, ny, frame, frames);
        const pixel = (y * width + frame * size + x);
        colorPixels.set([sample.color[0], sample.color[1], sample.color[2], sample.alpha], pixel * 4);
        depthValues[pixel] = sample.depth * (sample.alpha / 255);
      }
    }
  }
  const normalPixels = new Uint8ClampedArray(colorPixels.length);
  const depthPixels = new Uint8ClampedArray(colorPixels.length);
  for (let frame = 0; frame < frames; frame += 1) {
    for (let y = 0; y < size; y += 1) {
      for (let x = 0; x < size; x += 1) {
        const pixel = y * width + frame * size + x;
        const left = depthValues[y * width + frame * size + Math.max(0, x - 1)] as number;
        const right = depthValues[y * width + frame * size + Math.min(size - 1, x + 1)] as number;
        const down = depthValues[Math.min(size - 1, y + 1) * width + frame * size + x] as number;
        const up = depthValues[Math.max(0, y - 1) * width + frame * size + x] as number;
        const normal = new THREE.Vector3((left - right) * 2, (down - up) * 2, 1).normalize();
        const alpha = colorPixels[pixel * 4 + 3] as number;
        normalPixels.set([
          Math.round((normal.x * 0.5 + 0.5) * 255),
          Math.round((normal.y * 0.5 + 0.5) * 255),
          Math.round((normal.z * 0.5 + 0.5) * 255),
          alpha,
        ], pixel * 4);
        const depth = Math.round((depthValues[pixel] as number) * 255);
        depthPixels.set([depth, depth, depth, alpha], pixel * 4);
      }
    }
  }
  putPixels(colorCanvas, colorPixels);
  putPixels(normalCanvas, normalPixels);
  putPixels(depthCanvas, depthPixels);
  const color = new THREE.CanvasTexture(colorCanvas);
  const normal = new THREE.CanvasTexture(normalCanvas);
  const depth = new THREE.CanvasTexture(depthCanvas);
  configureTexture(color, 'srgb', frames);
  configureTexture(normal, 'linear', frames);
  configureTexture(depth, 'linear', frames);
  return { color, normal, depth };
}

function fixtureSample(
  kind: FixtureRecipe['kind'],
  x: number,
  y: number,
  frame: number,
  frames: number,
): { readonly color: readonly [number, number, number]; readonly alpha: number; readonly depth: number } {
  let distance = 2;
  let color: readonly [number, number, number] = [220, 110, 55];
  if (kind === 'cutout') {
    distance = 0.78 - Math.abs(x) - Math.abs(y);
    if (x > 0.05 && y > -0.2) distance -= 0.18;
  } else if (kind === 'soft') {
    distance = 0.72 + 0.08 * Math.sin(x * 8) * Math.cos(y * 7) - Math.hypot(x * 0.9, y * 1.1);
    color = [90, 175, 255];
  } else if (kind === 'flipbook') {
    const pulse = 0.5 + 0.16 * Math.sin(frame / frames * Math.PI * 2);
    distance = pulse - Math.hypot(x - Math.sin(frame) * 0.08, y - Math.cos(frame) * 0.06);
    color = [255, 155 + frame * 18, 65];
  } else if (kind === 'foliage') {
    const lobes = [[-0.32, 0.2], [0.3, 0.28], [0, 0.58], [0, -0.05]] as const;
    distance = Math.max(...lobes.map(([lx, ly]) => 0.38 - Math.hypot(x - lx, y - ly)));
    distance = Math.max(distance, Math.min(0.11 - Math.abs(x), 0.75 - Math.abs(y + 0.28)));
    color = [75, 190, 85];
  } else {
    const head = 0.3 - Math.hypot(x, y - 0.48);
    const body = Math.min(0.34 - Math.abs(x), 0.48 - Math.abs(y + 0.05));
    const legs = Math.max(
      Math.min(0.13 - Math.abs(x - 0.17), 0.27 - Math.abs(y + 0.63)),
      Math.min(0.13 - Math.abs(x + 0.17), 0.27 - Math.abs(y + 0.63)),
    );
    distance = Math.max(head, body, legs);
    color = y > 0.25 ? [238, 185, 135] : x > 0 ? [175, 80, 95] : [70, 120, 205];
  }
  const alpha = kind === 'soft'
    ? Math.round(THREE.MathUtils.smoothstep(distance, -0.16, 0.12) * 255)
    : distance >= 0 ? 255 : 0;
  const depth = THREE.MathUtils.clamp(distance * 1.6 + 0.45, 0, 1);
  return { color, alpha, depth };
}

function configureTexture(texture: THREE.Texture, colorSpace: 'srgb' | 'linear', frames: number): void {
  texture.colorSpace = colorSpace === 'srgb' ? THREE.SRGBColorSpace : THREE.NoColorSpace;
  texture.magFilter = THREE.NearestFilter;
  texture.minFilter = THREE.NearestFilter;
  texture.generateMipmaps = false;
  texture.wrapS = THREE.RepeatWrapping;
  texture.wrapT = THREE.ClampToEdgeWrapping;
  texture.repeat.set(1 / frames, 1);
  texture.needsUpdate = true;
}

function cloneAtlasTexture(
  source: THREE.Texture,
  colorSpace: 'srgb' | 'linear',
  frames: number,
): THREE.Texture {
  const texture = source.clone();
  configureTexture(texture, colorSpace, frames);
  return texture;
}

function putPixels(canvas: HTMLCanvasElement, pixels: Uint8ClampedArray): void {
  const context = canvas.getContext('2d');
  if (context === null) throw new Error('2D canvas unavailable');
  const owned = new Uint8ClampedArray(pixels.length);
  owned.set(pixels);
  context.putImageData(new ImageData(owned, canvas.width, canvas.height), 0, 0);
}

function requiredElement<T extends HTMLElement>(id: string): T {
  const element = document.getElementById(id);
  if (element === null) throw new Error(`missing #${id}`);
  return element as T;
}

async function animationFrames(count: number): Promise<void> {
  for (let index = 0; index < count; index += 1) {
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
  }
}
