export type RenderHandle = number & { readonly __brand: 'RenderHandle' };

export function assertJsonSafeUnsignedInteger(raw: number, label: string): number {
  if (!Number.isSafeInteger(raw) || raw < 0) {
    throw new RangeError(`${label} must be an unsigned JSON-safe integer`);
  }
  return raw;
}

export const renderHandle = (raw: number): RenderHandle =>
  assertJsonSafeUnsignedInteger(raw, 'render handle') as RenderHandle;

export type Vec2 = readonly [number, number];
export type Vec3 = readonly [number, number, number];
export type Vec4 = readonly [number, number, number, number];

export interface Transform {
  readonly translation: Vec3;
  readonly rotation: Vec4;
  readonly scale: Vec3;
}

export type Geometry =
  | { readonly kind: 'group' }
  | { readonly kind: 'cube' }
  | { readonly kind: 'sphere' }
  | { readonly kind: 'quad' }
  | { readonly kind: 'point' }
  | { readonly kind: 'line'; readonly a: Vec3; readonly b: Vec3 };

export interface Material {
  readonly color: Vec4;
  readonly wireframe: boolean;
}

/**
 * Retained composition channel.
 *
 * `viewmodel` is camera-relative presentation composed after world depth. It
 * carries no camera, input, picking, or gameplay authority.
 */
export type RenderLayer = 'scene' | 'debug' | 'ui' | 'viewmodel';

export interface RenderMetadata {
  readonly sourceEntity: number | null;
  readonly sourceSceneNode: number | null;
  readonly tags: readonly string[];
  readonly label: string | null;
}

export interface RenderNode {
  readonly geometry: Geometry;
  readonly material: Material;
  readonly transform: Transform;
  readonly visible: boolean;
  readonly layer: RenderLayer;
  readonly metadata: RenderMetadata;
}

export type MeshAttributeKind = 'f32';
export type MeshAttributeName = 'position' | 'normal' | 'uv' | 'color';

export interface MeshAttribute {
  readonly name: MeshAttributeName;
  readonly components: number;
  readonly kind: MeshAttributeKind;
}

export type MeshIndexWidth = 'u32';
export type MeshResourceEncoding = 'packedStreamsLeV1' | 'packedStreamsLeV2';

export interface MeshBufferLayout {
  readonly vertexCount: number;
  readonly indexCount: number;
  readonly indexWidth: MeshIndexWidth;
  readonly attributes: readonly MeshAttribute[];
}

export interface MeshGroupDescriptor {
  readonly materialSlot: number;
  readonly start: number;
  readonly count: number;
}

export interface MeshBoundsDescriptor {
  readonly min: Vec3;
  readonly max: Vec3;
}

export type MeshProvenance = 'voxelChunk' | 'voxelObject' | 'staticAsset' | 'generated' | 'debug';

export type MeshPayloadSource =
  | {
      readonly kind: 'inline';
      readonly positions: readonly number[];
      readonly normals: readonly number[];
      readonly uvs?: readonly number[];
      readonly indices: readonly number[];
    }
  | {
      readonly kind: 'sharedBuffer';
      readonly buffer: number;
      readonly positionsByteOffset: number;
      readonly normalsByteOffset: number;
      readonly uvsByteOffset?: number;
      readonly indicesByteOffset: number;
    }
  | {
      readonly kind: 'resource';
      readonly resource: string;
      readonly contentHash: string;
      readonly byteLength: number;
      readonly encoding: MeshResourceEncoding;
      readonly positionsByteOffset: number;
      readonly normalsByteOffset: number;
      readonly uvsByteOffset?: number;
      readonly indicesByteOffset: number;
    };

export interface MeshPayloadDescriptor {
  readonly layout: MeshBufferLayout;
  readonly groups: readonly MeshGroupDescriptor[];
  readonly bounds: MeshBoundsDescriptor;
  readonly source: MeshPayloadSource;
  readonly provenance: MeshProvenance;
}

export interface MeshMaterialSlot {
  readonly slot: number;
  readonly material: string;
}

