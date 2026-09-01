import type {
  GhostPlateCaptureSettings,
  GhostPlateConfig,
  GhostPlateHandle,
  GhostPlateDescriptor,
  GhostPlatePatch,
  GhostPlateProjectionOp,
  PresentationFrameDiff,
} from '@rusty-engine/render-contracts';

export interface RendererGhostPlateOperationReceipt {
  readonly applied: boolean;
  readonly diagnostics: readonly { readonly code: string; readonly message: string }[];
}

export interface RendererGhostPlatePresentationReadout {
  readonly source: number;
  readonly sourceMatch: boolean;
  readonly currentSector: number;
  readonly localAzimuthDegrees: number | null;
  readonly capture: GhostPlateCaptureSettings;
  readonly config: GhostPlateConfig;
  readonly fallbackActive: boolean;
  readonly fallbackReason: string | null;
  readonly preparationCpuMilliseconds: number | null;
  readonly captureCpuSubmissionMilliseconds: number | null;
  readonly retainedResourceCounts: {
    readonly sectors: number;
    readonly meshes: number;
    readonly materials: number;
    readonly borrowedTextures: number;
  };
  readonly disposed: boolean;
}

/** Backend-private realization capability injected through the renderer surface. */
export interface RendererGhostPlatePresentation {
  create(descriptor: GhostPlateDescriptor): RendererGhostPlateOperationReceipt;
  update(patch: GhostPlatePatch): RendererGhostPlateOperationReceipt;
  recapture(capture: GhostPlateCaptureSettings | null): RendererGhostPlateOperationReceipt;
  destroy(): RendererGhostPlateOperationReceipt;
  readout(): RendererGhostPlatePresentationReadout;
  dispose(): void;
}

export interface RendererGhostPlateReadout {
  readonly activePlates: number;
  readonly plates: readonly {
    readonly handle: GhostPlateHandle;
    readonly source: number;
    readonly sourceMatch: boolean;
    readonly currentSector: number;
    readonly localAzimuthDegrees: number | null;
    readonly capture: GhostPlateCaptureSettings;
    readonly config: GhostPlateConfig;
    readonly fallbackActive: boolean;
    readonly fallbackReason: string | null;
    readonly preparationCpuMilliseconds: number | null;
    readonly captureCpuSubmissionMilliseconds: number | null;
    readonly retainedResourceCounts: {
      readonly sectors: number;
      readonly meshes: number;
      readonly materials: number;
      readonly borrowedTextures: number;
    };
  }[];
}

export interface RendererGhostPlateFrameReceipt {
  readonly applied: number;
  readonly diagnostics: readonly {
    readonly code: string;
    readonly sequence: number;
    readonly handle: number | null;
    readonly message: string;
  }[];
}

/** Renderer-neutral presentation host over the backend-private ghost owner. */
export class RendererGhostPlateHost {
  readonly #createPresentation: (id: string) => RendererGhostPlatePresentation;
  readonly #active = new Map<GhostPlateHandle, RendererGhostPlatePresentation>();

  constructor(options: { readonly createPresentation: (id: string) => RendererGhostPlatePresentation }) {
    this.#createPresentation = options.createPresentation;
  }

  applyPresentation(frame: PresentationFrameDiff): RendererGhostPlateFrameReceipt {
    const diagnostics: RendererGhostPlateFrameReceipt['diagnostics'][number][] = [];
    let applied = 0;
    for (const operation of frame.ops) {
      if (operation.domain !== 'ghostPlate') continue;
      const receipt = this.#apply(operation.meta.sequence, operation.op);
      if (receipt.applied) applied += 1;
      else diagnostics.push(receipt.diagnostic);
    }
    return Object.freeze({ applied, diagnostics: Object.freeze(diagnostics) });
  }

  readout(): RendererGhostPlateReadout {
    return Object.freeze({
      activePlates: this.#active.size,
      plates: Object.freeze([...this.#active.entries()].map(([handle, presentation]) => {
        const value = presentation.readout();
        return Object.freeze({ handle, ...value });
      }).sort((left, right) => Number(left.handle) - Number(right.handle))),
    });
  }

  dispose(): void {
    for (const presentation of this.#active.values()) presentation.dispose();
    this.#active.clear();
  }

  #apply(sequence: number, operation: GhostPlateProjectionOp):
    | { readonly applied: true }
    | { readonly applied: false; readonly diagnostic: RendererGhostPlateFrameReceipt['diagnostics'][number] } {
    const handle = operation.handle;
    try {
      if (operation.op === 'create') {
        if (this.#active.has(handle)) return this.#rejected(sequence, handle, 'duplicateHandle', 'ghost plate handle is already active');
        const presentation = this.#createPresentation(`ghost-plate-${String(handle)}`);
        const receipt = presentation.create(operation.descriptor);
        if (!receipt.applied) {
          presentation.dispose();
          return this.#rejected(sequence, handle, receipt.diagnostics[0]?.code ?? 'hostFailure', receipt.diagnostics[0]?.message ?? 'ghost plate creation failed');
        }
        this.#active.set(handle, presentation);
        return { applied: true };
      }
      const presentation = this.#active.get(handle);
      if (presentation === undefined) return this.#rejected(sequence, handle, 'unknownHandle', 'ghost plate handle is not active');
      if (operation.op === 'update') {
        const receipt = presentation.update(operation.patch);
        return receipt.applied ? { applied: true } : this.#rejected(sequence, handle, receipt.diagnostics[0]?.code ?? 'hostFailure', receipt.diagnostics[0]?.message ?? 'ghost plate update failed');
      }
      if (operation.op === 'recapture') {
        const receipt = presentation.recapture(operation.capture);
        return receipt.applied ? { applied: true } : this.#rejected(sequence, handle, receipt.diagnostics[0]?.code ?? 'hostFailure', receipt.diagnostics[0]?.message ?? 'ghost plate recapture failed');
      }
      const receipt = presentation.destroy();
      if (!receipt.applied) return this.#rejected(sequence, handle, receipt.diagnostics[0]?.code ?? 'hostFailure', receipt.diagnostics[0]?.message ?? 'ghost plate destruction failed');
      presentation.dispose();
      this.#active.delete(handle);
      return { applied: true };
    } catch (cause) {
      return this.#rejected(sequence, handle, 'hostFailure', cause instanceof Error ? cause.message : String(cause));
    }
  }

  #rejected(sequence: number, handle: GhostPlateHandle, code: string, message: string) {
    return { applied: false as const, diagnostic: Object.freeze({ code, sequence, handle: Number(handle), message }) };
  }
}
