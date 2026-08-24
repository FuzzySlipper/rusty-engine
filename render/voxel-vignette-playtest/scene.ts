import type {
  RustyApplicationContent,
  RustyApplicationFrame,
  RustyApplicationResource,
} from '@rusty-engine/application-host';

type Vec3 = readonly [number, number, number];

const RUN_ID = 'voxel-shading-comparison-6923-001';
const VOXEL_SCALE = 1 / 128;

export interface VignetteLighting {
  readonly ambientIntensity: number;
  readonly directionalIntensity: number;
  readonly pointEnabled: boolean;
  readonly pointIntensity: number;
  readonly pointRange: number;
  readonly pointPosition: Vec3;
}

export const INITIAL_VIGNETTE_LIGHTING: VignetteLighting = Object.freeze({
  ambientIntensity: 0.35,
  directionalIntensity: 1.5,
  pointEnabled: false,
  pointIntensity: 60,
  pointRange: 10,
  pointPosition: [0, 1.85, 13] as const,
});

export type VignetteVariantId =
  | 'original-pbr'
  | 'producer-normals'
  | 'producer-normals-matte-pbr'
  | 'palette-unlit'
  | 'occupancy-axis-control'
  | 'occupancy-adjacency-normals';

export interface VignetteVariant {
  readonly id: VignetteVariantId;
  readonly label: string;
  readonly normalTreatment: string;
  readonly materialModel: string;
  readonly lighting: string;
  readonly assets: Readonly<Record<VignetteAssetId, VignetteAssetFile>>;
}

type VignetteAssetId = 'shrine-nano' | 'gpt-tree' | 'door';

interface VignetteAssetFile {
  readonly file: string;
  readonly contentHash: string;
  readonly byteLength: number;
}

interface VignetteAssetLayout {
  readonly id: VignetteAssetId;
  readonly translation: Vec3;
  readonly rotation: readonly [number, number, number, number];
  readonly scale: Vec3;
  readonly label: string;
}

const ASSET_LAYOUT: readonly VignetteAssetLayout[] = [
  {
    id: 'shrine-nano', translation: [0, 64 / 128, 0], rotation: [0, 0, 0, 1],
    scale: [VOXEL_SCALE, VOXEL_SCALE, VOXEL_SCALE], label: 'shrine nano',
  },
  {
    id: 'gpt-tree', translation: [-4, 51 / 128, 5], rotation: [0, 0, 0, 1],
    scale: [VOXEL_SCALE, VOXEL_SCALE, VOXEL_SCALE], label: 'tree',
  },
  {
    id: 'door', translation: [4, 64 / 128, 6], rotation: [0, 0, 0, 1],
    scale: [VOXEL_SCALE, VOXEL_SCALE, VOXEL_SCALE], label: 'door',
  },
];

