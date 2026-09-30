import {
  completeLiveDebug,
  createLiveDebugHttpTransport,
  diagnosticEventAgeMilliseconds,
  type ProductHostDebugCatalog,
  type ProductHostDebugCommandDescriptor,
  type RuntimeDiagnosticEvent,
  type ProductHostDiagnosticsReadResponse,
  type ProductHostTelemetrySnapshot,
  type LiveDebugTransport,
} from '@rusty-engine/live-debug-client';

import {
  appendLiveDebugTranscript,
  commandSummary,
  historyCommand,
  LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES,
  updateAttributionLabel,
  type LiveDebugPanelPresentation,
  type LiveDebugTranscriptEntry,
} from './live-debug-panel-model.js';

type LiveDebugConnectionState = 'disconnected' | 'connecting' | 'ready' | 'unavailable' | 'error';

const LIVE_DEBUG_PANEL_MAX_HISTORY_ENTRIES = 64;
const LIVE_DEBUG_PANEL_MAX_COMPLETIONS = 12;
const LIVE_DEBUG_PANEL_MAX_DIAGNOSTICS = 128;
const LIVE_DEBUG_PANEL_POLL_MS = 1_000;
let nextLiveDebugPanelInstance = 1;

export interface LiveDebugPanelOptions {
  /** The host must opt in before the panel creates or uses a transport. */
  readonly enabled: boolean;
  /** A packaged host can retain transport ownership by supplying one. */
  readonly transport: LiveDebugTransport | null;
  readonly presentation: LiveDebugPanelPresentation;
}

const STYLE = `
  .rusty-live-debug-panel [hidden] { display: none !important; }
  .rusty-live-debug-panel { background: #17191d; color: #f0f3f5; border: 1px solid #4b5563; border-radius: 0.4rem; padding: 0.75rem; font: 0.875rem/1.4 ui-monospace, SFMono-Regular, Menlo, monospace; }
  .rusty-live-debug-panel--dock { border-radius: 0; }
  .rusty-live-debug-panel--overlay { position: fixed; z-index: 1000; inset: auto 1rem 1rem auto; width: min(38rem, calc(100vw - 2rem)); box-shadow: 0 0.8rem 2rem rgb(0 0 0 / 45%); }
  .rusty-live-debug-panel__header, .rusty-live-debug-panel__form, .rusty-live-debug-panel__transcript-actions { display: flex; gap: 0.5rem; align-items: center; }
  .rusty-live-debug-panel__status { margin-left: auto; color: #b8c5d6; }
  .rusty-live-debug-panel__form { margin-top: 0.75rem; }
  .rusty-live-debug-panel__form input { flex: 1; min-width: 0; }
  .rusty-live-debug-panel__hint, .rusty-live-debug-panel__unavailable, .rusty-live-debug-panel__disabled { color: #b8c5d6; }
  .rusty-live-debug-panel__error { color: #ffb4ab; }
  .rusty-live-debug-panel__completions { list-style: none; padding: 0; margin: 0.5rem 0; }
  .rusty-live-debug-panel__completions button { display: grid; grid-template-columns: minmax(12rem, auto) 1fr; gap: 0.5rem; width: 100%; text-align: left; }
  .rusty-live-debug-panel__transcript { max-height: 18rem; overflow: auto; margin: 0.5rem 0 0; padding-left: 1.5rem; cursor: text; user-select: text; }
  .rusty-live-debug-panel__transcript pre { white-space: pre-wrap; overflow-wrap: anywhere; margin: 0.25rem 0 0.75rem; }
  .rusty-live-debug-panel__response--failure pre { color: #ffb4ab; }
  .rusty-live-debug-panel__diagnostics { display: grid; gap: 0.35rem; margin-top: 0.75rem; cursor: text; user-select: text; }
  .rusty-live-debug-panel__diagnostic-log { max-height: 12rem; overflow: auto; margin: 0; padding-left: 1.5rem; cursor: text; user-select: text; }
  .rusty-live-debug-panel__product-telemetry { display: grid; gap: 0.35rem; margin-top: 0.75rem; cursor: text; user-select: text; }
`;

/**
 * Optional developer UI for the product-owned live-debug host. It has no
 * product state or command semantics: it only presents the generated catalog
 * and forwards one raw command line through a host-supplied transport.
 */
