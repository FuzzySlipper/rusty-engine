import { type RustyApplicationFrame, type RustyApplicationHost, type RustyApplicationHostReadout, type RustyApplicationPresentationFrame, type RustyApplicationRendererOptions, type RustyApplicationRuntimeIdentity, type RustyApplicationRuntimeInputEnvelope, type RustyApplicationRuntimeInputOptions, type RustyApplicationUiMount, type RustyApplicationUiProjectionEnvelope, type RustyApplicationPresentationAspectBounds } from '@rusty-engine/application-host';
/** Fixed current artifact identity; compatibility follows actual code changes. */
export declare const PRODUCT_BROWSER_HOST_ARTIFACT: "rusty.product.browser-host";
export type ProductBrowserRuntimeMode = 'realtime' | 'demand' | 'external';
/**
 * Selects the owner that admits fixed-step realtime work.
 *
 * Browser products use the Engine renderer cadence by default. A packaged
 * product with an in-process Rust service can select `rust-host`; the
 * WebView still drains typed input on each animation frame and receives
 * retained outputs through the runtime subscription, but it never asks the
 * runtime to advance from its presentation clock.
 */
export type ProductBrowserRealtimeAdvanceOwner = 'browser' | 'rust-host';
/**
 * The fixed lifecycle operation vocabulary carried by the local bridge. The
 * operation union is deliberately closed: product code cannot turn the
 * bridge into a method-name RPC or invoke an arbitrary runtime operation.
 */
export type ProductBrowserLifecycleOperation = {
    readonly kind: 'start';
} | {
    readonly kind: 'pause';
} | {
    readonly kind: 'resume';
} | {
    readonly kind: 'restart';
} | {
    readonly kind: 'shutdown';
} | {
    readonly kind: 'report-fault';
};
export type ProductBrowserRuntimeOperationKind = ProductBrowserLifecycleOperation['kind'] | 'advance-realtime' | 'admit-demand-step' | 'admit-external-step';
export interface ProductBrowserRuntimeOperationResult {
    readonly accepted: boolean;
    readonly operation: ProductBrowserRuntimeOperationKind;
    readonly binding?: RustyApplicationRuntimeIdentity;
    readonly readout?: ProductBrowserRuntimeReadout;
    readonly diagnostic?: string;
}
export interface ProductBrowserRuntimeInputResult {
    readonly accepted: boolean;
    readonly count: number;
    readonly binding?: RustyApplicationRuntimeIdentity;
    readonly readout?: ProductBrowserRuntimeReadout;
    readonly diagnostic?: string;
}
export interface ProductBrowserTimelineCompletion {
    /** Canonical decimal u64 ticket issued by runtime-timeline. */
    readonly ticket: string;
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly correlation: string;
    readonly outcome: {
        readonly kind: 'success';
        readonly data?: ProductBrowserJson;
    } | {
        readonly kind: 'failure';
        readonly data?: ProductBrowserJson;
    };
    readonly provenance: {
        readonly correlation: string;
        readonly detail?: ProductBrowserJson;
    };
}
export interface ProductBrowserTimelineCompletionResult {
    readonly accepted: boolean;
    /** Canonical decimal u64 ticket echoed by runtime-timeline. */
    readonly ticket: string;
    readonly binding?: RustyApplicationRuntimeIdentity;
    readonly readout?: ProductBrowserRuntimeReadout;
    readonly diagnostic?: string;
}
/** A bounded semantic-neutral readout emitted by the Rust runtime owner. */
export interface ProductBrowserRuntimeReadout {
    readonly artifact: 'rusty.product.runtime-readout';
    readonly runtime: RustyApplicationRuntimeIdentity;
    readonly mode: ProductBrowserRuntimeMode;
    readonly state: 'created' | 'running' | 'paused' | 'faulted' | 'shutdown';
    readonly admittedSimulationSteps: string;
    readonly admittedPresentations: string;
    readonly droppedRealtimeSteps: string;
    readonly clockRegressions: string;
    readonly scaledRemainder: number | null;
    readonly lastObservedTimeNs: string | null;
    readonly fault: 'owner-reported' | 'counter-exhausted' | null;
}
export interface ProductBrowserRuntimeBindingOutput {
    readonly kind: 'binding';
    readonly runtime: RustyApplicationRuntimeIdentity;
}
export type ProductBrowserRuntimeOutput = ProductBrowserRuntimeBindingOutput
/** Fixed host evidence that one Rust-owned realtime advance was accepted. */
 | {
    readonly kind: 'runtime-progress';
    readonly owner: 'rust-host';
} | {
    readonly kind: 'frame';
    readonly frame: RustyApplicationFrame;
} | {
    readonly kind: 'presentation';
    readonly frame: RustyApplicationPresentationFrame;
} | {
    readonly kind: 'ui-projection';
    readonly envelope: RustyApplicationUiProjectionEnvelope;
} | {
    readonly kind: 'runtime-readout';
    readonly readout: ProductBrowserRuntimeReadout;
};
export type ProductBrowserRuntimeOutputListener = (output: ProductBrowserRuntimeOutput) => void;
/**
 * Terminal local-transport failures. A dropped retained-output diff cannot
 * be recovered by EventSource retry; the host must stop until a fresh runtime
 * snapshot is mounted.
 */
