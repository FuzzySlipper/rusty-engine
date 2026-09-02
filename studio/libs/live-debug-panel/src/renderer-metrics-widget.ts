import {
  createLiveDebugHttpTransport,
  type LiveDebugResult,
  type LiveDebugTransport,
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

interface RendererMetricsSummary {
  readonly available: boolean;
  readonly widget?: { readonly visible?: boolean };
  readonly diagnostic?: string;
  readonly renderer?: { readonly name?: unknown; readonly vendor?: unknown; readonly class?: unknown };
  readonly canvas?: Record<string, unknown>;
  readonly frame?: Record<string, unknown>;
  readonly pacing?: Record<string, unknown>;
  readonly statistics?: Record<string, unknown>;
  readonly resources?: Record<string, unknown>;
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

function decodeSummary(result: LiveDebugResult): RendererMetricsSummary {
  const parsed: unknown = JSON.parse(result.message);
  if (parsed === null || typeof parsed !== 'object' || Array.isArray(parsed)) {
    throw new TypeError('Renderer metrics status response is not an object.');
  }
  const summary = parsed as RendererMetricsSummary;
  if (typeof summary.available !== 'boolean') {
    throw new TypeError('Renderer metrics status response does not describe availability.');
  }
  return summary;
}

function renderSummary(root: HTMLElement, summary: RendererMetricsSummary): void {
  const visible = summary.widget?.visible === true;
  root.hidden = !visible;
  root.dataset.visible = String(visible);
  if (!summary.available) {
    root.textContent = `Renderer metrics\nSnapshot unavailable: ${text(summary.diagnostic)}`;
    return;
  }
  const frame = summary.frame ?? {};
  const pacing = summary.pacing ?? {};
  const canvas = summary.canvas ?? {};
  const resources = summary.resources ?? {};
  const statistics = summary.statistics ?? {};
  root.textContent = [
    'Renderer metrics',
    `FPS: ${fixed(frame['fps'])} | interval: ${milliseconds(frame['intervalMs'])} | sync submit: ${milliseconds(frame['syncSubmissionMs'])}`,
    `GPU timer: ${milliseconds(pacing['timerDurationMs'])} | effective pacing: ${milliseconds(pacing['effectiveDurationMs'])} | ${text(pacing['state'])}/${text(pacing['mode'])}`,
    `Renderer: ${text(summary.renderer?.class)} | ${text(summary.renderer?.name)} | ${text(summary.renderer?.vendor)}`,
    `Canvas: ${text(canvas['backingWidth'])}×${text(canvas['backingHeight'])} px | CSS ${text(canvas['cssWidth'])}×${text(canvas['cssHeight'])} | DPR ${fixed(canvas['effectivePixelRatio'])}`,
    `Draws: ${statistic(statistics['drawCallCount'])} | triangles: ${statistic(statistics['triangleCount'])} | live handles: ${statistic(statistics['renderHandleCount'])}`,
    `Live resources: geometry ${statistic(statistics['geometryResourceCount'])}, material ${statistic(statistics['materialResourceCount'])}, texture ${statistic(statistics['textureResourceCount'])}`,
    `Defined textures: ${text(resources['definedTextureCount'])} | fallbacks: sprite ${text(resources['spriteFallbackCount'])}, material ${text(resources['materialFallbackCount'])}`,
  ].join('\n');
}

function renderError(root: HTMLElement, error: unknown): void {
  root.hidden = false;
  root.dataset.visible = 'unknown';
  root.textContent = `Renderer metrics\nUnavailable: ${error instanceof Error ? error.message : String(error)}`;
}

function statistic(value: unknown): string {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return 'unavailable';
  return text((value as Record<string, unknown>)['value']);
}

function fixed(value: unknown): string {
  return typeof value === 'number' && Number.isFinite(value) ? value.toFixed(1) : 'unavailable';
}

function milliseconds(value: unknown): string {
  return typeof value === 'number' && Number.isFinite(value) ? `${value.toFixed(2)} ms` : 'unavailable';
}

function text(value: unknown): string {
  return value === null || value === undefined ? 'unavailable' : String(value);
}