export type MeshCollisionPolicy =
  | { readonly kind: 'visualOnly' }
  | { readonly kind: 'proxy'; readonly proxyAsset: string }
  | { readonly kind: 'aabbFallback' }
  | { readonly kind: 'trimesh' };

export interface StaticMeshAsset {
  readonly asset: string;
  readonly payload: MeshPayloadDescriptor;
  readonly materialSlots: readonly MeshMaterialSlot[];
  readonly collision: MeshCollisionPolicy;
}

export interface StaticMeshInstanceDescriptor {
  readonly asset: string;
  readonly transform: Transform;
  readonly visible: boolean;
  readonly materialOverrides: readonly MeshMaterialSlot[];
  readonly metadata: RenderMetadata;
}

export type AnimatedMeshRuntimeFormat = 'glb';
export type AnimationLoopMode = 'once' | 'repeat' | 'pingPong';

export interface AnimationClipDescriptor {
  readonly id: string;
  readonly name: string | null;
  readonly durationSeconds: number | null;
}

export interface AnimatedMeshAsset {
  readonly asset: string;
  readonly runtimeFormat: AnimatedMeshRuntimeFormat;
  readonly contentHash: string | null;
  readonly clips: readonly AnimationClipDescriptor[];
  readonly defaultClip: string | null;
  readonly materialSlots: readonly MeshMaterialSlot[];
  readonly bounds: MeshBoundsDescriptor;
}

export type AnimatedMeshPlaybackCommand =
  | {
      readonly kind: 'play';
      readonly clip: string;
      readonly loop: AnimationLoopMode;
      readonly speed: number;
      readonly weight: number;
      readonly restart: boolean;
      readonly fadeSeconds: number | null;
    }
  | { readonly kind: 'stop'; readonly fadeSeconds: number | null }
  | { readonly kind: 'pause' }
  | { readonly kind: 'resume' };

export interface AnimatedMeshInstanceDescriptor {
  readonly asset: string;
  readonly transform: Transform;
  readonly visible: boolean;
  readonly materialOverrides: readonly MeshMaterialSlot[];
  readonly playback: AnimatedMeshPlaybackCommand | null;
  readonly metadata: RenderMetadata;
}

export interface VoxelObjectRenderMesh {
  readonly payload: MeshPayloadDescriptor;
}

export interface VoxelObjectRenderFrame {
  readonly id: string;
  readonly mesh: number;
}

/** Presentation-only frame resources. Collision and navigation are not renderer concerns. */
export interface VoxelObjectRenderAsset {
  readonly asset: string;
  readonly contentHash: string;
  readonly meshes: readonly VoxelObjectRenderMesh[];
  readonly frames: readonly VoxelObjectRenderFrame[];
  readonly materialSlots: readonly MeshMaterialSlot[];
}

export interface VoxelObjectInstanceDescriptor {
  readonly asset: string;
  readonly frame: number;
  readonly transform: Transform;
  readonly visible: boolean;
  readonly materialOverrides: readonly MeshMaterialSlot[];
  readonly metadata: RenderMetadata;
}

export type TextureFilter = 'nearest' | 'linear';
export type TextureWrap = 'clamp' | 'repeat';
export type TextureEncoding = 'pngRgba8';
export type TextureColorSpace = 'srgb' | 'linear';

export type TexturePayloadSource =
  | { readonly kind: 'inline'; readonly encodedBytes: readonly number[] }
  | { readonly kind: 'resource'; readonly resource: string };

export interface TexturePayloadDescriptor {
  readonly encoding: TextureEncoding;
  readonly colorSpace: TextureColorSpace;
  readonly contentHash: string;
  readonly byteLength: number;
  readonly source: TexturePayloadSource;
}

export interface TextureDescriptor {
  readonly id: string;
  readonly width: number;
  readonly height: number;
  readonly filter: TextureFilter;
  readonly wrap: TextureWrap;
  readonly contentHash: string | null;
  readonly version: number;
  readonly payload?: TexturePayloadDescriptor;
}

