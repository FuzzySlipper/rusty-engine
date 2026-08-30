import type { LiveDebugTransport } from './live-debug-client.js';
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
export declare function mountLiveDebugPanel(host: HTMLElement, options: LiveDebugPanelMountOptions): Promise<LiveDebugPanelMount>;
