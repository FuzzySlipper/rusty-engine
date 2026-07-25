import { ChangeDetectionStrategy, Component, HostBinding, inject } from '@angular/core';
import { FormsModule } from '@angular/forms';

import { STUDIO_WORKSPACE } from './tokens.js';

@Component({
  selector: 'rusty-studio-shell',
  standalone: true,
  imports: [FormsModule],
  templateUrl: './studio-shell.component.html',
  styleUrl: './studio-shell.component.css',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class StudioShellComponent {
  readonly store = inject(STUDIO_WORKSPACE);
  readonly state = this.store.snapshot;

  projectRoot = '';
  projectFile = 'content/projects/loading-bay.project.json';

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

  beginSelectedPreview(): void {
    const selected = this.store.selectedEntity();
    if (selected !== null) this.store.beginTranslationPreview(selected.entityId);
  }

  updateTranslation(axis: 0 | 1 | 2, raw: string): void {
    const selected = this.store.selectedEntity();
    if (selected === null) return;
    if (this.state().preview?.entityId !== selected.entityId) {
      this.store.beginTranslationPreview(selected.entityId);
    }
    this.store.setPreviewTranslationAxis(axis, Number(raw));
  }

  translation(axis: 0 | 1 | 2): number | null {
    const preview = this.state().preview;
    if (preview !== null) return preview.translation[axis];
    return this.store.selectedEntity()?.transform?.translation[axis] ?? null;
  }

  commitPreview(): void {
    void this.store.commitPreview();
  }

  ownerDiagnosticCount(): number {
    const inspections = this.state().authoringDocument?.inspections;
    if (inspections === undefined) return 0;
    return inspections.catalog.diagnostics.diagnostics.length
      + inspections.scene.diagnostics.diagnostics.length
      + inspections.entityState.diagnostics.diagnostics.length
      + inspections.persistence.diagnostics.diagnostics.length;
  }
}
