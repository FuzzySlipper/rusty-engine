import type { LiveDebugTransport } from '@rusty-engine/live-debug-client';

import { LiveDebugPanel } from './live-debug-panel.js';
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
  type ProductHostDebugCatalog,
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

/** Releases the panel's DOM and every request owned by this mounted panel. */
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
  const panel = new LiveDebugPanel(host.ownerDocument, {
    enabled: options.enabled,
    transport: options.transport ?? null,
    presentation: options.presentation ?? 'inline',
  });
  host.appendChild(panel.element);
  return { dispose: () => panel.dispose() };
}
