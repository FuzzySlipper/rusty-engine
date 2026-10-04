/**
 * Requests pointer lock on the canvas and releases it again if the browser
 * grants the request after `wanted` stops holding.
 *
 * The browser grants pointer lock asynchronously. Between `focusGameplay` and
 * the grant the host can enter interface mode, close or dispose; releasing an
 * already-held lock at that moment finds nothing to release, so the late grant
 * is reconciled here against the host's current wish.
 */
export function requestPointerLockWhile(canvas: HTMLCanvasElement, wanted: () => boolean): void {
  let request: Promise<void> | undefined;
  try {
    request = canvas.requestPointerLock() as Promise<void> | undefined;
  } catch {
    // Pointer lock can be rejected by host policy or a missing user gesture.
    return;
  }
  void request?.then(
    () => {
      const document = canvas.ownerDocument;
      if (!wanted() && document.pointerLockElement === canvas) document.exitPointerLock();
    },
    () => undefined,
  );
}
