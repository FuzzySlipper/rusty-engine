import type { RenderPublicationFrontier } from '@rusty-engine/render-contracts';

/** These are draw-submission facts, not proof of GPU completion or stream capture. */
export interface RendererPresentationObservation<TFrame> {
  readonly surfaceId: string;
  readonly state: 'submitted' | 'pending' | 'unavailable';
  readonly pendingRealizations: number;
  readonly realizedPublicationFrontiers: readonly RenderPublicationFrontier[];
  readonly realizedViewRevision: number;
  readonly submitted: TFrame | null;
  readonly gpuCompletion: 'unavailable';
  readonly captureCorrelation: 'unavailable';
}

export interface RendererSubmittedFrontier {
  readonly publicationFrontiers: readonly RenderPublicationFrontier[];
  readonly viewRevision: number;
  readonly viewport: {
    readonly cssWidth: number;
    readonly cssHeight: number;
    readonly backingWidth: number;
    readonly backingHeight: number;
  };
}

export function observeRendererPresentation<TFrame extends RendererSubmittedFrontier>(
  surfaceId: string,
  available: boolean,
  pendingRealizations: number,
  frontiers: readonly RenderPublicationFrontier[],
  viewRevision: number,
  viewport: RendererSubmittedFrontier['viewport'],
  submitted: TFrame | null,
  submissionRequested = false,
): RendererPresentationObservation<TFrame> {
  const pending = submissionRequested || submitted === null || pendingRealizations > 0
    || viewRevision !== submitted.viewRevision
    || frontiers.length !== submitted.publicationFrontiers.length
    || frontiers.some((frontier) => !submitted.publicationFrontiers.some((drawn) =>
      drawn.stream === frontier.stream && drawn.revision === frontier.revision))
    || viewport.cssWidth !== submitted.viewport.cssWidth
    || viewport.cssHeight !== submitted.viewport.cssHeight
    || viewport.backingWidth !== submitted.viewport.backingWidth
    || viewport.backingHeight !== submitted.viewport.backingHeight;
  return Object.freeze({
    surfaceId, state: !available ? 'unavailable' : pending ? 'pending' : 'submitted',
    pendingRealizations, realizedPublicationFrontiers: frontiers,
    realizedViewRevision: viewRevision, submitted,
    gpuCompletion: 'unavailable', captureCorrelation: 'unavailable',
  });
}
