import {
  ChangeDetectionStrategy,
  Component,
  HostBinding,
  computed,
  inject,
  signal,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import type {
  AssetEntryReadout,
  OwnerDiagnostic,
  StoredLight,
  StudioSceneAppearance,
} from '@rusty-engine/studio-adapter-client';
import type { EditorGridDescriptor } from '@rusty-engine/render-contracts';
import type { StudioKeyboardBindings } from '@rusty-engine/studio-user-settings';
import {
  StudioViewportComponent,
  type StudioVoxelPreview,
  type VoxelViewportPickCandidate,
} from '@rusty-engine/studio-viewport';
import {
  VoxelEditorComponent,
  type VoxelBrushPreviewPresentation,
  type VoxelEditorAction,
} from '@rusty-engine/studio-voxel-editor';

import { STUDIO_WORKSPACE } from './tokens.js';

@Component({
  selector: 'rusty-studio-shell',
  standalone: true,
  imports: [FormsModule, StudioViewportComponent, VoxelEditorComponent],
  templateUrl: './studio-shell.component.html',
  styleUrl: './studio-shell.component.css',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StudioShellComponent {
  readonly store = inject(STUDIO_WORKSPACE);
  readonly state = this.store.snapshot;
  readonly brushPreview = signal<VoxelBrushPreviewPresentation | null>(null);
  readonly voxelPreview = computed<StudioVoxelPreview | null>(() => {
    const brush = this.brushPreview();
    if (brush !== null) return brush;
    const conversion = this.state().voxelWorkspace.conversion;
    if (conversion === null) return null;
    return {
      kind: 'conversion',
      cellSize: conversion.plan.settings.conversion.cellSize,
      samples: conversion.preview.sampleVoxels,
    };
  });
  readonly viewportGrid = computed<EditorGridDescriptor | null>(() => {
    const settings = this.state().settings;
    if (!settings.gridVisible) return null;
    return {
      visible: true,
      grid: {
        coordinateSystem: 'rightHandedYUp',
        origin: [0, 0, 0],
        spacing: [settings.translationSnap, settings.translationSnap, settings.translationSnap],
      },
      plane: 'xz',
      snapAnchor: 'boundary',
      style: {
        minorColor: settings.minorColor,
        majorColor: settings.majorColor,
        xAxisColor: settings.xAxisColor,
        yAxisColor: settings.yAxisColor,
        zAxisColor: settings.zAxisColor,
        majorLineEvery: settings.majorLineEvery,
        opacity: settings.opacity,
        fadeStart: settings.fadeStart,
        fadeEnd: settings.fadeEnd,
      },
    };
  });
  readonly viewportControlPreferences = computed(() => {
    const settings = this.state().settings;
    return {
      moveSpeed: settings.cameraMoveSpeed,
      boostMultiplier: settings.cameraBoostMultiplier,
      invertLookY: settings.invertLookY,
      invertPanY: settings.invertPanY,
      keyboard: { ...settings.keyboard },
    };
  });
  readonly gridColors = [
    { key: 'minorColor', label: 'Minor lines' },
    { key: 'majorColor', label: 'Major lines' },
    { key: 'xAxisColor', label: 'X axis' },
    { key: 'yAxisColor', label: 'Y axis' },
    { key: 'zAxisColor', label: 'Z axis' },
  ] as const;
  readonly keyboardBindings = [
    { key: 'moveForward', label: 'Move forward' },
    { key: 'moveBackward', label: 'Move backward' },
    { key: 'moveLeft', label: 'Move left' },
    { key: 'moveRight', label: 'Move right' },
    { key: 'moveDown', label: 'Move down' },
    { key: 'moveUp', label: 'Move up' },
    { key: 'boost', label: 'Boost' },
  ] as const;

  projectRoot = '';
  projectFile = 'content/projects/converted-wall.project.json';
  inspectorMode: 'entity' | 'voxel' = 'voxel';
  authoringDialog: 'createProject' | 'saveProjectAs' | 'scene' | 'object' | 'assetImport' | null = null;

  projectDraft = {
    root: '',
    projectFile: 'content/projects/new-project.project.json',
    projectId: 'new-project',
    name: 'New Project',
    entryScene: 'scene/main',
    entrySceneName: 'Main',
  };
  sceneDraft = { sceneId: 'scene/main', name: 'Main', makeEntry: true };
  objectDraft = {
    entityId: 1,
    name: 'New Entity',
    parentEntityId: null as number | null,
    childOrder: 0,
    kind: 'empty' as 'empty' | 'staticMesh' | 'light',
    asset: '',
  };
  appearanceKind: 'empty' | 'staticMesh' | 'light' = 'empty';
  appearanceAsset = '';
  appearanceVisible = true;
  lightKind: StoredLight['kind'] = 'directional';
  lightColor: [number, number, number] = [1, 1, 1];
  lightIntensity = 1;
  lightRange: number | null = null;
  lightDecay = 2;
  lightOuterAngle = Math.PI / 4;
  lightPenumbra = 0;
  lightEnabled = true;
  lightShadows = false;
  collisionEnabled = true;
  staticCollider = true;
  kinematicHalfExtents: [number, number, number] = [0.5, 0.5, 0.5];
  kinematicVelocity: [number, number, number] = [0, 0, 0];
  assetImportScope: 'project' | 'host' = 'project';
  assetImportPath = 'content/assets/studio-triangle.mesh.json';
  assetImportScale = 1;
  assetImportGenerateCollision = false;
  assetImportMaterialNamespace = '';

  @HostBinding('class.theme-high-contrast')
  get highContrast(): boolean {
    return this.state().settings.theme === 'highContrast';
  }

  openProject(): void {
    void this.store.openProject(this.projectRoot, this.projectFile);
  }

  openProjectDialog(mode: 'createProject' | 'saveProjectAs'): void {
    const identity = this.state().authoringDocument?.identity;
    this.projectDraft = mode === 'createProject'
      ? {
          root: this.projectRoot,
          projectFile: 'content/projects/new-project.project.json',
          projectId: 'new-project',
          name: 'New Project',
          entryScene: 'scene/main',
          entrySceneName: 'Main',
        }
      : {
          root: this.projectRoot,
          projectFile: identity?.relativeProjectFile ?? this.projectFile,
          projectId: identity === undefined ? 'project-copy' : `${identity.projectId}-copy`,
          name: identity === undefined ? 'Project Copy' : `${identity.name} Copy`,
          entryScene: identity?.entryScene ?? 'scene/main',
          entrySceneName: this.state().authoringDocument?.sceneHierarchy.name ?? 'Main',
        };
    this.authoringDialog = mode;
    this.store.toggleMenu(null);
  }

  submitProjectDialog(): void {
    if (this.authoringDialog === 'createProject') {
      void this.store.createProject(this.projectDraft);
    } else if (this.authoringDialog === 'saveProjectAs') {
      void this.store.saveProjectAs({
        root: this.projectDraft.root,
        projectFile: this.projectDraft.projectFile,
        projectId: this.projectDraft.projectId,
        name: this.projectDraft.name,
      });
    }
    this.projectRoot = this.projectDraft.root;
    this.projectFile = this.projectDraft.projectFile;
    this.authoringDialog = null;
  }

  openSceneDialog(): void {
    const document = this.state().authoringDocument;
    this.sceneDraft = {
      sceneId: document?.identity.entryScene ?? 'scene/main',
      name: document?.sceneHierarchy.name ?? 'Main',
      makeEntry: true,
    };
    this.authoringDialog = 'scene';
  }

  createScene(): void {
    void this.store.createScene(
      this.sceneDraft.sceneId,
      this.sceneDraft.name,
      this.sceneDraft.makeEntry,
    );
    this.authoringDialog = null;
  }

  renameScene(): void {
    void this.store.renameScene(this.sceneDraft.sceneId, this.sceneDraft.name);
    this.authoringDialog = null;
  }

  setEntryScene(): void {
    void this.store.setEntryScene(this.sceneDraft.sceneId);
    this.authoringDialog = null;
  }

  deleteScene(): void {
    void this.store.deleteScene(this.sceneDraft.sceneId);
    this.authoringDialog = null;
  }

  openObjectDialog(): void {
    const document = this.state().authoringDocument;
    const entityIds = document?.inspections.entityState.entityIds ?? [];
    const parentEntityId = this.store.selectedHierarchyNode()?.entityId ?? null;
    const childOrder = document?.sceneHierarchy.nodes.filter(
      (node) => node.parentNodeId === this.store.selectedHierarchyNode()?.nodeId,
    ).length ?? 0;
    this.objectDraft = {
      entityId: Math.max(0, ...entityIds) + 1,
      name: 'New Entity',
      parentEntityId,
      childOrder,
      kind: 'empty',
      asset: '',
    };
    this.authoringDialog = 'object';
  }

  createObject(): void {
    void this.store.createSceneObject({
      entityId: this.objectDraft.entityId,
      name: this.objectDraft.name,
      parentEntityId: this.objectDraft.parentEntityId,
      childOrder: this.objectDraft.childOrder,
      transform: {
        translation: [0, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      appearance: this.objectDraft.kind === 'staticMesh'
        ? { kind: 'staticMesh', asset: this.objectDraft.asset, visible: true }
        : this.objectDraft.kind === 'light'
          ? { kind: 'light', light: this.light() }
          : { kind: 'empty' },
      collision: null,
      kinematic: null,
    });
    this.authoringDialog = null;
  }

  refreshProject(): void {
    void this.store.refreshProject();
  }

  closeProject(): void {
    void this.store.closeProject();
  }

  openAssetImportDialog(): void {
    this.authoringDialog = 'assetImport';
    this.store.toggleMenu(null);
  }

  prepareAssetImport(): void {
    void this.store.prepareAssetImport(
      { scope: this.assetImportScope, path: this.assetImportPath },
      {
        scale: this.assetImportScale,
        generateCollision: this.assetImportGenerateCollision,
        materialNamespace: this.assetImportMaterialNamespace.trim() === ''
          ? null
          : this.assetImportMaterialNamespace.trim(),
      },
    );
    this.authoringDialog = null;
  }

  prepareAssetReimport(assetId: string): void {
    void this.store.prepareAssetReimport(assetId);
  }

  applyAssetImport(): void {
    void this.store.applyAssetImport();
  }

  discardAssetImport(): void {
    void this.store.discardAssetImport();
  }

  selectedAsset(): AssetEntryReadout | null {
    const assetId = this.state().assetWorkspace.selectedAssetId;
    return this.state().authoringDocument?.assetBrowser.assets.find(
      (asset) => asset.assetId === assetId,
    ) ?? null;
  }

  gridColorHex(
    key: 'minorColor' | 'majorColor' | 'xAxisColor' | 'yAxisColor' | 'zAxisColor',
  ): string {
    const color = this.state().settings[key];
    return `#${color.slice(0, 3).map((value) => Math.round(value * 255)
      .toString(16)
      .padStart(2, '0')).join('')}`;
  }

  captureKeyboardBinding(event: KeyboardEvent, key: keyof StudioKeyboardBindings): void {
    event.preventDefault();
    event.stopPropagation();
    if (event.code.length > 0) this.store.setKeyboardBinding(key, event.code);
  }

  setInspectorMode(mode: 'entity' | 'voxel'): void {
    this.inspectorMode = mode;
  }

  validateVoxelPick(candidate: VoxelViewportPickCandidate): void {
    void this.store.validateVoxelViewportPick(candidate);
  }

  runVoxelAction(action: VoxelEditorAction): void {
    void this.store.runVoxelAction(action);
  }

  setBrushPreview(preview: VoxelBrushPreviewPresentation | null): void {
    this.brushPreview.set(preview);
  }

  beginSelectedPreview(): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId !== null && entityId !== undefined) {
      this.store.beginTranslationPreview(entityId);
    }
  }

  canPreviewTranslation(): boolean {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    return entityId !== null && entityId !== undefined;
  }

  updateTranslation(axis: 0 | 1 | 2, raw: string): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    if (this.state().preview?.entityId !== entityId) {
      this.store.beginTranslationPreview(entityId);
    }
    this.store.setPreviewTranslationAxis(axis, Number(raw));
  }

  updateRotation(axis: 0 | 1 | 2 | 3, raw: string): void {
    this.ensureSelectedPreview();
    this.store.setPreviewRotationAxis(axis, Number(raw));
  }

  updateScale(axis: 0 | 1 | 2, raw: string): void {
    this.ensureSelectedPreview();
    this.store.setPreviewScaleAxis(axis, Number(raw));
  }

  translation(axis: 0 | 1 | 2): number | null {
    const preview = this.state().preview;
    if (preview !== null) return preview.translation[axis];
    return this.store.selectedHierarchyNode()?.localTransform.translation[axis] ?? null;
  }

  rotation(axis: 0 | 1 | 2 | 3): number | null {
    const preview = this.state().preview;
    if (preview !== null) return preview.rotation[axis];
    return this.store.selectedHierarchyNode()?.localTransform.rotation[axis] ?? null;
  }

  scale(axis: 0 | 1 | 2): number | null {
    const preview = this.state().preview;
    if (preview !== null) return preview.scale[axis];
    return this.store.selectedHierarchyNode()?.localTransform.scale[axis] ?? null;
  }

  renameSelected(name: string): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId !== null && entityId !== undefined) {
      void this.store.renameSceneObject(entityId, name);
    }
  }

  deleteSelected(): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId !== null && entityId !== undefined) {
      void this.store.deleteSceneObject(entityId);
    }
  }

  reparentSelected(parentRaw: string, orderRaw: string): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    const parent = parentRaw.trim() === '' ? null : Number(parentRaw);
    void this.store.reparentSceneObject(entityId, parent, Number(orderRaw));
  }

  applyAppearance(): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    let appearance: StudioSceneAppearance;
    if (this.appearanceKind === 'staticMesh') {
      appearance = { kind: 'staticMesh', asset: this.appearanceAsset, visible: this.appearanceVisible };
    } else if (this.appearanceKind === 'light') {
      appearance = { kind: 'light', light: this.light() };
    } else {
      appearance = { kind: 'empty' };
    }
    void this.store.setSceneObjectAppearance(entityId, appearance);
  }

  applyCollision(attached: boolean): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    void this.store.setEntityCollision(entityId, attached ? {
      enabled: this.collisionEnabled,
      staticCollider: this.staticCollider,
    } : null);
  }

  applyKinematic(attached: boolean): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    void this.store.setEntityKinematic(entityId, attached ? {
      halfExtents: this.kinematicHalfExtents,
      velocity: this.kinematicVelocity,
    } : null);
  }

  nodeIcon(kind: string): string {
    switch (kind) {
      case 'emptyGroup': return '▾';
      case 'light': return '☀';
      case 'voxelVolume': return '▦';
      case 'marker': return '⌖';
      default: return '◇';
    }
  }

  commitPreview(): void {
    void this.store.commitPreview();
  }

  ownerDiagnosticCount(): number {
    return this.ownerDiagnostics().length;
  }

  ownerDiagnostics(): readonly OwnerDiagnostic[] {
    const inspections = this.state().authoringDocument?.inspections;
    if (inspections === undefined) return [];
    return [
      ...inspections.catalog.diagnostics.diagnostics,
      ...inspections.scene.diagnostics.diagnostics,
      ...inspections.entityState.diagnostics.diagnostics,
      ...inspections.persistence.diagnostics.diagnostics,
    ];
  }

  private ensureSelectedPreview(): void {
    const entityId = this.store.selectedHierarchyNode()?.entityId;
    if (entityId === null || entityId === undefined) return;
    if (this.state().preview?.entityId !== entityId) {
      this.store.beginTranslationPreview(entityId);
    }
  }

  private light(): StoredLight {
    const base = {
      color: this.lightColor,
      intensity: this.lightIntensity,
      enabled: this.lightEnabled,
      shadows: this.lightShadows,
    };
    switch (this.lightKind) {
      case 'ambient': return { kind: 'ambient', ...base };
      case 'directional': return { kind: 'directional', ...base };
      case 'point': return {
        kind: 'point', ...base, range: this.lightRange, decay: this.lightDecay,
      };
      case 'spot': return {
        kind: 'spot', ...base, range: this.lightRange, decay: this.lightDecay,
        outerAngleRadians: this.lightOuterAngle, penumbra: this.lightPenumbra,
      };
    }
  }
}
