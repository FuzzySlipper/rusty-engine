import * as THREE from 'three';

// Only uploaded-mesh owners acquire these materials. Other material families
// keep their existing ownership; disposing a shared acquisition releases one ref.
const releases = new WeakMap<THREE.Material, () => void>();
export function releaseUploadedMaterial(material: THREE.Material): void {
  const release = releases.get(material);
  if (release === undefined) material.dispose();
  else release();
}

export class UploadedMaterialPool {
  readonly #entries = new Map<string, { material: THREE.MeshStandardMaterial; references: number }>();

  acquire(key: string, create: () => THREE.MeshStandardMaterial): THREE.MeshStandardMaterial {
    const existing = this.#entries.get(key);
    if (existing !== undefined) {
      existing.references += 1;
      return existing.material;
    }
    const entry = { material: create(), references: 1 };
    this.#entries.set(key, entry);
    releases.set(entry.material, () => {
      entry.references -= 1;
      if (entry.references === 0) {
        this.#entries.delete(key);
        releases.delete(entry.material);
        entry.material.dispose();
      }
    });
    return entry.material;
  }
}

type OriginalGroup = { start: number; count: number; materialIndex?: number | undefined };
const originals = new WeakMap<THREE.BufferGeometry, readonly OriginalGroup[]>();

/** Coalesce adjacent compatible ranges inside one retained chunk.
 * Index order stays exact, including coplanar depth ties and raycast face indices.
 * Alpha groups retain their original separate submissions and order.
 */
export function consolidateUploadedGroups(
  geometry: THREE.BufferGeometry,
  materials: readonly THREE.Material[],
): void {
  let original = originals.get(geometry);
  if (original === undefined) {
    original = geometry.groups.map(group => ({ ...group }));
    originals.set(geometry, original);
  }
  // A material edit can change compatibility; start from admitted ranges.
  geometry.clearGroups();
  for (const group of original) {
    const materialIndex = group.materialIndex ?? 0;
    const material = materials[materialIndex]!;
    const previous = geometry.groups.at(-1);
    if (previous !== undefined && !material.transparent && material.depthWrite
      && materials[previous.materialIndex ?? 0] === material
      && previous.start + previous.count === group.start) {
      previous.count += group.count;
    } else {
      geometry.addGroup(group.start, group.count, materialIndex);
    }
  }
}
