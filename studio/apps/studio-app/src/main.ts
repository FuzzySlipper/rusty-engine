import { provideZonelessChangeDetection } from '@angular/core';
import { bootstrapApplication } from '@angular/platform-browser';
import { StudioAdapterClient } from '@rusty-engine/studio-adapter-client';
import {
  HttpStudioAdapterTransport,
  STUDIO_WORKSPACE,
  StudioWorkspaceStore,
} from '@rusty-engine/studio-editor-shell';

import { App } from './app/app.js';

void bootstrapApplication(App, {
  providers: [
    provideZonelessChangeDetection(),
    {
      provide: STUDIO_WORKSPACE,
      useFactory: () => new StudioWorkspaceStore(
        new StudioAdapterClient(new HttpStudioAdapterTransport()),
      ),
    },
  ],
});
