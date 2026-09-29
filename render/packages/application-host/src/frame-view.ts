/**
 * The world the runtime renders, shown on the Engine canvas.
 *
 * The runtime renders the world with wgpu. With `stream` this view pulls the
 * frames it draws, one request per frame, and paints them on the canvas under
 * the product UI. A frame a video clip covers is shown above the product UI.
 * With `window` the desktop shell presents the world to its native window
 * under this page, so the canvas stays transparent. Either way the canvas
 * stays the application's focus, pointer-lock and input target, and this view
 * runs the one page cadence the application samples input on.
 *
 * Wire format: see `rust/crates/product-dev-host/src/frames.rs`.
 */
export const RUSTY_APPLICATION_FRAME_STREAM_PATH = '/__rusty/product/runtime/frames';

/** Where the runtime draws the world: streamed to this page, or to the desktop window. */
export type RustyApplicationRenderOutput = 'stream' | 'window';

const FRAME_MAGIC = 0x31465352; // "RSF1", little-endian
const FRAME_MIN_HEADER_BYTES = 40;
const FRAME_FORMAT_JPEG = 1;
const FRAME_FORMAT_RGBA8 = 2;
const FRAME_FLAG_HELD = 1;
const FRAME_FLAG_VIDEO = 2;
/** Above the product UI while a video clip covers the frame. */
const VIDEO_Z_INDEX = '1000';
const RETRY_DELAY_MS = 500;

export interface RustyApplicationStreamedFrame {
  readonly sequence: number;
  readonly step: number;
  readonly width: number;
  readonly height: number;
  readonly format: number;
  readonly held: boolean;
  readonly video: boolean;
  readonly payload: Uint8Array;
}

/** Parses one `RSF1` frame: its header, then its payload. */
export function parseRustyApplicationStreamedFrame(bytes: Uint8Array): RustyApplicationStreamedFrame {
  const header = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (bytes.byteLength < FRAME_MIN_HEADER_BYTES || header.getUint32(0, true) !== FRAME_MAGIC) {
    throw new Error('frame response is not an RSF1 frame');
  }
  const headerBytes = header.getUint32(4, true);
  return {
    sequence: Number(header.getBigUint64(8, true)),
    step: Number(header.getBigUint64(16, true)),
    width: header.getUint32(24, true),
    height: header.getUint32(28, true),
    format: header.getUint8(32),
    held: (header.getUint8(33) & FRAME_FLAG_HELD) !== 0,
    video: (header.getUint8(33) & FRAME_FLAG_VIDEO) !== 0,
    payload: bytes.subarray(headerBytes, headerBytes + header.getUint32(36, true)),
  };
}

async function decodeFrame(frame: RustyApplicationStreamedFrame): Promise<ImageBitmap> {
  if (frame.format === FRAME_FORMAT_JPEG) {
    return createImageBitmap(new Blob([frame.payload as Uint8Array<ArrayBuffer>], { type: 'image/jpeg' }));
  }
  if (frame.format === FRAME_FORMAT_RGBA8) {
    const pixels = new Uint8ClampedArray(frame.payload.buffer as ArrayBuffer, frame.payload.byteOffset, frame.payload.byteLength);
    return createImageBitmap(new ImageData(pixels, frame.width, frame.height));
  }
  throw new Error(`frame stream sent unknown format ${frame.format}`);
}

export interface RustyApplicationFrameView {
  readonly dispose: () => void;
}

/**
 * Starts showing `output` on `canvas` and calls `onCadence` on every
 * animation frame until disposed.
 */
