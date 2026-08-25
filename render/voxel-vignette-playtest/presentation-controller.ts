import type {
  RustyApplicationCameraPose,
  RustyApplicationRendererPort,
} from '@rusty-engine/application-host';

import {
  loadVignetteContent,
  vignetteLightingFrame,
  type VignetteLighting,
  type VignetteVariantId,
} from './scene.js';

export interface VignettePresentationResult {
  readonly applied: boolean;
  readonly message: string;
}

export interface VignettePresentationActions {
  readonly applyLighting: (lighting: VignetteLighting) => VignettePresentationResult;
  readonly publishPose: (pose: RustyApplicationCameraPose) => void;
  readonly render: () => void;
  readonly replaceVariant: (variant: VignetteVariantId) => Promise<VignettePresentationResult>;
}

/** Named visual-gate adapter; DOM UI never receives the application renderer. */
export function createVignettePresentationActions(
  renderer: () => RustyApplicationRendererPort | null,
): VignettePresentationActions {
  return Object.freeze({
    applyLighting: (lighting: VignetteLighting): VignettePresentationResult => {
      const current = renderer();
      if (current === null) return { applied: false, message: 'application renderer is not ready' };
      const receipt = current.applyFrame(vignetteLightingFrame(lighting));
      if (receipt.applied) current.renderOnce();
      return {
        applied: receipt.applied,
        message: receipt.diagnostics.map((diagnostic) => diagnostic.message).join('; '),
      };
    },
    publishPose: (pose: RustyApplicationCameraPose): void => {
      renderer()?.setCameraPose(pose);
    },
    render: (): void => {
      renderer()?.renderOnce();
    },
    replaceVariant: async (variant: VignetteVariantId): Promise<VignettePresentationResult> => {
      const current = renderer();
      if (current === null) return { applied: false, message: 'application renderer is not ready' };
      const receipt = await current.replaceContent(await loadVignetteContent(variant));
      return {
        applied: receipt.applied,
        message: receipt.diagnostics.map((diagnostic) => diagnostic.message).join('; ')
          || 'application host rejected replacement',
      };
    },
  });
}
