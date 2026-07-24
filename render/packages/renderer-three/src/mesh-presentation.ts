import type * as THREE from 'three';
import type { Material, RenderHandle } from '@rusty-engine/render-contracts';

export const MaterialFallback: Material = {
  color: [1, 1, 1, 1],
  wireframe: false,
};

export interface RendererMeshPresentationReadout {
  readonly handle: RenderHandle;
  readonly lit: boolean;
  readonly materialSlots: readonly number[];
  readonly opacity: number;
  readonly wireframe: boolean;
}

export function meshMaterials(object: THREE.Object3D): THREE.Material[] {
  const material = (object as THREE.Mesh).material;
  return Array.isArray(material) ? material : [material];
}