export const VIGNETTE_VARIANTS: readonly VignetteVariant[] = [
  {
    id: 'original-pbr', label: '1 · Original PBR / no normals',
    normalTreatment: 'source GLB has no NORMAL attribute',
    materialModel: 'original palette PBR values',
    lighting: 'same ambient + directional lights',
    assets: {
      'shrine-nano': { file: 'shrine-nano-128.glb', contentHash: 'sha256:5e5db140a13772263c9336923f0dac43dedc896e5eac9cec077f78822fb8b02e', byteLength: 14_257_592 },
      'gpt-tree': { file: 'gpt-tree-128.glb', contentHash: 'sha256:74f2e423679c9ef72551d6861ae9927fdd11671e45ece0f5173c3ffb7aa329c0', byteLength: 13_371_020 },
      door: { file: 'door-t3-128.glb', contentHash: 'sha256:37068569f77e28b1109426c9663d880472a7a8e62444d1ff83e154739e7f1fbd', byteLength: 5_853_560 },
    },
  },
  {
    id: 'producer-normals', label: '2 · Producer normals / current PBR',
    normalTreatment: 'pinned Vengi producer NORMAL output',
    materialModel: 'current palette PBR values',
    lighting: 'same ambient + directional lights',
    assets: {
      'shrine-nano': { file: 'shrine-nano-normals.glb', contentHash: 'sha256:151de53cbdff767844fff51be6ed14e1c4a793b0492ec2614fbd07a61281870a', byteLength: 20_772_492 },
      'gpt-tree': { file: 'gpt-tree-normals.glb', contentHash: 'sha256:c997ae83d56651136d64867d8c88ea87ca648512de93277fb293cd0e73a694b9', byteLength: 19_476_664 },
      door: { file: 'door-normals.glb', contentHash: 'sha256:33758a14facfa7a3086ceee164d72a105a381eee27c79e4b6ec50719ba41fb3c', byteLength: 8_489_932 },
    },
  },
  {
    id: 'producer-normals-matte-pbr', label: '3 · Producer normals / matte PBR',
    normalTreatment: 'same pinned producer NORMAL output',
    materialModel: 'explicit producer matte PBR: roughness 1, metalness 0',
    lighting: 'same ambient + directional lights',
    assets: {
      'shrine-nano': { file: 'shrine-nano-normals-matte-pbr.glb', contentHash: 'sha256:97dff90a7f6d09f8846b4d96ddb00b831067cbd68bcefda774e7a50fb31120fc', byteLength: 20_769_652 },
      'gpt-tree': { file: 'gpt-tree-normals-matte-pbr.glb', contentHash: 'sha256:6c65bb93bf267bfe5a5d90e62e5898ad0b78a71ae120ec5c0b493066671186d7', byteLength: 19_473_828 },
      door: { file: 'door-normals-matte-pbr.glb', contentHash: 'sha256:4e6cf0527b0fbf662ef7869cc5f8db46527e13aa8af2daaa6c78bac1f74ea34f', byteLength: 8_487_096 },
    },
  },
  {
    id: 'palette-unlit', label: '4 · Accepted KHR_materials_unlit',
    normalTreatment: 'source GLB has no NORMAL attribute',
    materialModel: 'accepted KHR_materials_unlit derivative',
    lighting: 'lights remain constant but unlit materials do not respond to them',
    assets: {
      'shrine-nano': { file: 'shrine-nano-128-palette-unlit.glb', contentHash: 'sha256:9f97e2700962191ea015f87bb1f63670919581e3cf9f32807a72ca585ad563f5', byteLength: 14_260_896 },
      'gpt-tree': { file: 'gpt-tree-128-palette-unlit.glb', contentHash: 'sha256:cc5b1f15c31dbf114717c5f078637ef84752224f9f32734da7abc0794c595460', byteLength: 13_374_324 },
      door: { file: 'door-t3-128-palette-unlit.glb', contentHash: 'sha256:f3c8c66bc7b97bfbccdf19452b8042589b13209614c3e8a53098a4f5c32e03b5', byteLength: 5_856_864 },
    },
  },
  {
    id: 'occupancy-axis-control', label: '5 · Occupancy axis-normal control',
    normalTreatment: 'direct VOX occupancy compiler: axis face normals',
    materialModel: 'one explicit matte PBR; VOX palette is COLOR_0, not a GLB texture',
    lighting: 'same ambient + directional lights',
    assets: {
      'shrine-nano': { file: 'shrine-axis.glb', contentHash: 'sha256:848175abac411a5ebe57c85c8e9fdf1ef99b0107d83e02e2185f09c0720a08a1', byteLength: 13_797_108 },
      'gpt-tree': { file: 'tree-axis.glb', contentHash: 'sha256:53a1a3c771e034cde9a01d5f1d8b94882f62f746f736e6c0c9ffd3d8a1f60ccb', byteLength: 12_940_712 },
      door: { file: 'door-axis.glb', contentHash: 'sha256:92d469e377b5fef649b99a57aa424dc1f2dd07c59d702be8cef49cb4691c4fbd', byteLength: 5_566_652 },
    },
  },
  {
    id: 'occupancy-adjacency-normals', label: '6 · Occupancy adjacency normals',
    normalTreatment: 'direct VOX occupancy compiler: exposed-face adjacency donor normals',
    materialModel: 'one explicit matte PBR; VOX palette is COLOR_0, not a GLB texture',
    lighting: 'same ambient + directional lights',
    assets: {
      'shrine-nano': { file: 'shrine-adjacency.glb', contentHash: 'sha256:f67461784ee76b20a613450dfca3821573556d6cb54f986658436894a415cf04', byteLength: 14_558_580 },
      'gpt-tree': { file: 'tree-adjacency.glb', contentHash: 'sha256:c217f0b6731acb900adc6dca24d077e4a4cdc0148f9bf0b2c5d34d2ac7f0faa0', byteLength: 13_513_144 },
      door: { file: 'door-adjacency.glb', contentHash: 'sha256:c99294af25134e42337cc9d0f2faadb88e9342c3628bf58ab1b85576de7f509a', byteLength: 5_732_852 },
    },
  },
];