export interface SpriteFrameRect {
  readonly frame: number;
  /**
   * Inclusive normalized minimum in decoded PNG image space. The image origin
   * is its top-left, U increases right, and V increases down.
   */
  readonly uvMin: Vec2;
  /** Exclusive normalized maximum in the same top-left image space. */
  readonly uvMax: Vec2;
  readonly size?: Vec2;
}

export interface SpriteAtlasDescriptor {
  readonly id: string;
  readonly texture: string;
  readonly frames: readonly SpriteFrameRect[];
}

export type MaterialUvStrategy = 'flat' | 'planar' | 'atlas';

export type VoxelSurfaceAlphaMode =
  | { readonly kind: 'opaque' }
  | { readonly kind: 'mask'; readonly cutoff: number }
  | { readonly kind: 'blend' };

export interface VoxelAtlasPaddingDescriptor {
  readonly left: number;
  readonly right: number;
  readonly bottom: number;
  readonly top: number;
}

export interface VoxelAtlasRegionDescriptor {
  readonly id: string;
  readonly contentMin: readonly [number, number];
  readonly contentExtent: readonly [number, number];
  readonly padding: VoxelAtlasPaddingDescriptor;
  readonly inset: 'halfTexel';
}

export type VoxelSurfaceMappingDescriptor =
  | {
      readonly kind: 'repeat';
      readonly texture: string;
      readonly textureVersion: number;
      readonly textureContentHash: string;
      readonly tileScaleCells: Vec2;
      readonly tileOriginCells: Vec2;
    }
  | {
      readonly kind: 'atlas';
      readonly atlas: string;
      readonly atlasVersion: number;
      readonly atlasContentHash: string;
      readonly texture: string;
      readonly textureVersion: number;
      readonly textureContentHash: string;
      readonly region: VoxelAtlasRegionDescriptor;
      readonly tileScaleCells: Vec2;
      readonly tileOriginCells: Vec2;
    };

export interface VoxelSurfaceDescriptor {
  readonly schemaVersion: 1;
  readonly filter: 'nearest' | 'linear';
  readonly wrap: 'clamp' | 'repeat';
  readonly alphaMode: VoxelSurfaceAlphaMode;
  readonly mapping: VoxelSurfaceMappingDescriptor;
}

export interface RenderMaterialDescriptor {
  readonly schemaVersion: number;
  readonly id: string;
  readonly color: Vec4;
  readonly texture: string | null;
  readonly roughness: number;
  readonly textureTint: Vec4;
  readonly emissionColor: Vec3;
  readonly emissionIntensity: number;
  readonly uvStrategy: MaterialUvStrategy;
  readonly voxelSurface?: VoxelSurfaceDescriptor;
}

export interface MaterialInstanceParameters {
  readonly textureTint: Vec4;
  readonly emissionColor: Vec3;
  readonly emissionIntensity: number;
}

export type SpriteSizeMode = 'world' | 'pixel';
export type BillboardMode = 'none' | 'spherical' | 'cylindrical';
export type SpriteDepthPolicy = 'default' | 'depthTestOff' | 'depthWriteOff';
export type SpriteShading = 'unlit' | 'lit' | 'shadowed' | 'custom';
export type SpriteLightingMode =
  | 'unlit'
  | 'authoredNormal'
  | 'authoredDepth'
  | 'derivedGradient'
  | 'synthetic';
export type SpriteAlphaMode =
  | { readonly kind: 'opaque' }
  | { readonly kind: 'mask'; readonly cutoff: number }
  | { readonly kind: 'blend' };
export type SpriteShadowPolicy = 'none' | 'cast' | 'receive' | 'castAndReceive';

export interface SpriteMaterialDescriptor {
  readonly lighting: SpriteLightingMode;
  readonly normalTexture: string | null;
  readonly depthTexture: string | null;
  readonly normalStrength: number;
  readonly normalBias: number;
  readonly alpha: SpriteAlphaMode;
  readonly shadow: SpriteShadowPolicy;
}

