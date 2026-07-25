import type { EditorGridDescriptor } from '@rusty-engine/render-contracts';

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
