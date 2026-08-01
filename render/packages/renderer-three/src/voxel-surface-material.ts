import * as THREE from 'three';
import type {
  RenderMaterialDescriptor,
  TextureDescriptor,
  VoxelSurfaceDescriptor,
} from '@rusty-engine/render-contracts';

export interface VoxelSurfaceMaterialReadout {
  readonly material: string;
  readonly texture: string;
  readonly mapping: 'repeat' | 'atlas';
  readonly tileScaleCells: readonly [number, number];
  readonly tileOriginCells: readonly [number, number];
  readonly sampleUvMin: readonly [number, number];
  readonly sampleUvMax: readonly [number, number];
  readonly alphaMode: 'opaque' | 'mask' | 'blend';
  readonly alphaCutoff: number | null;
}

export class VoxelSurfaceMaterialError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'VoxelSurfaceMaterialError';
  }
}

/** Validate the resolved texture and derive its normalized safe sampling rect. */
export function resolveVoxelSurfaceMaterial(
  material: RenderMaterialDescriptor,
  texture: TextureDescriptor,
): VoxelSurfaceMaterialReadout {
  const surface = material.voxelSurface;
  if (surface === undefined) {
    throw new VoxelSurfaceMaterialError(`material ${material.id} has no voxel surface`);
  }
  const mapping = surface.mapping;
  if (material.texture !== mapping.texture || texture.id !== mapping.texture) {
    throw new VoxelSurfaceMaterialError(
      `material ${material.id} resolved texture ${mapping.texture} does not match ${texture.id}`,
    );
  }
  if (texture.version !== mapping.textureVersion) {
    throw new VoxelSurfaceMaterialError(
      `material ${material.id} needs texture ${texture.id} version ${String(mapping.textureVersion)}`,
    );
  }
  if (texture.contentHash !== mapping.textureContentHash) {
    throw new VoxelSurfaceMaterialError(
      `material ${material.id} needs texture ${texture.id} hash ${mapping.textureContentHash}`,
    );
  }
  if (texture.payload === undefined) {
    throw new VoxelSurfaceMaterialError(
      `material ${material.id} needs retained texture payload ${texture.id}`,
    );
  }
  if (texture.filter !== surface.filter || texture.wrap !== surface.wrap) {
    throw new VoxelSurfaceMaterialError(
      `material ${material.id} texture sampling policy does not match ${texture.id}`,
    );
  }

  let sampleUvMin: readonly [number, number] = [0, 0];
  let sampleUvMax: readonly [number, number] = [1, 1];
  if (mapping.kind === 'atlas') {
    const [x, y] = mapping.region.contentMin;
    const [width, height] = mapping.region.contentExtent;
    if (x + width > texture.width || y + height > texture.height) {
      throw new VoxelSurfaceMaterialError(
        `material ${material.id} atlas region ${mapping.region.id} exceeds ${texture.id}`,
      );
    }
    sampleUvMin = [(x + 0.5) / texture.width, (y + 0.5) / texture.height];
    sampleUvMax = [
      (x + width - 0.5) / texture.width,
      (y + height - 0.5) / texture.height,
    ];
  }

  return Object.freeze({
    material: material.id,
    texture: texture.id,
    mapping: mapping.kind,
    tileScaleCells: Object.freeze([...mapping.tileScaleCells]) as readonly [number, number],
    tileOriginCells: Object.freeze([...mapping.tileOriginCells]) as readonly [number, number],
    sampleUvMin: Object.freeze([...sampleUvMin]) as readonly [number, number],
    sampleUvMax: Object.freeze([...sampleUvMax]) as readonly [number, number],
    alphaMode: surface.alphaMode.kind,
    alphaCutoff: surface.alphaMode.kind === 'mask' ? surface.alphaMode.cutoff : null,
  });
}

