import { createLiveDebugHttpTransport, type LiveDebugTransport } from '@rusty-engine/live-debug-client';

import {
  atStep,
  boundFiles,
  controlValueOf,
  fillTemplate,
  isUnsaved,
  parseDescription,
  resolvePointer,
  saveEdits,
  type SourceFile,
  type TunableControl,
  type TunablesDescription,
} from './tunables-model.js';

const STYLE_ID = 'rusty-tunables-style';

/**
 * The panel's look, overridable by the product: set the `--rusty-tunables-*`
 * custom properties on `.rusty-tunables` or an ancestor, or style its classes.
 */
const STYLE = `
.rusty-tunables {
  --_rt-background: var(--rusty-tunables-background, rgb(14 18 26 / 94%));
  --_rt-foreground: var(--rusty-tunables-foreground, #e9eef5);
  --_rt-muted: var(--rusty-tunables-muted, #9aa8b8);
  --_rt-accent: var(--rusty-tunables-accent, #7fb2ff);
  --_rt-border: var(--rusty-tunables-border, #3a4656);
  --_rt-unsaved: var(--rusty-tunables-unsaved, #f0b45a);
  --_rt-radius: var(--rusty-tunables-radius, 0.4rem);
  --_rt-font: var(--rusty-tunables-font, 0.85rem/1.4 system-ui, sans-serif);
  background: var(--_rt-background); color: var(--_rt-foreground); font: var(--_rt-font);
  border: 1px solid var(--_rt-border); border-radius: var(--_rt-radius);
  padding: 0.7rem 0.8rem; box-sizing: border-box; max-height: 100%; overflow: auto;
}
.rusty-tunables__title { margin: 0 0 0.5rem; font-size: 1.05em; }
.rusty-tunables button, .rusty-tunables select, .rusty-tunables input[type=number] {
  font: inherit; color: inherit; background: transparent;
  border: 1px solid var(--_rt-border); border-radius: var(--_rt-radius); padding: 0.1rem 0.45rem;
}
.rusty-tunables button { cursor: pointer; }
.rusty-tunables button:hover { border-color: var(--_rt-accent); }
.rusty-tunables button:disabled { opacity: 0.45; cursor: default; }
.rusty-tunables select { background: var(--_rt-background); }
.rusty-tunables__row { display: flex; gap: 0.4rem; align-items: center; flex-wrap: wrap; margin: 0.3rem 0; }
.rusty-tunables__file { justify-content: space-between; }
.rusty-tunables__file-name { overflow-wrap: anywhere; }
.rusty-tunables__unsaved { color: var(--_rt-unsaved); }
.rusty-tunables__group { margin-top: 0.5rem; border-top: 1px solid var(--_rt-border); padding-top: 0.3rem; }
.rusty-tunables__group > summary { cursor: pointer; color: var(--_rt-muted); text-transform: uppercase; font-size: 0.8em; letter-spacing: 0.05em; }
.rusty-tunables__control {
  display: grid; grid-template-columns: minmax(6rem, 1fr) minmax(7rem, 1.6fr) auto; gap: 0.2rem 0.5rem;
  align-items: center; padding: 0.15rem 0;
}
.rusty-tunables__control-input { display: flex; gap: 0.35rem; align-items: center; }
.rusty-tunables__control-input input[type=range] { flex: 1; min-width: 4rem; accent-color: var(--_rt-accent); }
.rusty-tunables__control-input input[type=number] { width: 4.5rem; }
.rusty-tunables__control-input input[type=checkbox] { accent-color: var(--_rt-accent); }
.rusty-tunables__control--unsaved > .rusty-tunables__label::after { content: ' \\25CF'; color: var(--_rt-unsaved); }
.rusty-tunables__reset { visibility: hidden; padding: 0 0.35rem; }
.rusty-tunables__control--unsaved > .rusty-tunables__reset { visibility: visible; }
.rusty-tunables__status { color: var(--_rt-muted); font-size: 0.85em; margin-top: 0.5rem; min-height: 1.2em; }
.rusty-tunables__status[data-failed=true], .rusty-tunables__notice { color: var(--_rt-unsaved); }
`;

/** Configuration for one mounted tunables panel. */
export interface TunablesPanelOptions {
  /** Uses the same-origin live-debug HTTP transport when omitted. */
  readonly transport?: LiveDebugTransport;
  /** The command that answers the description; `tunables.describe` when omitted. */
  readonly describe?: string;
}

interface ControlView {
  readonly control: TunableControl;
  readonly row: HTMLElement;
  show(value: number): void;
}

