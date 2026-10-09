import {
  createVideoOptionsHttpTransport,
  formatRange,
  groupOptions,
  shownValue,
  type EngineVideoOption,
  type ProductVideoOption,
  type VideoOptionRow,
  type VideoOptionsCatalogue,
  type VideoOptionsChange,
  type VideoOptionsPresentation,
  type VideoOptionsTransport,
  type VideoOptionValue,
} from './video-options-model.js';

export {
  createVideoOptionsHttpTransport,
  VIDEO_OPTIONS_PATH,
  type EngineVideoOption,
  type ProductVideoOption,
  type VideoOptionChoice,
  type VideoOptionPreset,
  type VideoOptionsCatalogue,
  type VideoOptionsChange,
  type VideoOptionsHttpTransportOptions,
  type VideoOptionsPresentation,
  type VideoOptionsTransport,
  type VideoOptionValue,
} from './video-options-model.js';

/** Product-owned configuration for one mounted video options panel. */
export interface VideoOptionsMountOptions extends VideoOptionsPresentation {
  /** The game's own options, drawn beside the Engine's in their groups. */
  readonly productOptions?: readonly ProductVideoOption[];
  /** The panel's heading; "Video" when omitted, none when empty. */
  readonly title?: string;
  /** Uses the same-origin HTTP transport when omitted. */
  readonly transport?: VideoOptionsTransport;
}

/** A mounted panel. */
export interface VideoOptionsMount {
  /** Reads the catalogue again, as after the game changed its own settings. */
  refresh(): Promise<void>;
  /** Replaces the game's own options, as after one of them changed. */
  setProductOptions(options: readonly ProductVideoOption[]): void;
  /** Removes the panel and stops its requests. */
  dispose(): void;
}

const STYLE_ID = 'rusty-video-options-style';

/**
 * The panel's look, overridable by the game: set the `--rusty-video-options-*`
 * custom properties on `.rusty-video-options` or any ancestor, or style its
 * classes directly. The panel declares none of them itself, only the private
 * `--_rvo-*` copies with their defaults as fallbacks, so an inherited value
 * wins.
 */
const STYLE = `
.rusty-video-options {
  --_rvo-background: var(--rusty-video-options-background, rgb(14 18 26 / 94%));
  --_rvo-foreground: var(--rusty-video-options-foreground, #e9eef5);
  --_rvo-muted: var(--rusty-video-options-muted, #9aa8b8);
  --_rvo-accent: var(--rusty-video-options-accent, #7fb2ff);
  --_rvo-border: var(--rusty-video-options-border, #3a4656);
  --_rvo-warning: var(--rusty-video-options-warning, #f0b45a);
  --_rvo-radius: var(--rusty-video-options-radius, 0.4rem);
  --_rvo-font: var(--rusty-video-options-font, 0.9rem/1.4 system-ui, sans-serif);
  background: var(--_rvo-background);
  color: var(--_rvo-foreground);
  font: var(--_rvo-font);
  border: 1px solid var(--_rvo-border);
  border-radius: var(--_rvo-radius);
  padding: 0.9rem 1rem;
  max-width: 34rem;
  box-sizing: border-box;
}
.rusty-video-options__title { margin: 0 0 0.6rem; font-size: 1.1em; }
.rusty-video-options__presets { display: flex; gap: 0.4rem; flex-wrap: wrap; margin-bottom: 0.6rem; }
.rusty-video-options__presets:empty { display: none; }
.rusty-video-options button {
  font: inherit; color: inherit; background: transparent; cursor: pointer;
  border: 1px solid var(--_rvo-border); border-radius: var(--_rvo-radius);
  padding: 0.2rem 0.65rem;
}
.rusty-video-options button:hover { border-color: var(--_rvo-accent); }
.rusty-video-options__group { border: 0; padding: 0; margin: 0.6rem 0 0; }
.rusty-video-options__group > legend {
  color: var(--_rvo-muted); font-size: 0.8em; text-transform: uppercase;
  letter-spacing: 0.06em; padding: 0; margin-bottom: 0.25rem;
}
.rusty-video-options__option {
  display: grid; grid-template-columns: minmax(9rem, 1fr) minmax(10rem, 1.3fr);
  align-items: center; gap: 0.15rem 0.8rem; padding: 0.3rem 0;
  border-top: 1px solid color-mix(in srgb, var(--_rvo-border) 50%, transparent);
}
.rusty-video-options__label { cursor: default; }
.rusty-video-options__control { display: flex; align-items: center; gap: 0.5rem; }
.rusty-video-options__control input[type=range] { flex: 1; accent-color: var(--_rvo-accent); }
.rusty-video-options__control input[type=checkbox] { accent-color: var(--_rvo-accent); }
.rusty-video-options__control select {
  font: inherit; color: inherit; background: var(--_rvo-background);
  border: 1px solid var(--_rvo-border); border-radius: var(--_rvo-radius);
  padding: 0.1rem 0.3rem;
}
.rusty-video-options__value { min-width: 3.5rem; text-align: right; font-variant-numeric: tabular-nums; }
.rusty-video-options__note { grid-column: 1 / -1; color: var(--_rvo-muted); font-size: 0.8em; }
.rusty-video-options__note--refused { color: var(--_rvo-warning); }
.rusty-video-options__reset { font-size: 0.8em; padding: 0 0.4rem; white-space: nowrap; }
.rusty-video-options__footer { display: flex; justify-content: space-between; align-items: center; margin-top: 0.8rem; gap: 0.5rem; }
.rusty-video-options__status { color: var(--_rvo-muted); font-size: 0.8em; }
.rusty-video-options__status--error { color: var(--_rvo-warning); }
`;

