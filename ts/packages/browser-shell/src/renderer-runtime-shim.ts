import type { RenderFrameDiff } from "@asha/contracts";

/** The retained renderer's optional encoded-frame path is not part of this host. */
export class RuntimeBridgeError extends Error {
  readonly kind = "unsupported_operation";

  constructor(message: string) {
    super(message);
    this.name = "RuntimeBridgeError";
  }
}

export function decodeRenderFrameDiff(_payload: unknown): RenderFrameDiff {
  throw new RuntimeBridgeError(
    "encoded Asha runtime frames are disabled; Rusty Engine supplies typed projection diffs",
  );
}
