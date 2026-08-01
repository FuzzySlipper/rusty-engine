import type {
  EditorGridDescriptor,
  RenderDiff,
  RenderFrameDiff,
  RenderHandle,
  RenderMetadata,
  Transform,
} from '@rusty-engine/render-contracts';
import { renderHandle } from '@rusty-engine/render-contracts';

export const STUDIO_EDITOR_GRID: EditorGridDescriptor = {
  visible: true,
  grid: {
    coordinateSystem: 'rightHandedYUp',
    origin: [0, 0, 0],
    spacing: [0.5, 0.5, 0.5],
  },
  plane: 'xz',
  snapAnchor: 'boundary',
  style: {
    minorColor: [0.24, 0.4, 0.42, 0.36],
    majorColor: [0.32, 0.58, 0.58, 0.62],
    xAxisColor: [0.86, 0.28, 0.26, 0.92],
    yAxisColor: [0.28, 0.82, 0.46, 0.92],
    zAxisColor: [0.28, 0.5, 0.9, 0.92],
    majorLineEvery: 4,
    opacity: 0.82,
    fadeStart: 22,
    fadeEnd: 62,
  },
};

export type StudioLightingMode = 'work_light' | 'authored_lights';

export interface StudioLightingPresentation {
  readonly frame: RenderFrameDiff;
  readonly authoredLightCount: number;
  readonly activeLightCount: number;
  readonly workLightActive: boolean;
}

/**
 * Replaces authored lights with a disposable, shadow-free editor rig when the
 * human-facing work-light mode is active. The accepted Rust frame is never
 * changed and authored lighting can be restored by presenting it again.
 */
export function presentStudioLighting(
  frame: RenderFrameDiff,
  mode: StudioLightingMode,
): StudioLightingPresentation {
  const authoredLightHandles = new Set(
    frame.ops
      .filter((operation) => operation.op === 'createLight')
      .map((operation) => operation.handle),
  );
  const authoredLightCount = authoredLightHandles.size;
  if (mode === 'authored_lights') {
    return {
      frame,
      authoredLightCount,
      activeLightCount: frame.ops.filter(
        (operation) => operation.op === 'createLight' && operation.light.enabled,
      ).length,
      workLightActive: false,
    };
  }

  const handles = availablePreviewHandles(frame, 2);
  const ambientHandle = handles[0] as RenderHandle;
  const directionalHandle = handles[1] as RenderHandle;
  const retainedOps = frame.ops.filter((operation) => {
    if (operation.op === 'createLight' || operation.op === 'updateLight') return false;
    return operation.op !== 'destroy' || !authoredLightHandles.has(operation.handle);
  });
  return {
    frame: {
      schemaVersion: 1,
      ops: [
        ...retainedOps,
        {
          op: 'createLight',
          handle: ambientHandle,
          parent: null,
          light: {
            kind: 'ambient',
            color: [1, 1, 1],
            intensity: 0.62,
            enabled: true,
            shadowIntent: 'disabled',
          },
        },
        {
          op: 'createLight',
          handle: directionalHandle,
          parent: null,
          light: {
            kind: 'directional',
            color: [1, 0.96, 0.9],
            intensity: 1.15,
            enabled: true,
            direction: [-0.55, -0.8, -0.45],
            shadowIntent: 'disabled',
          },
        },
      ],
    },
    authoredLightCount,
    activeLightCount: 2,
    workLightActive: true,
  };
}

export function canvasPoint(
  client: readonly [number, number],
  bounds: Pick<DOMRect, 'left' | 'top'>,
): readonly [number, number] {
  return [client[0] - bounds.left, client[1] - bounds.top];
}

export function movedPastPickThreshold(
  start: readonly [number, number],
  current: readonly [number, number],
): boolean {
  return Math.hypot(current[0] - start[0], current[1] - start[1]) > 4;
}

export interface StudioPresentationFrame {
  readonly frame: RenderFrameDiff;
  readonly selectedHandle: RenderHandle | null;
  readonly previewApplied: boolean;
  readonly voxelPreviewKind: StudioVoxelPreview['kind'] | null;
}

export interface StudioGroundingInspection {
  readonly origin: readonly [number, number, number];
  readonly bounds: {
    readonly min: readonly [number, number, number];
    readonly max: readonly [number, number, number];
  };
  readonly contactPlaneY: number;
  readonly clearance: number;
}

