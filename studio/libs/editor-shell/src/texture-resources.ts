import type { TextureResourceReadout } from '@rusty-engine/studio-adapter-client';

export interface StudioTextureResourceDescriptor {
  readonly resource: string;
  readonly contentHash: string;
  readonly byteLength: number;
}

export type StudioTextureResourceRead = (
  projectRoot: string,
  sourcePath: string,
  contentHash: string,
) => Promise<ArrayBuffer>;

/** Resolve only a texture admitted into the exact renderer manifest. */
export async function resolveStudioTextureResource(
  projectRoot: string,
  resources: readonly TextureResourceReadout[],
  descriptor: StudioTextureResourceDescriptor,
  read: StudioTextureResourceRead,
): Promise<ArrayBuffer> {
  const resource = resources.find((candidate) => candidate.resource === descriptor.resource);
  if (resource === undefined
    || resource.contentHash !== descriptor.contentHash
    || resource.byteLength !== descriptor.byteLength) {
    throw new Error(`Texture resource ${descriptor.resource} is not in the current Rust readout.`);
  }
  return read(projectRoot, resource.sourcePath, resource.contentHash);
}
