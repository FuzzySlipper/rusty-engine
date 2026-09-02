import { NgFor, NgIf } from '@angular/common';
import {
  ChangeDetectionStrategy,
  Component,
  computed,
  effect,
  input,
  type OnDestroy,
  signal,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import {
  completeLiveDebug,
  createLiveDebugHttpTransport,
  diagnosticRendererObservationAgeMilliseconds,
  type LiveDebugCatalog,
  type LiveDebugCommandDescriptor,
  type LiveDebugDiagnosticEvent,
  type LiveDebugTransport,
} from '@rusty-engine/live-debug-client';

import {
  appendLiveDebugTranscript,
  commandSummary,
  historyCommand,
  type LiveDebugPanelPresentation,
  type LiveDebugTranscriptEntry,
} from './live-debug-panel-model.js';

export type { LiveDebugPanelPresentation } from './live-debug-panel-model.js';

type LiveDebugConnectionState = 'disconnected' | 'connecting' | 'ready' | 'unavailable' | 'error';

const LIVE_DEBUG_PANEL_MAX_HISTORY_ENTRIES = 64;
const LIVE_DEBUG_PANEL_MAX_COMPLETIONS = 12;
const LIVE_DEBUG_PANEL_MAX_DIAGNOSTICS = 128;
const LIVE_DEBUG_PANEL_POLL_MS = 1_000;
let nextLiveDebugPanelInstance = 1;

/**
 * Optional developer UI for the product-owned live-debug host. It has no
 * product state or command semantics: it only presents the generated catalog
 * and forwards one raw command line through a host-supplied transport.
 */
@Component({
  selector: 'rusty-live-debug-panel',
  standalone: true,
  imports: [FormsModule, NgFor, NgIf],
  template: `
    <section
      class="rusty-live-debug-panel"
      [class.rusty-live-debug-panel--dock]="presentation() === 'dock'"
      [class.rusty-live-debug-panel--overlay]="presentation() === 'overlay'"
      [attr.aria-hidden]="!enabled()"
      aria-label="Live debug console"
    >
      <header class="rusty-live-debug-panel__header">
        <strong>Live debug</strong>
        <span class="rusty-live-debug-panel__status" aria-live="polite">{{ statusText() }}</span>
        <button type="button" (click)="reconnect()" [disabled]="!enabled() || executing()">Reconnect</button>
      </header>

      <ng-container *ngIf="enabled(); else disabledPanel">
        <p class="rusty-live-debug-panel__error" *ngIf="error()" role="alert">{{ error() }}</p>
        <p class="rusty-live-debug-panel__unavailable" *ngIf="connection() === 'unavailable'">
          Live debugging is not enabled by this host.
        </p>

        <form class="rusty-live-debug-panel__form" (ngSubmit)="execute()">
          <label [attr.for]="commandInputId">Command</label>
          <input
            [attr.id]="commandInputId"
            name="command"
            autocomplete="off"
            spellcheck="false"
            [ngModel]="command()"
            (ngModelChange)="command.set($event)"
            (keydown)="onCommandKeydown($event)"
            [disabled]="connection() !== 'ready' || executing()"
            [attr.aria-describedby]="commandHintId"
          >
          <button type="submit" [disabled]="connection() !== 'ready' || executing() || command().trim().length === 0">
            {{ executing() ? 'Running…' : 'Run' }}
          </button>
        </form>

        <p [attr.id]="commandHintId" class="rusty-live-debug-panel__hint" *ngIf="completionHint() as hint">
          {{ hint }}
        </p>
        <ul class="rusty-live-debug-panel__completions" *ngIf="completions().length > 0" aria-label="Command completions">
          <li *ngFor="let completion of completions()">
            <button type="button" (click)="applyCompletion(completion)">
              <code>{{ commandLabel(completion) }}</code>
              <span>{{ completion.description }}</span>
            </button>
          </li>
        </ul>

        <div class="rusty-live-debug-panel__transcript-actions">
          <strong [attr.id]="transcriptLabelId">Responses</strong>
          <button type="button" (click)="clearTranscript()" [disabled]="transcript().length === 0">Clear</button>
          <button type="button" (click)="copyTranscript()" [disabled]="transcript().length === 0">Copy</button>
        </div>
        <ol class="rusty-live-debug-panel__transcript" role="log" aria-live="polite" [attr.aria-labelledby]="transcriptLabelId">
          <li *ngFor="let entry of transcript()" [class.rusty-live-debug-panel__response--failure]="!entry.succeeded">
            <code>&gt; {{ entry.command }}</code>
            <pre>{{ entry.message }}</pre>
          </li>
        </ol>
        <section class="rusty-live-debug-panel__diagnostics" aria-label="Engine diagnostics">
          <strong>Diagnostics</strong>
          <span>warn {{ diagnosticWarningCount() }} · error {{ diagnosticErrorCount() }} · dropped {{ diagnosticDroppedCount() }}</span>
          <span *ngIf="diagnosticLagged()" class="rusty-live-debug-panel__error">Earlier diagnostics were evicted; reconnected at the current floor.</span>
          <ol class="rusty-live-debug-panel__diagnostic-log" role="log" aria-live="polite">
            <li *ngFor="let event of diagnosticEvents()"><code>#{{ event.sequence }} {{ event.source }}/{{ event.code }}</code> {{ event.message }} <small>{{ diagnosticDetail(event) }}</small></li>
          </ol>
        </section>
      </ng-container>

      <ng-template #disabledPanel>
        <p class="rusty-live-debug-panel__disabled">Enable this optional panel before it connects to a host.</p>
      </ng-template>
    </section>
  `,
  styles: [`
    :host { display: block; }
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
    .rusty-live-debug-panel__transcript { max-height: 18rem; overflow: auto; margin: 0.5rem 0 0; padding-left: 1.5rem; }
    .rusty-live-debug-panel__transcript pre { white-space: pre-wrap; overflow-wrap: anywhere; margin: 0.25rem 0 0.75rem; }
    .rusty-live-debug-panel__response--failure pre { color: #ffb4ab; }
    .rusty-live-debug-panel__diagnostics { display: grid; gap: 0.35rem; margin-top: 0.75rem; }
    .rusty-live-debug-panel__diagnostic-log { max-height: 12rem; overflow: auto; margin: 0; padding-left: 1.5rem; }
  `],
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class LiveDebugPanelComponent implements OnDestroy {
  /** The host must opt in before this component creates or uses a transport. */
  readonly enabled = input(false);
  /** A packaged host can retain transport ownership by supplying this input. */
  readonly transport = input<LiveDebugTransport | null>(null);
  readonly presentation = input<LiveDebugPanelPresentation>('inline');

  /** IDs are instance-local so separately mounted panels remain accessible. */
  readonly commandInputId = `rusty-live-debug-command-${nextLiveDebugPanelInstance}`;
  readonly commandHintId = `rusty-live-debug-hint-${nextLiveDebugPanelInstance}`;
  readonly transcriptLabelId = `rusty-live-debug-transcript-label-${nextLiveDebugPanelInstance++}`;

  readonly connection = signal<LiveDebugConnectionState>('disconnected');
  readonly error = signal('');
  readonly command = signal('');
  readonly executing = signal(false);
  readonly catalog = signal<LiveDebugCatalog | null>(null);
  readonly transcript = signal<readonly LiveDebugTranscriptEntry[]>([]);
  readonly history = signal<readonly string[]>([]);
  readonly historyCursor = signal<number | null>(null);
  readonly diagnosticEvents = signal<readonly LiveDebugDiagnosticEvent[]>([]);
  readonly diagnosticWarningCount = signal('0');
  readonly diagnosticErrorCount = signal('0');
  readonly diagnosticDroppedCount = signal('0');
  readonly diagnosticLagged = signal(false);
  readonly diagnosticReadMonotonicNanoseconds = signal('0');
  readonly completions = computed(() => {
    const catalog = this.catalog();
    if (catalog === null || !catalog.available) return [];
    return completeLiveDebug(catalog, this.command().trim()).slice(0, LIVE_DEBUG_PANEL_MAX_COMPLETIONS);
  });
  readonly completionHint = computed(() => {
    const completion = this.completions()[0];
    return completion === undefined ? '' : `${commandSummary(completion)} — ${completion.description}`;
  });
  readonly statusText = computed(() => {
    if (!this.enabled()) return 'Disabled';
    switch (this.connection()) {
      case 'connecting': return 'Connecting…';
      case 'ready': return 'Connected';
      case 'unavailable': return 'Unavailable';
      case 'error': return 'Connection error';
      default: return 'Disconnected';
    }
  });

  #requestRevision = 0;
  #catalogAbort: AbortController | null = null;
  #executeAbort: AbortController | null = null;
  #diagnosticTimer: ReturnType<typeof setTimeout> | null = null;
  #diagnosticCursor: string | undefined;

  constructor() {
    effect((onCleanup) => {
      const enabled = this.enabled();
      const transport = this.transport();
      if (!enabled) {
        this.disconnect();
        return;
      }
      const abort = new AbortController();
      this.#catalogAbort = abort;
      void this.loadCatalog(transport ?? createLiveDebugHttpTransport(), abort.signal);
      onCleanup(() => abort.abort());
    });
  }

  reconnect(): void {
    if (!this.enabled()) return;
    this.#catalogAbort?.abort();
    const abort = new AbortController();
    this.#catalogAbort = abort;
    this.catalog.set(null);
    this.error.set('');
    void this.loadCatalog(this.transport() ?? createLiveDebugHttpTransport(), abort.signal);
  }

  execute(): void {
    const command = this.command().trim();
    if (!this.enabled() || this.connection() !== 'ready' || this.executing() || command.length === 0) return;
    const transport = this.transport() ?? createLiveDebugHttpTransport();
    this.executing.set(true);
    this.error.set('');
    this.history.set(appendBounded(this.history(), command, LIVE_DEBUG_PANEL_MAX_HISTORY_ENTRIES));
    this.historyCursor.set(null);
    this.command.set('');
    this.#executeAbort?.abort();
    const abort = new AbortController();
    this.#executeAbort = abort;
    void transport.execute(command, abort.signal).then((result) => {
      if (abort.signal.aborted) return;
      this.transcript.set(appendLiveDebugTranscript(this.transcript(), {
        command,
        message: result.message,
        succeeded: result.succeeded,
      }));
    }).catch((error: unknown) => {
      if (abort.signal.aborted) return;
      this.error.set(errorMessage(error));
    }).finally(() => {
      if (!abort.signal.aborted) this.executing.set(false);
    });
  }

  onCommandKeydown(event: KeyboardEvent): void {
    if (event.key === 'ArrowUp' || event.key === 'ArrowDown') {
      event.preventDefault();
      const next = historyCommand(this.history(), this.historyCursor(), event.key === 'ArrowUp' ? -1 : 1);
      this.historyCursor.set(next.cursor);
      this.command.set(next.command);
      return;
    }
    if (event.key === 'Tab') {
      const completion = this.completions()[0];
      if (completion === undefined) return;
      event.preventDefault();
      this.applyCompletion(completion);
    }
  }

  applyCompletion(completion: LiveDebugCommandDescriptor): void {
    this.command.set(`${completion.name}${completion.parameters.length === 0 ? '' : ' '}`);
    this.historyCursor.set(null);
  }

  commandLabel(command: LiveDebugCommandDescriptor): string {
    return commandSummary(command);
  }

  diagnosticDetail(event: LiveDebugDiagnosticEvent): string {
    const fields = event.fields?.map((field) => `${field.key}=${field.value}`) ?? [];
    const age = diagnosticRendererObservationAgeMilliseconds({
      events: [], floorSequence: '0', throughSequence: '0', nextCursor: '0',
      readMonotonicNanoseconds: this.diagnosticReadMonotonicNanoseconds(),
      lagged: false, warningCount: '0', errorCount: '0', droppedCount: '0',
    }, event);
    if (age !== null) fields.push(`renderer-age-ms=${String(Math.floor(age))}`);
    return fields.join(' · ');
  }

  clearTranscript(): void {
    this.transcript.set([]);
  }

  async copyTranscript(): Promise<void> {
    const text = this.transcript().map((entry) => `> ${entry.command}\n${entry.message}`).join('\n\n');
    try {
      await globalThis.navigator.clipboard.writeText(text);
    } catch (error: unknown) {
      this.error.set(`Could not copy responses: ${errorMessage(error)}`);
    }
  }

  ngOnDestroy(): void {
    this.disconnect();
  }

  private disconnect(): void {
    this.#requestRevision += 1;
    this.#catalogAbort?.abort();
    this.#executeAbort?.abort();
    if (this.#diagnosticTimer !== null) clearTimeout(this.#diagnosticTimer);
    this.#diagnosticTimer = null;
    this.#diagnosticCursor = undefined;
    this.diagnosticReadMonotonicNanoseconds.set('0');
    this.diagnosticEvents.set([]);
    this.diagnosticWarningCount.set('0');
    this.diagnosticErrorCount.set('0');
    this.diagnosticDroppedCount.set('0');
    this.diagnosticLagged.set(false);
    this.catalog.set(null);
    this.connection.set('disconnected');
    this.executing.set(false);
    this.error.set('');
  }

  private async loadCatalog(transport: LiveDebugTransport, signal: AbortSignal): Promise<void> {
    const revision = ++this.#requestRevision;
    this.connection.set('connecting');
    this.error.set('');
    try {
      const catalog = await transport.catalog(signal);
      if (signal.aborted || revision !== this.#requestRevision) return;
      this.catalog.set(catalog);
      this.connection.set(catalog.available ? 'ready' : 'unavailable');
      if (catalog.available && transport.diagnostics !== undefined) {
        this.pollDiagnostics(transport, signal, revision);
      }
    } catch (error: unknown) {
      if (signal.aborted || revision !== this.#requestRevision) return;
      this.connection.set('error');
      this.error.set(errorMessage(error));
    }
  }

  private pollDiagnostics(transport: LiveDebugTransport, signal: AbortSignal, revision: number): void {
    const diagnostics = transport.diagnostics;
    if (diagnostics === undefined) return;
    void diagnostics.call(transport, this.#diagnosticCursor, signal).then((batch) => {
      if (signal.aborted || revision !== this.#requestRevision) return;
      this.#diagnosticCursor = batch.nextCursor;
      this.diagnosticReadMonotonicNanoseconds.set(batch.readMonotonicNanoseconds);
      this.diagnosticLagged.set(batch.lagged);
      this.diagnosticWarningCount.set(batch.warningCount);
      this.diagnosticErrorCount.set(batch.errorCount);
      this.diagnosticDroppedCount.set(batch.droppedCount);
      if (batch.events.length > 0) {
        this.diagnosticEvents.set(
          [...this.diagnosticEvents(), ...batch.events].slice(-LIVE_DEBUG_PANEL_MAX_DIAGNOSTICS),
        );
      }
    }).catch((error: unknown) => {
      if (!signal.aborted && revision === this.#requestRevision) this.error.set(errorMessage(error));
    }).finally(() => {
      if (!signal.aborted && revision === this.#requestRevision) {
        this.#diagnosticTimer = setTimeout(
          () => this.pollDiagnostics(transport, signal, revision),
          LIVE_DEBUG_PANEL_POLL_MS,
        );
      }
    });
  }
}

function appendBounded(values: readonly string[], value: string, maxEntries: number): readonly string[] {
  return [...values, value].slice(-maxEntries);
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