export interface StudioVoxelBrushPreview {
  readonly kind: 'brush';
  readonly transform: Transform;
  readonly radius: number;
  readonly mode: 'paint' | 'erase';
}

export interface StudioVoxelConversionPreview {
  readonly kind: 'conversion';
  readonly cellSize: number;
  readonly samples: readonly {
    readonly coordinate: readonly [number, number, number];
    readonly materialSlot: number;
  }[];
}

export interface StudioVoxelObjectPlacementPreview {
  readonly kind: 'objectPlacement';
  readonly assetId: string;
  readonly assetContentHash: string;
  readonly frameId: string;
  readonly transform: Transform;
  readonly materialOverrides: readonly {
    readonly slot: number;
    readonly material: string;
  }[];
  readonly label: string;
}

export type StudioVoxelPreview =
  | StudioVoxelBrushPreview
  | StudioVoxelConversionPreview
  | StudioVoxelObjectPlacementPreview;

/**
 * Produces a disposable renderer presentation from the canonical Rust frame.
 * Selection and transform preview never alter the accepted authoring document.
 */
export function presentStudioSelection(
  frame: RenderFrameDiff,
  selectedEntityId: number | null,
  previewEntityId: number | null,
  previewTransform: Transform | null,
  voxelPreview: StudioVoxelPreview | null = null,
  voxelObjectPlacementResourceFrame: RenderFrameDiff | null = null,
  groundingInspection: StudioGroundingInspection | null = null,
): StudioPresentationFrame {
  const resolvedFrame = voxelPreview?.kind === 'objectPlacement'
    ? mergeVoxelObjectPlacementResources(
        frame,
        voxelObjectPlacementResourceFrame,
        voxelPreview,
      )
    : frame;
  const entityId = previewEntityId ?? selectedEntityId;
  const creation = entityId === null
    ? null
    : resolvedFrame.ops.map(createdPresentation).find(
      (candidate) => candidate?.metadata.sourceEntity === entityId,
    ) ?? null;
  const transformPreviewApplied = creation !== null
    && previewEntityId === entityId
    && previewTransform !== null
    && validTransform(previewTransform);
  const transform: Transform | null = transformPreviewApplied
    ? previewTransform
    : null;
  const selectionOps: readonly RenderDiff[] = creation === null
    ? []
    : [{
        op: 'update',
        handle: creation.handle,
        transform,
        material: creation.supportsMaterialUpdate
          ? { color: [0.96, 0.64, 0.2, 1], wireframe: true }
          : null,
        visible: null,
        metadata: null,
      }];
  const voxelPreviewOps = presentVoxelPreview(resolvedFrame, voxelPreview);
  const groundingOps = presentGroundingInspection(resolvedFrame, groundingInspection);
  return {
    frame: {
      schemaVersion: 1,
      ops: [
        ...resolvedFrame.ops,
        ...selectionOps,
        ...voxelPreviewOps,
        ...groundingOps,
      ],
    },
    selectedHandle: creation?.handle ?? null,
    previewApplied: transformPreviewApplied || voxelPreviewOps.length > 0 || groundingOps.length > 0,
    voxelPreviewKind: voxelPreviewOps.length === 0 ? null : voxelPreview?.kind ?? null,
  };
}

