import type { RendererGpuSubmissionClass } from './gpu-submission-duty.js';

const SOFTWARE_RENDERER_PIXEL_RATIO_CEILING = 0.375;

/**
 * Resolve the backing-buffer ratio owned by the concrete browser backend.
 *
 * Hardware-backed and unknown renderers preserve the caller's requested
 * ratio. Positively identified software rasterizers cap only the backing
 * buffer, leaving CSS layout, camera projection, pointer coordinates, and
 * retained content unchanged.
 */
export function resolveRendererPixelRatio(
  requestedPixelRatio: number,
  rendererClass: RendererGpuSubmissionClass,
): number {
  if (!Number.isFinite(requestedPixelRatio) || requestedPixelRatio <= 0) {
    throw new RangeError('renderer pixel ratio must be finite and greater than zero');
  }
  return rendererClass === 'software'
    ? Math.min(requestedPixelRatio, SOFTWARE_RENDERER_PIXEL_RATIO_CEILING)
    : requestedPixelRatio;
}
