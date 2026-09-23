import type { RenderFrameDiff, RenderHandle } from './render.js';
import type { RendererCompositionCamera } from './view-composition.js';

/** Frozen Engine scene output, delivered only after its immutable resources. */
export interface RenderOutputJob {
  readonly id: number;
  readonly source: RenderHandle;
  readonly frame: RenderFrameDiff;
  readonly operation:
    | { readonly kind: 'image'; readonly camera: RendererCompositionCamera;
        readonly width: number; readonly height: number;
        readonly background: readonly [number, number, number, number];
        readonly useCameraBackground: boolean;
        readonly exposure: number; readonly acesFilmic: boolean; readonly samples: number;
        readonly pose: { readonly handle: RenderHandle; readonly clip: string; readonly normalizedTime: number } | null }
    | { readonly kind: 'glb'; readonly includeAnimations: boolean };
}
export interface RenderOutputChunk {
  readonly id: number;
  readonly offset: number;
  readonly bytes: readonly number[];
  readonly complete: boolean;
  readonly error: string | null;
}