export class LiveDebugPanel {
  /** The mount-owned node; the caller-owned host stays reusable after dispose. */
  readonly element: HTMLElement;

  readonly #document: Document;
  readonly #enabled: boolean;
  readonly #transport: LiveDebugTransport | null;

  #connection: LiveDebugConnectionState = 'disconnected';
  #catalog: ProductHostDebugCatalog | null = null;
  #executing = false;
  #transcript: readonly LiveDebugTranscriptEntry[] = [];
  #history: readonly string[] = [];
  #historyCursor: number | null = null;
  #diagnosticCursor: string | undefined;
  /** Retained events whose age label follows the latest diagnostics read. */
  #diagnosticEntries: { readonly event: RuntimeDiagnosticEvent; readonly detail: HTMLElement }[] = [];
  #requestRevision = 0;
  #catalogAbort: AbortController | null = null;
  #executeAbort: AbortController | null = null;
  #diagnosticTimer: ReturnType<typeof setTimeout> | null = null;
  #disposed = false;

  readonly #status: HTMLElement;
  readonly #reconnect: HTMLButtonElement;
  readonly #error: HTMLElement;
  readonly #unavailable: HTMLElement;
  readonly #form: HTMLFormElement;
  readonly #input: HTMLInputElement;
  readonly #run: HTMLButtonElement;
  readonly #hint: HTMLElement;
  readonly #completions: HTMLElement;
  readonly #clear: HTMLButtonElement;
  readonly #copy: HTMLButtonElement;
  readonly #transcriptLog: HTMLElement;
  readonly #diagnosticCounts: HTMLElement;
  readonly #diagnosticLagged: HTMLElement;
  readonly #diagnosticLog: HTMLElement;
  readonly #telemetry: HTMLElement;