function presentGroundingInspection(
  frame: RenderFrameDiff,
  inspection: StudioGroundingInspection | null,
): readonly RenderDiff[] {
  if (inspection === null) return [];
  const values = [
    ...inspection.origin,
    ...inspection.bounds.min,
    ...inspection.bounds.max,
    inspection.contactPlaneY,
    inspection.clearance,
  ];
  if (!values.every(Number.isFinite)) return [];
  const [minX, minY, minZ] = inspection.bounds.min;
  const [maxX, maxY, maxZ] = inspection.bounds.max;
  if (minX > maxX || minY > maxY || minZ > maxZ) return [];
  const corners = [
    [minX, minY, minZ], [maxX, minY, minZ], [maxX, maxY, minZ], [minX, maxY, minZ],
    [minX, minY, maxZ], [maxX, minY, maxZ], [maxX, maxY, maxZ], [minX, maxY, maxZ],
  ] as const;
  const boundsEdges = [
    [0, 1], [1, 2], [2, 3], [3, 0],
    [4, 5], [5, 6], [6, 7], [7, 4],
    [0, 4], [1, 5], [2, 6], [3, 7],
  ] as const;
  const extent = Math.max(maxX - minX, maxZ - minZ, 1) * 0.6;
  const centerX = (minX + maxX) / 2;
  const centerZ = (minZ + maxZ) / 2;
  const planeY = inspection.contactPlaneY;
  const lines: { readonly a: readonly [number, number, number]; readonly b: readonly [number, number, number]; readonly color: readonly [number, number, number, number]; readonly tag: string }[] = [
    { a: inspection.origin, b: [inspection.origin[0] + extent, inspection.origin[1], inspection.origin[2]], color: [0.95, 0.2, 0.18, 1], tag: 'origin-x' },
    { a: inspection.origin, b: [inspection.origin[0], inspection.origin[1] + extent, inspection.origin[2]], color: [0.2, 0.9, 0.38, 1], tag: 'origin-y' },
    { a: inspection.origin, b: [inspection.origin[0], inspection.origin[1], inspection.origin[2] + extent], color: [0.24, 0.48, 1, 1], tag: 'origin-z' },
    ...boundsEdges.map(([a, b]) => ({ a: corners[a], b: corners[b], color: [1, 0.72, 0.16, 0.95] as const, tag: 'visual-bounds' })),
    { a: [centerX - extent, planeY, centerZ - extent], b: [centerX + extent, planeY, centerZ - extent], color: [0.2, 0.88, 0.86, 0.82], tag: 'contact-plane' },
    { a: [centerX + extent, planeY, centerZ - extent], b: [centerX + extent, planeY, centerZ + extent], color: [0.2, 0.88, 0.86, 0.82], tag: 'contact-plane' },
    { a: [centerX + extent, planeY, centerZ + extent], b: [centerX - extent, planeY, centerZ + extent], color: [0.2, 0.88, 0.86, 0.82], tag: 'contact-plane' },
    { a: [centerX - extent, planeY, centerZ + extent], b: [centerX - extent, planeY, centerZ - extent], color: [0.2, 0.88, 0.86, 0.82], tag: 'contact-plane' },
  ];
  const handles = availablePreviewHandles(frame, lines.length);
  return lines.map((line, index) => ({
    op: 'create' as const,
    handle: handles[index] as RenderHandle,
    parent: null,
    node: {
      geometry: { kind: 'line' as const, a: line.a, b: line.b },
      material: { color: line.color, wireframe: false },
      transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      visible: true,
      layer: 'debug' as const,
      metadata: {
        sourceEntity: null,
        sourceSceneNode: null,
        tags: ['studio-presentation', 'grounding-inspection', line.tag],
        label: 'Renderable grounding inspection',
      },
    },
  }));
}

/**
 * Adds a single adapter-provided placement resource without replacing or
 * weakening canonical retained definitions. Identity conflicts fail closed so
 * the caller receives the original frame and no ghost can be presented.
 */
export function mergeVoxelObjectPlacementResources(
  frame: RenderFrameDiff,
  resourceFrame: RenderFrameDiff | null,
  preview: StudioVoxelObjectPlacementPreview,
): RenderFrameDiff {
  if (resourceFrame === null) return frame;
  const additions: RenderDiff[] = [];
  let matchingObjectDefinitions = 0;
  for (const operation of resourceFrame.ops) {
    if (operation.op === 'defineMaterial') {
      const existing = frame.ops.find((candidate) =>
        candidate.op === 'defineMaterial'
        && candidate.material.id === operation.material.id);
      if (existing !== undefined) {
        if (existing.op !== 'defineMaterial' || !sameValue(existing.material, operation.material)) {
          return frame;
        }
      } else {
        additions.push(operation);
      }
      continue;
    }
    if (operation.op === 'defineTexture') {
      const existing = frame.ops.find((candidate) =>
        candidate.op === 'defineTexture'
        && candidate.texture.id === operation.texture.id);
      if (existing !== undefined) {
        if (existing.op !== 'defineTexture' || !sameValue(existing.texture, operation.texture)) {
          return frame;
        }
      } else {
        additions.push(operation);
      }
      continue;
    }
    if (operation.op !== 'defineVoxelObject') return frame;
    if (
      operation.asset.asset !== preview.assetId
      || operation.asset.contentHash !== preview.assetContentHash
    ) return frame;
    matchingObjectDefinitions += 1;
    const existing = frame.ops.find((candidate) =>
      candidate.op === 'defineVoxelObject'
      && candidate.asset.asset === operation.asset.asset);
    if (existing !== undefined) {
      if (existing.op !== 'defineVoxelObject' || !sameValue(existing.asset, operation.asset)) {
        return frame;
      }
    } else {
      additions.push(operation);
    }
  }
  if (matchingObjectDefinitions !== 1) return frame;
  return additions.length === 0
    ? frame
    : { schemaVersion: 1, ops: [...frame.ops, ...additions] };
}