/**
 * Renders a product's `tunables.v1` description and sends commands; it holds
 * no product state. Controls bound to a source file show unsaved changes, and
 * each bound file has Save (the Engine content source's pointer edits) and
 * Reset (the saved values set back live).
 */
export class TunablesPanel {
  readonly element: HTMLElement;
  readonly #document: Document;
  readonly #transport: LiveDebugTransport;
  readonly #describe: string;
  readonly #abort = new AbortController();
  readonly #folded = new Set<string>();
  readonly #values = new Map<string, number>();
  readonly #files = new Map<string, SourceFile>();
  readonly #views = new Map<string, ControlView>();
  /** Per key, the work waiting behind the request in flight (null: none). */
  readonly #queued = new Map<string, (() => Promise<void>) | null>();
  #description: TunablesDescription | null = null;
  #sourceError: string | null = null;
  #status: HTMLElement;
  #fileRows = new Map<string, { count: HTMLElement; save: HTMLButtonElement; reset: HTMLButtonElement }>();

  constructor(document: Document, options: TunablesPanelOptions) {
    this.#document = document;
    this.#transport = options.transport ?? createLiveDebugHttpTransport();
    this.#describe = options.describe ?? 'tunables.describe';
    installStyle(document);
    this.element = document.createElement('section');
    this.element.className = 'rusty-tunables';
    this.element.setAttribute('data-rusty-ui-interactive', '');
    this.#status = this.#make('p', 'rusty-tunables__status');
    this.#status.setAttribute('role', 'status');
    this.element.append(this.#status);
  }

  /** Describes again and re-reads the bound files. */
  async refresh(): Promise<void> {
    try {
      const command = this.#description?.commands['describe'] ?? this.#describe;
      await this.#apply(parseDescription(await this.#send(command)));
    } catch (error) {
      this.#report(error, true);
    }
  }

  dispose(): void {
    this.#abort.abort();
    this.element.remove();
  }

  async #send(command: string): Promise<string> {
    const result = await this.#transport.execute(command, this.#abort.signal);
    if (!result.succeeded) throw new Error(result.message);
    return result.message;
  }

