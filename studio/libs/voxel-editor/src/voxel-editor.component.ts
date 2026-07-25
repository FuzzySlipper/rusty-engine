import {
  ChangeDetectionStrategy,
  Component,
  effect,
  input,
  output,
  signal,
} from '@angular/core';
import { JsonPipe } from '@angular/common';
import { FormsModule } from '@angular/forms';
import type {
  MaterialAssetReadout,
  StoredMaterialDefinition,
  VoxelAnnotationKind,
  VoxelAssetAuthoringReadout,
  VoxelAuthoringReadout,
  VoxelConversionPlan,
  VoxelConversionPreview,
  VoxelInstanceReadout,
  VoxelPickReadout,
  VoxelReadout,
} from '@rusty-engine/studio-adapter-client';

import type { VoxelEditorAction } from './voxel-editor-model.js';

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
  readonly entryScene = input('');
  readonly validatedPick = input<VoxelPickReadout | null>(null);
  readonly lastReadout = input<VoxelReadout | null>(null);
  readonly conversion = input<{
    readonly plan: VoxelConversionPlan;
    readonly preview: VoxelConversionPreview;
  } | null>(null);
  readonly busy = input(false);
  readonly action = output<VoxelEditorAction>();

  readonly tab = signal<EditorTab>('assets');
  readonly brushPreview = signal(false);

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

  annotationLayerId = 'voxel-annotation/studio-semantics';
  annotationRegionId = 'region/studio-selection';
  annotationLabel = 'Studio selection';
  annotationKind: VoxelAnnotationKind = 'selection';
  annotationTags = 'authored,studio';

  conversionSourceAsset = 'mesh/kenney-wall-a';
  conversionSourcePath = 'fixtures/voxel-conversion/kenney-wall-a.glb';
  conversionTargetAsset = 'voxel-volume/converted-studio';
  conversionLicensePath = 'fixtures/voxel-conversion/KENNEY-RETRO-URBAN-KIT-LICENSE.txt';
  conversionResolution = [4, 3, 2];
  conversionCellSize = 1;
  conversionChunkSize = 16;
  conversionOrigin = [0, 0, 0];
  conversionFitPolicy: 'contain' | 'cover' | 'stretch' = 'contain';
  conversionOriginPolicy: 'sourceOrigin' | 'targetMin' | 'centered' = 'targetMin';
  conversionMode: 'surface' | 'solid' = 'surface';

  constructor() {
    effect(() => {
      const pick = this.validatedPick();
      if (pick === null) return;
      this.selectedAssetId = pick.assetId;
      this.selectedInstanceId = pick.instanceId;
    });
  }

  setTab(tab: EditorTab): void {
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

  previewBrush(): void {
    if (this.validatedPick() !== null) this.brushPreview.set(true);
  }

  cancelBrushPreview(): void {
    this.brushPreview.set(false);
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

  editAnnotationLabel(): void {
    const asset = this.selectedAsset();
    const layer = this.selectedLayer();
    if (asset === null || layer === null) return;
    this.action.emit({
      kind: 'editAnnotation',
      assetId: asset.inspection.assetId,
      layerId: layer.layerId,
      transaction: {
        expectedLayerHash: layer.canonicalLayerHash,
        commands: [{
          kind: 'setLabel',
          regionId: this.annotationRegionId,
          label: this.annotationLabel,
        }],
      },
    });
  }

  queryAnnotation(): void {
    const asset = this.selectedAsset();
    const layer = this.selectedLayer();
    if (asset === null || layer === null) return;
    this.action.emit({
      kind: 'queryAnnotation',
      assetId: asset.inspection.assetId,
      layerId: layer.layerId,
      query: {
        expectedLayerHash: layer.canonicalLayerHash,
        mode: { kind: 'layerSummary' },
        maxResults: 256,
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
    const palette = this.selectedAsset()?.palette ?? [{
      materialSlot: integer(this.newMaterialSlot, 1),
      materialAssetId: this.newMaterialId,
      displayName: this.newMaterialId,
    }];
    const action: VoxelEditorAction = {
      kind: 'prepareConversion',
      sourceAssetId: this.conversionSourceAsset,
      sourcePath: this.conversionSourcePath,
      targetAssetId: this.conversionTargetAsset,
      ...(this.conversionLicensePath.trim() === ''
        ? {}
        : { licensePath: this.conversionLicensePath.trim() }),
      settings: {
        conversion: {
          resolution: tuple3i(this.conversionResolution),
          cellSize: positive(this.conversionCellSize, 1),
          chunkSize: integer(this.conversionChunkSize, 16),
          origin: tuple3i(this.conversionOrigin),
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
        transform: [
          1, 0, 0, 0,
          0, 1, 0, 0,
          0, 0, 1, 0,
          0, 0, 0, 1,
        ],
        materialPolicy: { textureAssets: [], textureBindings: [] },
      },
      maxPreviewSamples: 256,
    };
    this.action.emit(action);
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