export function mountRustyApplicationFrameView(
  canvas: HTMLCanvasElement,
  output: RustyApplicationRenderOutput,
  onCadence: (timeMs: number) => void,
): RustyApplicationFrameView {
  const document = canvas.ownerDocument;
  const window = document.defaultView;
  if (window === null) throw new Error('the frame view needs a window');
  const streamed = output === 'stream';
  const restingZIndex = canvas.style.zIndex;
  const context = streamed ? canvas.getContext('2d', { alpha: false }) : null;
  if (streamed && context === null) throw new Error('the streaming view needs a 2D canvas context');
  let latest: ImageBitmap | null = null;
  let pending: RustyApplicationStreamedFrame | null = null;
  let decoding = false;
  let animationFrame: number | null = null;
  let disposed = false;
  const pulling = new AbortController();

  // One frame pixel per CSS pixel: the development stream's measured size.
  const backingSize = (): [number, number] => [
    Math.max(1, Math.round(canvas.clientWidth)),
    Math.max(1, Math.round(canvas.clientHeight)),
  ];

  const showFrame = (frame: RustyApplicationStreamedFrame, bitmap: ImageBitmap): void => {
    latest?.close();
    latest = bitmap;
    // The harness can correlate a screenshot with the step it shows.
    canvas.dataset['rustyFrameSequence'] = String(frame.sequence);
    canvas.dataset['rustyFrameStep'] = String(frame.step);
    canvas.dataset['rustyFrameHeld'] = frame.held ? 'true' : 'false';
    canvas.dataset['rustyFrameVideo'] = frame.video ? 'true' : 'false';
    canvas.style.zIndex = frame.video ? VIDEO_Z_INDEX : restingZIndex;
    context?.drawImage(bitmap, 0, 0, canvas.width, canvas.height);
  };

  // Decode only the newest frame; frames that arrive meanwhile replace it.
  const decodeLatest = (): void => {
    if (decoding || pending === null || disposed) return;
    const frame = pending;
    pending = null;
    decoding = true;
    void decodeFrame(frame).then((bitmap) => {
      if (disposed) bitmap.close();
      else showFrame(frame, bitmap);
    }, () => undefined).finally(() => {
      decoding = false;
      decodeLatest();
    });
  };

  // Frames are pulled, never pushed, so a slow page skips frames instead of
  // queueing them in socket buffers. Two requests are outstanding: one takes
  // the next frame and the other already waits for the frame after it, so a
  // request's setup is never between a frame being drawn and its display.
  const pull = (): void => {
    let newest = 0;
    let nextAfter = 0;
    const ask = async (after: number): Promise<void> => {
      while (!pulling.signal.aborted) {
        const [width, height] = backingSize();
        // The CSS width gives the runtime this page's pixel ratio, so labels
        // and pixel-sized sprites keep their CSS size in the frame.
        const cssWidth = Math.max(1, Math.round(canvas.clientWidth));
        try {
          const response = await fetch(
            `${RUSTY_APPLICATION_FRAME_STREAM_PATH}?after=${after}&width=${width}&height=${height}&cssWidth=${cssWidth}`,
            { cache: 'no-store', signal: pulling.signal },
          );
          if (response.status === 204) continue;
          if (!response.ok) throw new Error(`frame request refused: HTTP ${response.status}`);
          const frame = parseRustyApplicationStreamedFrame(new Uint8Array(await response.arrayBuffer()));
          if (frame.sequence > newest) {
            newest = frame.sequence;
            pending = frame;
            decodeLatest();
          }
          nextAfter = Math.max(nextAfter + 1, newest);
          void ask(nextAfter);
          return;
        } catch {
          if (pulling.signal.aborted) return;
          await new Promise((resolve) => setTimeout(resolve, RETRY_DELAY_MS));
        }
      }
    };
    void ask(0);
    void ask(0);
  };

  // The canvas backs its CSS size; the next request asks for that size.
  const resize = (): void => {
    const [width, height] = backingSize();
    if (canvas.width === width && canvas.height === height) return;
    canvas.width = width;
    canvas.height = height;
    if (latest !== null) context?.drawImage(latest, 0, 0, width, height);
  };
  const resizeObserver = streamed ? new ResizeObserver(resize) : null;
  resizeObserver?.observe(canvas);

  const onAnimationFrame = (timeMs: number): void => {
    animationFrame = null;
    if (disposed) return;
    onCadence(timeMs);
    animationFrame = window.requestAnimationFrame(onAnimationFrame);
  };

  if (streamed) {
    resize();
    pull();
  }
  animationFrame = window.requestAnimationFrame(onAnimationFrame);

  return Object.freeze({
    dispose: () => {
      if (disposed) return;
      disposed = true;
      pulling.abort();
      if (animationFrame !== null) window.cancelAnimationFrame(animationFrame);
      animationFrame = null;
      resizeObserver?.disconnect();
      canvas.style.zIndex = restingZIndex;
      latest?.close();
      latest = null;
    },
  });
}
