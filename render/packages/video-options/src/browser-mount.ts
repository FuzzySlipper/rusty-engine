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
 * The panel's look, overridable by the game: set these custom properties on
 * `.rusty-video-options` (or any ancestor), or style its classes directly.
 */
const STYLE = `
.rusty-video-options {
  --rusty-video-options-background: rgb(14 18 26 / 94%);
  --rusty-video-options-foreground: #e9eef5;
  --rusty-video-options-muted: #9aa8b8;
  --rusty-video-options-accent: #7fb2ff;
  --rusty-video-options-border: #3a4656;
  --rusty-video-options-warning: #f0b45a;
  --rusty-video-options-radius: 0.4rem;
  --rusty-video-options-font: 0.9rem/1.4 system-ui, sans-serif;
  background: var(--rusty-video-options-background);
  color: var(--rusty-video-options-foreground);
  font: var(--rusty-video-options-font);
  border: 1px solid var(--rusty-video-options-border);
  border-radius: var(--rusty-video-options-radius);
  padding: 0.9rem 1rem;
  max-width: 34rem;
  box-sizing: border-box;
}
.rusty-video-options__title { margin: 0 0 0.6rem; font-size: 1.1em; }
.rusty-video-options__presets { display: flex; gap: 0.4rem; flex-wrap: wrap; margin-bottom: 0.6rem; }
.rusty-video-options button {
  font: inherit; color: inherit; background: transparent; cursor: pointer;
  border: 1px solid var(--rusty-video-options-border); border-radius: var(--rusty-video-options-radius);
  padding: 0.2rem 0.65rem;
}
.rusty-video-options button:hover { border-color: var(--rusty-video-options-accent); }
.rusty-video-options__group { border: 0; padding: 0; margin: 0.6rem 0 0; }
.rusty-video-options__group > legend {
  color: var(--rusty-video-options-muted); font-size: 0.8em; text-transform: uppercase;
  letter-spacing: 0.06em; padding: 0; margin-bottom: 0.25rem;
}
.rusty-video-options__option {
  display: grid; grid-template-columns: minmax(9rem, 1fr) minmax(10rem, 1.3fr);
  align-items: center; gap: 0.15rem 0.8rem; padding: 0.3rem 0;
  border-top: 1px solid color-mix(in srgb, var(--rusty-video-options-border) 50%, transparent);
}
.rusty-video-options__label { cursor: default; }
.rusty-video-options__control { display: flex; align-items: center; gap: 0.5rem; }
.rusty-video-options__control input[type=range] { flex: 1; accent-color: var(--rusty-video-options-accent); }
.rusty-video-options__control input[type=checkbox] { accent-color: var(--rusty-video-options-accent); }
.rusty-video-options__control select {
  font: inherit; color: inherit; background: var(--rusty-video-options-background);
  border: 1px solid var(--rusty-video-options-border); border-radius: var(--rusty-video-options-radius);
  padding: 0.1rem 0.3rem;
}
.rusty-video-options__value { min-width: 3.5rem; text-align: right; font-variant-numeric: tabular-nums; }
.rusty-video-options__note { grid-column: 1 / -1; color: var(--rusty-video-options-muted); font-size: 0.8em; }
.rusty-video-options__note--refused { color: var(--rusty-video-options-warning); }
.rusty-video-options__reset { font-size: 0.8em; padding: 0 0.4rem; white-space: nowrap; }
.rusty-video-options__footer { display: flex; justify-content: space-between; align-items: center; margin-top: 0.8rem; gap: 0.5rem; }
.rusty-video-options__status { color: var(--rusty-video-options-muted); font-size: 0.8em; }
.rusty-video-options__status--error { color: var(--rusty-video-options-warning); }
`;

