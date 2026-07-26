import {
  ChangeDetectionStrategy,
  Component,
  DestroyRef,
  effect,
  inject,
  input,
  output,
  signal,
} from '@angular/core';
import { JsonPipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import type {
  MaterialAssetReadout,
  StoredMaterialDefinition,
  TextureMaterialBinding,
  TextureSampleAsset,
  VoxelAnnotationKind,
  VoxelAnnotationEditCommand,
  VoxelAnnotationQueryMode,
  VoxelAssetAuthoringReadout,
  VoxelAuthoringReadout,
  VoxelConversionPlan,
  VoxelConversionPreview,
  VoxelConversionSettings,
  VoxelHistoryRevertPreview,
  VoxelInstanceReadout,
  VoxelObjectAssetAuthoringReadout,
  VoxelObjectAuthoringReadout,
  VoxelObjectConversionPlan,
  VoxelObjectConversionPreview,
  VoxelObjectFrameSelection,
  VoxelObjectSourceInspection,
  VoxelPickReadout,
  VoxelReadout,
} from '@rusty-engine/studio-adapter-client';

import type {
  VoxelBrushPreviewPresentation,
  VoxelObjectClipControlOutput,
  VoxelEditorAction,
  VoxelHostPathChooser,
} from './voxel-editor-model.js';
import { buildVoxelObjectClipControlForSource } from './voxel-editor-model.js';

type EditorTab = 'assets' | 'edit' | 'annotations' | 'convert';

@Component({
  selector: 'rusty-voxel-editor',
  standalone: true,
  imports: [FormsModule, JsonPipe],
  templateUrl: './voxel-editor.component.html',
  styleUrl: './voxel-editor.component.css',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class VoxelEditorComponent {
  readonly authoring = input<VoxelAuthoringReadout | null>(null);
  readonly objectAuthoring = input<VoxelObjectAuthoringReadout | null>(null);
  readonly entryScene = input('');
  readonly validatedPick = input<VoxelPickReadout | null>(null);
  readonly lastReadout = input<VoxelReadout | null>(null);
  readonly conversion = input<{
    readonly plan: VoxelConversionPlan;
    readonly preview: VoxelConversionPreview;
  } | null>(null);
  readonly objectSourceInspection = input<VoxelObjectSourceInspection | null>(null);
  readonly objectConversion = input<{
    readonly plan: VoxelObjectConversionPlan;
    readonly preview: VoxelObjectConversionPreview;
  } | null>(null);
  readonly historyPreview = input<VoxelHistoryRevertPreview | null>(null);
  readonly busy = input(false);
  readonly chooseHostPath = input<VoxelHostPathChooser>(async () => null);
  readonly action = output<VoxelEditorAction>();
  readonly previewChange = output<VoxelBrushPreviewPresentation | null>();

  readonly tab = signal<EditorTab>('assets');
  readonly brushPreview = signal(false);
  readonly formError = signal<string | null>(null);
  readonly objectPlaying = signal(false);
  readonly #destroyRef = inject(DestroyRef);
  #playbackTimer: ReturnType<typeof setTimeout> | null = null;

  selectedAssetId = '';
  selectedInstanceId = '';
  selectedLayerId = '';

  materialId = 'material/studio-accent';
  materialColor = '#e58b42';
  materialRoughness = 0.8;
  materialEmissive = 0;

  newAssetId = 'voxel-volume/studio-volume';
  duplicateAssetId = 'voxel-volume/studio-volume-copy';
  newCellSize = 1;
  newChunkSize = 16;
  newSizeX = 4;
  newSizeY = 4;
  newSizeZ = 4;
  newMaterialSlot = 7;
  newMaterialId = 'material/wall-lines';

  newInstanceId = 'voxel-instance';
  instanceTranslation = [0, 0, 0];
  instanceRotation = [0, 0, 0, 1];
  instanceScale = [1, 1, 1];

  brushMode: 'paint' | 'erase' = 'paint';
  brushRadius = 0;
  brushMaterialSlot = 7;
  revertCursor = 0;
  historyMaxEntries = 128;
  historyMaxDeltas = 256;
  historyMaxSamples = 256;

  primitiveKind: 'block' | 'box' | 'line' = 'block';
  primitiveStart = [0, 0, 0];
  primitiveEnd = [0, 0, 0];
  primitiveFill: 'filled' | 'shell' | 'edges' = 'filled';
  primitiveRadius = 0;
  primitiveMaterialMode: 'set' | 'clear' = 'set';
  primitiveMaterialSlot = 7;

  templateAssetId = 'voxel-volume/house-template';
  templateOrigin = [0, 0, 0];
  templateMaterialSlot = 7;

  importSourcePath = '/tmp/rusty-engine-import.voxel.json';
  importTargetAssetId = 'voxel-volume/imported';
  exportTargetPath = '/tmp/rusty-engine-export.voxel.json';
  exportExpectedSha256 = '';

  environmentSeed = 1;
  environmentAssetId = 'voxel-volume/tiny-enclosed';
  environmentInstanceId = 'environment/tiny-enclosed';
  environmentTranslation = [0, 0, 0];
  environmentPlayerEntityId = 1;
  environmentExitEntityId = 2;
  environmentWallMaterial = 7;
  environmentFloorMaterial = 8;
  environmentAccentMaterial = 9;
  environmentWallMaterialId = 'material/wall-lines';
  environmentFloorMaterialId = 'material/concrete';
  environmentAccentMaterialId = 'material/wall-lines';

  annotationLayerId = 'voxel-annotation/studio-semantics';
  annotationRegionId = 'region/studio-selection';
  annotationLabel = 'Studio selection';
  annotationKind: VoxelAnnotationKind = 'selection';
  annotationTags = 'authored,studio';
  annotationParentRegionId = '';
  annotationCommand: 'upsertRegion' | 'removeRegion' | 'addRuns' | 'removeRuns'
    | 'replaceSelection' | 'setParent' | 'setTags' | 'setLabel' | 'setKind'
    | 'setBounds' = 'setLabel';
  annotationQueryMode: 'cell' | 'bounds' | 'region' | 'layerSummary' = 'layerSummary';
  annotationStart = [0, 0, 0];
  annotationEnd = [0, 0, 0];
  annotationRunLength = 1;
  annotationMaxResults = 256;

  conversionSourceAsset = 'mesh/kenney-wall-a';
  conversionSourcePath = 'fixtures/voxel-conversion/kenney-wall-a.glb';
  conversionTargetAsset = 'voxel-volume/converted-studio';
  conversionLicensePath = 'fixtures/voxel-conversion/KENNEY-RETRO-URBAN-KIT-LICENSE.txt';
  conversionSourceScope: 'project' | 'host' = 'project';
  conversionLicenseScope: 'project' | 'host' = 'project';
  conversionMeshPrimitive = '';
  conversionResolution = [4, 3, 2];
  conversionCellSize = 1;
  conversionChunkSize = 16;
  conversionOrigin = [0, 0, 0];
  conversionFitPolicy: 'contain' | 'cover' | 'stretch' = 'contain';
  conversionOriginPolicy: 'sourceOrigin' | 'targetMin' | 'centered' = 'targetMin';
  conversionMode: 'surface' | 'solid' = 'surface';
  conversionTransform = '1,0,0,0, 0,1,0,0, 0,0,1,0, 0,0,0,1';
  conversionDefaultMaterial = '';
  conversionTextureAssets = '[]';
  conversionTextureBindings = '[]';
  conversionMaxPreviewSamples = 256;
  conversionTarget: 'volume' | 'object' = 'volume';
  objectSourceKind: 'static' | 'animated' = 'static';
  objectTargetAsset = 'voxel-object/converted-studio';
  objectPivot = [0, 0, 0];
  objectAnchorPolicy: 'preserveSourceSpace' | 'lockNodeToBindPose' = 'preserveSourceSpace';
  objectAnchorNode = 0;
  objectSelectedClips: string[] = [];
  objectSampleRateHz = 12;
  objectStartSeconds = 0;
  objectEndSeconds = '';
  objectEndPolicy: 'includeClipEnd' | 'excludeLoopSeam' = 'excludeLoopSeam';
  objectDefaultClip = '';
  objectPreviewClip = '';
  objectPreviewFrame = 0;
  objectSelectedAssetId = '';
  objectInstanceId = 'voxel-object-instance';
  objectInstanceTranslation = [0, 0, 0];
  objectInstanceRotation = [0, 0, 0, 1];
  objectInstanceScale = [1, 1, 1];
  objectInstanceClip = '';
  objectInstanceFrame = 0;

  constructor() {
    effect(() => {
      const pick = this.validatedPick();
      if (pick === null) return;
      this.selectedAssetId = pick.assetId;
      this.selectedInstanceId = pick.instanceId;
    });
    effect(() => {
      const inspection = this.objectSourceInspection();
      if (inspection === null) return;
      const names = inspection.clips.map((clip) => clip.name);
      this.objectSelectedClips = this.objectSelectedClips.filter((name) => names.includes(name));
      if (inspection.sourceKind === 'animated' && this.objectSelectedClips.length === 0) {
        this.objectSelectedClips = names;
      }
      const selected = this.objectSelectedClips[0] ?? '';
      if (!names.includes(this.objectDefaultClip)) this.objectDefaultClip = selected;
      if (!names.includes(this.objectPreviewClip)) this.objectPreviewClip = selected;
    });
    effect(() => {
      const selection = this.objectConversion()?.preview.selectedFrame.selection;
      if (selection?.kind !== 'clip') return;
      this.objectPreviewClip = selection.clipId;
      this.objectPreviewFrame = selection.frameIndex;
    });
    this.#destroyRef.onDestroy(() => this.pauseObjectPreview());
  }

  setTab(tab: EditorTab): void {
    if (tab !== 'edit') this.cancelBrushPreview();
    this.tab.set(tab);
  }

  assets(): readonly VoxelAssetAuthoringReadout[] {
    return this.authoring()?.assets ?? [];
  }

  instances(): readonly VoxelInstanceReadout[] {
    return this.authoring()?.instances ?? [];
  }

  materials(): readonly MaterialAssetReadout[] {
    return this.authoring()?.materials ?? [];
  }

  selectedAsset(): VoxelAssetAuthoringReadout | null {
    const assets = this.assets();
    return assets.find((asset) => asset.inspection.assetId === this.selectedAssetId)
      ?? assets[0]
      ?? null;
  }

  selectedInstance(): VoxelInstanceReadout | null {
    const instances = this.instances();
    return instances.find((entry) => entry.instance.instanceId === this.selectedInstanceId)
      ?? instances[0]
      ?? null;
  }

  selectedLayer() {
    const asset = this.selectedAsset();
    return asset?.annotations.find((layer) => layer.layerId === this.selectedLayerId)
      ?? asset?.annotations[0]
      ?? null;
  }

  chooseAsset(assetId: string): void {
    this.selectedAssetId = assetId;
    this.selectedLayerId = '';
    const asset = this.selectedAsset();
    const firstSlot = asset?.palette[0]?.materialSlot;
    if (firstSlot !== undefined) this.brushMaterialSlot = firstSlot;
  }

  chooseInstance(instanceId: string): void {
    this.selectedInstanceId = instanceId;
    const instance = this.selectedInstance()?.instance;
    if (instance === undefined) return;
    this.instanceTranslation = [...instance.translation];
    this.instanceRotation = [...instance.rotation];
    this.instanceScale = [...instance.scale];
  }

  upsertMaterial(): void {
    this.action.emit({
      kind: 'upsertMaterial',
      assetId: this.materialId,
      definition: materialDefinition(this.materialColor, this.materialRoughness, this.materialEmissive),
    });
  }

  initializeAsset(): void {
    const slot = integer(this.newMaterialSlot, 1);
    this.action.emit({
      kind: 'initializeAsset',
      assetId: this.newAssetId,
      cellSize: positive(this.newCellSize, 1),
      chunkSize: integer(this.newChunkSize, 16),
      origin: [0, 0, 0],
      bounds: {
        min: [0, 0, 0],
        max: [
          integer(this.newSizeX, 1) - 1,
          integer(this.newSizeY, 1) - 1,
          integer(this.newSizeZ, 1) - 1,
        ],
      },
      materialPalette: [{
        materialSlot: slot,
        materialAssetId: this.newMaterialId,
        displayName: this.newMaterialId.split('/').at(-1) ?? this.newMaterialId,
      }],
      initialMaterialSlot: slot,
    });
  }

  duplicateAsset(): void {
    const asset = this.selectedAsset();
    if (asset === null) return;
    this.action.emit({
      kind: 'duplicateAsset',
      sourceAssetId: asset.inspection.assetId,
      expectedSourceContentHash: asset.inspection.contentHash,
      targetAssetId: this.duplicateAssetId,
    });
  }

  attachInstance(): void {
    const asset = this.selectedAsset();
    if (asset === null) return;
    this.action.emit({
      kind: 'attachInstance',
      sceneId: this.entryScene(),
      instance: {
        instanceId: this.newInstanceId,
        voxelAssetId: asset.inspection.assetId,
        translation: [0, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
    });
  }

  commitInstanceTransform(): void {
    const selected = this.selectedInstance();
    if (selected === null) return;
    this.action.emit({
      kind: 'setInstanceTransform',
      sceneId: selected.sceneId,
      instanceId: selected.instance.instanceId,
      translation: tuple3(this.instanceTranslation),
      rotation: tuple4(this.instanceRotation),
      scale: tuple3(this.instanceScale),
    });
  }

  removeInstance(): void {
    const selected = this.selectedInstance();
    if (selected === null) return;
    this.action.emit({
      kind: 'removeInstance',
      sceneId: selected.sceneId,
      instanceId: selected.instance.instanceId,
    });
  }

  replacePalette(): void {
    const asset = this.selectedAsset();
    if (asset === null) return;
    this.action.emit({
      kind: 'replacePalette',
      assetId: asset.inspection.assetId,
      expectedAssetContentHash: asset.inspection.contentHash,
      expectedVoxelDataHash: asset.inspection.voxelDataHash,
      replacement: [{
        materialSlot: integer(this.newMaterialSlot, 1),
        materialAssetId: this.newMaterialId,
        displayName: this.newMaterialId.split('/').at(-1) ?? this.newMaterialId,
      }],
    });
  }

  initializeTemplate(): void {
    const slot = integer(this.templateMaterialSlot, 1);
    this.action.emit({
      kind: 'initializeTemplate',
      assetId: this.templateAssetId.trim(),
      cellSize: positive(this.newCellSize, 1),
      chunkSize: integer(this.newChunkSize, 16),
      materialPalette: [{
        materialSlot: slot,
        materialAssetId: this.newMaterialId,
        displayName: this.newMaterialId.split('/').at(-1) ?? this.newMaterialId,
      }],
      request: {
        template: 'house',
        origin: tuple3i(this.templateOrigin),
        materialSlot: slot,
      },
    });
  }

  importAssetFile(): void {
    this.action.emit({
      kind: 'importAssetFile',
      sourcePath: this.importSourcePath.trim(),
      targetAssetId: this.importTargetAssetId.trim(),
    });
  }

  exportAssetFile(): void {
    const asset = this.selectedAsset();
    if (asset === null) return;
    this.action.emit({
      kind: 'exportAssetFile',
      assetId: asset.inspection.assetId,
      expectedAssetContentHash: asset.inspection.contentHash,
      targetPath: this.exportTargetPath.trim(),
      ...(this.exportExpectedSha256.trim() === ''
        ? {}
        : { expectedTargetSha256: this.exportExpectedSha256.trim() }),
    });
  }

  materializeEnvironment(): void {
    const slots = [
      integer(this.environmentWallMaterial, 1),
      integer(this.environmentFloorMaterial, 1),
      integer(this.environmentAccentMaterial, 1),
    ];
    this.action.emit({
      kind: 'materializeEnvironment',
      sceneId: this.entryScene(),
      preset: 'tinyEnclosed',
      seed: integer(this.environmentSeed, 1),
      voxelAssetId: this.environmentAssetId.trim(),
      voxelInstanceId: this.environmentInstanceId.trim(),
      voxelTranslation: tuple3(this.environmentTranslation),
      playerEntityId: integer(this.environmentPlayerEntityId, 1),
      exitEntityId: integer(this.environmentExitEntityId, 2),
      wallMaterial: slots[0] ?? 1,
      floorMaterial: slots[1] ?? 1,
      accentMaterial: slots[2] ?? 1,
      materialPalette: [
        { materialSlot: slots[0] ?? 1, materialAssetId: this.environmentWallMaterialId.trim() },
        { materialSlot: slots[1] ?? 1, materialAssetId: this.environmentFloorMaterialId.trim() },
        { materialSlot: slots[2] ?? 1, materialAssetId: this.environmentAccentMaterialId.trim() },
      ],
    });
  }

  previewBrush(): void {
    const pick = this.validatedPick();
    const asset = this.selectedAsset();
    if (pick === null || asset === null) return;
    this.brushPreview.set(true);
    this.previewChange.emit({
      kind: 'brush',
      transform: this.brushMode === 'paint'
        ? pick.placePreviewTransform
        : pick.hitPreviewTransform,
      radius: Math.max(0, integer(this.brushRadius, 0)),
      mode: this.brushMode,
    });
  }

  cancelBrushPreview(): void {
    this.brushPreview.set(false);
    this.previewChange.emit(null);
  }

  browseImportSource(): void {
    void this.chooseHostPath()({
      kind: 'file',
      title: 'Open voxel asset file',
      initialPath: this.importSourcePath,
      extensions: ['.json'],
    }).then((path) => { if (path !== null) this.importSourcePath = path; });
  }

  browseExportTarget(): void {
    void this.chooseHostPath()({
      kind: 'directory',
      title: 'Choose voxel export directory',
      initialPath: hostParent(this.exportTargetPath),
    }).then((path) => {
      if (path !== null) this.exportTargetPath = `${path.replace(/\/+$/, '')}/voxel-export.avxl.json`;
    });
  }

  browseConversionSource(): void {
    void this.chooseHostPath()({
      kind: 'file',
      title: 'Choose mesh conversion source',
      initialPath: this.conversionSourcePath,
      extensions: ['.glb', '.gltf', '.json'],
    }).then((path) => { if (path !== null) this.conversionSourcePath = path; });
  }

  browseConversionLicense(): void {
    void this.chooseHostPath()({
      kind: 'file',
      title: 'Choose conversion license',
      initialPath: this.conversionLicensePath,
      extensions: ['.txt', '.md', '.license'],
    }).then((path) => { if (path !== null) this.conversionLicensePath = path; });
  }

  applyBrush(): void {
    const asset = this.selectedAsset();
    const pick = this.validatedPick();
    if (asset === null || pick === null || pick.assetId !== asset.inspection.assetId) return;
    this.action.emit({
      kind: 'applyBrush',
      assetId: asset.inspection.assetId,
      expectedAssetContentHash: asset.inspection.contentHash,
      center: this.brushMode === 'paint' ? pick.placeVoxel : pick.hitVoxel,
      radius: integer(this.brushRadius, 0),
      mode: this.brushMode,
      materialSlot: this.brushMode === 'paint' ? integer(this.brushMaterialSlot, 1) : null,
    });
    this.brushPreview.set(false);
    this.previewChange.emit(null);
  }

  applyPrimitive(): void {
    const asset = this.selectedAsset();
    if (asset === null) return;
    const start = tuple3i(this.primitiveStart);
    const primitive = this.primitiveKind === 'block'
      ? { kind: 'block' as const, address: start }
      : this.primitiveKind === 'box'
        ? {
            kind: 'box' as const,
            start,
            end: tuple3i(this.primitiveEnd),
            fill: this.primitiveFill,
          }
        : {
            kind: 'line' as const,
            start,
            end: tuple3i(this.primitiveEnd),
            radius: integer(this.primitiveRadius, 0),
          };
    this.action.emit({
      kind: 'applyPrimitive',
      assetId: asset.inspection.assetId,
      expectedAssetContentHash: asset.inspection.contentHash,
      request: {
        primitive,
        material: this.primitiveMaterialMode === 'clear'
          ? { kind: 'clear' }
          : { kind: 'set', materialSlot: integer(this.primitiveMaterialSlot, 1) },
      },
    });
  }

  history(kind: 'undo' | 'redo' | 'revert'): void {
    const asset = this.selectedAsset();
    if (asset === null) return;
    const common = {
      assetId: asset.inspection.assetId,
      expectedAssetContentHash: asset.inspection.contentHash,
    };
    if (kind === 'revert') {
      this.action.emit({ ...common, kind, targetCursor: integer(this.revertCursor, 0) });
    } else {
      this.action.emit({ ...common, kind });
    }
  }

  queryHistory(): void {
    const asset = this.selectedAsset();
    if (asset === null) return;
    this.action.emit({
      kind: 'queryHistory',
      assetId: asset.inspection.assetId,
      expectedAssetContentHash: asset.inspection.contentHash,
      maxEntries: integer(this.historyMaxEntries, 128),
      maxDeltasPerEntry: integer(this.historyMaxDeltas, 256),
    });
  }

  prepareHistoryRevert(): void {
    const asset = this.selectedAsset();
    if (asset === null) return;
    this.action.emit({
      kind: 'prepareHistoryRevert',
      assetId: asset.inspection.assetId,
      expectedAssetContentHash: asset.inspection.contentHash,
      targetCursor: integer(this.revertCursor, 0),
      maxSamples: integer(this.historyMaxSamples, 256),
    });
  }

  applyHistoryRevert(): void {
    const preview = this.historyPreview();
    if (preview !== null) this.action.emit({ kind: 'applyHistoryRevert', previewId: preview.previewId });
  }

  discardHistoryRevert(): void {
    const preview = this.historyPreview();
    if (preview !== null) this.action.emit({ kind: 'discardHistoryRevert', previewId: preview.previewId });
  }

  queryModel(): void {
    const asset = this.selectedAsset();
    if (asset === null) return;
    this.action.emit({
      kind: 'queryModel',
      assetId: asset.inspection.assetId,
      expectedAssetContentHash: asset.inspection.contentHash,
      window: {
        expectedContentHash: asset.inspection.contentHash,
        bounds: { min: asset.inspection.boundsMin, max: asset.inspection.boundsMax },
        includeEmpty: false,
        materialFilter: [],
        maxSamples: 128,
      },
    });
  }

  createAnnotation(): void {
    const asset = this.selectedAsset();
    const pick = this.validatedPick();
    if (asset === null) return;
    const coordinate = pick?.assetId === asset.inspection.assetId
      ? pick.hitVoxel
      : asset.inspection.boundsMin;
    this.action.emit({
      kind: 'createAnnotation',
      assetId: asset.inspection.assetId,
      draft: {
        layerId: this.annotationLayerId,
        targetVoxelAssetId: asset.inspection.assetId,
        targetVoxelDataHash: asset.inspection.voxelDataHash,
        targetBounds: { min: asset.inspection.boundsMin, max: asset.inspection.boundsMax },
        regions: [{
          regionId: this.annotationRegionId,
          label: this.annotationLabel,
          kind: this.annotationKind,
          tags: tags(this.annotationTags),
          bounds: { min: coordinate, max: coordinate },
          selection: { sparseRuns: [{ start: coordinate, length: 1 }] },
        }],
        provenance: [],
      },
    });
  }

  editAnnotation(): void {
    const asset = this.selectedAsset();
    const layer = this.selectedLayer();
    if (asset === null || layer === null) return;
    const start = tuple3i(this.annotationStart);
    const bounds = { min: start, max: tuple3i(this.annotationEnd) };
    const sparseRuns = [{ start, length: Math.max(1, integer(this.annotationRunLength, 1)) }];
    let command: VoxelAnnotationEditCommand;
    switch (this.annotationCommand) {
      case 'upsertRegion':
        command = {
          kind: 'upsertRegion',
          region: {
            regionId: this.annotationRegionId,
            label: this.annotationLabel,
            kind: this.annotationKind,
            tags: tags(this.annotationTags),
            ...(this.annotationParentRegionId.trim() === ''
              ? {}
              : { parentRegionId: this.annotationParentRegionId.trim() }),
            bounds,
            selection: { sparseRuns },
          },
        };
        break;
      case 'removeRegion':
        command = { kind: 'removeRegion', regionId: this.annotationRegionId };
        break;
      case 'addRuns':
        command = { kind: 'addRuns', regionId: this.annotationRegionId, sparseRuns };
        break;
      case 'removeRuns':
        command = { kind: 'removeRuns', regionId: this.annotationRegionId, sparseRuns };
        break;
      case 'replaceSelection':
        command = {
          kind: 'replaceSelection',
          regionId: this.annotationRegionId,
          selection: { sparseRuns },
        };
        break;
      case 'setParent':
        command = {
          kind: 'setParent',
          regionId: this.annotationRegionId,
          parentRegionId: this.annotationParentRegionId.trim() || null,
        };
        break;
      case 'setTags':
        command = { kind: 'setTags', regionId: this.annotationRegionId, tags: tags(this.annotationTags) };
        break;
      case 'setLabel':
        command = { kind: 'setLabel', regionId: this.annotationRegionId, label: this.annotationLabel };
        break;
      case 'setKind':
        command = { kind: 'setKind', regionId: this.annotationRegionId, annotationKind: this.annotationKind };
        break;
      case 'setBounds':
        command = { kind: 'setBounds', regionId: this.annotationRegionId, bounds };
        break;
    }
    this.action.emit({
      kind: 'editAnnotation',
      assetId: asset.inspection.assetId,
      layerId: layer.layerId,
      transaction: {
        expectedLayerHash: layer.canonicalLayerHash,
        commands: [command],
      },
    });
  }

  queryAnnotation(): void {
    const asset = this.selectedAsset();
    const layer = this.selectedLayer();
    if (asset === null || layer === null) return;
    const mode: VoxelAnnotationQueryMode = this.annotationQueryMode === 'cell'
      ? { kind: 'cell', coordinate: tuple3i(this.annotationStart) }
      : this.annotationQueryMode === 'bounds'
        ? {
            kind: 'bounds',
            bounds: { min: tuple3i(this.annotationStart), max: tuple3i(this.annotationEnd) },
          }
        : this.annotationQueryMode === 'region'
          ? { kind: 'region', regionId: this.annotationRegionId }
          : { kind: 'layerSummary' };
    this.action.emit({
      kind: 'queryAnnotation',
      assetId: asset.inspection.assetId,
      layerId: layer.layerId,
      query: {
        expectedLayerHash: layer.canonicalLayerHash,
        mode,
        maxResults: Math.max(1, integer(this.annotationMaxResults, 256)),
      },
    });
  }

  exportAnnotation(): void {
    const asset = this.selectedAsset();
    const layer = this.selectedLayer();
    if (asset === null || layer === null) return;
    this.action.emit({
      kind: 'exportAnnotation',
      assetId: asset.inspection.assetId,
      layerId: layer.layerId,
      expectedLayerHash: layer.canonicalLayerHash,
    });
  }

  prepareConversion(): void {
    const settings = this.meshConversionSettings(false);
    if (settings === null) return;
    const action: VoxelEditorAction = {
      kind: 'prepareConversion',
      sourceAssetId: this.conversionSourceAsset,
      source: { scope: this.conversionSourceScope, path: this.conversionSourcePath.trim() },
      targetAssetId: this.conversionTargetAsset,
      ...(this.conversionLicensePath.trim() === ''
        ? {}
        : {
            license: {
              scope: this.conversionLicenseScope,
              path: this.conversionLicensePath.trim(),
            },
          }),
      ...(this.conversionMeshPrimitive.trim() === ''
        ? {}
        : { meshPrimitive: this.conversionMeshPrimitive.trim() }),
      settings,
      maxPreviewSamples: Math.max(1, integer(this.conversionMaxPreviewSamples, 256)),
    };
    this.action.emit(action);
  }

  inspectObjectSource(): void {
    this.pauseObjectPreview();
    this.action.emit({
      kind: 'inspectObjectSource',
      sourceKind: this.objectSourceKind,
      sourceAssetId: this.conversionSourceAsset,
      source: { scope: this.conversionSourceScope, path: this.conversionSourcePath.trim() },
    });
  }

  toggleObjectClip(sourceClipName: string, selected: boolean): void {
    this.objectSelectedClips = selected
      ? [...new Set([...this.objectSelectedClips, sourceClipName])]
      : this.objectSelectedClips.filter((name) => name !== sourceClipName);
    if (!this.objectSelectedClips.includes(this.objectDefaultClip)) {
      this.objectDefaultClip = this.objectSelectedClips[0] ?? '';
    }
  }

  objectClipSelected(sourceClipName: string): boolean {
    return this.objectSelectedClips.includes(sourceClipName);
  }

  prepareObjectConversion(): void {
    const settings = this.meshConversionSettings(true);
    if (settings === null) return;
    const inspection = this.objectSourceInspection();
    if (
      inspection === null
      || inspection.sourceKind !== this.objectSourceKind
      || inspection.source.assetId !== this.conversionSourceAsset
      || inspection.sourcePath !== this.conversionSourcePath.trim()
    ) {
      this.formError.set('Inspect the selected source before preparing a voxel object.');
      return;
    }
    let clipControl: VoxelObjectClipControlOutput;
    try {
      clipControl = buildVoxelObjectClipControlForSource(this.objectSourceKind, inspection.clips, {
        selectedSourceClipNames: this.objectSelectedClips,
        sampleRateHz: this.objectSampleRateHz,
        startSeconds: this.objectStartSeconds,
        endSeconds: this.objectEndSeconds,
        endPolicy: this.objectEndPolicy,
        defaultSourceClipName: this.objectDefaultClip,
      });
    } catch (error) {
      this.formError.set(error instanceof Error ? error.message : 'Clip controls are malformed.');
      return;
    }
    const { clips, defaultClip, initialFrame } = clipControl;
    this.objectPreviewClip = clips[0]?.outputClipId ?? '';
    this.objectPreviewFrame = 0;
    this.action.emit({
      kind: 'prepareObjectConversion',
      sourceKind: this.objectSourceKind,
      sourceAssetId: this.conversionSourceAsset,
      source: { scope: this.conversionSourceScope, path: this.conversionSourcePath.trim() },
      targetAssetId: this.objectTargetAsset,
      ...(this.conversionLicensePath.trim() === ''
        ? {}
        : {
            license: {
              scope: this.conversionLicenseScope,
              path: this.conversionLicensePath.trim(),
            },
          }),
      ...(this.objectSourceKind === 'static' && this.conversionMeshPrimitive.trim() !== ''
        ? { meshPrimitive: this.conversionMeshPrimitive.trim() }
        : {}),
      settings: {
        mesh: settings,
        pivot: tuple3(this.objectPivot),
        anchorPolicy: this.objectAnchorPolicy === 'preserveSourceSpace'
          ? { kind: 'preserveSourceSpace' }
          : {
              kind: 'lockNodeToBindPose',
              sourceNodeIndex: Math.max(0, integer(this.objectAnchorNode, 0)),
            },
      },
      clips,
      ...(defaultClip === undefined ? {} : { defaultClip }),
      frame: initialFrame,
      maxPreviewSamples: Math.max(1, integer(this.conversionMaxPreviewSamples, 256)),
    });
  }

  previewObjectFrame(frameIndex = this.objectPreviewFrame): void {
    const conversion = this.objectConversion();
    if (conversion === null) return;
    const clip = conversion.preview.clips.find(
      (candidate) => candidate.outputClipId === this.objectPreviewClip,
    );
    const boundedFrame = clip === undefined
      ? 0
      : Math.min(Math.max(0, integer(frameIndex, 0)), Math.max(0, clip.storedFrameCount - 1));
    this.objectPreviewFrame = boundedFrame;
    const frame: VoxelObjectFrameSelection = clip === undefined
      ? { kind: 'default' }
      : { kind: 'clip', clipId: clip.outputClipId, frameIndex: boundedFrame };
    this.action.emit({
      kind: 'previewObjectFrame',
      planId: conversion.plan.planId,
      expectedPlanHash: conversion.plan.planHash,
      frame,
      maxPreviewSamples: Math.max(1, integer(this.conversionMaxPreviewSamples, 256)),
    });
  }

  selectedObjectPreviewFrameMax(): number {
    const clip = this.objectConversion()?.preview.clips.find(
      (candidate) => candidate.outputClipId === this.objectPreviewClip,
    );
    return Math.max(0, (clip?.storedFrameCount ?? 1) - 1);
  }

  playObjectPreview(): void {
    if (this.objectPlaying() || this.objectConversion() === null) return;
    this.objectPlaying.set(true);
    this.#scheduleObjectPlayback();
  }

  pauseObjectPreview(): void {
    this.objectPlaying.set(false);
    if (this.#playbackTimer !== null) clearTimeout(this.#playbackTimer);
    this.#playbackTimer = null;
  }

  applyObjectConversion(): void {
    const conversion = this.objectConversion();
    if (conversion === null) return;
    this.pauseObjectPreview();
    this.action.emit({
      kind: 'applyObjectConversion',
      planId: conversion.plan.planId,
      expectedPlanHash: conversion.plan.planHash,
      expectedOutputHash: conversion.preview.outputHash,
    });
  }

  discardObjectConversion(): void {
    const conversion = this.objectConversion();
    if (conversion === null) return;
    this.pauseObjectPreview();
    this.action.emit({ kind: 'discardObjectConversion', planId: conversion.plan.planId });
  }

  objectAssets(): readonly VoxelObjectAssetAuthoringReadout[] {
    return this.objectAuthoring()?.assets ?? [];
  }

  selectedObjectAsset(): VoxelObjectAssetAuthoringReadout | null {
    return this.objectAssets().find((asset) => asset.assetId === this.objectSelectedAssetId) ?? null;
  }

  chooseObjectAsset(assetId: string): void {
    this.objectSelectedAssetId = assetId;
    const asset = this.selectedObjectAsset();
    this.objectInstanceClip = asset?.defaultClip ?? asset?.clips[0]?.clipId ?? '';
    this.objectInstanceFrame = 0;
  }

  attachObjectInstance(): void {
    const asset = this.selectedObjectAsset();
    if (asset === null) return;
    const clip = asset.clips.find((candidate) => candidate.clipId === this.objectInstanceClip);
    const frame: VoxelObjectFrameSelection = clip === undefined
      ? { kind: 'default' }
      : {
          kind: 'clip',
          clipId: clip.clipId,
          frameIndex: Math.min(
            Math.max(0, integer(this.objectInstanceFrame, 0)),
            Math.max(0, clip.frames.length - 1),
          ),
        };
    this.action.emit({
      kind: 'attachObjectInstance',
      sceneId: this.entryScene(),
      instance: {
        instanceId: this.objectInstanceId.trim(),
        voxelObjectAssetId: asset.assetId,
        frame,
        translation: tuple3(this.objectInstanceTranslation),
        rotation: tuple4(this.objectInstanceRotation),
        scale: tuple3(this.objectInstanceScale),
        materialOverrides: [],
      },
    });
  }

  private meshConversionSettings(objectLocal: boolean): VoxelConversionSettings | null {
    const palette = this.selectedAsset()?.palette ?? [{
      materialSlot: integer(this.newMaterialSlot, 1),
      materialAssetId: this.newMaterialId,
      displayName: this.newMaterialId,
    }];
    let transform: readonly number[];
    let textureAssets: readonly TextureSampleAsset[];
    let textureBindings: readonly TextureMaterialBinding[];
    try {
      transform = numericList(this.conversionTransform, 16, 'Affine transform');
      textureAssets = parseTextureAssets(this.conversionTextureAssets);
      textureBindings = parseTextureBindings(this.conversionTextureBindings);
      this.formError.set(null);
    } catch (error) {
      this.formError.set(error instanceof Error ? error.message : 'Conversion settings are malformed.');
      return null;
    }
    return {
      conversion: {
        resolution: tuple3i(this.conversionResolution),
        cellSize: positive(this.conversionCellSize, 1),
        chunkSize: integer(this.conversionChunkSize, 16),
        origin: objectLocal ? [0, 0, 0] : tuple3i(this.conversionOrigin),
        fitPolicy: this.conversionFitPolicy,
        originPolicy: this.conversionOriginPolicy,
        mode: this.conversionMode,
        materialPalette: palette,
        materialMap: palette.map((binding, sourceMaterialSlot) => ({
          sourceMaterialSlot,
          voxelMaterialSlot: binding.materialSlot,
        })),
        maxOutputVoxels: Math.max(
          1,
          integer(this.conversionResolution[0], 1)
            * integer(this.conversionResolution[1], 1)
            * integer(this.conversionResolution[2], 1),
        ),
      },
      transform,
      materialPolicy: {
        textureAssets,
        textureBindings,
        ...(this.conversionDefaultMaterial.trim() === ''
          ? {}
          : { defaultVoxelMaterial: integer(Number(this.conversionDefaultMaterial), 1) }),
      },
    };
  }

  #scheduleObjectPlayback(): void {
    if (!this.objectPlaying()) return;
    const conversion = this.objectConversion();
    if (conversion === null) {
      this.pauseObjectPreview();
      return;
    }
    const clip = conversion.preview.clips.find(
      (candidate) => candidate.outputClipId === this.objectPreviewClip,
    );
    if (clip === undefined || clip.storedFrameCount === 0) {
      this.pauseObjectPreview();
      return;
    }
    const planId = conversion.plan.planId;
    const frame = clip.frames[this.objectPreviewFrame];
    const delay = Math.max(16, Math.round((frame?.durationMicroseconds ?? 83_333) / 1_000));
    this.#playbackTimer = setTimeout(() => {
      if (!this.objectPlaying() || this.objectConversion()?.plan.planId !== planId) {
        this.pauseObjectPreview();
        return;
      }
      this.previewObjectFrame((this.objectPreviewFrame + 1) % clip.storedFrameCount);
      this.#scheduleObjectPlayback();
    }, delay);
  }

  applyConversion(): void {
    const conversion = this.conversion();
    if (conversion === null) return;
    this.action.emit({
      kind: 'applyConversion',
      planId: conversion.plan.planId,
      expectedPlanHash: conversion.plan.planHash,
      expectedOutputHash: conversion.preview.outputHash,
    });
  }

  discardConversion(): void {
    const conversion = this.conversion();
    if (conversion === null) return;
    this.action.emit({ kind: 'discardConversion', planId: conversion.plan.planId });
  }
}

function materialDefinition(
  color: string,
  roughness: number,
  emissive: number,
): StoredMaterialDefinition {
  const rgba = parseHexColor(color);
  return {
    authority: {
      solid: true,
      collidable: true,
      occludes: true,
      structuralClass: 'structural',
    },
    style: {
      color: rgba,
      texture: null,
      textureTint: [1, 1, 1, 1],
      emissionColor: rgba,
      roughness: Math.max(0, finite(roughness, 0.8)),
      emissive: Math.max(0, finite(emissive, 0)),
      uvStrategy: 'flat',
    },
  };
}

function parseHexColor(value: string): readonly [number, number, number, number] {
  const match = /^#([0-9a-f]{6})$/i.exec(value.trim());
  if (match?.[1] === undefined) return [0.9, 0.5, 0.25, 1];
  return [
    Number.parseInt(match[1].slice(0, 2), 16) / 255,
    Number.parseInt(match[1].slice(2, 4), 16) / 255,
    Number.parseInt(match[1].slice(4, 6), 16) / 255,
    1,
  ];
}

function hostParent(path: string): string {
  const normalized = path.replace(/\/+$/, '');
  const separator = normalized.lastIndexOf('/');
  return separator <= 0 ? '/' : normalized.slice(0, separator);
}

function tuple3(values: readonly number[]): readonly [number, number, number] {
  return [finite(values[0], 0), finite(values[1], 0), finite(values[2], 0)];
}

function tuple3i(values: readonly number[]): readonly [number, number, number] {
  return [integer(values[0], 0), integer(values[1], 0), integer(values[2], 0)];
}

function tuple4(values: readonly number[]): readonly [number, number, number, number] {
  return [
    finite(values[0], 0),
    finite(values[1], 0),
    finite(values[2], 0),
    finite(values[3], 1),
  ];
}


function finite(value: number | undefined, fallback: number): number {
  return value !== undefined && Number.isFinite(value) ? value : fallback;
}

function integer(value: number | undefined, fallback: number): number {
  return Math.max(0, Math.trunc(finite(value, fallback)));
}

function positive(value: number | undefined, fallback: number): number {
  const result = finite(value, fallback);
  return result > 0 ? result : fallback;
}

function tags(value: string): readonly string[] {
  return [...new Set(value.split(',').map((tag) => tag.trim()).filter((tag) => tag !== ''))];
}

function numericList(value: string, length: number, label: string): readonly number[] {
  const entries = value.split(',').map((entry) => Number(entry.trim()));
  if (entries.length !== length || entries.some((entry) => !Number.isFinite(entry))) {
    throw new TypeError(`${label} must contain exactly ${String(length)} finite comma-separated numbers.`);
  }
  return entries;
}

function parseTextureAssets(value: string): readonly TextureSampleAsset[] {
  return jsonArray(value, 'Texture assets').map((entry, index) => {
    const item = closed(entry, `Texture assets[${String(index)}]`, ['texture', 'texelMaterials']);
    const texels = unknownArray(item['texelMaterials'], 'texelMaterials').map(numberValue);
    return { texture: textureSource(item['texture']), texelMaterials: texels };
  });
}

function parseTextureBindings(value: string): readonly TextureMaterialBinding[] {
  return jsonArray(value, 'Texture bindings').map((entry, index) => {
    const label = `Texture bindings[${String(index)}]`;
    const item = closed(entry, label, [
      'sourceMaterialSlot', 'texture', 'uvAttribute', 'sampleUv', 'samplingPolicy',
      'wrapPolicy', 'materialMode',
    ]);
    const uv = closed(item['uvAttribute'], `${label}.uvAttribute`, ['attributeName', 'sourceHash']);
    const sample = unknownArray(item['sampleUv'], `${label}.sampleUv`).map(numberValue);
    if (sample.length !== 2 || sample[0] === undefined || sample[1] === undefined) {
      throw new TypeError(`${label}.sampleUv must have two numbers.`);
    }
    return {
      sourceMaterialSlot: numberValue(item['sourceMaterialSlot']),
      texture: textureSource(item['texture']),
      uvAttribute: {
        attributeName: stringValue(uv['attributeName']),
        sourceHash: stringValue(uv['sourceHash']),
      },
      sampleUv: [sample[0], sample[1]],
      samplingPolicy: literal(item['samplingPolicy'], 'nearest_texel'),
      wrapPolicy: literal(item['wrapPolicy'], 'clamp_to_edge'),
      materialMode: literal(item['materialMode'], 'sample_palette_index'),
    };
  });
}

function textureSource(value: unknown): TextureSampleAsset['texture'] {
  const item = closed(value, 'Texture source', [
    'textureAssetId', 'assetVersion', 'contentHash', 'width', 'height',
    'colorSpace', 'channelLayout',
  ]);
  const colorSpace = item['colorSpace'];
  if (colorSpace !== 'linear' && colorSpace !== 'srgb') {
    throw new TypeError('Texture source colorSpace must be linear or srgb.');
  }
  return {
    textureAssetId: stringValue(item['textureAssetId']),
    assetVersion: numberValue(item['assetVersion']),
    contentHash: stringValue(item['contentHash']),
    width: numberValue(item['width']),
    height: numberValue(item['height']),
    colorSpace,
    channelLayout: literal(item['channelLayout'], 'palette_index_u16'),
  };
}

function jsonArray(value: string, label: string): readonly unknown[] {
  let parsed: unknown;
  try {
    parsed = JSON.parse(value) as unknown;
  } catch {
    throw new TypeError(`${label} must be valid JSON.`);
  }
  return unknownArray(parsed, label);
}

function unknownArray(value: unknown, label: string): readonly unknown[] {
  if (!Array.isArray(value)) throw new TypeError(`${label} must be an array.`);
  return value;
}

function closed(
  value: unknown,
  label: string,
  fields: readonly string[],
): Readonly<Record<string, unknown>> {
  if (typeof value !== 'object' || value === null || Array.isArray(value)) {
    throw new TypeError(`${label} must be an object.`);
  }
  const record = value as Readonly<Record<string, unknown>>;
  if (fields.some((field) => !Object.hasOwn(record, field))) {
    throw new TypeError(`${label} is missing a required field.`);
  }
  if (Object.keys(record).some((field) => !fields.includes(field))) {
    throw new TypeError(`${label} contains an unknown field.`);
  }
  return record;
}

function stringValue(value: unknown): string {
  if (typeof value !== 'string') throw new TypeError('Texture field must be text.');
  return value;
}

function numberValue(value: unknown): number {
  if (typeof value !== 'number' || !Number.isFinite(value)) {
    throw new TypeError('Texture field must be a finite number.');
  }
  return value;
}

function literal<Value extends string>(value: unknown, expected: Value): Value {
  if (value !== expected) throw new TypeError(`Texture field must equal ${expected}.`);
  return expected;
}