export interface SpriteAttachment {
  readonly sourceEntity: number | null;
  readonly sourceSceneNode: number | null;
  readonly attachmentPoint: string | null;
}

export interface SpriteInstanceDescriptor {
  readonly asset: string;
  readonly frame: number;
  readonly pivot: Vec2;
  readonly size: Vec2;
  readonly sizeMode: SpriteSizeMode;
  readonly billboard: BillboardMode;
  readonly tint: Vec4;
  readonly renderOrder: number;
  readonly depth: SpriteDepthPolicy;
  readonly shading: SpriteShading;
  /** Omitted legacy descriptors resolve from `shading`; new writers use this bounded shape. */
  readonly material?: SpriteMaterialDescriptor;
  readonly visible: boolean;
  readonly transform: Transform;
  readonly attachment: SpriteAttachment;
  readonly metadata: RenderMetadata;
}

export interface SpritePickHit {
  readonly handle: RenderHandle;
  readonly sourceEntity: number | null;
  readonly sourceSceneNode: number | null;
  readonly asset: string;
  readonly attachmentPoint: string | null;
}

export interface MeshPickHit {
  readonly handle: RenderHandle;
  readonly provenance: MeshProvenance;
  readonly sourceEntity: number | null;
  readonly sourceSceneNode: number | null;
}

export type LightShadowIntent = 'disabled' | 'requested';

/** Maximum retained light intensity accepted by every renderer boundary. */
export const MAX_RENDER_LIGHT_INTENSITY = 10_000;

export type LightDescriptor =
  | {
      readonly kind: 'ambient';
      readonly color: Vec3;
      readonly intensity: number;
      readonly enabled: boolean;
      readonly shadowIntent: LightShadowIntent;
    }
  | {
      readonly kind: 'directional';
      readonly color: Vec3;
      readonly intensity: number;
      readonly enabled: boolean;
      readonly direction: Vec3;
      readonly shadowIntent: LightShadowIntent;
    }
  | {
      readonly kind: 'point';
      readonly color: Vec3;
      readonly intensity: number;
      readonly enabled: boolean;
      readonly position: Vec3;
      readonly range: number | null;
      readonly decay: number;
      readonly shadowIntent: LightShadowIntent;
    }
  | {
      readonly kind: 'spot';
      readonly color: Vec3;
      readonly intensity: number;
      readonly enabled: boolean;
      readonly position: Vec3;
      readonly direction: Vec3;
      readonly range: number | null;
      readonly decay: number;
      readonly outerAngleRadians: number;
      readonly penumbra: number;
      readonly shadowIntent: LightShadowIntent;
    };

