import type { RustyApplicationFrame } from '@rusty-engine/application-host';

type Vec3 = readonly [number, number, number];
type Color = readonly [number, number, number, number];

export function productPlaytestFrame(): RustyApplicationFrame {
  return {
    schemaVersion: 1,
    ops: [
      cube(1, 'floor', [0, -1, -6], [16, 0.25, 24], [0.16, 0.22, 0.21, 1]),
      cube(2, 'gateway-left', [-2.4, 0.5, -1], [0.65, 3, 0.65], [0.18, 0.75, 0.58, 1]),
      cube(3, 'gateway-right', [2.4, 0.5, -1], [0.65, 3, 0.65], [0.18, 0.75, 0.58, 1]),
      cube(4, 'gateway-top', [0, 2, -1], [5.45, 0.5, 0.65], [0.24, 0.94, 0.72, 1]),
      cube(5, 'amber-marker', [-2.6, -0.25, -5], [1.25, 1.25, 1.25], [0.96, 0.48, 0.13, 1]),
      cube(6, 'blue-marker', [2.8, 0.25, -8.5], [1.4, 2.2, 1.4], [0.19, 0.48, 0.95, 1]),
      cube(7, 'far-marker', [0, 1, -13], [2.4, 4, 1], [0.72, 0.24, 0.58, 1]),
      cube(8, 'left-wall', [-7.5, 1.5, -6], [0.3, 5, 24], [0.11, 0.18, 0.2, 1]),
      cube(9, 'right-wall', [7.5, 1.5, -6], [0.3, 5, 24], [0.11, 0.18, 0.2, 1]),
    ],
  };
}

function cube(handle: number, label: string, translation: Vec3, scale: Vec3, color: Color) {
  return {
    op: 'create',
    handle,
    parent: null,
    node: {
      geometry: { kind: 'cube' },
      material: { color, wireframe: false },
      transform: {
        translation,
        rotation: [0, 0, 0, 1],
        scale,
      },
      visible: true,
      layer: 'scene',
      metadata: {
        sourceEntity: null,
        sourceSceneNode: null,
        tags: ['product-playtest'],
        label,
      },
    },
  };
}
