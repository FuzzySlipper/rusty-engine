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

export type StudioVoxelPreview =
  | StudioVoxelBrushPreview
  | StudioVoxelConversionPreview;

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
): StudioPresentationFrame {
  const entityId = previewEntityId ?? selectedEntityId;
  const creation = entityId === null
    ? null
    : frame.ops.map(createdPresentation).find(
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
  const voxelPreviewOps = presentVoxelPreview(frame, voxelPreview);
  return {
    frame: {
      schemaVersion: 1,
      ops: [
        ...frame.ops,
        ...selectionOps,
        ...voxelPreviewOps,
      ],
    },
    selectedHandle: creation?.handle ?? null,
    previewApplied: transformPreviewApplied || voxelPreviewOps.length > 0,
    voxelPreviewKind: voxelPreviewOps.length === 0 ? null : voxelPreview?.kind ?? null,
  };
}

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
