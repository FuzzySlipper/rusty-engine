import { ChangeDetectionStrategy, Component, inject, type OnInit } from '@angular/core';
import {
  STUDIO_WORKSPACE,
  StudioShellComponent,
} from '@rusty-engine/studio-editor-shell';

import { readStudioStartupProject } from './studio-startup.js';

@Component({
  selector: 'rusty-root',
  standalone: true,
  imports: [StudioShellComponent],
  template: '<rusty-studio-shell />',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class App implements OnInit {
  readonly #store = inject(STUDIO_WORKSPACE);

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
