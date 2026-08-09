import type {
  StoredVoxelObjectInstance,
  VoxelObjectAssetAuthoringReadout,
  VoxelObjectFrameSelection,
  VoxelObjectInstanceReadout,
} from '@rusty-engine/studio-adapter-client';

export const MAX_VOXEL_OBJECT_PLACEMENT_PALETTE_ASSETS = 256;
export const MAX_VOXEL_OBJECT_PLACEMENTS_PER_SESSION = 4_096;
export const MAX_VOXEL_OBJECT_PLACEMENT_ID_BYTES = 128;
export const MAX_VOXEL_OBJECT_PLACEMENT_MATERIAL_OVERRIDES = 32;

export interface VoxelObjectPlacementPresentation {
  readonly kind: 'objectPlacement';
  readonly assetId: string;
  readonly assetContentHash: string;
  readonly frameId: string;
  readonly transform: {
    readonly translation: readonly [number, number, number];
    readonly rotation: readonly [number, number, number, number];
    readonly scale: readonly [number, number, number];
  };
  readonly materialOverrides: readonly {
    readonly slot: number;
    readonly material: string;
  }[];
  readonly label: string;
}

export interface VoxelObjectPlacementHistoryReadout {
  readonly state: 'placed' | 'undone';
  readonly instanceId: string;
  readonly ownerEntityId: number | null;
}

export interface VoxelObjectPlacementResourceReadout {
  readonly assetId: string;
  readonly objectContentHash: string;
}

export interface VoxelObjectPlacementCandidateInput {
  readonly sceneId: string;
  readonly asset: VoxelObjectAssetAuthoringReadout;
  readonly instanceId: string;
  readonly clipId: string;
  readonly frameIndex: number;
  readonly translation: readonly number[];
  readonly rotation: readonly number[];
  readonly scale: readonly number[];
  readonly materialOverrides: readonly {
    readonly materialSlot: number;
    readonly materialAssetId: string;
  }[];
  readonly canonicalMaterialIds: ReadonlySet<string>;
}

export interface VoxelObjectPlacementCandidate {
  readonly sceneId: string;
  readonly instance: StoredVoxelObjectInstance;
  readonly presentation: VoxelObjectPlacementPresentation;
}

export function boundedVoxelObjectPlacementPalette(
  assets: readonly VoxelObjectAssetAuthoringReadout[],
): readonly VoxelObjectAssetAuthoringReadout[] {
  return assets.slice(0, MAX_VOXEL_OBJECT_PLACEMENT_PALETTE_ASSETS);
}

/**
 * Builds the complete candidate sent through the existing closed attach
 * operation and its structurally separate disposable renderer presentation.
 */
export function buildVoxelObjectPlacementCandidate(
  input: VoxelObjectPlacementCandidateInput,
): VoxelObjectPlacementCandidate {
  const sceneId = boundedIdentity(input.sceneId, 'Scene identity');
  const instanceId = boundedIdentity(input.instanceId, 'Voxel-object instance identity');
  const frame = selectedFrame(input.asset, input.clipId, input.frameIndex);
  const translation = tuple(input.translation, 3, 'Placement translation');
  const rotation = tuple(input.rotation, 4, 'Placement rotation');
  if (Math.hypot(...rotation) <= Number.EPSILON) {
    throw new TypeError('Placement rotation must be a non-zero quaternion.');
  }
  const scale = tuple(input.scale, 3, 'Placement scale');
  if (scale.some((value) => value <= 0)) {
    throw new TypeError('Placement scale axes must be greater than zero.');
  }
  const materialOverrides = validatedOverrides(input);
  const instance: StoredVoxelObjectInstance = {
    instanceId,
    voxelObjectAssetId: input.asset.assetId,
    surfaceMode: 'greedyCubes',
    frame,
    translation,
    rotation,
    scale,
    materialOverrides,
  };
  return {
    sceneId,
    instance,
    presentation: {
      kind: 'objectPlacement',
      assetId: input.asset.assetId,
      assetContentHash: input.asset.contentHash,
      frameId: voxelObjectFrameId(frame),
      transform: { translation, rotation, scale },
      materialOverrides: materialOverrides.map((entry) => ({
        slot: entry.materialSlot,
        material: entry.materialAssetId,
      })),
      label: `Place ${instanceId}`,
    },
  };
}

export function duplicateVoxelObjectInstance(
  source: VoxelObjectInstanceReadout,
  existingInstanceIds: ReadonlySet<string>,
  translationStep: number,
): StoredVoxelObjectInstance {
  if (!Number.isFinite(translationStep) || translationStep <= 0) {
    throw new TypeError('Duplicate translation step must be finite and greater than zero.');
  }
  const instance = source.instance;
  return {
    instanceId: nextVoxelObjectInstanceId(`${instance.instanceId}-copy`, existingInstanceIds),
    voxelObjectAssetId: instance.voxelObjectAssetId,
    surfaceMode: instance.surfaceMode,
    frame: structuredClone(instance.frame),
    translation: [
      instance.translation[0] + translationStep,
      instance.translation[1],
      instance.translation[2],
    ],
    rotation: [...instance.rotation],
    scale: [...instance.scale],
    materialOverrides: instance.materialOverrides.map((entry) => ({ ...entry })),
  };
}

