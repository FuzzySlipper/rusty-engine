import { ChangeDetectionStrategy, Component, HostBinding, inject } from '@angular/core';
import { FormsModule } from '@angular/forms';
import type { OwnerDiagnostic } from '@rusty-engine/studio-adapter-client';
import {
  StudioViewportComponent,
  type VoxelViewportPickCandidate,
} from '@rusty-engine/studio-viewport';
import {
  VoxelEditorComponent,
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

  projectRoot = '';
  projectFile = 'content/projects/converted-wall.project.json';
  inspectorMode: 'entity' | 'voxel' = 'voxel';

  @HostBinding('class.theme-high-contrast')
  get highContrast(): boolean {
    return this.state().settings.theme === 'highContrast';
  }

  openProject(): void {
    void this.store.openProject(this.projectRoot, this.projectFile);
  }

  refreshProject(): void {
    void this.store.refreshProject();
  }

  closeProject(): void {
    void this.store.closeProject();
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

  translation(axis: 0 | 1 | 2): number | null {
    const preview = this.state().preview;
    if (preview !== null) return preview.translation[axis];
    return this.store.selectedHierarchyNode()?.localTransform.translation[axis] ?? null;
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
}