export interface ProductBrowserRuntimeTerminalFailure {
    /** The fixed Engine failure lane; products never supply an arbitrary event name. */
    readonly kind: 'output-lag' | 'runtime-failure';
    readonly diagnostic: string;
}
export type ProductBrowserRuntimeTerminalFailureListener = (failure: ProductBrowserRuntimeTerminalFailure) => void;
/**
 * A source-linked local runtime adapter. The implementation may be a Rust
 * worker, an in-process native bridge, or a deterministic test adapter, but
 * the operation surface is fixed and named. It has no generic `call` or
 * arbitrary message method.
 */
export interface ProductBrowserRuntimeAdapter {
    readonly lifecycle: (operation: ProductBrowserLifecycleOperation) => Promise<ProductBrowserRuntimeOperationResult>;
    readonly input: (batch: readonly RustyApplicationRuntimeInputEnvelope[]) => Promise<ProductBrowserRuntimeInputResult>;
    readonly advanceRealtime: (observedTimeNs: string) => Promise<ProductBrowserRuntimeOperationResult>;
    readonly admitDemandStep?: () => Promise<ProductBrowserRuntimeOperationResult>;
    readonly admitExternalStep?: (step: string) => Promise<ProductBrowserRuntimeOperationResult>;
    readonly completeTimeline?: (completion: ProductBrowserTimelineCompletion) => Promise<ProductBrowserTimelineCompletionResult>;
    readonly subscribeTerminalFailures?: (listener: ProductBrowserRuntimeTerminalFailureListener) => () => void;
    readonly subscribeOutputs: (listener: ProductBrowserRuntimeOutputListener) => () => void;
    readonly dispose: () => Promise<void> | void;
}
/** The transport kept by the generated bridge and consumed by the host. */
export interface ProductBrowserRuntimeTransport {
    readonly lifecycle: ProductBrowserRuntimeAdapter['lifecycle'];
    readonly input: ProductBrowserRuntimeAdapter['input'];
    readonly advanceRealtime: ProductBrowserRuntimeAdapter['advanceRealtime'];
    readonly admitDemandStep?: NonNullable<ProductBrowserRuntimeAdapter['admitDemandStep']>;
    readonly admitExternalStep?: NonNullable<ProductBrowserRuntimeAdapter['admitExternalStep']>;
    readonly completeTimeline?: NonNullable<ProductBrowserRuntimeAdapter['completeTimeline']>;
    readonly subscribeTerminalFailures?: NonNullable<ProductBrowserRuntimeAdapter['subscribeTerminalFailures']>;
    readonly subscribeOutputs: ProductBrowserRuntimeAdapter['subscribeOutputs'];
    readonly dispose: ProductBrowserRuntimeAdapter['dispose'];
}
export declare function createProductBrowserRuntimeTransport(adapter: ProductBrowserRuntimeAdapter): ProductBrowserRuntimeTransport;
export interface ProductBrowserHostOptions {
    readonly root: HTMLElement;
    readonly transport: ProductBrowserRuntimeTransport;
    readonly lifecycleMode: ProductBrowserRuntimeMode;
    /**
     * Owner of realtime simulation admission. Defaults to `browser`; use
     * `rust-host` only when a packaged in-process Rust host advances the runtime
     * and publishes outputs through `transport.subscribeOutputs`.
     */
    readonly realtimeAdvanceOwner?: ProductBrowserRealtimeAdvanceOwner;
    readonly mountUi: RustyApplicationUiMount;
    readonly runtimeInput?: Omit<RustyApplicationRuntimeInputOptions, 'binding' | 'onAvailable'> & {
        readonly binding?: RustyApplicationRuntimeIdentity;
    };
    readonly uiProjection?: Omit<ProductBrowserUiProjectionOptions, 'binding'> & {
        readonly binding?: RustyApplicationRuntimeIdentity;
    };
    readonly renderer?: Omit<RustyApplicationRendererOptions, 'onCadence'>;
    readonly presentationAspectBounds?: RustyApplicationPresentationAspectBounds;
    readonly initialInteractionMode?: 'gameplay' | 'interface' | 'modal';
    readonly inputContext?: string;
    readonly loadingLabel?: string;
    readonly failureLabel?: string;
    /** Start the Rust runtime after the Engine host has mounted. Defaults true. */
    readonly autoStart?: boolean;
}
export interface ProductBrowserUiProjectionOptions {
    readonly expectedStream?: string;
    readonly expectedContract: string;
    readonly maximumBytes?: number;
    readonly maximumWireBytes?: number;
    readonly maximumNodes?: number;
    readonly maximumDepth?: number;
    readonly maximumStringBytes?: number;
    readonly maximumArrayLength?: number;
    readonly maximumObjectKeys?: number;
    readonly maximumSubscribers?: number;
}
export interface ProductBrowserHostReadout {
    readonly artifact: typeof PRODUCT_BROWSER_HOST_ARTIFACT;
    readonly state: 'starting' | 'ready' | 'failed' | 'disposed';
    readonly mode: ProductBrowserRuntimeMode;
    readonly realtimeAdvanceOwner: ProductBrowserRealtimeAdvanceOwner;
    readonly host: RustyApplicationHostReadout | null;
    readonly runtime: ProductBrowserRuntimeReadout | null;
    readonly lastFailure: string | null;
}
export interface ProductBrowserHost {
    readonly kind: 'rusty.product.browser-host';
    readonly application: RustyApplicationHost;
    readonly transport: ProductBrowserRuntimeTransport;
    readonly readout: () => ProductBrowserHostReadout;
    readonly completeTimeline: (completion: ProductBrowserTimelineCompletion) => Promise<ProductBrowserTimelineCompletionResult>;
    readonly admitDemandStep: () => Promise<ProductBrowserRuntimeOperationResult>;
    readonly admitExternalStep: (step: string) => Promise<ProductBrowserRuntimeOperationResult>;
    readonly dispose: () => Promise<void>;
}
export declare class ProductBrowserHostError extends Error {
    readonly code: 'invalid_options' | 'startup_failed' | 'output_failed' | 'transport_failed' | 'timeline_unavailable' | 'disposed';
    constructor(code: ProductBrowserHostError['code'], message: string, options?: ErrorOptions);
}
type ProductBrowserJson = null | boolean | number | string | readonly ProductBrowserJson[] | {
    readonly [key: string]: ProductBrowserJson;
};
/**
 * Mounts the one Engine-owned application composition root. The browser host
 * has no renderer implementation, product state, evaluator, or own cadence;
 * it drains the public input port from the application-host's existing
 * renderer cadence callback. Browser-owned realtime products also advance
 * from that callback; `realtimeAdvanceOwner: 'rust-host'` leaves advancement
 * to the packaged Rust host while subscribed outputs continue to drive the
 * retained presentation.
 */