export function nextVoxelObjectInstanceId(
  requestedBase: string,
  existingInstanceIds: ReadonlySet<string>,
): string {
  if (existingInstanceIds.size >= MAX_VOXEL_OBJECT_PLACEMENTS_PER_SESSION) {
    throw new RangeError(
      `Voxel-object placement is limited to ${String(MAX_VOXEL_OBJECT_PLACEMENTS_PER_SESSION)} candidate identities per Studio session.`,
    );
  }
  const base = boundedIdentity(requestedBase, 'Voxel-object instance identity');
  if (!existingInstanceIds.has(base)) return base;
  const suffixMatch = /^(.*)-(\d+)$/u.exec(base);
  const parsedSuffix = suffixMatch === null ? null : Number(suffixMatch[2]);
  const root = suffixMatch !== null && Number.isSafeInteger(parsedSuffix) && (parsedSuffix ?? 0) >= 2
    ? suffixMatch[1] as string
    : base;
  const firstSuffix = parsedSuffix !== null && Number.isSafeInteger(parsedSuffix) && parsedSuffix >= 2
    ? parsedSuffix + 1
    : 2;
  for (
    let suffix = firstSuffix;
    suffix <= firstSuffix + MAX_VOXEL_OBJECT_PLACEMENTS_PER_SESSION;
    suffix += 1
  ) {
    const suffixText = `-${String(suffix)}`;
    const availableBytes = MAX_VOXEL_OBJECT_PLACEMENT_ID_BYTES - suffixText.length;
    const candidate = `${root.slice(0, availableBytes)}${suffixText}`;
    if (!existingInstanceIds.has(candidate)) return candidate;
  }
  throw new RangeError('No bounded voxel-object placement identity remains available.');
}

export function voxelObjectFrameId(frame: VoxelObjectFrameSelection): string {
  return frame.kind === 'default'
    ? 'default'
    : `${frame.clipId}/${String(frame.frameIndex)}`;
}

function selectedFrame(
  asset: VoxelObjectAssetAuthoringReadout,
  clipId: string,
  rawFrameIndex: number,
): VoxelObjectFrameSelection {
  if (clipId === '') return { kind: 'default' };
  const clip = asset.clips.find((candidate) => candidate.clipId === clipId);
  if (clip === undefined) throw new TypeError(`Unknown voxel-object clip ${clipId}.`);
  if (!Number.isSafeInteger(rawFrameIndex) || rawFrameIndex < 0 || rawFrameIndex >= clip.frames.length) {
    throw new RangeError(
      `Voxel-object frame must be an integer in 0..=${String(Math.max(0, clip.frames.length - 1))}.`,
    );
  }
  return { kind: 'clip', clipId, frameIndex: rawFrameIndex };
}

function validatedOverrides(
  input: VoxelObjectPlacementCandidateInput,
): StoredVoxelObjectInstance['materialOverrides'] {
  if (input.materialOverrides.length > MAX_VOXEL_OBJECT_PLACEMENT_MATERIAL_OVERRIDES) {
    throw new RangeError(
      `Voxel-object placement accepts at most ${String(MAX_VOXEL_OBJECT_PLACEMENT_MATERIAL_OVERRIDES)} material overrides.`,
    );
  }
  const slots = new Set(input.asset.materialPalette.map((entry) => entry.materialSlot));
  const seen = new Set<number>();
  return input.materialOverrides.map((entry) => {
    if (!Number.isSafeInteger(entry.materialSlot) || !slots.has(entry.materialSlot)) {
      throw new TypeError(`Material override slot ${String(entry.materialSlot)} is not bound by the selected object.`);
    }
    if (seen.has(entry.materialSlot)) {
      throw new TypeError(`Material override slot ${String(entry.materialSlot)} is duplicated.`);
    }
    seen.add(entry.materialSlot);
    const materialAssetId = boundedIdentity(entry.materialAssetId, 'Material override identity');
    if (!input.canonicalMaterialIds.has(materialAssetId)) {
      throw new TypeError(`Material override ${materialAssetId} is not a canonical project material.`);
    }
    return { materialSlot: entry.materialSlot, materialAssetId };
  });
}

function tuple<const Size extends 3 | 4>(
  values: readonly number[],
  size: Size,
  label: string,
): Size extends 3
  ? readonly [number, number, number]
  : readonly [number, number, number, number] {
  if (values.length !== size || values.some((value) => !Number.isFinite(value))) {
    throw new TypeError(`${label} must contain ${String(size)} finite values.`);
  }
  return [...values] as never;
}

function boundedIdentity(raw: string, label: string): string {
  const value = raw.trim();
  if (value.length === 0 || !/^[\x21-\x7e]+$/u.test(value)) {
    throw new TypeError(`${label} must contain printable ASCII without whitespace.`);
  }
  if (new TextEncoder().encode(value).byteLength > MAX_VOXEL_OBJECT_PLACEMENT_ID_BYTES) {
    throw new RangeError(
      `${label} exceeds ${String(MAX_VOXEL_OBJECT_PLACEMENT_ID_BYTES)} UTF-8 bytes.`,
    );
  }
  return value;
}
