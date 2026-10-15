import type { LiveDebugTransport } from '@rusty-engine/live-debug-client';

import { LiveDebugPanel } from './live-debug-panel.js';
import type { LiveDebugPanelPresentation } from './live-debug-panel-model.js';
import { SceneTreePanel, type SceneTreePanelOptions } from './scene-tree-panel.js';
import { TunablesPanel, type TunablesPanelOptions } from './tunables-panel.js';

export type { SceneTreePanelOptions } from './scene-tree-panel.js';
export type { TunablesPanelOptions } from './tunables-panel.js';
export type {
  TunableChooser,
  TunableControl,
  TunableGroup,
  TunableKind,
  TunableTimeline,
  TunablesDescription,
} from './tunables-model.js';

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

/** A mounted tunables panel. */
export interface TunablesPanelMount {
  /** Describes again and re-reads the bound source files. */
  refresh(): Promise<void>;
  /** Removes the panel and stops its requests. */
  dispose(): void;
}

/**
 * Mounts the Engine tunables panel into a product-owned element. It renders
 * the product's `tunables.v1` description (from its `TunablesDebugModule`)
 * and sends commands: a control applies live, a control bound to a source
 * file shows when it differs from the file, and each file has Save (written
 * through the Engine content source under `rusty dev`) and Reset.
 */
export function mountTunablesPanel(host: HTMLElement, options: TunablesPanelOptions = {}): TunablesPanelMount {
  if (!(host instanceof HTMLElement)) {
    throw new TypeError('Tunables panel mounting requires an HTMLElement host.');
  }
  const panel = new TunablesPanel(host.ownerDocument, options);
  host.appendChild(panel.element);
  void panel.refresh();
  return { refresh: () => panel.refresh(), dispose: () => panel.dispose() };
}

/** A mounted scene tree panel. */
export interface SceneTreePanelMount {
  /** Reads the tree and the selected object again. */
  refresh(): Promise<void>;
  /** Removes the panel and stops its requests. */
  dispose(): void;
}

/**
 * Mounts the Engine scene tree into a product-owned element: the committed
 * scene's objects as a tree (`engine.scene.objects`) and the selected
 * object's facts (`engine.scene.object`). It holds no product state.
 */
export function mountSceneTreePanel(host: HTMLElement, options: SceneTreePanelOptions = {}): SceneTreePanelMount {
  if (!(host instanceof HTMLElement)) {
    throw new TypeError('Scene tree panel mounting requires an HTMLElement host.');
  }
  const panel = new SceneTreePanel(host.ownerDocument, options);
  host.appendChild(panel.element);
  void panel.refresh();
  return { refresh: () => panel.refresh(), dispose: () => panel.dispose() };
}
