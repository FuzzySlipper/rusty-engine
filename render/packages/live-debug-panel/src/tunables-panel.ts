import { createLiveDebugHttpTransport, type LiveDebugTransport } from '@rusty-engine/live-debug-client';

import {
  atStep,
  fillTemplate,
  LiveValues,
  parseApplied,
  parseDescription,
  parseDiff,
  refusals,
  TunableSources,
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
 * no product state. Controls bound to a source file show the unsaved changes
 * `tunables.diff` answers, and each bound file has Save (`tunables.save`) and
 * Reset (`tunables.reset`).
 */
export class TunablesPanel {
  readonly element: HTMLElement;
  readonly #document: Document;
  readonly #transport: LiveDebugTransport;
  readonly #describe: string;
  readonly #abort = new AbortController();
  readonly #folded = new Set<string>();
  readonly #live = new LiveValues((id, value) => this.#applyValue(id, value));
  readonly #views = new Map<string, ControlView>();
  /** Per key, the work waiting behind the request in flight (null: none). */
  readonly #queued = new Map<string, (() => Promise<void>) | null>();
  #description: TunablesDescription | null = null;
  readonly #sources = new TunableSources();
  /** The status shows a comparison that failed, until one answers. */
  #compareFailed = false;
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

  /** Describes again and compares the bound files. */
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
    this.#live.reset(
      description.groups.flatMap((group) => group.controls.map((control) => [control.id, control.value] as const)),
    );
    await this.#readDiff(true);
    this.#render();
  }

  /**
   * Compares the bound files; `adopt` (with a description) makes the answer
   * what Save names. Answers whether the comparison answered.
   */
  async #readDiff(adopt: boolean): Promise<boolean> {
    try {
      const diff = parseDiff(await this.#send(this.#command('diff', {}) ?? 'tunables.diff'));
      if (adopt) this.#sources.adopt(diff);
      else this.#sources.compared(diff);
      return true;
    } catch (error) {
      this.#sources.failed(error instanceof Error ? error.message : String(error));
      return false;
    }
  }

  /** Compares again once the sets in flight answer, then marks what is unsaved, or why it cannot. */
  #compare(): void {
    this.#latest('diff', async () => {
      const answered = await this.#readDiff(false);
      this.#updateMarks();
      this.#reportComparison(answered, `Could not compare the source files: ${this.#sources.error}`);
    });
  }

  /** Reports a comparison that failed, and that one answers again after. */
  #reportComparison(answered: boolean, failure: string): void {
    if (!answered) this.#report(failure, true);
    else if (this.#compareFailed) this.#report('The source files compare again.', false);
    this.#compareFailed = !answered;
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
    this.#renderFiles();
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

  #renderFiles(): void {
    if (this.#sources.error !== null) {
      this.element.append(this.#make('p', 'rusty-tunables__notice', `Source unavailable: ${this.#sources.error}`));
      return;
    }
    for (const file of this.#sources.diff.files) {
      const row = this.#make('div', 'rusty-tunables__row rusty-tunables__file');
      const name = this.#make('span', 'rusty-tunables__file-name', file.path);
      const count = this.#make('span', 'rusty-tunables__unsaved');
      const save = this.#make('button', '', 'Save') as HTMLButtonElement;
      const reset = this.#make('button', '', 'Reset') as HTMLButtonElement;
      save.type = reset.type = 'button';
      save.addEventListener('click', () => void this.#save(file.path));
      reset.addEventListener('click', () => void this.#resetFile(file.path));
      const actions = this.#make('span', 'rusty-tunables__row');
      actions.append(count, save, reset);
      row.append(name, actions);
      this.#fileRows.set(file.path, { count, save, reset });
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
    show(this.#live.shown(control.id) ?? control.value);
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

  /** Sends one set and answers the value the product kept. */
  async #applyValue(id: string, value: number): Promise<number> {
    const command = this.#command('set', { id, value });
    if (command === null) throw new Error('This page has no set command.');
    const kept = Number(await this.#send(command));
    if (!Number.isFinite(kept)) throw new Error(`The set of ${id} answered no value.`);
    return kept;
  }

  /** Applies a value live; the control then shows what the product kept. */
  #set(control: TunableControl, value: number, committed: boolean): void {
    const settled = this.#live.set(control.id, value);
    void settled
      .then(() => {
        if (committed && control.reshapes) return this.refresh();
        this.#compare();
        return undefined;
      })
      .catch((error: unknown) => this.#report(error, true))
      .finally(() => {
        const shown = this.#live.shown(control.id);
        if (shown !== undefined) this.#views.get(control.id)?.show(shown);
      });
  }

  /** Sets one control back to its file's value, as the last diff answered it. */
  #reset(control: TunableControl): void {
    const saved = this.#sources.unsaved().get(control.id)?.saved;
    if (saved === undefined) return;
    this.#views.get(control.id)?.show(saved);
    this.#set(control, saved, true);
  }

  /** Sets every unsaved control of a file back to the file's values. */
  async #resetFile(path: string): Promise<void> {
    const command = this.#command('reset', { path });
    if (command === null) return;
    try {
      await this.#live.settled(this.#controlsOf(path).map((control) => control.id));
      const applied = parseApplied(await this.#send(command));
      await this.#apply(applied.description);
      const refused = refusals(applied);
      this.#report(refused || `Reset ${applied.results.length} value${applied.results.length === 1 ? '' : 's'} from ${path}.`, refused !== '');
    } catch (error) {
      this.#report(error, true);
    }
  }

  /**
   * Saves a file's unsaved controls if it still has the SHA-256 the panel
   * last described or saved it with, so a change to the file since is
   * refused, not saved over.
   */
  async #save(path: string): Promise<void> {
    // Save what the product accepted, once the file's sets have answered.
    await this.#live.settled(this.#controlsOf(path).map((control) => control.id));
    const sha256 = this.#sources.baseline(path);
    const command = sha256 === undefined ? null : this.#command('save', { path, sha256 });
    if (command === null) return;
    let saved: { saved: string[]; sha256: string };
    try {
      saved = JSON.parse(await this.#send(command)) as { saved: string[]; sha256: string };
    } catch (error) {
      // The file changed elsewhere, and a live file has reached the product
      // too: show what the product and the file hold now.
      if (error instanceof Error && error.message.startsWith('CONTENT_SOURCE_STALE')) await this.refresh();
      this.#report(error, true);
      return;
    }
    this.#sources.saved(path, saved.sha256);
    const answered = await this.#readDiff(false);
    this.#updateMarks();
    const count = `${saved.saved.length} value${saved.saved.length === 1 ? '' : 's'}`;
    if (answered) {
      this.#compareFailed = false;
      this.#report(`Saved ${count} to ${path}.`, false);
    } else {
      this.#reportComparison(false, `Saved ${count} to ${path}, but could not compare it again: ${this.#sources.error}`);
    }
  }

  #controlsOf(path: string): TunableControl[] {
    return [...this.#views.values()]
      .map((view) => view.control)
      .filter((control) => control.source?.path === path);
  }

  #updateMarks(): void {
    const unsaved = this.#sources.unsaved();
    for (const { control, row } of this.#views.values()) {
      row.classList.toggle('rusty-tunables__control--unsaved', unsaved.has(control.id));
    }
    for (const [path, { count, save, reset }] of this.#fileRows) {
      const state = this.#sources.state(path);
      count.textContent = state.text;
      save.disabled = !state.canSave;
      reset.disabled = !state.canReset;
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
