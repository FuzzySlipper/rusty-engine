import type { MeshResourceReadout } from '@rusty-engine/studio-adapter-client';

export interface StudioMeshResourceDescriptor {
  readonly resource: string;
  readonly contentHash: string;
  readonly byteLength: number;
}

export type StudioMeshResourceRead = (
  projectRoot: string,
  sourcePath: string,
  contentHash: string,
) => Promise<ArrayBuffer>;

/**
 * Compose the renderer's admitted resource set with canonical resources taking
 * precedence over a disposable placement candidate on identity collisions.
 */
export function activeStudioMeshResources(
  canonical: readonly MeshResourceReadout[],
  placement: readonly MeshResourceReadout[],
): readonly MeshResourceReadout[] {
  const resources = new Map(canonical.map((resource) => [resource.resource, resource]));
  for (const resource of placement) {
    if (!resources.has(resource.resource)) resources.set(resource.resource, resource);
  }
  return [...resources.values()].sort((left, right) =>
    left.resource.localeCompare(right.resource));
}

/** Resolve only a descriptor admitted into the exact manifest mounted by Studio. */
export async function resolveStudioMeshResource(
  projectRoot: string,
  resources: readonly MeshResourceReadout[],
  descriptor: StudioMeshResourceDescriptor,
  read: StudioMeshResourceRead,
): Promise<ArrayBuffer> {
  const resource = resources.find((candidate) => candidate.resource === descriptor.resource);
  if (resource === undefined
    || resource.contentHash !== descriptor.contentHash
    || resource.byteLength !== descriptor.byteLength) {
    throw new Error(`Mesh resource ${descriptor.resource} is not in the current Rust readout.`);
  }
  return read(projectRoot, resource.sourcePath, resource.contentHash);
}
