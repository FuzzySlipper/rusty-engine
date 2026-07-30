import type { RenderFrameDiff } from '@rusty-engine/render-contracts';
import type {
  RendererEditorViewportChannelReceipt,
  RendererInspectionSurface,
  RendererSurfaceSubmissionSample,
} from '@rusty-engine/renderer-host';

export type StudioViewportFrameUpdateKind =
  | 'complete'
  | 'incremental'
  | 'presentation';

/** Public immutable observation emitted after one accepted Studio frame submission. */
export interface StudioViewportFrameSubmitted {
  readonly kind: 'rusty_studio_viewport_frame_submitted.v1';
  readonly generation: number;
  readonly updateKind: StudioViewportFrameUpdateKind;
  readonly submission: RendererSurfaceSubmissionSample;
}

type StudioViewportSubmissionSurface = Pick<
  RendererInspectionSurface,
  'applyAuthoredFrame' | 'renderOnce' | 'replaceFrame' | 'submission'
>;

export interface StudioViewportFrameSubmissionResult {
  readonly event: StudioViewportFrameSubmitted | null;
  readonly receipt: RendererEditorViewportChannelReceipt;
}

/**
 * Apply one complete or incremental Studio frame, explicitly submit it, then
 * read the renderer-owned sample. Reading after the submission is intentional:
 * an earlier automatic sample must never be associated with the new generation.
 */
export function submitStudioViewportFrame(
  surface: StudioViewportSubmissionSurface,
  frame: RenderFrameDiff,
  generation: number,
  updateKind: StudioViewportFrameUpdateKind,
): StudioViewportFrameSubmissionResult {
  const receipt = updateKind === 'incremental'
    ? surface.applyAuthoredFrame(frame)
    : surface.replaceFrame(frame);
  if (!receipt.applied) {
    return Object.freeze({ event: null, receipt });
  }

  surface.renderOnce();
  const event = Object.freeze({
    kind: 'rusty_studio_viewport_frame_submitted.v1',
    generation,
    updateKind,
    submission: surface.submission(),
  } satisfies StudioViewportFrameSubmitted);
  return Object.freeze({ event, receipt });
}
