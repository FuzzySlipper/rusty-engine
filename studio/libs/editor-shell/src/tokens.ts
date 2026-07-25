import { InjectionToken } from '@angular/core';

import type { StudioWorkspaceStore } from './state.js';

export const STUDIO_WORKSPACE = new InjectionToken<StudioWorkspaceStore>('StudioWorkspace');