function sameValue(left: unknown, right: unknown): boolean {
  return JSON.stringify(left) === JSON.stringify(right);
}

// Conversion samples are diagnostic preview geometry. Bound their temporary
// render-node cost without changing the complete owner conversion result.
const MAX_CONVERSION_PREVIEW_NODES = 512;

function presentVoxelPreview(
  frame: RenderFrameDiff,
  preview: StudioVoxelPreview | null,
): readonly RenderDiff[] {
  if (preview === null) return [];
  if (preview.kind === 'brush') {
    if (!validTransform(preview.transform) || !Number.isSafeInteger(preview.radius)) return [];
    const diameter = Math.max(0, preview.radius) * 2 + 1;
    return [previewNodeWithTransform(
      availablePreviewHandles(frame, 1)[0] as RenderHandle,
      {
        ...preview.transform,
        scale: preview.transform.scale.map((value) => value * diameter) as unknown as Transform['scale'],
      },
      preview.mode === 'paint' ? [0.2, 0.9, 0.55, 0.55] : [0.95, 0.24, 0.18, 0.55],
      ['studio-preview', 'voxel-brush-preview', `brush-mode:${preview.mode}`],
      'Voxel brush preview',
    )];
  }
  if (preview.kind === 'objectPlacement') {
    return presentVoxelObjectPlacement(frame, preview);
  }
  if (!Number.isFinite(preview.cellSize) || preview.cellSize <= 0) return [];
  const samples = preview.samples.slice(0, MAX_CONVERSION_PREVIEW_NODES).filter(
    (sample) => sample.coordinate.every(Number.isFinite),
  );
  const handles = availablePreviewHandles(frame, samples.length);
  const size = preview.cellSize * 0.82;
  return samples.map((sample, index) => previewNode(
    handles[index] as RenderHandle,
    [
      (sample.coordinate[0] + 0.5) * preview.cellSize,
      (sample.coordinate[1] + 0.5) * preview.cellSize,
      (sample.coordinate[2] + 0.5) * preview.cellSize,
    ],
    [size, size, size],
    [0.28, 0.78, 1, 0.62],
    ['studio-preview', 'voxel-conversion-preview', `material-slot:${String(sample.materialSlot)}`],
    'Voxel conversion sample',
  ));
}

function presentVoxelObjectPlacement(
  frame: RenderFrameDiff,
  preview: StudioVoxelObjectPlacementPreview,
): readonly RenderDiff[] {
  if (!validTransform(preview.transform)) return [];
  const definition = frame.ops.find((operation) =>
    operation.op === 'defineVoxelObject'
    && operation.asset.asset === preview.assetId
    && operation.asset.contentHash === preview.assetContentHash);
  if (definition?.op !== 'defineVoxelObject') return [];
  const frameIndex = definition.asset.frames.findIndex((entry) => entry.id === preview.frameId);
  if (frameIndex < 0) return [];
  const boundSlots = new Set(definition.asset.materialSlots.map((entry) => entry.slot));
  const definedMaterials = new Set(frame.ops.flatMap((operation) =>
    operation.op === 'defineMaterial' ? [operation.material.id] : []));
  const overrideSlots = new Set<number>();
  if (preview.materialOverrides.some((entry) => {
    if (
      !Number.isSafeInteger(entry.slot)
      || !boundSlots.has(entry.slot)
      || !definedMaterials.has(entry.material)
      || overrideSlots.has(entry.slot)
    ) return true;
    overrideSlots.add(entry.slot);
    return false;
  })) return [];
  const handles = availablePreviewHandles(frame, 2);
  const root = handles[0] as RenderHandle;
  const instance = handles[1] as RenderHandle;
  return [
    {
      op: 'create',
      handle: root,
      parent: null,
      node: {
        geometry: { kind: 'group' },
        material: { color: [0.25, 0.9, 0.8, 0.45], wireframe: true },
        transform: {
          translation: [0, 0, 0],
          rotation: [0, 0, 0, 1],
          scale: [1, 1, 1],
        },
        visible: true,
        layer: 'debug',
        metadata: {
          sourceEntity: null,
          sourceSceneNode: null,
          tags: ['studio-preview', 'voxel-object-placement-root'],
          label: 'Voxel-object placement preview',
        },
      },
    },
    {
      op: 'createVoxelObjectInstance',
      handle: instance,
      parent: root,
      instance: {
        asset: definition.asset.asset,
        frame: frameIndex,
        transform: preview.transform,
        visible: true,
        materialOverrides: preview.materialOverrides,
        metadata: {
          sourceEntity: null,
          sourceSceneNode: null,
          tags: ['studio-preview', 'voxel-object-placement-ghost'],
          label: preview.label,
        },
      },
    },
  ];
}

