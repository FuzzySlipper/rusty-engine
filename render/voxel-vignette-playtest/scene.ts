import type {
  RustyApplicationContent,
  RustyApplicationFrame,
  RustyApplicationResource,
} from '@rusty-engine/application-host';

type Vec3 = readonly [number, number, number];

const RUN_ID = 'voxel-vignette-128-20260823-003';
const VOXEL_SCALE = 1 / 128;

interface VignetteAsset {
  readonly id: string;
  readonly file: string;
  readonly contentHash: string;
  readonly byteLength: number;
  readonly translation: Vec3;
  readonly rotation: readonly [number, number, number, number];
  readonly scale: Vec3;
  readonly label: string;
}

const ASSETS: readonly VignetteAsset[] = [
  {
    id: 'terrain', file: 'terrain-128-palette-unlit.glb',
    contentHash: 'sha256:254c7ed1d5376d3fbeaff1239a9125b769dbf3d1182e349956c69d7584c30c1a',
    byteLength: 1_067_908,
    translation: [0, 1 / 128, 0],
    rotation: [-Math.SQRT1_2, 0, 0, Math.SQRT1_2],
    scale: [16 / 128, 16 / 128, 1 / 128],
    label: 'terrain route: palette-unlit GLB',
  },
  {
    id: 'shrine-nano', file: 'shrine-nano-128-palette-unlit.glb',
    contentHash: 'sha256:9f97e2700962191ea015f87bb1f63670919581e3cf9f32807a72ca585ad563f5',
    byteLength: 14_260_896,
    translation: [0, 64 / 128, 0],
    rotation: [0, 0, 0, 1],
    scale: [VOXEL_SCALE, VOXEL_SCALE, VOXEL_SCALE],
    label: 'shrine nano: palette-unlit GLB',
  },
  {
    id: 'gpt-tree', file: 'gpt-tree-128-palette-unlit.glb',
    contentHash: 'sha256:cc5b1f15c31dbf114717c5f078637ef84752224f9f32734da7abc0794c595460',
    byteLength: 13_374_324,
    translation: [-4, 51 / 128, 5],
    rotation: [0, 0, 0, 1],
    scale: [VOXEL_SCALE, VOXEL_SCALE, VOXEL_SCALE],
    label: 'tree: palette-unlit GLB',
  },
  {
    id: 'door', file: 'door-t3-128-palette-unlit.glb',
    contentHash: 'sha256:f3c8c66bc7b97bfbccdf19452b8042589b13209614c3e8a53098a4f5c32e03b5',
    byteLength: 5_856_864,
    translation: [4, 64 / 128, 6],
    rotation: [0, 0, 0, 1],
    scale: [VOXEL_SCALE, VOXEL_SCALE, VOXEL_SCALE],
    label: 'door: palette-unlit GLB',
  },
];

export async function loadVignetteContent(): Promise<RustyApplicationContent> {
  const resources = await Promise.all(ASSETS.map(loadAsset));
  return { frame: vignetteFrame(), resources };
}

async function loadAsset(asset: VignetteAsset): Promise<RustyApplicationResource> {
  const response = await fetch(new URL(`./assets/${asset.file}`, import.meta.url));
  if (!response.ok) throw new Error(`${asset.file} returned ${String(response.status)}; run scripts/stage-voxel-vignette-playtest-assets.sh`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  if (bytes.byteLength !== asset.byteLength) {
    throw new Error(`${asset.file} size mismatch; expected ${String(asset.byteLength)}, received ${String(bytes.byteLength)}`);
  }
  // The application host validates these bytes against contentHash using the
  // renderer's LAN-safe digest implementation when it admits the resource.
  return {
    identity: `mesh-resource/${asset.contentHash.slice('sha256:'.length)}`,
    contentHash: asset.contentHash,
    mediaType: 'application/octet-stream',
    bytes,
  };
}

function vignetteFrame(): RustyApplicationFrame {
  return {
    schemaVersion: 1,
    ops: [
      { op: 'createLight', handle: 1, parent: null, light: {
        kind: 'ambient', color: [1, 1, 1], intensity: 2, enabled: true, shadowIntent: 'disabled',
      } },
      { op: 'createLight', handle: 2, parent: null, light: {
        kind: 'directional', color: [1, 0.95, 0.86], intensity: 4, enabled: true,
        direction: [-1, -2, -1], shadowIntent: 'requested',
      } },
      ...ASSETS.flatMap((asset, index) => [
        {
          op: 'defineAnimatedMesh' as const,
          asset: {
            asset: `visual-gate/${RUN_ID}/${asset.id}`,
            runtimeFormat: 'glb' as const,
            contentHash: asset.contentHash,
            clips: [],
            defaultClip: null,
            materialSlots: [],
            bounds: { min: [-1, -1, -1], max: [1, 1, 1] },
          },
        },
        {
          op: 'createAnimatedMeshInstance' as const,
          handle: 100 + index,
          parent: null,
          instance: {
            asset: `visual-gate/${RUN_ID}/${asset.id}`,
            transform: {
              translation: asset.translation,
              rotation: asset.rotation,
              scale: asset.scale,
            },
            visible: true,
            materialOverrides: [],
            playback: null,
            metadata: {
              sourceEntity: null,
              sourceSceneNode: null,
              tags: ['voxel-vignette', 'compiled-glb', 'temporary-comparison-route'],
              label: asset.label,
            },
          },
        },
      ]),
    ],
  };
}
