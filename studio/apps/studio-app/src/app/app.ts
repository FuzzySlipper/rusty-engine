import { ChangeDetectionStrategy, Component, inject, type OnInit } from '@angular/core';
import {
  RUSTY_ENGINE_ENTITY_INSPECTOR_CONTRIBUTIONS,
  STUDIO_WORKSPACE,
  StudioShellComponent,
} from '@rusty-engine/studio-editor-shell';

import { readStudioStartupProject } from './studio-startup.js';

@Component({
  selector: 'rusty-root',
  standalone: true,
  imports: [StudioShellComponent],
  template: `
    <rusty-studio-shell
      [entityInspectorContributions]="entityInspectorContributions"
    />
  `,
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class App implements OnInit {
  readonly #store = inject(STUDIO_WORKSPACE);
  readonly entityInspectorContributions =
    RUSTY_ENGINE_ENTITY_INSPECTOR_CONTRIBUTIONS;

  ngOnInit(): void {
    const startup = readStudioStartupProject(globalThis.location?.href ?? '');
    if (startup.status === 'open') {
      void this.#store.openProject(startup.root, startup.projectFile);
      return;
    }
    void this.#store.connect();
    if (startup.status === 'invalid') this.#store.reportUiError(startup.diagnostic);
  }
}