  #command(name: string, values: Readonly<Record<string, string | number>>): string | null {
    const template = this.#description?.commands[name];
    return template === undefined ? null : fillTemplate(template, values);
  }

  async #apply(description: TunablesDescription): Promise<void> {
    this.#description = description;
    this.#values.clear();
    for (const group of description.groups) {
      for (const control of group.controls) this.#values.set(control.id, control.value);
    }
    await this.#readFiles();
    this.#render();
  }

  async #readFiles(): Promise<void> {
    this.#files.clear();
    this.#sourceError = null;
    for (const path of boundFiles(this.#description!)) {
      try {
        await this.#readFile(path);
      } catch (error) {
        this.#sourceError = error instanceof Error ? error.message : String(error);
        this.#files.clear();
        return;
      }
    }
  }

  async #readFile(path: string): Promise<void> {
    const command = this.#command('read', { path }) ?? `engine.content.source.read ${path}`;
    const answer = JSON.parse(await this.#send(command)) as { sha256: string; text: string };
    this.#files.set(path, { sha256: answer.sha256, document: JSON.parse(answer.text) as unknown });
  }

  #render(): void {
    const description = this.#description!;
    const scroll = this.element.scrollTop;
    this.#views.clear();
    this.#fileRows.clear();
    this.element.replaceChildren();
    if (description.title) this.element.append(this.#make('h2', 'rusty-tunables__title', description.title));
    if (description.notice) this.element.append(this.#make('p', 'rusty-tunables__notice', description.notice));
    this.#renderChoosers(description);
    this.#renderTimeline(description);
    this.#renderFiles(description);
    for (const group of description.groups) {
      const details = this.#make('details', 'rusty-tunables__group') as HTMLDetailsElement;
      details.open = !this.#folded.has(group.id);
      details.addEventListener('toggle', () => {
        if (details.open) this.#folded.delete(group.id);
        else this.#folded.add(group.id);
      });
      details.append(this.#make('summary', '', group.file ? `${group.title} · ${group.file}` : group.title));
      for (const control of group.controls) details.append(this.#renderControl(control));
      this.element.append(details);
    }
    this.element.append(this.#status);
    this.#updateMarks();
    this.element.scrollTop = scroll;
  }

  #renderChoosers(description: TunablesDescription): void {
    if (description.choosers.length === 0) return;
    const row = this.#make('div', 'rusty-tunables__row');
    for (const chooser of description.choosers) {
      const label = this.#make('label', '', `${chooser.label} `);
      const select = this.#document.createElement('select');
      for (const option of chooser.options) select.append(new Option(option, option, false, option === chooser.value));
      select.addEventListener('change', () => {
        const command = this.#command('select', { chooser: chooser.id, value: select.value });
        if (command === null) return;
        void this.#send(command)
          .then((answer) => this.#apply(parseDescription(answer)))
          .catch((error: unknown) => this.#report(error, true));
      });
      label.append(select);
      row.append(label);
    }
    this.element.append(row);
  }

  #renderTimeline(description: TunablesDescription): void {
    const timeline = description.timeline;
    if (timeline === null) return;
    const keys = this.#make('div', 'rusty-tunables__row');
    for (const key of timeline.keys) {
      const button = this.#make('button', '', key.label) as HTMLButtonElement;
      button.type = 'button';
      button.setAttribute('aria-pressed', String(key.index === timeline.key));
      button.addEventListener('click', () => this.#run('key', { key: key.index }, true));
      keys.append(button);
    }
    const play = this.#make('div', 'rusty-tunables__row');
    const scrub = this.#document.createElement('input');
    scrub.type = 'range';
    scrub.min = '0';
    scrub.max = String(timeline.duration);
    scrub.step = '0.01';
    scrub.setAttribute('aria-label', 'Time');
    scrub.addEventListener('input', () =>
      this.#latest('timeline.at', async () => {
        const command = this.#command('at', { seconds: scrub.value });
        if (command !== null) this.#report(await this.#send(command), false);
      }),
    );
    play.append(scrub);
    for (const speed of timeline.speeds) {
      const button = this.#make('button', '', `▶ ${speed}×`) as HTMLButtonElement;
      button.type = 'button';
      button.addEventListener('click', () => this.#run('play', { speed }, false));
      play.append(button);
    }
    this.element.append(keys, play);
  }

  /** Sends one of the page's own commands and reports its answer. */
  #run(name: string, values: Readonly<Record<string, string | number>>, describeAfter: boolean): void {
    const command = this.#command(name, values);
    if (command === null) return;
    void this.#send(command)
      .then(async (answer) => {
        if (describeAfter) await this.refresh();
        this.#report(answer, false);
      })
      .catch((error: unknown) => this.#report(error, true));
  }

  #renderFiles(description: TunablesDescription): void {
    if (this.#sourceError !== null) {
      this.element.append(this.#make('p', 'rusty-tunables__notice', `Source unavailable: ${this.#sourceError}`));
      return;
    }
    for (const path of boundFiles(description)) {
      const row = this.#make('div', 'rusty-tunables__row rusty-tunables__file');
      const name = this.#make('span', 'rusty-tunables__file-name', path);
      const count = this.#make('span', 'rusty-tunables__unsaved');
      const save = this.#make('button', '', 'Save') as HTMLButtonElement;
      const reset = this.#make('button', '', 'Reset') as HTMLButtonElement;
      save.type = reset.type = 'button';
      save.addEventListener('click', () => void this.#save(path));
      reset.addEventListener('click', () => {
        for (const control of this.#controlsOf(path)) this.#reset(control);
      });
      const actions = this.#make('span', 'rusty-tunables__row');
      actions.append(count, save, reset);
      row.append(name, actions);
      this.#fileRows.set(path, { count, save, reset });
      this.element.append(row);
    }
  }

  #renderControl(control: TunableControl): HTMLElement {
    const row = this.#make('div', 'rusty-tunables__control');
    const label = this.#make('label', 'rusty-tunables__label', control.label);
    const input = this.#make('span', 'rusty-tunables__control-input');
    const reset = this.#make('button', 'rusty-tunables__reset', '↺') as HTMLButtonElement;
    reset.type = 'button';
    reset.title = 'Reset to the saved value';
    reset.addEventListener('click', () => this.#reset(control));
    let show: (value: number) => void;
    if (control.kind === 'toggle') {
      const box = this.#document.createElement('input');
      box.type = 'checkbox';
      box.addEventListener('change', () => this.#set(control, box.checked ? 1 : 0, true));
      input.append(box);
      show = (value) => (box.checked = value >= 0.5);
    } else if (control.kind === 'choice') {
      const select = this.#document.createElement('select');
      (control.options ?? []).forEach((option, index) => select.append(new Option(option, String(index))));
      select.addEventListener('change', () => this.#set(control, Number(select.value), true));
      input.append(select);
      show = (value) => (select.value = String(Math.round(value)));
    } else {
      const range = this.#document.createElement('input');
      const number = this.#document.createElement('input');
      range.type = 'range';
      number.type = 'number';
      for (const field of [range, number]) {
        field.min = String(control.min);
        field.max = String(control.max);
        field.step = String(control.step);
      }
      range.addEventListener('input', () => this.#set(control, Number(range.value), false));
      range.addEventListener('change', () => this.#set(control, Number(range.value), true));
      number.addEventListener('change', () => this.#set(control, Number(number.value), true));
      input.append(range, number);
      show = (value) => {
        range.value = String(value);
        number.value = String(atStep(control, value));
      };
    }
    label.title = control.source ? `${control.source.path} ${control.source.pointer}` : 'Not saved to a file';
    row.append(label, input, reset);
    show(this.#values.get(control.id) ?? control.value);
    this.#views.set(control.id, { control, row, show });
    return row;
  }

  /** Runs `work` now, or after the request in flight for `key`, replacing older waiting work. */
  #latest(key: string, work: () => Promise<void>): void {
    if (this.#queued.has(key)) {
      this.#queued.set(key, work);
      return;
    }
    this.#queued.set(key, null);
    const run = async (next: () => Promise<void>): Promise<void> => {
      await next();
      const waiting = this.#queued.get(key);
      if (waiting) {
        this.#queued.set(key, null);
        await run(waiting);
      }
    };
    void run(work)
      .catch((error: unknown) => this.#report(error, true))
      .finally(() => this.#queued.delete(key));
  }

  /** Applies a value live: one set per control in flight, then its latest value. */
  #set(control: TunableControl, value: number, committed: boolean): void {
    this.#values.set(control.id, value);
    this.#updateMarks();
    this.#latest(control.id, async () => {
      const command = this.#command('set', { id: control.id, value });
      if (command === null) return;
      const kept = Number(await this.#send(command));
      // A newer value waiting to be sent wins over this answer.
      if (Number.isFinite(kept) && !this.#queued.get(control.id)) {
        this.#values.set(control.id, kept);
        this.#views.get(control.id)?.show(kept);
      }
      this.#updateMarks();
      if (committed && control.reshapes) await this.refresh();
    });
  }

  #reset(control: TunableControl): void {
    const file = control.source && this.#files.get(control.source.path);
    if (!file || !control.source) return;
    const saved = controlValueOf(control, resolvePointer(file.document, control.source.pointer));
    if (saved === undefined) return;
    this.#views.get(control.id)?.show(saved);
    this.#set(control, saved, true);
  }

  async #save(path: string): Promise<void> {
    const file = this.#files.get(path);
    if (!file) return;
    const edits = saveEdits(this.#controlsOf(path), this.#values, file);
    if (edits.length === 0) return;
    const command =
      this.#command('edit', { path, sha256: file.sha256, edits: JSON.stringify(edits) }) ??
      `engine.content.source.edit ${path} ${file.sha256} ${JSON.stringify(edits)}`;
    try {
      await this.#send(command);
      await this.#readFile(path);
      this.#updateMarks();
      this.#report(`Saved ${edits.length} value${edits.length === 1 ? '' : 's'} to ${path}.`, false);
    } catch (error) {
      // The file changed elsewhere, and a live file has reached the product
      // too: show what the product and the file hold now.
      if (error instanceof Error && error.message.startsWith('CONTENT_SOURCE_STALE')) await this.refresh();
      this.#report(error, true);
    }
  }

  #controlsOf(path: string): TunableControl[] {
    return [...this.#views.values()]
      .map((view) => view.control)
      .filter((control) => control.source?.path === path);
  }

  #updateMarks(): void {
    const unsaved = new Map<string, number>();
    for (const { control, row } of this.#views.values()) {
      const file = control.source && this.#files.get(control.source.path);
      const dirty =
        !!file &&
        !!control.source &&
        isUnsaved(control, this.#values.get(control.id) ?? control.value, resolvePointer(file.document, control.source.pointer));
      row.classList.toggle('rusty-tunables__control--unsaved', dirty);
      if (dirty && control.source) unsaved.set(control.source.path, (unsaved.get(control.source.path) ?? 0) + 1);
    }
    for (const [path, { count, save, reset }] of this.#fileRows) {
      const changed = unsaved.get(path) ?? 0;
      count.textContent = changed === 0 ? 'saved' : `${changed} unsaved`;
      save.disabled = reset.disabled = changed === 0;
    }
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