  constructor(document: Document, options: LiveDebugPanelOptions) {
    this.#document = document;
    this.#enabled = options.enabled;
    this.#transport = options.transport;
    const instance = nextLiveDebugPanelInstance++;
    const commandInputId = `rusty-live-debug-command-${instance}`;
    const commandHintId = `rusty-live-debug-hint-${instance}`;
    const transcriptLabelId = `rusty-live-debug-transcript-label-${instance}`;

    this.element = this.#el('div');
    const style = this.#el('style');
    style.textContent = STYLE;
    const panel = this.#el('section', 'rusty-live-debug-panel');
    panel.setAttribute('data-rusty-ui-interactive', '');
    panel.setAttribute('aria-label', 'Live debug console');
    panel.setAttribute('aria-hidden', String(!options.enabled));
    if (options.presentation !== 'inline') {
      panel.classList.add(`rusty-live-debug-panel--${options.presentation}`);
    }
    this.element.append(style, panel);

    const header = this.#el('header', 'rusty-live-debug-panel__header');
    this.#status = this.#el('span', 'rusty-live-debug-panel__status');
    this.#status.setAttribute('aria-live', 'polite');
    this.#reconnect = this.#button('Reconnect', () => this.reconnect());
    header.append(this.#el('strong', undefined, 'Live debug'), this.#status, this.#reconnect);
    panel.append(header);

    this.#error = this.#el('p', 'rusty-live-debug-panel__error');
    this.#error.setAttribute('role', 'alert');
    this.#unavailable = this.#el('p', 'rusty-live-debug-panel__unavailable', 'Live debugging is not enabled by this host.');

    this.#form = this.#el('form', 'rusty-live-debug-panel__form');
    const label = this.#el('label', undefined, 'Command');
    label.htmlFor = commandInputId;
    this.#input = this.#el('input');
    this.#input.id = commandInputId;
    this.#input.name = 'command';
    this.#input.autocomplete = 'off';
    this.#input.spellcheck = false;
    this.#input.setAttribute('aria-describedby', commandHintId);
    this.#run = this.#el('button');
    this.#run.type = 'submit';
    this.#form.append(label, this.#input, this.#run);
    this.#form.addEventListener('submit', (event) => {
      event.preventDefault();
      this.execute();
    });
    this.#input.addEventListener('input', () => {
      this.#historyCursor = null;
      this.#renderCommand();
    });
    this.#input.addEventListener('keydown', (event) => this.#onCommandKeydown(event));

    this.#hint = this.#el('p', 'rusty-live-debug-panel__hint');
    this.#hint.id = commandHintId;
    this.#completions = this.#el('ul', 'rusty-live-debug-panel__completions');
    this.#completions.setAttribute('aria-label', 'Command completions');

    const actions = this.#el('div', 'rusty-live-debug-panel__transcript-actions');
    const transcriptLabel = this.#el('strong', undefined, 'Responses');
    transcriptLabel.id = transcriptLabelId;
    this.#clear = this.#button('Clear', () => this.clearTranscript());
    this.#copy = this.#button('Copy', () => void this.copyTranscript());
    actions.append(transcriptLabel, this.#clear, this.#copy);
    this.#transcriptLog = this.#el('ol', 'rusty-live-debug-panel__transcript');
    this.#transcriptLog.setAttribute('role', 'log');
    this.#transcriptLog.setAttribute('aria-live', 'polite');
    this.#transcriptLog.setAttribute('aria-labelledby', transcriptLabelId);

    const diagnostics = this.#el('section', 'rusty-live-debug-panel__diagnostics');
    diagnostics.setAttribute('aria-label', 'Engine diagnostics');
    this.#diagnosticCounts = this.#el('span');
    this.#diagnosticLagged = this.#el(
      'span',
      'rusty-live-debug-panel__error',
      'Earlier diagnostics were evicted; reconnected at the current floor.',
    );
    this.#diagnosticLog = this.#el('ol', 'rusty-live-debug-panel__diagnostic-log');
    this.#diagnosticLog.setAttribute('role', 'log');
    this.#diagnosticLog.setAttribute('aria-live', 'polite');
    diagnostics.append(
      this.#el('strong', undefined, 'Diagnostics'),
      this.#diagnosticCounts,
      this.#diagnosticLagged,
      this.#diagnosticLog,
    );

    this.#telemetry = this.#el('section', 'rusty-live-debug-panel__product-telemetry');
    this.#telemetry.setAttribute('aria-label', 'Product/runtime lane');

    if (options.enabled) {
      panel.append(
        this.#error, this.#unavailable, this.#form, this.#hint, this.#completions,
        actions, this.#transcriptLog, diagnostics, this.#telemetry,
      );
    } else {
      panel.append(this.#el('p', 'rusty-live-debug-panel__disabled', 'Enable this optional panel before it connects to a host.'));
    }
    this.#resetDiagnostics();
    this.#render();
    if (options.enabled) this.reconnect();
  }

  reconnect(): void {
    if (!this.#enabled || this.#disposed) return;
    this.#catalogAbort?.abort();
    const abort = new AbortController();
    this.#catalogAbort = abort;
    this.#catalog = null;
    this.#setError('');
    void this.#loadCatalog(this.#activeTransport(), abort.signal);
  }

  execute(): void {
    const command = this.#input.value.trim();
    if (!this.#enabled || this.#connection !== 'ready' || this.#executing || command.length === 0) return;
    this.#executing = true;
    this.#setError('');
    this.#history = [...this.#history, command].slice(-LIVE_DEBUG_PANEL_MAX_HISTORY_ENTRIES);
    this.#historyCursor = null;
    this.#input.value = '';
    this.#executeAbort?.abort();
    const abort = new AbortController();
    this.#executeAbort = abort;
    this.#render();
    void this.#activeTransport().execute(command, abort.signal).then((result) => {
      if (abort.signal.aborted) return;
      this.#appendTranscript({ command, message: result.message, succeeded: result.succeeded });
    }).catch((error: unknown) => {
      if (!abort.signal.aborted) this.#setError(errorMessage(error));
    }).finally(() => {
      if (abort.signal.aborted) return;
      this.#executing = false;
      this.#render();
    });
  }

  clearTranscript(): void {
    this.#transcript = [];
    this.#transcriptLog.replaceChildren();
    this.#render();
  }

  async copyTranscript(): Promise<void> {
    const text = this.#transcript.map((entry) => `> ${entry.command}\n${entry.message}`).join('\n\n');
    try {
      await writeLiveDebugClipboard(text, this.#document);
    } catch (error: unknown) {
      this.#setError(`Could not copy responses: ${errorMessage(error)}`);
    }
  }

  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    this.#requestRevision += 1;
    this.#catalogAbort?.abort();
    this.#executeAbort?.abort();
    if (this.#diagnosticTimer !== null) clearTimeout(this.#diagnosticTimer);
    this.#diagnosticTimer = null;
    this.element.remove();
  }

  #activeTransport(): LiveDebugTransport {
    return this.#transport ?? createLiveDebugHttpTransport();
  }

  async #loadCatalog(transport: LiveDebugTransport, signal: AbortSignal): Promise<void> {
    const revision = ++this.#requestRevision;
    if (this.#diagnosticTimer !== null) clearTimeout(this.#diagnosticTimer);
    this.#diagnosticTimer = null;
    this.#connection = 'connecting';
    this.#render();
    try {
      const catalog = await transport.catalog(signal);
      if (signal.aborted || revision !== this.#requestRevision) return;
      this.#catalog = catalog;
      this.#connection = catalog.available ? 'ready' : 'unavailable';
      this.#render();
      if (catalog.available && transport.diagnostics !== undefined) {
        this.#pollDiagnostics(transport, signal, revision);
      }
    } catch (error: unknown) {
      if (signal.aborted || revision !== this.#requestRevision) return;
      this.#connection = 'error';
      this.#setError(errorMessage(error));
    }
  }

  #pollDiagnostics(transport: LiveDebugTransport, signal: AbortSignal, revision: number): void {
    const diagnostics = transport.diagnostics;
    if (diagnostics === undefined) return;
    const current = () => !signal.aborted && revision === this.#requestRevision;
    void diagnostics.call(transport, this.#diagnosticCursor, signal).then((batch) => {
      if (current()) this.#applyDiagnostics(batch);
    }).catch((error: unknown) => {
      if (current()) this.#setError(errorMessage(error));
    }).finally(() => {
      if (!current()) return;
      this.#diagnosticTimer = setTimeout(
        () => this.#pollDiagnostics(transport, signal, revision),
        LIVE_DEBUG_PANEL_POLL_MS,
      );
    });
  }

  #applyDiagnostics(batch: ProductHostDiagnosticsReadResponse): void {
    this.#diagnosticCursor = batch.nextCursor;
    this.#diagnosticCounts.textContent =
      `warn ${batch.warningCount} · error ${batch.errorCount} · dropped ${batch.droppedCount}`;
    this.#diagnosticLagged.hidden = !batch.lagged;
    for (const event of batch.events) {
      const detail = this.#el('small');
      const item = this.#el('li');
      item.append(this.#el('code', undefined, `#${event.sequence} ${event.source}/${event.code}`), ` ${event.message} `, detail);
      this.#diagnosticLog.append(item);
      this.#diagnosticEntries.push({ event, detail });
    }
    trimChildren(this.#diagnosticLog, LIVE_DEBUG_PANEL_MAX_DIAGNOSTICS);
    this.#diagnosticEntries = this.#diagnosticEntries.slice(-LIVE_DEBUG_PANEL_MAX_DIAGNOSTICS);
    for (const { event, detail } of this.#diagnosticEntries) detail.textContent = diagnosticDetail(batch, event);
    // Keep the last snapshot when a host omits the optional field.
    if (batch.telemetry !== undefined) this.#renderTelemetry(batch.telemetry);
  }

  #resetDiagnostics(): void {
    this.#diagnosticCursor = undefined;
    this.#diagnosticCounts.textContent = 'warn 0 · error 0 · dropped 0';
    this.#diagnosticLagged.hidden = true;
    this.#diagnosticLog.replaceChildren();
    this.#diagnosticEntries = [];
    this.#telemetry.hidden = true;
    this.#telemetry.replaceChildren();
  }

  #renderTelemetry(telemetry: ProductHostTelemetrySnapshot): void {
    const lines = [
      `In flight: ${telemetry.inFlightOperation || 'none'} · age ${milliseconds(telemetry.inFlightAgeMs)}`,
      `Admission: product ${milliseconds(telemetry.lastProductAdmissionLatencyMs)} · input ${milliseconds(telemetry.lastInputAdmissionLatencyMs)}`,
    ];
    const attribution = telemetry.updateAttribution;
    if (attribution !== null) {
      const slowest = attribution.rollingSlowest;
      lines.push(
        `C# update callback (${attribution.sampleCount} retained): p50 ${microseconds(attribution.callbackDurationUsP50)} · p95 ${microseconds(attribution.callbackDurationUsP95)} · rolling max ${microseconds(attribution.callbackDurationUsMax)}`,
        `Latest update: ${updateAttributionLabel(attribution.latest)}. Callback includes native services; post-callback is separate Rust staging, reduction, conversion, and completion.`,
        `Rolling slowest callback ${microseconds(slowest.callbackDurationUs)} · ${milliseconds(attribution.rollingSlowestAgeMs)} ago. Nested service totals: character ${slowest.characterStepCalls} call(s), ${microseconds(slowest.characterStepDurationUs)}, ${slowest.characterStepCastCount} controller cast(s), ${slowest.characterStepCandidateCount} eligible collider(s), ${slowest.characterStepNarrowPhaseCount} Parry narrow-phase queries; residency ${slowest.voxelResidencyCalls} call(s), ${microseconds(slowest.voxelResidencyDurationUs)}; scene presentation ${slowest.voxelScenePresentationCalls} call(s), ${microseconds(slowest.voxelScenePresentationDurationUs)}.`,
        `Incarnation slowest callback ${microseconds(attribution.slowest.callbackDurationUs)} · ${milliseconds(attribution.slowestAgeMs)} ago.`,
      );
    }
    lines.push(
      `Input queue: ${telemetry.queuedInputBatches}/${telemetry.inputBatchCapacity} batches · ${telemetry.queuedInputEvents} events · oldest ${milliseconds(telemetry.oldestInputAgeMs)}${telemetry.inputOverflowPending ? ' · overflow pending' : ''}`,
      telemetry.runtimeProgressUnavailableReason !== null
        ? `Runtime progress unavailable: ${telemetry.runtimeProgressUnavailableReason}`
        : `Runtime progress: ${millihertz(telemetry.runtimeProgressRateMillihertz)} · last ${milliseconds(telemetry.runtimeProgressAgeMs)}`,
      `Transport: ${telemetry.connections} connection(s) · ${telemetry.subscribers} subscriber(s) · largest subscriber backlog ${telemetry.outputQueueItems}/${telemetry.outputQueueCapacity} · binding ${telemetry.outputBindingActive ? 'active' : 'inactive'}`,
    );
    this.#telemetry.replaceChildren(
      this.#el('strong', undefined, 'Product/runtime lane'),
      ...lines.map((line) => this.#el('span', undefined, line)),
    );
    this.#telemetry.hidden = false;
  }

  #appendTranscript(entry: LiveDebugTranscriptEntry): void {
    this.#transcript = appendLiveDebugTranscript(this.#transcript, entry);
    const item = this.#el('li');
    if (!entry.succeeded) item.classList.add('rusty-live-debug-panel__response--failure');
    item.append(this.#el('code', undefined, `> ${entry.command}`), this.#el('pre', undefined, entry.message));
    this.#transcriptLog.append(item);
    trimChildren(this.#transcriptLog, LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES);
  }

  #onCommandKeydown(event: KeyboardEvent): void {
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      const next = historyCommand(this.#history, this.#historyCursor, event.key === 'ArrowUp' ? -1 : 1);
      this.#historyCursor = next.cursor;
      this.#input.value = next.command;
      this.#renderCommand();
      return;
    }
    if (event.key === 'Tab') {
      const completion = this.#currentCompletions()[0];
      if (completion === undefined) return;
      event.preventDefault();
      this.#applyCompletion(completion);
    }
  }

  #applyCompletion(completion: ProductHostDebugCommandDescriptor): void {
    this.#input.value = `${completion.name}${completion.parameters.length === 0 ? '' : ' '}`;
    this.#historyCursor = null;
    this.#renderCommand();
  }

  #currentCompletions(): readonly ProductHostDebugCommandDescriptor[] {
    const catalog = this.#catalog;
    if (catalog === null || !catalog.available) return [];
    return completeLiveDebug(catalog, this.#input.value.trim()).slice(0, LIVE_DEBUG_PANEL_MAX_COMPLETIONS);
  }

  #setError(message: string): void {
    this.#error.textContent = message;
    this.#error.hidden = message.length === 0;
  }

  #render(): void {
    this.#status.textContent = statusText(this.#enabled, this.#connection);
    this.#reconnect.disabled = !this.#enabled || this.#executing;
    this.#unavailable.hidden = this.#connection !== 'unavailable';
    this.#input.disabled = this.#connection !== 'ready' || this.#executing;
    this.#run.textContent = this.#executing ? 'Running…' : 'Run';
    this.#clear.disabled = this.#transcript.length === 0;
    this.#copy.disabled = this.#transcript.length === 0;
    if (this.#error.textContent === '') this.#error.hidden = true;
    this.#renderCommand();
  }

  #renderCommand(): void {
    this.#run.disabled = this.#connection !== 'ready' || this.#executing || this.#input.value.trim().length === 0;
    const completions = this.#currentCompletions();
    const first = completions[0];
    this.#hint.textContent = first === undefined ? '' : `${commandSummary(first)} — ${first.description}`;
    this.#hint.hidden = first === undefined;
    this.#completions.hidden = completions.length === 0;
    this.#completions.replaceChildren(...completions.map((completion) => {
      const item = this.#el('li');
      const button = this.#button('', () => this.#applyCompletion(completion));
      button.append(
        this.#el('code', undefined, commandSummary(completion)),
        this.#el('span', undefined, completion.description),
      );
      item.append(button);
      return item;
    }));
  }

  #button(text: string, onClick: () => void): HTMLButtonElement {
    const button = this.#el('button', undefined, text);
    button.type = 'button';
    button.addEventListener('click', onClick);
    return button;
  }

  #el<K extends keyof HTMLElementTagNameMap>(tag: K, className?: string, text?: string): HTMLElementTagNameMap[K] {
    const element = this.#document.createElement(tag);
    if (className !== undefined) element.className = className;
    if (text !== undefined) element.textContent = text;
    return element;
  }
}

