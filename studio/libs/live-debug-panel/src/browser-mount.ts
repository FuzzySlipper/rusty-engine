import { createComponent, provideZonelessChangeDetection } from '@angular/core';
import { createApplication } from '@angular/platform-browser';
import type { LiveDebugTransport } from '@rusty-engine/live-debug-client';

import { LiveDebugPanelComponent } from './live-debug-panel.component.js';
import type { LiveDebugPanelPresentation } from './live-debug-panel-model.js';

/** Explicit, product-owned configuration for one optional live-debug panel. */
export interface LiveDebugPanelMountOptions {
  /** False keeps the mounted panel inert: it does not contact a debug host. */
  readonly enabled: boolean;
  /** Uses the panel's same-origin HTTP transport when omitted. */
  readonly transport?: LiveDebugTransport;
  /** Controls only the panel's DOM presentation. */
  readonly presentation?: LiveDebugPanelPresentation;
}

/** Releases the Angular view and every request owned by this mounted panel. */
export interface LiveDebugPanelMount {
  dispose(): void;
}

/**
 * Mounts the optional Engine-owned debug UI into a product-owned element.
 * The panel receives no product state and exposes no command semantics: it
 * only forwards raw command lines through the injected or same-origin client.
 */
export async function mountLiveDebugPanel(
  host: HTMLElement,
  options: LiveDebugPanelMountOptions,
): Promise<LiveDebugPanelMount> {
  if (!(host instanceof HTMLElement)) {
    throw new TypeError('Live-debug panel mounting requires an HTMLElement host.');
  }

  const application = await createApplication({
    providers: [provideZonelessChangeDetection()],
  });
  const component = createComponent(LiveDebugPanelComponent, {
    environmentInjector: application.injector,
    hostElement: host,
  });

  application.attachView(component.hostView);
  component.setInput('transport', options.transport ?? null);
  component.setInput('presentation', options.presentation ?? 'inline');
  component.setInput('enabled', options.enabled);
  component.changeDetectorRef.detectChanges();

  let disposed = false;
  return {
    dispose(): void {
      if (disposed) return;
      disposed = true;
      application.detachView(component.hostView);
      component.destroy();
      application.destroy();
    },
  };
}
