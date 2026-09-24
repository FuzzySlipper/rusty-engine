import * as THREE from 'three';
import type { AnimatedMeshInspection } from '@rusty-engine/render-contracts';

type V3 = [number, number, number];
const DIRECTIONS: readonly V3[] = [[-1,0,0],[1,0,0],[0,-1,0],[0,1,0],[0,0,-1],[0,0,1]];
const EPSILON = 1e-4;

// Geometry-only reconstruction adapted from Asset Pipeline's gallery donor.
// Coordinates describe unit voxel faces in the mesh's local space.
export function wholeVoxelNormalGeometry(source: THREE.BufferGeometry): THREE.BufferGeometry | null {
  const geometry = source.index ? source.toNonIndexed() : source.clone();
  const position = geometry.getAttribute('position');
  if (!position || position.count < 3 || position.count % 3 !== 0) { geometry.dispose(); return null; }
  const faces: ({ cell: string; direction: number; normal: V3 } | null)[] = [];
  const voxels = new Map<string, number>();
  for (let i = 0; i < position.count; i += 3) {
    const points = [0,1,2].map(offset => new THREE.Vector3().fromBufferAttribute(position, i + offset));
    const a = points[0]!, b = points[1]!, c = points[2]!;
    const cross = b.clone().sub(a).cross(c.clone().sub(a));
    const abs = cross.toArray().map(Math.abs);
    const axis = abs.indexOf(Math.max(...abs));
    const mins = [0,1,2].map(k => Math.min(...points.map(p => p.getComponent(k))));
    const maxs = [0,1,2].map(k => Math.max(...points.map(p => p.getComponent(k))));
    const eligible = abs[axis]! >= 0.5
      && abs.every((v,k) => k === axis || v <= EPSILON)
      && points.every(p => p.toArray().every(v => Math.abs(v - Math.round(v)) <= EPSILON))
      && mins.every((v,k) => Math.abs(maxs[k]! - v - (k === axis ? 0 : 1)) <= EPSILON);
    if (!eligible) { faces.push(null); continue; }
    const positive = cross.getComponent(axis) > 0;
    const cell = mins.map(Math.round);
    if (positive) cell[axis] = cell[axis]! - 1;
    const key = cell.join(',');
    const direction = axis * 2 + (positive ? 1 : 0);
    const normal = DIRECTIONS[direction]!;
    faces.push({ cell: key, direction, normal });
    voxels.set(key, (voxels.get(key) ?? 0) | (1 << direction));
  }
  if (faces.filter(Boolean).length / faces.length < 0.98) { geometry.dispose(); return null; }
  // Preserve the donor's small allowance for non-voxel triangles without zeroing their normals.
  if (!geometry.getAttribute('normal')) geometry.computeVertexNormals();
  const normals = geometry.getAttribute('normal');
  faces.forEach((face, triangle) => {
    if (!face) return;
    const sum = new THREE.Vector3();
    const mask = voxels.get(face.cell)!;
    DIRECTIONS.forEach((normal, direction) => { if (mask & (1 << direction)) sum.add(new THREE.Vector3(...normal)); });
    const normal = new THREE.Vector3(...face.normal);
    if (sum.lengthSq() > 0 && sum.dot(normal) > 0) normal.copy(sum.normalize());
    for (let vertex = 0; vertex < 3; vertex++) normals.setXYZ(triangle * 3 + vertex, normal.x, normal.y, normal.z);
  });
  normals.needsUpdate = true;
  return geometry;
}

/** Owns only inspection clones; source materials, textures and geometry stay borrowed. */
export class MeshInspection {
  readonly #meshes: { mesh: THREE.Mesh; material: THREE.Material | THREE.Material[]; geometry: THREE.BufferGeometry; inspected: THREE.Material[]; voxel: THREE.BufferGeometry | null | undefined }[] = [];
  voxelNormalMeshes = 0;
  pendingBoundsRequest = 0;
  #lastBoundsRequest = 0;
  #modes = '';
  constructor(object: THREE.Object3D) {
    object.traverse(node => {
      if (node instanceof THREE.Mesh) this.#meshes.push({ mesh: node, material: node.material, geometry: node.geometry, inspected: [], voxel: undefined });
    });
  }
  apply(options: AnimatedMeshInspection): void {
    if (options.boundsRequest !== this.#lastBoundsRequest) {
      this.#lastBoundsRequest = options.boundsRequest;
      this.pendingBoundsRequest = options.boundsRequest;
    }
    const modes = `${options.wireframe}/${options.matte}/${options.wholeVoxelNormals}`;
    if (modes === this.#modes) return;
    this.#modes = modes;
    this.voxelNormalMeshes = 0;
    for (const item of this.#meshes) {
      // Recreate from the original surface so disabling an override restores all physical properties.
      item.inspected.forEach(material => material.dispose());
      item.inspected = [];
      if (options.wireframe || options.matte) {
        item.inspected = (Array.isArray(item.material) ? item.material : [item.material]).map(original => {
          const material = original.clone();
          if ('wireframe' in material) (material as THREE.MeshStandardMaterial).wireframe = options.wireframe;
          if (options.matte && material instanceof THREE.MeshStandardMaterial) {
            material.roughness = 1; material.metalness = 0; material.envMapIntensity = 0.35;
          }
          return material;
        });
        item.mesh.material = Array.isArray(item.material) ? item.inspected : item.inspected[0]!;
      } else item.mesh.material = item.material;
      if (options.wholeVoxelNormals && !(item.mesh instanceof THREE.SkinnedMesh)) {
        // Morph targets change the source geometry too; retain their authored normals.
        if (item.voxel === undefined) item.voxel = Object.keys(item.geometry.morphAttributes).length === 0 ? wholeVoxelNormalGeometry(item.geometry) : null;
        if (item.voxel) this.voxelNormalMeshes++;
        item.mesh.geometry = item.voxel ?? item.geometry;
      } else item.mesh.geometry = item.geometry;
    }
  }
  dispose(): void {
    for (const item of this.#meshes) {
      item.mesh.material = item.material; item.mesh.geometry = item.geometry;
      item.inspected.forEach(material => material.dispose());
      item.voxel?.dispose();
    }
  }
}
