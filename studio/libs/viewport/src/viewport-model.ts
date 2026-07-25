import type {
  EditorGridDescriptor,
  RenderDiff,
  RenderFrameDiff,
  RenderHandle,
  RenderMetadata,
  Transform,
} from '@rusty-engine/render-contracts';

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
}

/**
 * Produces a disposable renderer presentation from the canonical Rust frame.
 * Selection and transform preview never alter the accepted authoring document.
 */
export function presentStudioSelection(
  frame: RenderFrameDiff,
  selectedEntityId: number | null,
  previewEntityId: number | null,
  previewTranslation: readonly [number, number, number] | null,
): StudioPresentationFrame {
  const entityId = previewEntityId ?? selectedEntityId;
  if (entityId === null) return { frame, selectedHandle: null, previewApplied: false };
  const creation = frame.ops.map(createdPresentation).find(
    (candidate) => candidate?.metadata.sourceEntity === entityId,
  );
  if (creation === undefined || creation === null) {
    return { frame, selectedHandle: null, previewApplied: false };
  }
  const previewApplied = previewEntityId === entityId
    && previewTranslation !== null
    && previewTranslation.every(Number.isFinite);
  const transform: Transform | null = previewApplied
    ? { ...creation.transform, translation: previewTranslation }
    : null;
  return {
    frame: {
      schemaVersion: 1,
      ops: [
        ...frame.ops,
        {
          op: 'update',
          handle: creation.handle,
          transform,
          material: { color: [0.96, 0.64, 0.2, 1], wireframe: true },
          visible: null,
          metadata: null,
        },
      ],
    },
    selectedHandle: creation.handle,
    previewApplied,
  };
}

interface CreatedPresentation {
  readonly handle: RenderHandle;
  readonly metadata: RenderMetadata;
  readonly transform: Transform;
}

function createdPresentation(operation: RenderDiff): CreatedPresentation | null {
  switch (operation.op) {
    case 'create':
      return {
        handle: operation.handle,
        metadata: operation.node.metadata,
        transform: operation.node.transform,
      };
    case 'createStaticMeshInstance':
    case 'createAnimatedMeshInstance':
      return {
        handle: operation.handle,
        metadata: operation.instance.metadata,
        transform: operation.instance.transform,
      };
    case 'createSprite':
      return {
        handle: operation.handle,
        metadata: operation.sprite.metadata,
        transform: operation.sprite.transform,
      };
    default:
      return null;
  }
}
