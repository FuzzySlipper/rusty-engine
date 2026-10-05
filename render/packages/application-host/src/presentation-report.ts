/**
 * How this page presents the product, reported to the runtime on change.
 *
 * Each page cadence measures the presentation surface (the Engine canvas),
 * the device pixel ratio, the UI scale and every anchored element's rect,
 * normalized to the surface and bottom-left based like a camera viewport.
 * When anything differs from the last report, the page posts the new one;
 * one post is in flight at a time and a change during it is sent after.
 * The runtime draws anchored camera views at those rects without a product
 * call, and the product reads the surface through `CameraView.ReadSurface`.
 * This is presentation, not gameplay input: it carries no binding or
 * sequence, and the latest report from any page wins.
 *
 * Wire format: `rust/crates/product-host/src/presentation.rs`.
 */
import {
  PRESENTATION_PATH,
  type ProductHostPresentationReport,
  type ProductHostViewportAnchorReport,
} from './generated/contracts.js';

export interface RustyApplicationPresentationReporter {
  /** Anchor `name` to `element`; returns a function that removes it. */
  readonly anchor: (name: string, element: Element) => () => void;
  /** Measure on this cadence and report a change. */
  readonly tick: () => void;
  readonly dispose: () => void;
}

/** One element's rect within `surface`, normalized and bottom-left based. */
export function anchorRect(
  surface: { left: number; top: number; width: number; height: number },
  element: { left: number; top: number; width: number; height: number },
): Omit<ProductHostViewportAnchorReport, 'name'> {
  const width = Math.max(1, surface.width);
  const height = Math.max(1, surface.height);
  return {
    x: (element.left - surface.left) / width,
    y: (surface.top + height - (element.top + element.height)) / height,
    width: element.width / width,
    height: element.height / height,
  };
}

export function createRustyApplicationPresentationReporter(
  surface: HTMLElement,
  uiScale: () => number,
): RustyApplicationPresentationReporter {
  const window = surface.ownerDocument.defaultView;
  const anchors = new Map<string, Element>();
  let sent = '';
  let sending = false;
  let disposed = false;

  const measure = (): ProductHostPresentationReport | null => {
    const bounds = surface.getBoundingClientRect();
    if (bounds.width <= 0 || bounds.height <= 0) return null;
    const reported: ProductHostViewportAnchorReport[] = [];
    for (const [name, element] of anchors) {
      if (!element.isConnected) continue;
      reported.push({ name, ...anchorRect(bounds, element.getBoundingClientRect()) });
    }
    return {
      cssWidth: bounds.width,
      cssHeight: bounds.height,
      devicePixelRatio: window?.devicePixelRatio ?? 1,
      uiScale: uiScale(),
      anchors: reported,
    };
  };

  const send = (): void => {
    if (disposed || sending) return;
    const report = measure();
    if (report === null) return;
    const body = JSON.stringify(report);
    if (body === sent) return;
    sending = true;
    void fetch(PRESENTATION_PATH, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
      cache: 'no-store',
    }).then(async (response) => {
      // Reading the empty body finishes the request; left unread, Chromium
      // reports it as aborted.
      await response.arrayBuffer();
      // A refused report is sent again on the next change.
      if (response.ok) sent = body;
    }).catch(() => undefined).finally(() => {
      sending = false;
    });
  };

  return {
    anchor: (name, element) => {
      anchors.set(name, element);
      return () => {
        if (anchors.get(name) === element) anchors.delete(name);
      };
    },
    tick: send,
    dispose: () => {
      disposed = true;
      anchors.clear();
    },
  };
}