function installStyle(document: Document): void {
  if (document.getElementById(STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = STYLE;
  document.head.appendChild(style);
}

let mounted = 0;

/**
 * Mounts the Engine's video options panel into a game-owned element. It
 * draws the Engine's renderer settings catalogue (every option this Engine
 * pair has), applies a player's change at once through the product host and
 * keeps it for the install, and draws the game's own options beside them,
 * which the game applies itself through `onChange`.
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
  let status: { text: string; error: boolean } = { text: 'Loading…', error: false };
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

  const send = async (change: VideoOptionsChange): Promise<void> => {
    pending?.abort();
    const request = new AbortController();
    pending = request;
    status = { text: 'Applying…', error: false };
    draw();
    try {
      catalogue = await transport.change(change, request.signal);
      status = { text: catalogue.stored ? 'Saved for this install.' : 'Applied for this session.', error: false };
    } catch (error) {
      if (request.signal.aborted) return;
      status = { text: error instanceof Error ? error.message : String(error), error: true };
    }
    if (!disposed) draw();
  };

  const control = (row: VideoOptionRow, id: string): HTMLElement => {
    const option = row.option;
    const value = shownValue(row);
    const apply = (next: VideoOptionValue): void => {
      if (row.source === 'product') {
        (option as ProductVideoOption).onChange(next);
      } else {
        void send({ choose: { id: option.id, value: next } });
      }
    };
    const wrap = element('div', 'rusty-video-options__control');
    if (option.kind === 'toggle') {
      const input = element('input');
      input.type = 'checkbox';
      input.id = id;
      input.checked = value === true;
      // A toggle the device refuses changes nothing: shown, with the reason,
      // but not offered.
      input.disabled = row.source === 'engine' && (option as EngineVideoOption).refused !== null;
      input.addEventListener('change', () => apply(input.checked));
      wrap.append(input);
    } else if (option.kind === 'choice') {
      const select = element('select');
      select.id = id;
      for (const choice of option.choices) {
        const item = element('option', undefined, choice.label);
        item.value = choice.value;
        item.selected = choice.value === value;
        select.append(item);
      }
      select.addEventListener('change', () => apply(select.value));
      wrap.append(select);
    } else {
      const input = element('input');
      input.type = 'range';
      input.id = id;
      input.min = String(option.min);
      input.max = String(option.max);
      input.step = String(option.step);
      input.value = String(value);
      const shown = element('output', 'rusty-video-options__value');
      shown.htmlFor.add(id);
      const unit = option.unit ?? '';
      shown.textContent = formatRange(Number(value), option.step, unit);
      input.addEventListener('input', () => {
        shown.textContent = formatRange(Number(input.value), option.step, unit);
      });
      input.addEventListener('change', () => apply(Number(input.value)));
      wrap.append(input, shown);
    }
    return wrap;
  };

  const draw = (): void => {
    root.replaceChildren();
    if (options.title !== '') root.append(element('h2', 'rusty-video-options__title', options.title ?? 'Video'));
    if (catalogue && !options.hidePresets && catalogue.presets.length > 0) {
      const presets = element('div', 'rusty-video-options__presets');
      for (const preset of catalogue.presets) {
        const button = element('button', 'rusty-video-options__preset', preset.label);
        button.type = 'button';
        button.addEventListener('click', () => void send({ preset: preset.id }));
        presets.append(button);
      }
      root.append(presets);
    }
    const groups = catalogue ? groupOptions(catalogue, productOptions, options) : [];
    for (const group of groups) {
      const fieldset = element('fieldset', 'rusty-video-options__group');
      fieldset.append(element('legend', undefined, group.label));
      for (const row of group.rows) {
        const id = `${prefix}-${row.source}-${row.option.id}`;
        const line = element('div', 'rusty-video-options__option');
        line.dataset['option'] = row.option.id;
        const label = element('label', 'rusty-video-options__label', row.label);
        label.htmlFor = id;
        label.title = row.option.description;
        const controlled = control(row, id);
        if (row.source === 'engine' && (row.option as EngineVideoOption).chosen) {
          const reset = element('button', 'rusty-video-options__reset', 'Game default');
          reset.type = 'button';
          reset.title = 'Forget your choice and use the game’s setting.';
          reset.addEventListener('click', () => void send({ forget: row.option.id }));
          controlled.append(reset);
        }
        line.append(label, controlled);
        const refused = row.source === 'engine' ? (row.option as EngineVideoOption).refused : null;
        if (refused) {
          line.append(element('div', 'rusty-video-options__note rusty-video-options__note--refused', refused));
        }
        fieldset.append(line);
      }
      root.append(fieldset);
    }
    const footer = element('div', 'rusty-video-options__footer');
    const shown = element(
      'span',
      `rusty-video-options__status${status.error ? ' rusty-video-options__status--error' : ''}`,
      status.text,
    );
    shown.setAttribute('role', 'status');
    footer.append(shown);
    if (catalogue?.options.some((option) => option.chosen)) {
      const resetAll = element('button', 'rusty-video-options__reset-all', 'Reset to game defaults');
      resetAll.type = 'button';
      resetAll.addEventListener('click', () => void send({ forget: null }));
      footer.append(resetAll);
    }
    root.append(footer);
  };

  const refresh = async (): Promise<void> => {
    pending?.abort();
    const request = new AbortController();
    pending = request;
    try {
      catalogue = await transport.read(request.signal);
      status = { text: catalogue.stored ? '' : 'Choices last for this session only.', error: false };
    } catch (error) {
      if (request.signal.aborted) return;
      status = { text: error instanceof Error ? error.message : String(error), error: true };
    }
    if (!disposed) draw();
  };

  draw();
  await refresh();
  return {
    refresh,
    setProductOptions(next) {
      productOptions = next;
      if (!disposed) draw();
    },
    dispose() {
      disposed = true;
      pending?.abort();
      root.remove();
    },
  };
}