/** Patch one MeshStandardMaterial without changing or expanding its geometry. */
export function specializeVoxelSurfaceMaterial(
  material: THREE.MeshStandardMaterial,
  descriptor: RenderMaterialDescriptor,
  texture: TextureDescriptor,
): VoxelSurfaceMaterialReadout {
  const readout = resolveVoxelSurfaceMaterial(descriptor, texture);
  material.userData['rustyVoxelSurface'] = readout;
  material.customProgramCacheKey = () => [
    'rusty-engine.voxel-surface.v1',
    readout.mapping,
    descriptor.voxelSurface!.filter,
    readout.alphaMode,
  ].join(':');
  material.onBeforeCompile = (shader) => {
    shader.uniforms['rustyVoxelTileScale'] = {
      value: new THREE.Vector2(...readout.tileScaleCells),
    };
    shader.uniforms['rustyVoxelTileOrigin'] = {
      value: new THREE.Vector2(...readout.tileOriginCells),
    };
    shader.uniforms['rustyVoxelUvMin'] = {
      value: new THREE.Vector2(...readout.sampleUvMin),
    };
    shader.uniforms['rustyVoxelUvMax'] = {
      value: new THREE.Vector2(...readout.sampleUvMax),
    };
    shader.fragmentShader = shader.fragmentShader
      .replace(
        '#include <map_pars_fragment>',
        [
          '#include <map_pars_fragment>',
          '#ifdef USE_MAP',
          'uniform vec2 rustyVoxelTileScale;',
          'uniform vec2 rustyVoxelTileOrigin;',
          'uniform vec2 rustyVoxelUvMin;',
          'uniform vec2 rustyVoxelUvMax;',
          '#endif',
        ].join('\n'),
      )
      .replace(
        '#include <map_fragment>',
        [
          '#ifdef USE_MAP',
          'vec2 rustyVoxelRepeat = fract((vMapUv - rustyVoxelTileOrigin) / rustyVoxelTileScale);',
          'vec2 rustyVoxelUv = mix(rustyVoxelUvMin, rustyVoxelUvMax, rustyVoxelRepeat);',
          'vec4 sampledDiffuseColor = texture2D(map, rustyVoxelUv);',
          '#ifdef DECODE_VIDEO_TEXTURE',
          'sampledDiffuseColor = sRGBTransferEOTF(sampledDiffuseColor);',
          '#endif',
          'diffuseColor *= sampledDiffuseColor;',
          '#endif',
        ].join('\n'),
      );
  };
  applyAlphaPolicy(material, descriptor.voxelSurface!);
  material.needsUpdate = true;
  return readout;
}

/** CPU reference for the exact shader mapping used by deterministic tests. */
export function sampleVoxelSurfaceUv(
  readout: VoxelSurfaceMaterialReadout,
  tileCoordinate: readonly [number, number],
): readonly [number, number] {
  const repeated = tileCoordinate.map((coordinate, axis) => {
    const scaled = (coordinate - readout.tileOriginCells[axis]!)
      / readout.tileScaleCells[axis]!;
    return scaled - Math.floor(scaled);
  });
  return Object.freeze([
    readout.sampleUvMin[0]
      + (readout.sampleUvMax[0] - readout.sampleUvMin[0]) * repeated[0]!,
    readout.sampleUvMin[1]
      + (readout.sampleUvMax[1] - readout.sampleUvMin[1]) * repeated[1]!,
  ]);
}

function applyAlphaPolicy(
  material: THREE.MeshStandardMaterial,
  surface: VoxelSurfaceDescriptor,
): void {
  switch (surface.alphaMode.kind) {
    case 'opaque':
      material.alphaTest = 0;
      material.transparent = false;
      material.depthWrite = true;
      break;
    case 'mask':
      material.alphaTest = surface.alphaMode.cutoff;
      material.transparent = false;
      material.depthWrite = true;
      break;
    case 'blend':
      material.alphaTest = 0;
      material.transparent = true;
      material.depthWrite = false;
      break;
  }
}