export type RenderDiff =
  | { readonly op: 'create'; readonly handle: RenderHandle; readonly parent: RenderHandle | null; readonly node: RenderNode }
  | { readonly op: 'update'; readonly handle: RenderHandle; readonly transform: Transform | null; readonly material: Material | null; readonly visible: boolean | null; readonly metadata: RenderMetadata | null }
  | { readonly op: 'destroy'; readonly handle: RenderHandle }
  | { readonly op: 'replaceMeshPayload'; readonly handle: RenderHandle; readonly payload: MeshPayloadDescriptor }
  | { readonly op: 'createLight'; readonly handle: RenderHandle; readonly parent: RenderHandle | null; readonly light: LightDescriptor }
  | { readonly op: 'updateLight'; readonly handle: RenderHandle; readonly light: LightDescriptor }
  | { readonly op: 'defineMaterial'; readonly material: RenderMaterialDescriptor }
  | { readonly op: 'setMaterialInstanceParameters'; readonly handle: RenderHandle; readonly slot: number; readonly parameters: MaterialInstanceParameters | null }
  | { readonly op: 'defineTexture'; readonly texture: TextureDescriptor }
  | { readonly op: 'defineSpriteAtlas'; readonly atlas: SpriteAtlasDescriptor }
  | { readonly op: 'defineStaticMesh'; readonly asset: StaticMeshAsset }
  | { readonly op: 'defineAnimatedMesh'; readonly asset: AnimatedMeshAsset }
  | { readonly op: 'defineVoxelObject'; readonly asset: VoxelObjectRenderAsset }
  | { readonly op: 'releaseVoxelObject'; readonly asset: string }
  | { readonly op: 'createStaticMeshInstance'; readonly handle: RenderHandle; readonly parent: RenderHandle | null; readonly instance: StaticMeshInstanceDescriptor }
  | { readonly op: 'createAnimatedMeshInstance'; readonly handle: RenderHandle; readonly parent: RenderHandle | null; readonly instance: AnimatedMeshInstanceDescriptor }
  | { readonly op: 'setAnimatedMeshPlayback'; readonly handle: RenderHandle; readonly playback: AnimatedMeshPlaybackCommand }
  | { readonly op: 'createVoxelObjectInstance'; readonly handle: RenderHandle; readonly parent: RenderHandle | null; readonly instance: VoxelObjectInstanceDescriptor }
  | { readonly op: 'setVoxelObjectFrame'; readonly handle: RenderHandle; readonly frame: number }
  | { readonly op: 'createSprite'; readonly handle: RenderHandle; readonly parent: RenderHandle | null; readonly sprite: SpriteInstanceDescriptor }
  | { readonly op: 'updateSprite'; readonly handle: RenderHandle; readonly frame: number | null; readonly tint: Vec4 | null; readonly renderOrder: number | null; readonly visible: boolean | null };

export interface RenderFrameDiff {
  readonly schemaVersion: 1;
  readonly publication?: RenderFramePublication;
  readonly ops: readonly RenderDiff[];
}

export interface RenderFramePublication {
  readonly stream: string;
  readonly baseRevision: number;
  readonly revision: number;
  readonly operationCount: number;
}

export type RenderAssetKind =
  | 'material'
  | 'texture'
  | 'sprite'
  | 'spriteAtlas'
  | 'staticMesh'
  | 'animatedMesh'
  | 'voxelObject'
  | 'audio'
  | 'font';

export interface ResolvedRenderAsset {
  readonly id: string;
  readonly kind: RenderAssetKind;
  readonly contentHash: string | null;
  readonly version: number;
}

export type SpatialGridCoordinateSystem = 'rightHandedYUp';
export type EditorGridPlane = 'xz' | 'xy' | 'yz';
export type SpatialGridSnapAnchor = 'boundary' | 'cellCenter';

export interface SpatialGridSpec {
  readonly coordinateSystem: SpatialGridCoordinateSystem;
  readonly origin: Vec3;
  readonly spacing: Vec3;
}

export interface EditorGridStyle {
  readonly minorColor: Vec4;
  readonly majorColor: Vec4;
  readonly xAxisColor: Vec4;
  readonly yAxisColor: Vec4;
  readonly zAxisColor: Vec4;
  readonly majorLineEvery: number;
  readonly opacity: number;
  readonly fadeStart: number;
  readonly fadeEnd: number;
}

export interface EditorGridDescriptor {
  readonly visible: boolean;
  readonly grid: SpatialGridSpec;
  readonly plane: EditorGridPlane;
  readonly snapAnchor: SpatialGridSnapAnchor;
  readonly style: EditorGridStyle;
}

export interface EditorGridBounds {
  readonly min: Vec3;
  readonly max: Vec3;
}

export interface EditorGridProjectionReadout {
  readonly descriptor: EditorGridDescriptor;
  readonly bounds: EditorGridBounds | null;
  readonly minorLineStep: number;
  readonly renderedLineCount: number;
}

/** Renderer-facing camera values shared by browser and editor hosts. */
export interface CameraPose {
  readonly position: Vec3;
  readonly yawDegrees: number;
  readonly pitchDegrees: number;
}

export interface CameraBasis {
  readonly forward: Vec3;
  readonly right: Vec3;
  readonly up: Vec3;
}

export interface PerspectiveProjection {
  readonly fovYDegrees: number;
  readonly near: number;
  readonly far: number;
}