function statusText(enabled: boolean, connection: LiveDebugConnectionState): string {
  if (!enabled) return 'Disabled';
  switch (connection) {
    case 'connecting': return 'Connecting…';
    case 'ready': return 'Connected';
    case 'unavailable': return 'Unavailable';
    case 'error': return 'Connection error';
    default: return 'Disconnected';
  }
}

function diagnosticDetail(batch: ProductHostDiagnosticsReadResponse, event: RuntimeDiagnosticEvent): string {
  const fields = event.fields?.map((field) => `${field.key}=${field.value}`) ?? [];
  const eventAge = diagnosticEventAgeMilliseconds(batch, event);
  if (eventAge !== null) fields.push(`event-age-ms=${String(Math.floor(eventAge))}`);
  return fields.join(' · ');
}

function millihertz(value: string | null): string {
  if (value === null || !/^\d+$/u.test(value)) return 'unavailable';
  const encoded = BigInt(value);
  const whole = encoded / 1_000n;
  const fraction = String(encoded % 1_000n).padStart(3, '0');
  return `${whole.toString()}.${fraction}/s`;
}

function milliseconds(value: string | null): string {
  return value === null || !/^\d+$/u.test(value) ? 'unavailable' : `${value} ms`;
}

function microseconds(value: string): string {
  return /^\d+$/u.test(value) ? `${value} us` : 'unavailable';
}