function installStyle(document: Document): void {
  if (document.getElementById(STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = STYLE;
  document.head.appendChild(style);
}

let mounted = 0;

/** One drawn option row, kept across updates so its controls keep focus. */
interface RowView {
  /** The row as last drawn; its controls' listeners read it. */
  row: VideoOptionRow;
  /** What the row's elements were built for: its kind and, for a choice, its choices. */
  readonly shape: string;
  readonly line: HTMLDivElement;
  readonly label: HTMLLabelElement;
  readonly wrap: HTMLDivElement;
  readonly input: HTMLInputElement | HTMLSelectElement;
  readonly shown: HTMLOutputElement | null;
  reset: HTMLButtonElement | null;
  note: HTMLDivElement | null;
}

/** Changes when a row's controls must be built again rather than updated. */
function rowShape(row: VideoOptionRow): string {
  const option = row.option;
  return option.kind === 'choice'
    ? `choice:${JSON.stringify(option.choices.map((choice) => [choice.value, choice.label]))}`
    : option.kind;
}

/** Moves `node` to be `parent`'s child at `index`, only when it is not already there. */
function place(parent: Element, node: Element, index: number): void {
  if (parent.children[index] !== node) parent.insertBefore(node, parent.children[index] ?? null);
}

/**
 * Mounts the Engine's video options panel into a game-owned element. It
 * draws the Engine's renderer settings catalogue (every option this Engine
 * pair has), applies a player's change at once through the product host and
 * keeps it for the install, and draws the game's own options beside them,
 * which the game applies itself through `onChange`. A new catalogue or new
 * product options update the rows in place, so the control a player is using
 * keeps focus and position.
 */
export async function mountVideoOptions(
  host: HTMLElement,
  options: VideoOptionsMountOptions = {},
): Promise<VideoOptionsMount> {
  if (!(host instanceof HTMLElement)) {
    throw new TypeError('Video options mounting requires an HTMLElement host.');
  }
  const document = host.ownerDocument;
  installStyle(document);
  const transport = options.transport ?? createVideoOptionsHttpTransport();
  const prefix = `rusty-video-options-${++mounted}`;
  const root = document.createElement('section');
  root.className = 'rusty-video-options';
  root.setAttribute('aria-label', options.title === '' ? 'Video options' : (options.title ?? 'Video'));
  host.appendChild(root);

  let catalogue: VideoOptionsCatalogue | null = null;
  let productOptions = options.productOptions ?? [];
  let disposed = false;
  let pending: AbortController | null = null;

  const element = <K extends keyof HTMLElementTagNameMap>(
    tag: K,
    className?: string,
    text?: string,
  ): HTMLElementTagNameMap[K] => {
    const node = document.createElement(tag);
    if (className) node.className = className;
    if (text !== undefined) node.textContent = text;
    return node;
  };

  // The panel's fixed parts. Rows go in group fieldsets between the presets
  // and the footer.
  if (options.title !== '') root.append(element('h2', 'rusty-video-options__title', options.title ?? 'Video'));
  const presets = element('div', 'rusty-video-options__presets');
  let presetsDrawn = '';
  const footer = element('div', 'rusty-video-options__footer');
  const status = element('span', 'rusty-video-options__status', 'Loading…');
  status.setAttribute('role', 'status');
  const resetAll = element('button', 'rusty-video-options__reset-all', 'Reset to game defaults');
  resetAll.type = 'button';
  resetAll.hidden = true;
  resetAll.addEventListener('click', () => void send({ forget: null }));
  footer.append(status, resetAll);
  root.append(presets, footer);
  const groups = new Map<string, HTMLFieldSetElement>();
  const rows = new Map<string, RowView>();

  const setStatus = (text: string, error = false): void => {
    status.textContent = text;
    status.classList.toggle('rusty-video-options__status--error', error);
  };

  const send = async (change: VideoOptionsChange): Promise<void> => {
    pending?.abort();
    const request = new AbortController();
    pending = request;
    setStatus('Applying…');
    try {
      catalogue = await transport.change(change, request.signal);
      setStatus(catalogue.stored ? 'Saved for this install.' : 'Applied for this session.');
    } catch (error) {
      if (request.signal.aborted) return;
      setStatus(error instanceof Error ? error.message : String(error), true);
    }
    if (!disposed) update();
  };

  const apply = (view: RowView, next: VideoOptionValue): void => {
    if (view.row.source === 'product') {
      (view.row.option as ProductVideoOption).onChange(next);
    } else {
      void send({ choose: { id: view.row.option.id, value: next } });
    }
  };

  const buildRow = (row: VideoOptionRow, id: string): RowView => {
    const option = row.option;
    const line = element('div', 'rusty-video-options__option');
    line.dataset['option'] = option.id;
    const label = element('label', 'rusty-video-options__label');
    label.htmlFor = id;
    const wrap = element('div', 'rusty-video-options__control');
    let input: HTMLInputElement | HTMLSelectElement;
    let shown: HTMLOutputElement | null = null;
    if (option.kind === 'toggle') {
      const checkbox = element('input');
      checkbox.type = 'checkbox';
      checkbox.addEventListener('change', () => apply(view, checkbox.checked));
      input = checkbox;
      wrap.append(checkbox);
    } else if (option.kind === 'choice') {
      const select = element('select');
      for (const choice of option.choices) {
        const item = element('option', undefined, choice.label);
        item.value = choice.value;
        select.append(item);
      }
      select.addEventListener('change', () => apply(view, select.value));
      input = select;
      wrap.append(select);
    } else {
      const range = element('input');
      range.type = 'range';
      const output = element('output', 'rusty-video-options__value');
      output.htmlFor.add(id);
      range.addEventListener('input', () => {
        const current = view.row.option;
        if (current.kind === 'range') output.textContent = formatRange(Number(range.value), current.step, current.unit ?? '');
      });
      range.addEventListener('change', () => apply(view, Number(range.value)));
      input = range;
      shown = output;
      wrap.append(range, output);
    }
    input.id = id;
    line.append(label, wrap);
    const view: RowView = { row, shape: rowShape(row), line, label, wrap, input, shown, reset: null, note: null };
    return view;
  };

  /** Brings a row's text, value, disabled state, reset button and note up to date. */
  const updateRow = (view: RowView, row: VideoOptionRow): void => {
    view.row = row;
    const option = row.option;
    const engine = row.source === 'engine' ? (option as EngineVideoOption) : null;
    view.label.textContent = row.label;
    view.label.title = engine ? `${option.description}\n${engine.cost}` : option.description;
    // A control the player is using keeps the value they gave it; the
    // catalogue's answer to that change agrees with it anyway.
    const value = shownValue(row);
    const active = document.activeElement === view.input;
    if (option.kind === 'toggle') {
      const checkbox = view.input as HTMLInputElement;
      if (!active) checkbox.checked = value === true;
      // A toggle the device refuses changes nothing: shown, with the reason,
      // but not offered.
      checkbox.disabled = engine !== null && engine.refused !== null;
    } else if (option.kind === 'choice') {
      if (!active) (view.input as HTMLSelectElement).value = String(value);
    } else {
      const range = view.input as HTMLInputElement;
      range.min = String(option.min);
      range.max = String(option.max);
      range.step = String(option.step);
      if (!active) range.value = String(value);
      if (view.shown) view.shown.textContent = formatRange(Number(range.value), option.step, option.unit ?? '');
    }
    if (engine?.chosen && !view.reset) {
      const reset = element('button', 'rusty-video-options__reset', 'Game default');
      reset.type = 'button';
      reset.title = 'Forget your choice and use the game’s setting.';
      reset.addEventListener('click', () => void send({ forget: view.row.option.id }));
      view.wrap.append(reset);
      view.reset = reset;
    } else if (!engine?.chosen && view.reset) {
      // Removing the focused button would drop focus to the page: keep it
      // on the row's control instead.
      const hadFocus = document.activeElement === view.reset;
      view.reset.remove();
      view.reset = null;
      if (hadFocus) view.input.focus();
    }
    const refused = engine?.refused ?? null;
    if (refused && !view.note) {
      view.note = element('div', 'rusty-video-options__note rusty-video-options__note--refused');
      view.line.append(view.note);
    } else if (!refused && view.note) {
      view.note.remove();
      view.note = null;
    }
    if (view.note) view.note.textContent = refused;
  };

  const updatePresets = (): void => {
    const shown = catalogue && !options.hidePresets ? catalogue.presets : [];
    const drawn = JSON.stringify(shown.map((preset) => [preset.id, preset.label]));
    if (drawn === presetsDrawn) return;
    presetsDrawn = drawn;
    presets.replaceChildren(
      ...shown.map((preset) => {
        const button = element('button', 'rusty-video-options__preset', preset.label);
        button.type = 'button';
        button.addEventListener('click', () => void send({ preset: preset.id }));
        return button;
      }),
    );
  };

  /** Brings the panel up to date with the catalogue and product options, in place. */
  const update = (): void => {
    updatePresets();
    const drawnGroups = catalogue ? groupOptions(catalogue, productOptions, options) : [];
    const rowKey = (row: VideoOptionRow): string => `${row.source}:${row.option.id}`;
    // Rows and groups that went first, so the ones kept never have to move
    // past them: moving an element takes focus from it.
    const keptGroups = new Set(drawnGroups.map((group) => group.name));
    const keptRows = new Set(drawnGroups.flatMap((group) => group.rows.map(rowKey)));
    for (const [key, view] of rows) {
      if (!keptRows.has(key)) {
        view.line.remove();
        rows.delete(key);
      }
    }
    for (const [name, fieldset] of groups) {
      if (!keptGroups.has(name)) {
        fieldset.remove();
        groups.delete(name);
      }
    }
    // After the title and the presets.
    const firstGroup = options.title !== '' ? 2 : 1;
    drawnGroups.forEach((group, groupIndex) => {
      let fieldset = groups.get(group.name);
      if (!fieldset) {
        fieldset = element('fieldset', 'rusty-video-options__group');
        fieldset.append(element('legend'));
        groups.set(group.name, fieldset);
      }
      fieldset.firstElementChild!.textContent = group.label;
      place(root, fieldset, firstGroup + groupIndex);
      group.rows.forEach((row, rowIndex) => {
        const key = rowKey(row);
        let view = rows.get(key);
        if (view && view.shape !== rowShape(row)) {
          view.line.remove();
          view = undefined;
        }
        if (!view) {
          view = buildRow(row, `${prefix}-${row.source}-${row.option.id}`);
          rows.set(key, view);
        }
        updateRow(view, row);
        // After the legend.
        place(fieldset!, view.line, 1 + rowIndex);
      });
    });
    resetAll.hidden = !catalogue?.options.some((option) => option.chosen);
  };

  const refresh = async (): Promise<void> => {
    pending?.abort();
    const request = new AbortController();
    pending = request;
    try {
      catalogue = await transport.read(request.signal);
      setStatus(catalogue.stored ? '' : 'Choices last for this session only.');
    } catch (error) {
      if (request.signal.aborted) return;
      setStatus(error instanceof Error ? error.message : String(error), true);
    }
    if (!disposed) update();
  };

  await refresh();
  return {
    refresh,
    setProductOptions(next) {
      productOptions = next;
      if (!disposed) update();
    },
    dispose() {
      disposed = true;
      pending?.abort();
      root.remove();
    },
  };
}
