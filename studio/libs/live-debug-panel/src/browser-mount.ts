import { createComponent, provideZonelessChangeDetection } from '@angular/core';
import { createApplication } from '@angular/platform-browser';
import type { LiveDebugTransport } from '@rusty-engine/live-debug-client';

import { LiveDebugPanelComponent } from './live-debug-panel.component.js';
import type { LiveDebugPanelPresentation } from './live-debug-panel-model.js';

export {
  mountRendererMetricsWidget,
  type RendererMetricsWidgetMount,
  type RendererMetricsWidgetMountOptions,
} from './renderer-metrics-widget.js';

// Browser products occasionally need the same fixed command transport as the
// optional panel for a compact product-specific DOM control. Keep that
// convenience on the packaged browser entry rather than teaching products the
// debug endpoint paths.
export {
  createLiveDebugHttpTransport,
  type LiveDebugCatalog,
  type LiveDebugHttpTransportOptions,
  type LiveDebugResult,
  type LiveDebugTransport,
} from '@rusty-engine/live-debug-client';

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
  // Angular associates component host context with the element passed to
  // createComponent. Keep that context on a mount-owned node so the
  // caller-owned host can be reused after disposal.
  const mountHost = host.ownerDocument.createElement('div');
  host.appendChild(mountHost);
  const component = createComponent(LiveDebugPanelComponent, {
    environmentInjector: application.injector,
    hostElement: mountHost,
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
      mountHost.remove();
    },
  };
}
