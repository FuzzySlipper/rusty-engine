import {
  renderHandle,
  type MeshPayloadDescriptor,
  type RenderFrameDiff,
  type RenderMaterialDescriptor,
  type StaticMeshAsset,
  type StaticMeshInstanceDescriptor,
  type Transform,
} from '@rusty-engine/render-contracts';

export const STATIC_ROOM_FIXTURE_NAME = 'static-room';

type Vec3 = readonly [number, number, number];

const IDENTITY_ROTATION = [0, 0, 0, 1] as const;

export function createStaticRoomRenderFrame(): RenderFrameDiff {
  return {
    schemaVersion: 1,
    ops: [
      material('material/room-floor', [0.44, 0.48, 0.46, 1]),
      material('material/room-wall', [0.68, 0.73, 0.76, 1]),
      material('material/room-ceiling', [0.86, 0.88, 0.84, 1]),
      material('material/room-marker', [0.92, 0.42, 0.18, 1]),
      { op: 'defineStaticMesh', asset: panelAsset('mesh/room-floor', 'material/room-floor') },
      { op: 'defineStaticMesh', asset: panelAsset('mesh/room-wall', 'material/room-wall') },
      { op: 'defineStaticMesh', asset: panelAsset('mesh/room-ceiling', 'material/room-ceiling') },
      { op: 'defineStaticMesh', asset: panelAsset('mesh/room-marker', 'material/room-marker') },
      instance(1, 'mesh/room-floor', 'room-floor', [0, -1, 0], [8, 1, 8]),
      instance(2, 'mesh/room-wall', 'room-wall-north', [0, 1, -4], [8, 4, 1]),
      instance(3, 'mesh/room-wall', 'room-wall-south', [0, 1, 4], [8, 4, 1]),
      instance(4, 'mesh/room-wall', 'room-wall-west', [-4, 1, 0], [1, 4, 8]),
      instance(5, 'mesh/room-wall', 'room-wall-east', [4, 1, 0], [1, 4, 8]),
      instance(6, 'mesh/room-ceiling', 'room-ceiling', [0, 3, 0], [8, 1, 8]),
      instance(7, 'mesh/room-marker', 'room-origin-marker', [0, -0.48, 0], [0.5, 0.04, 0.5]),
    ],
  };
}

function material(
  id: string,
  color: readonly [number, number, number, number],
): { readonly op: 'defineMaterial'; readonly material: RenderMaterialDescriptor } {
  return {
    op: 'defineMaterial',
    material: {
      schemaVersion: 2,
      id,
      color,
      texture: null,
      roughness: 1,
      textureTint: [1, 1, 1, 1],
      emissionColor: [0, 0, 0],
      emissionIntensity: 0,
      uvStrategy: 'flat',
    },
  };
}

function panelAsset(asset: string, materialId: string): StaticMeshAsset {
  return {
    asset,
    payload: quadPayload(),
    materialSlots: [{ slot: 0, material: materialId }],
    collision: { kind: 'aabbFallback' },
  };
}

function quadPayload(): MeshPayloadDescriptor {
  return {
    layout: {
      vertexCount: 4,
      indexCount: 6,
      indexWidth: 'u32',
      attributes: [
        { name: 'position', components: 3, kind: 'f32' },
        { name: 'normal', components: 3, kind: 'f32' },
      ],
    },
    groups: [{ materialSlot: 0, start: 0, count: 6 }],
    bounds: { min: [-0.5, -0.5, 0], max: [0.5, 0.5, 0] },
    source: {
      kind: 'inline',
      positions: [-0.5, -0.5, 0, 0.5, -0.5, 0, 0.5, 0.5, 0, -0.5, 0.5, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2, 0, 2, 3],
    },
    provenance: 'generated',
  };
}

function instance(
  handle: number,
  asset: string,
  label: string,
  translation: Vec3,
  scale: Vec3,
): {
  readonly op: 'createStaticMeshInstance';
  readonly handle: ReturnType<typeof renderHandle>;
  readonly parent: null;
  readonly instance: StaticMeshInstanceDescriptor;
} {
  return {
    op: 'createStaticMeshInstance',
    handle: renderHandle(handle),
    parent: null,
    instance: {
      asset,
      transform: transform(translation, scale),
      visible: true,
      materialOverrides: [],
      metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label },
    },
  };
}

function transform(translation: Vec3, scale: Vec3): Transform {
  return {
    translation,
    rotation: IDENTITY_ROTATION,
    scale,
  };
}
