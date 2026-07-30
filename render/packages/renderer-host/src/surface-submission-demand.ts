export interface RendererSurfaceViewportState {
  readonly bufferHeight: number;
  readonly bufferWidth: number;
  readonly clientHeight: number;
  readonly clientWidth: number;
}

export interface RendererSurfaceContinuousDemand {
  readonly controls: boolean;
  readonly presentation: boolean;
  readonly retainedAnimation: boolean;
}

/**
 * Coalesces automatic surface submissions without creating another scheduler.
 *
 * The owning surface keeps its single RAF callback so input and resize demand
 * can be observed, but an unchanged static scene is not submitted to the
 * backend on every display refresh.
 */
export class RendererSurfaceSubmissionDemand {
  #requested = false;
  #viewport: RendererSurfaceViewportState;

  constructor(viewport: RendererSurfaceViewportState) {
    this.#viewport = viewport;
  }

  request(): void {
    this.#requested = true;
  }

  consume(
    viewport: RendererSurfaceViewportState,
    continuous: RendererSurfaceContinuousDemand,
  ): boolean {
    const viewportChanged = !sameViewport(this.#viewport, viewport);
    this.#viewport = viewport;
    const shouldSubmit = this.#requested
      || viewportChanged
      || continuous.controls
      || continuous.presentation
      || continuous.retainedAnimation;
    this.#requested = false;
    return shouldSubmit;
  }

  submitted(viewport: RendererSurfaceViewportState): void {
    this.#viewport = viewport;
    this.#requested = false;
  }
}

function sameViewport(
  left: RendererSurfaceViewportState,
  right: RendererSurfaceViewportState,
): boolean {
  return left.bufferHeight === right.bufferHeight
    && left.bufferWidth === right.bufferWidth
    && left.clientHeight === right.clientHeight
    && left.clientWidth === right.clientWidth;
}
