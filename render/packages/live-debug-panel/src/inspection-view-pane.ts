import { createLiveDebugHttpTransport, type LiveDebugTransport } from '@rusty-engine/live-debug-client';

import {
  captureUrl,
  dragOrbit,
  dragPan,
  focusDistance,
  inspectCommands,
  subjects,
  wheelZoom,
} from './inspection-view-model.js';

const STYLE_ID = 'rusty-inspection-view-style';

/**
 * The pane's look, overridable by the product: set the `--rusty-inspection-*`
 * custom properties on `.rusty-inspection-view` or an ancestor, or style its classes.
 */
const STYLE = `
.rusty-inspection-view {
  --_ri-background: var(--rusty-inspection-background, rgb(14 18 26 / 94%));
  --_ri-foreground: var(--rusty-inspection-foreground, #e9eef5);
  --_ri-border: var(--rusty-inspection-border, #3a4656);
  --_ri-accent: var(--rusty-inspection-accent, #7fb2ff);
  --_ri-failed: var(--rusty-inspection-failed, #f0b45a);
  --_ri-font: var(--rusty-inspection-font, 0.8rem/1.3 system-ui, sans-serif);
  display: flex; flex-direction: column; box-sizing: border-box; width: 100%; height: 100%;
  border: 1px solid var(--_ri-border); color: var(--_ri-foreground); font: var(--_ri-font);
}
.rusty-inspection-view:focus-within { border-color: var(--_ri-accent); }
.rusty-inspection-view__bar {
  display: flex; flex-wrap: wrap; gap: 0.25rem; align-items: center; padding: 0.25rem;
  background: var(--_ri-background);
}
.rusty-inspection-view__bar button, .rusty-inspection-view__bar select {
  font: inherit; color: inherit; background: transparent; cursor: pointer;
  border: 1px solid var(--_ri-border); border-radius: 0.3rem; padding: 0 0.35rem;
}
.rusty-inspection-view__bar select { background: var(--_ri-background); }
.rusty-inspection-view__bar button[aria-pressed=true] { border-color: var(--_ri-accent); color: var(--_ri-accent); }
.rusty-inspection-view__surface { flex: 1; min-height: 4rem; cursor: grab; touch-action: none; outline: none; }
.rusty-inspection-view__surface:active { cursor: grabbing; }
.rusty-inspection-view__status { padding: 0 0.3rem; min-height: 1.1em; background: var(--_ri-background); }
.rusty-inspection-view__status[data-failed=true] { color: var(--_ri-failed); }
`;

/** The product UI's viewport port (`context.viewport` in `mountProductUi`). */
export interface InspectionViewport {
  anchor(name: string, element: HTMLElement): () => void;
}

/** Configuration for one mounted inspection view pane. */
export interface InspectionViewOptions {
  /** Anchors the pane's surface, so the Engine draws the view there. */
  readonly viewport: InspectionViewport;
  /** The view's id; a pane-numbered one when omitted. */
  readonly id?: string;
  /** Uses the same-origin live-debug HTTP transport when omitted. */
  readonly transport?: LiveDebugTransport;
  /** The framing buttons, in order. */
  readonly framings?: readonly string[];
  /** The subject framed first: `camera`, `target NAME` or a scene handle. */
  readonly subject?: string;
}

let panes = 0;

/**
 * One inspection view in a DOM element: the Engine draws the view under the
 * pane's surface, and the pane sends commands. Dragging orbits about the
 * focus, Shift-drag pans, the wheel zooms; the bar frames the player's camera
 * or a product target, toggles overlays in the view and downloads a capture.
 * The surface takes pointer input while it is used and gives it back when the
 * pane closes; closing the pane closes its view. It holds no product state.
 */
export class InspectionViewPane {
  readonly element: HTMLElement;
  readonly id: string;
  readonly #document: Document;
  readonly #transport: LiveDebugTransport;
  readonly #abort = new AbortController();
  readonly #surface: HTMLElement;
  readonly #subject: HTMLSelectElement;
  readonly #status: HTMLElement;
  readonly #release: () => void;
  #overlays = false;
  #pending: Promise<unknown> = Promise.resolve();
  #drag: { x: number; y: number; dx: number; dy: number; pan: boolean; distance: number } | null = null;
  #flush: number | null = null;
  readonly #requested: string;

