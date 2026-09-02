import { type LiveDebugTransport } from './live-debug-client.js';
/** Explicit, product-owned configuration for one optional renderer metrics widget. */
export interface RendererMetricsWidgetMountOptions {
    /**
     * When supplied, establishes the shared Engine widget state at mount. Omit
     * it to preserve the Engine default (hidden) or a console-selected state.
     */
    readonly initiallyVisible?: boolean;
    /** Uses the widget's same-origin HTTP transport when omitted. */
    readonly transport?: LiveDebugTransport;
}
/** Releases polling and the DOM node owned by one widget mount. */
export interface RendererMetricsWidgetMount {
    dispose(): void;
}
/**
 * Mounts a small Engine-owned DOM readout of the latest admitted renderer
 * diagnostics. It only polls the live-debug route; it neither schedules a
 * browser animation frame nor submits renderer work.
 */
export declare function mountRendererMetricsWidget(host: HTMLElement, options?: RendererMetricsWidgetMountOptions): RendererMetricsWidgetMount;