export declare function mountProductBrowserHost(options: ProductBrowserHostOptions): Promise<ProductBrowserHost>;
/** Fixed relative location of the complete Engine runtime closure. */
export declare const PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE: "engine/product-browser-host.js";
export type ProductBrowserBundleAssetName = 'index.html' | 'main.js' | 'bridge.js' | typeof PRODUCT_BROWSER_BUNDLE_ENGINE_MODULE;
export interface ProductBrowserBundleAsset {
    readonly name: ProductBrowserBundleAssetName;
    readonly content: string;
}
export interface ProductBrowserBundleDescriptor {
    readonly artifact: 'rusty.product.bundle';
    readonly files: readonly {
        readonly name: ProductBrowserBundleAssetName;
        readonly content: string;
        readonly utf8Bytes: number;
    }[];
}
export interface ProductBrowserBundleTemplateOptions {
    /**
     * Exact built Engine-owned browser-host closure. The closure is copied into
     * the generated bundle instead of being resolved through a package manager
     * or a runtime import map. It must be ordinary JavaScript with no bare
     * package imports.
     */
    readonly engineHostModule: string;
    /** Product-relative compiled UI module, e.g. `./ui/main.js`. */
    readonly uiModule: string;
    /**
     * Product-relative generated Rust runtime route descriptor, e.g.
     * `./runtime-adapter.js`. It exports `PRODUCT_RUNTIME_HTTP_BASE_PATH`.
     */
    readonly runtimeAdapterModule: string;
    readonly lifecycleMode: ProductBrowserRuntimeMode;
    /** Defaults to `browser`; `rust-host` leaves realtime advancement to the packaged Rust host. */
    readonly realtimeAdvanceOwner?: ProductBrowserRealtimeAdvanceOwner;
    readonly uiProjection?: {
        readonly expectedStream: string;
        readonly expectedContract: string;
    } | null;
}
/**
 * Deterministic fixed composition assets copied by the Rusty CLI into the
 * ignored `generated/product-bundle` lane. Only source-linked module paths are
 * substituted; the HTML, main, bridge, and host topology remain Engine-owned.
 */
export declare function productBrowserBundleAssets(options: ProductBrowserBundleTemplateOptions): readonly ProductBrowserBundleAsset[];
/**
 * Returns the exact ordered byte descriptor used by the generator. The
 * descriptor intentionally contains no version counter: consumers can hash
 * each UTF-8 file and compare actual template changes when contracts evolve.
 */
export declare function productBrowserBundleDescriptor(options: ProductBrowserBundleTemplateOptions): ProductBrowserBundleDescriptor;
export {};
