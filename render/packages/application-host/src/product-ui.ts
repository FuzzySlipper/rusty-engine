/**
 * What a product UI module sees: the mount signature and the context ports
 * the Engine passes it. Types only, depending on nothing but the generated
 * wire contracts, so the SDK can ship them as `@rusty-engine/product-ui` for
 * product UIs to compile against.
 */
import type {
  RuntimeInputWireFact,
  RuntimeInputWireIntentValue,
  RuntimeUiProjectionEnvelope,
} from './generated/contracts.js';

export type {
  JsonValue,
  RuntimeInputWireIntentValue,
  RuntimeUiProjectionEnvelope,
} from './generated/contracts.js';

export type RustyApplicationInteractionMode =
  | 'gameplay'
  | 'interface'
  | 'modal';

export interface RustyApplicationUiPort {
  readonly active: () => boolean;
  /**
   * Classify one original host event before a downstream adapter gives it
   * gameplay meaning. Interactive UI is rejected synchronously even before a
   * later click handler changes the coarse interaction mode.
   */
  readonly allowsGameplayInput: (event: Event) => boolean;
  readonly focusGameplay: () => void;
  readonly interactionMode: () => RustyApplicationInteractionMode;
  readonly setInteractionMode: (mode: RustyApplicationInteractionMode) => void;
  /** The UI scale; 1 until set. The product reads it as `CameraView.ReadSurface().UiScale`. */
  readonly scale: () => number;
  /**
   * Set the UI scale (finite, 0.25 to 4), as a player's UI-scale setting: the
   * document root's font size and its `--rusty-ui-scale` property follow it,
   * so rem-sized UI scales.
   */
  readonly setScale: (scale: number) => void;
}

/**
 * Ties camera views to UI elements. A camera the product anchors under
 * `name` (`CameraView.SetViewportAnchor`) draws its primary views over the
 * element anchored under the same name, following it on resize and layout
 * change with no product code running.
 */
export interface RustyApplicationUiViewportPort {
  /** Anchor `name` to `element`; returns a function that removes the anchor. */
  readonly anchor: (name: string, element: Element) => () => void;
}

/**
 * A claim's value: a digital or axis value, or a product payload. A
 * payload's data is the product's own JSON value, typed by the product.
 */
export type RustyApplicationUiIntentValue =
  | Exclude<RuntimeInputWireIntentValue, { readonly kind: 'product-payload' }>
  | { readonly kind: 'product-payload'; readonly contract: string; readonly data: unknown };

/** Mounted DOM UI can emit a claim, but cannot drain or bind the input lane. */
export interface RustyApplicationUiIntentsPort {
  readonly claim: (
    intent: string,
    value: RustyApplicationUiIntentValue,
  ) => void;
}

export interface RustyApplicationUiContext {
  readonly ui: RustyApplicationUiPort;
  /** Read-only current Product UI projection and subscription view. */
  readonly projection?: RustyApplicationUiProjectionView;
  /** Claim-only adapter for the shared ordered Engine input lane. */
  readonly intents?: RustyApplicationUiIntentsPort;
  /** Read-only controller observations owned exclusively by interface mode. */
  readonly input?: RustyApplicationUiInputPort;
  /** Camera views that follow UI elements. */
  readonly viewport: RustyApplicationUiViewportPort;
}

export interface RustyApplicationUiInputPort {
  /** Synchronous observation on the host cadence; returns an unsubscribe function. */
  readonly subscribe: (observer: (input: RustyApplicationInterfaceInputObservation) => void) => () => void;
}

export interface RustyApplicationUiOwner {
  readonly dispose: () => void | Promise<void>;
}

/**
 * Mount trusted downstream product UI into the Engine-owned composition root.
 * This is an application composition seam, not an untrusted plugin boundary.
 * The root is hit-test transparent: native interactive controls and descendants
 * marked `data-rusty-ui-interactive` receive pointer events, while other overlay
 * regions pass through to the Engine canvas and its input arbitration.
 */
export type RustyApplicationUiMount = (
  root: HTMLElement,
  context: RustyApplicationUiContext,
) => void | RustyApplicationUiOwner | Promise<void | RustyApplicationUiOwner>;

/** UI-owned observations from the existing selected-controller sampler. */
export interface RustyApplicationInterfaceInputObservation {
  readonly context: 'interface';
  readonly fact: Extract<RuntimeInputWireFact, {
    readonly kind: 'controller-button' | 'controller-axis' | 'controller-button-value';
  }>;
}

export interface RustyApplicationUiProjectionView {
  /** Returns the current immutable envelope, or null before the first value. */
  readonly current: () => RuntimeUiProjectionEnvelope | null;
  /** Subscribe to the current value. Rebinding publishes null before later values. */
  readonly subscribe: (
    listener: (value: RuntimeUiProjectionEnvelope | null) => void,
  ) => () => void;
}
