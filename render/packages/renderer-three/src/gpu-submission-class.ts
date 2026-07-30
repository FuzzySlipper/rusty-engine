import type { RendererGpuSubmissionClass } from './gpu-submission-duty.js';

/**
 * Classify only renderer identities with a concrete browser scheduling
 * distinction. The raw driver string remains backend-local.
 */
export function classifyGpuSubmissionRendererName(
  renderer: unknown,
): RendererGpuSubmissionClass {
  if (typeof renderer !== 'string' || renderer.length === 0) {
    return 'unknown';
  }
  return /swiftshader|llvmpipe|software rasterizer|software renderer|microsoft basic render/iu
    .test(renderer)
    ? 'software'
    : 'accelerated';
}