function trimChildren(list: HTMLElement, maxChildren: number): void {
  while (list.childElementCount > maxChildren) list.firstElementChild?.remove();
}

async function writeLiveDebugClipboard(text: string, document: Document): Promise<void> {
  const clipboard = globalThis.navigator?.clipboard;
  if (clipboard?.writeText !== undefined) {
    try {
      await clipboard.writeText(text);
      return;
    } catch {
      // Trusted-LAN HTTP and browser permission policy can reject the modern
      // API even during a user gesture. Preserve the same gesture for the
      // bounded DOM fallback below.
    }
  }
  if (document.body === null) throw new Error('document body is unavailable');
  const active = document.activeElement instanceof HTMLElement ? document.activeElement : null;
  const selection = document.getSelection();
  const ranges = selection === null
    ? []
    : Array.from({ length: selection.rangeCount }, (_, index) => selection.getRangeAt(index).cloneRange());
  const textarea = document.createElement('textarea');
  textarea.value = text;
  textarea.readOnly = true;
  textarea.setAttribute('aria-hidden', 'true');
  textarea.style.cssText = 'position:fixed;inset:-10000px auto auto -10000px;opacity:0;pointer-events:none;';
  document.body.append(textarea);
  let copied = false;
  try {
    textarea.select();
    copied = document.execCommand('copy');
  } finally {
    textarea.remove();
    selection?.removeAllRanges();
    for (const range of ranges) selection?.addRange(range);
    active?.focus({ preventScroll: true });
  }
  if (!copied) throw new Error('browser clipboard access is unavailable');
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