  constructor(document: Document, options: InspectionViewOptions) {
    this.#document = document;
    this.#transport = options.transport ?? createLiveDebugHttpTransport();
    this.id = options.id ?? `pane${++panes}`;
    installStyle(document);
    this.element = this.#make('section', 'rusty-inspection-view');
    this.element.setAttribute('data-rusty-ui-interactive', '');
    const bar = this.#make('div', 'rusty-inspection-view__bar');
    this.#subject = this.#make('select', '') as HTMLSelectElement;
    this.#subject.setAttribute('aria-label', 'Subject');
    for (const subject of subjects(null)) this.#subject.append(this.#option(subject.value, subject.label));
    if (options.subject && options.subject !== 'camera') this.#subject.append(this.#option(options.subject, options.subject));
    this.#subject.value = options.subject ?? 'camera';
    this.#requested = this.#subject.value;
    bar.append(this.#make('strong', '', this.id), this.#subject);
    for (const framing of options.framings ?? ['front', 'side', 'underside', 'close']) {
      bar.append(this.#button(framing, () => this.#run(inspectCommands.frame(this.id, framing, this.#subject.value))));
    }
    const overlays = this.#button('Overlays', async () => {
      this.#overlays = !this.#overlays;
      overlays.setAttribute('aria-pressed', String(this.#overlays));
      return this.#run(this.#overlays ? inspectCommands.showSets() : inspectCommands.hideSets());
    });
    overlays.setAttribute('aria-pressed', 'false');
    bar.append(overlays, this.#button('Capture', () => this.#capture()));
    this.#surface = this.#make('div', 'rusty-inspection-view__surface');
    this.#surface.tabIndex = 0;
    this.#surface.setAttribute('aria-label', `Inspection view ${this.id}`);
    this.#status = this.#make('div', 'rusty-inspection-view__status');
    this.#status.setAttribute('role', 'status');
    this.element.append(bar, this.#surface, this.#status);
    this.#release = options.viewport.anchor(this.#anchor(), this.#surface);
    this.#listen();
  }

  /** Creates the view, lists the product's targets and frames the first subject. */
  async open(): Promise<void> {
    try {
      await this.#send(inspectCommands.create(this.id, this.#anchor()));
      const targets = await this.#transport.execute(inspectCommands.targets(), this.#abort.signal).catch(() => null);
      const listed = subjects(targets?.succeeded ? targets.message : null);
      let chosen = this.#subject.value;
      this.#subject.replaceChildren(...listed.map((subject) => this.#option(subject.value, subject.label)));
      // A target the product does not offer now frames the player's camera instead.
      const missing = chosen.startsWith('target ') && !listed.some((subject) => subject.value === chosen);
      if (missing) chosen = 'camera';
      else if (!listed.some((subject) => subject.value === chosen)) this.#subject.append(this.#option(chosen, chosen));
      this.#subject.value = chosen;
      const framing = listed.find((subject) => subject.value === chosen)?.framing ?? 'side';
      await this.#run(inspectCommands.frame(this.id, framing, chosen));
      if (missing) this.#report(`No ${this.#requested}: framing the player camera.`, false);
    } catch (error) {
      this.#report(error, true);
    }
  }

  /** Closes the view and gives the surface's input back. */
  dispose(): void {
    if (this.#flush !== null) this.#document.defaultView?.clearTimeout(this.#flush);
    this.#release();
    void this.#transport.execute(inspectCommands.close(this.id)).catch(() => undefined);
    this.#abort.abort();
    this.element.remove();
  }

  #anchor(): string {
    return `inspect-${this.id}`;
  }

  #listen(): void {
    const surface = this.#surface;
    surface.addEventListener('pointerdown', (event) => {
      surface.setPointerCapture(event.pointerId);
      surface.focus();
      this.#drag = { x: event.clientX, y: event.clientY, dx: 0, dy: 0, pan: event.shiftKey, distance: 1 };
      void this.#run(inspectCommands.select(this.id), false);
      void this.#send('engine.inspect.view list')
        .then((listing) => {
          if (this.#drag) this.#drag.distance = focusDistance(listing, this.id);
        })
        .catch(() => undefined);
    });
    surface.addEventListener('pointermove', (event) => {
      const drag = this.#drag;
      if (!drag) return;
      drag.dx += event.clientX - drag.x;
      drag.dy += event.clientY - drag.y;
      drag.x = event.clientX;
      drag.y = event.clientY;
      this.#schedule();
    });
    const end = (event: PointerEvent): void => {
      if (surface.hasPointerCapture(event.pointerId)) surface.releasePointerCapture(event.pointerId);
      this.#schedule();
      this.#drag = null;
    };
    surface.addEventListener('pointerup', end);
    surface.addEventListener('pointercancel', end);
    surface.addEventListener(
      'wheel',
      (event) => {
        event.preventDefault();
        void this.#run(inspectCommands.zoom(this.id, wheelZoom(event.deltaY)), false);
      },
      { passive: false },
    );
  }

  /** Sends the drag so far, at most every 50 ms, one command behind another. */
  #schedule(): void {
    if (this.#flush !== null) return;
    const window = this.#document.defaultView;
    const send = (): void => {
      this.#flush = null;
      const drag = this.#drag;
      if (!drag || (drag.dx === 0 && drag.dy === 0)) return;
      const command = drag.pan
        ? inspectCommands.pan(this.id, ...dragPan(drag.dx, drag.dy, drag.distance))
        : inspectCommands.orbit(this.id, ...dragOrbit(drag.dx, drag.dy));
      drag.dx = 0;
      drag.dy = 0;
      void this.#run(command, false);
    };
    if (window) this.#flush = window.setTimeout(send, 50);
    else send();
  }

  async #capture(): Promise<string> {
    const rect = this.#surface.getBoundingClientRect();
    const ratio = this.#document.defaultView?.devicePixelRatio ?? 1;
    const width = Math.max(1, Math.round(rect.width * ratio * 2));
    const height = Math.max(1, Math.round(rect.height * ratio * 2));
    const response = await fetch(captureUrl(this.id, width, height), { signal: this.#abort.signal });
    if (!response.ok) throw new Error(`capture: HTTP ${response.status}`);
    const bytes = new Uint8Array(await response.arrayBuffer());
    // An RSF1 frame wraps the PNG; the image starts at its signature.
    const start = bytes.findIndex((byte, index) => byte === 0x89 && bytes[index + 1] === 0x50 && bytes[index + 2] === 0x4e);
    const blob = new Blob([bytes.slice(Math.max(start, 0))], { type: 'image/png' });
    const link = this.#make('a', '') as HTMLAnchorElement;
    link.href = URL.createObjectURL(blob);
    link.download = `${this.id}.png`;
    link.click();
    URL.revokeObjectURL(link.href);
    return `Captured ${this.id} at ${width}×${height}.`;
  }

  /** Runs a command after the ones before it; reports its answer when `report`. */
  #run(command: string, report = true): Promise<string> {
    const run = this.#pending.then(() => this.#send(command));
    this.#pending = run.catch(() => undefined);
    return run.then(
      (message) => {
        if (report) this.#report('', false);
        return message;
      },
      (error: unknown) => {
        this.#report(error, true);
        throw error;
      },
    );
  }

  async #send(command: string): Promise<string> {
    const result = await this.#transport.execute(command, this.#abort.signal);
    if (!result.succeeded) throw new Error(result.message);
    return result.message;
  }

  #button(label: string, run: () => Promise<unknown>): HTMLButtonElement {
    const button = this.#make('button', '', label) as HTMLButtonElement;
    button.type = 'button';
    button.addEventListener('click', () => {
      void run().then(
        (message) => {
          if (typeof message === 'string' && message.startsWith('Captured')) this.#report(message, false);
        },
        (error: unknown) => this.#report(error, true),
      );
    });
    return button;
  }

  #option(value: string, label: string): HTMLOptionElement {
    const option = this.#make('option', '', label) as HTMLOptionElement;
    option.value = value;
    return option;
  }

  #report(message: unknown, failed: boolean): void {
    this.#status.textContent = message instanceof Error ? message.message : String(message);
    this.#status.dataset['failed'] = String(failed);
  }

  #make(tag: string, className: string, text?: string): HTMLElement {
    const element = this.#document.createElement(tag);
    if (className) element.className = className;
    if (text !== undefined) element.textContent = text;
    return element;
  }
}

function installStyle(document: Document): void {
  if (document.getElementById(STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = STYLE;
  document.head.append(style);
}