export async function loadVignetteContent(variantId: VignetteVariantId): Promise<RustyApplicationContent> {
  const variant = VIGNETTE_VARIANTS.find((candidate) => candidate.id === variantId);
  if (variant === undefined) throw new Error(`unknown voxel shading variant: ${variantId}`);
  const resources = await Promise.all(ASSET_LAYOUT.map((layout) => loadAsset(variant, layout)));
  return { frame: vignetteFrame(variant), resources };
}

async function loadAsset(variant: VignetteVariant, layout: VignetteAssetLayout): Promise<RustyApplicationResource> {
  const asset = variant.assets[layout.id];
  const response = await fetch(new URL(`./assets/${variant.id}/${asset.file}`, document.baseURI));
  if (!response.ok) throw new Error(`${variant.label}: ${asset.file} returned ${String(response.status)}; run scripts/stage-voxel-vignette-comparison-assets.sh`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength !== asset.byteLength) {
    throw new Error(`${variant.label}: ${asset.file} size mismatch; expected ${String(asset.byteLength)}, received ${String(bytes.byteLength)}`);
  }
  return {
    identity: `mesh-resource/${asset.contentHash.slice('sha256:'.length)}`,
    contentHash: asset.contentHash,
    mediaType: 'application/octet-stream',
    bytes,
  };
}

function vignetteFrame(variant: VignetteVariant): RustyApplicationFrame {
  return {
    schemaVersion: 1,
    ops: [
      { op: 'createLight', handle: 1, parent: null, light: ambientLight(INITIAL_VIGNETTE_LIGHTING) },
      { op: 'createLight', handle: 2, parent: null, light: directionalLight(INITIAL_VIGNETTE_LIGHTING) },
      { op: 'createLight', handle: 3, parent: null, light: pointLight(INITIAL_VIGNETTE_LIGHTING) },
      ...ASSET_LAYOUT.flatMap((layout, index) => {
        const asset = variant.assets[layout.id];
        return [
          { op: 'defineAnimatedMesh' as const, asset: { asset: `visual-gate/${RUN_ID}/${variant.id}/${layout.id}`, runtimeFormat: 'glb' as const, contentHash: asset.contentHash, clips: [], defaultClip: null, materialSlots: [], bounds: { min: [-1, -1, -1], max: [1, 1, 1] } } },
          { op: 'createAnimatedMeshInstance' as const, handle: 100 + index, parent: null, instance: { asset: `visual-gate/${RUN_ID}/${variant.id}/${layout.id}`, transform: { translation: layout.translation, rotation: layout.rotation, scale: layout.scale }, visible: true, materialOverrides: [], playback: null, metadata: { sourceEntity: null, sourceSceneNode: null, tags: ['voxel-vignette', 'compiled-glb', 'temporary-comparison-route', variant.id], label: `${layout.label}: ${variant.label}` } } },
        ];
      }),
    ],
  };
}

export function vignetteLightingFrame(lighting: VignetteLighting): RustyApplicationFrame {
  return {
    schemaVersion: 1,
    ops: [
      { op: 'updateLight', handle: 1, light: ambientLight(lighting) },
      { op: 'updateLight', handle: 2, light: directionalLight(lighting) },
      { op: 'updateLight', handle: 3, light: pointLight(lighting) },
    ],
  };
}

function ambientLight(lighting: VignetteLighting) {
  return {
    kind: 'ambient' as const,
    color: [1, 1, 1] as const,
    intensity: lighting.ambientIntensity,
    enabled: lighting.ambientIntensity > 0,
    shadowIntent: 'disabled' as const,
  };
}

function directionalLight(lighting: VignetteLighting) {
  return {
    kind: 'directional' as const,
    color: [1, 0.95, 0.86] as const,
    intensity: lighting.directionalIntensity,
    enabled: lighting.directionalIntensity > 0,
    direction: [-1, -2, -1] as const,
    shadowIntent: 'requested' as const,
  };
}

function pointLight(lighting: VignetteLighting) {
  return {
    kind: 'point' as const,
    color: [1, 0.92, 0.78] as const,
    intensity: lighting.pointIntensity,
    enabled: lighting.pointEnabled && lighting.pointIntensity > 0,
    position: lighting.pointPosition,
    range: lighting.pointRange,
    decay: 2,
    shadowIntent: 'disabled' as const,
  };
}