function previewNode(
  handle: RenderHandle,
  translation: readonly [number, number, number],
  scale: readonly [number, number, number],
  color: readonly [number, number, number, number],
  tags: readonly string[],
  label: string,
): RenderDiff {
  return previewNodeWithTransform(
    handle,
    { translation, rotation: [0, 0, 0, 1], scale },
    color,
    tags,
    label,
  );
}

function previewNodeWithTransform(
  handle: RenderHandle,
  transform: Transform,
  color: readonly [number, number, number, number],
  tags: readonly string[],
  label: string,
): RenderDiff {
  return {
    op: 'create',
    handle,
    parent: null,
    node: {
      geometry: { kind: 'cube' },
      material: { color, wireframe: true },
      transform,
      visible: true,
      layer: 'debug',
      metadata: { sourceEntity: null, sourceSceneNode: null, tags, label },
    },
  };
}

function validTransform(transform: Transform): boolean {
  return transform.translation.every(Number.isFinite)
    && transform.rotation.every(Number.isFinite)
    && transform.scale.every((value) => Number.isFinite(value) && value > 0);
}

function availablePreviewHandles(frame: RenderFrameDiff, count: number): readonly RenderHandle[] {
  const occupied = new Set<number>();
  for (const operation of frame.ops) {
    if ('handle' in operation) occupied.add(operation.handle);
  }
  const handles: RenderHandle[] = [];
  let candidate = Number.MAX_SAFE_INTEGER;
  while (handles.length < count && candidate >= 0) {
    if (!occupied.has(candidate)) handles.push(renderHandle(candidate));
    candidate -= 1;
  }
  return handles;
}

interface CreatedPresentation {
  readonly handle: RenderHandle;
  readonly metadata: RenderMetadata;
  readonly transform: Transform;
  readonly supportsMaterialUpdate: boolean;
}

function createdPresentation(operation: RenderDiff): CreatedPresentation | null {
  switch (operation.op) {
    case 'create':
      return {
        handle: operation.handle,
        metadata: operation.node.metadata,
        transform: operation.node.transform,
        supportsMaterialUpdate: operation.node.geometry.kind !== 'group',
      };
    case 'createStaticMeshInstance':
      return {
        handle: operation.handle,
        metadata: operation.instance.metadata,
        transform: operation.instance.transform,
        supportsMaterialUpdate: true,
      };
    case 'createAnimatedMeshInstance':
      return {
        handle: operation.handle,
        metadata: operation.instance.metadata,
        transform: operation.instance.transform,
        // Animated assets are object hierarchies. Their root has no single material
        // for the generic update operation to replace, so selection stays visible
        // through the Studio gizmo instead of mutating imported child materials.
        supportsMaterialUpdate: false,
      };
    case 'createVoxelObjectInstance':
      return {
        handle: operation.handle,
        metadata: operation.instance.metadata,
        transform: operation.instance.transform,
        supportsMaterialUpdate: false,
      };
    case 'createSprite':
      return {
        handle: operation.handle,
        metadata: operation.sprite.metadata,
        transform: operation.sprite.transform,
        supportsMaterialUpdate: true,
      };
    default:
      return null;
  }
}
