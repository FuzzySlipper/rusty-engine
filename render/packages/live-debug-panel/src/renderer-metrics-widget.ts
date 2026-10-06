import {
  createLiveDebugHttpTransport,
  type LiveDebugResult,
  type LiveDebugTransport,
  type ProductHostRendererStatus,
} from '@rusty-engine/live-debug-client';

const REFRESH_INTERVAL_MS = 750;

/** Explicit, product-owned configuration for one optional renderer metrics widget. */
export interface RendererMetricsWidgetMountOptions {
  /**
   * When supplied, establishes the shared Engine widget state at mount. Omit
   * it to preserve the Engine default (hidden) or a console-selected state.
   */
  readonly initiallyVisible?: boolean;
  /** Uses the widget's same-origin HTTP transport when omitted. */
  readonly transport?: LiveDebugTransport;
}

/** Releases polling and the DOM node owned by one widget mount. */
export interface RendererMetricsWidgetMount {
  dispose(): void;
}

/**
 * Mounts a small Engine-owned DOM readout of the latest admitted renderer
 * diagnostics. It only polls the live-debug route; it neither schedules a
 * browser animation frame nor submits renderer work.
 */
export function mountRendererMetricsWidget(
  host: HTMLElement,
  options: RendererMetricsWidgetMountOptions = {},
): RendererMetricsWidgetMount {
  if (!(host instanceof HTMLElement)) {
    throw new TypeError('Renderer metrics widget mounting requires an HTMLElement host.');
  }
  const transport = options.transport ?? createLiveDebugHttpTransport();
  const root = host.ownerDocument.createElement('section');
  root.className = 'rusty-renderer-metrics-widget';
  root.setAttribute('aria-live', 'polite');
  root.style.cssText = [
    'background:rgb(12 17 24 / 88%)',
    'border:1px solid #5d7289',
    'border-radius:0.35rem',
    'color:#edf5ff',
    'font:0.75rem/1.35 ui-monospace,SFMono-Regular,Menlo,monospace',
    'padding:0.55rem 0.65rem',
    'white-space:pre-line',
  ].join(';');
  host.appendChild(root);

  let disposed = false;
  let refreshing = false;
  let request: AbortController | null = null;
  let timer: ReturnType<typeof setInterval> | null = null;

  const refresh = async (): Promise<void> => {
    if (disposed || refreshing) return;
    refreshing = true;
    request?.abort();
    const abort = new AbortController();
    request = abort;
    try {
      const result = await transport.execute('engine.renderer.status', abort.signal);
      if (disposed || abort.signal.aborted) return;
      renderSummary(root, decodeSummary(result));
    } catch (error: unknown) {
      if (!disposed && !abort.signal.aborted) renderError(root, error);
    } finally {
      if (request === abort) request = null;
      refreshing = false;
    }
  };

  const establishInitialVisibility = async (): Promise<void> => {
    if (options.initiallyVisible === undefined) return;
    const command = options.initiallyVisible ? 'engine.renderer.show' : 'engine.renderer.hide';
    try {
      await transport.execute(command);
    } catch (error: unknown) {
      if (!disposed) renderError(root, error);
    }
  };

  void establishInitialVisibility().finally(() => {
    if (disposed) return;
    void refresh();
    timer = setInterval(() => void refresh(), REFRESH_INTERVAL_MS);
  });

  return {
    dispose(): void {
      if (disposed) return;
      disposed = true;
      request?.abort();
      if (timer !== null) clearInterval(timer);
      root.remove();
    },
  };
}

function decodeSummary(result: LiveDebugResult): ProductHostRendererStatus {
  return JSON.parse(result.message) as ProductHostRendererStatus;
}

function renderSummary(root: HTMLElement, summary: ProductHostRendererStatus): void {
  const visible = summary.widget.visible;
  root.hidden = !visible;
  root.dataset['visible'] = String(visible);
  const renderer = summary.renderer;
  if (!summary.available || renderer === undefined) {
    root.textContent = `Renderer metrics\nUnavailable: ${summary.diagnostic ?? 'no renderer'}`;
    return;
  }
  const lines = ['Renderer metrics', `Adapter: ${renderer.adapter} | ${renderer.output}`];
  const stream = renderer.stream;
  if (stream !== undefined) {
    lines.push(
      `Frames: ${stream.framesPerSecond.toFixed(1)} per second | render ${milliseconds(stream.medianMs.render)} | readback ${milliseconds(stream.medianMs.readback)} | encode ${milliseconds(stream.medianMs.encode)}`,
      `Stream: ${kilobytes(stream.medianBytesPerFrame)} per frame | ${kilobytes(stream.bytesPerSecond)} per second`,
    );
  }
  const window = renderer.window;
  if (window !== undefined) {
    lines.push(
      `Frames: ${window.framesPerSecond.toFixed(1)} per second | acquire ${milliseconds(window.medianMs.acquire)} | lock ${milliseconds(window.medianMs.lock)} | draw ${milliseconds(window.medianMs.draw)} | present ${milliseconds(window.medianMs.present)}`,
    );
  }
  const compute = renderer.compute;
  lines.push(
    compute.refused === undefined
      ? `Compute pass: ${milliseconds(compute.medianGpuMs)} GPU over ${String(compute.timedFrames)} frames${compute.timestamps ? '' : ' (no timestamp queries)'} | ${String(compute.workgroups)} workgroups`
      : `Compute pass: refused (${compute.refused})`,
  );
  const skipped = Object.entries(renderer.skippedOps).map(([op, count]) => `${op} ×${String(count)}`);
  lines.push(`Skipped ops: ${skipped.length === 0 ? 'none' : skipped.join(', ')}${renderer.lastSkip === null ? '' : ` | last: ${renderer.lastSkip}`}`);
  root.textContent = lines.join('\n');
}

function renderError(root: HTMLElement, error: unknown): void {
  root.hidden = false;
  root.dataset['visible'] = 'unknown';
  root.textContent = `Renderer metrics\nUnavailable: ${error instanceof Error ? error.message : String(error)}`;
}

function kilobytes(value: number): string {
  return `${(value / 1024).toFixed(1)} KiB`;
}

function milliseconds(value: number): string {
  return `${value.toFixed(2)} ms`;
}
