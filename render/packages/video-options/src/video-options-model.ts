/**
 * The video options the Engine serves at
 * `/__rusty/product/runtime/video-options`: its renderer settings catalogue
 * with the values in effect, and the changes the panel sends back. Pure data
 * and request helpers; the DOM is in `browser-mount.ts`.
 */

/** Where the product host serves the catalogue and takes changes. */
export const VIDEO_OPTIONS_PATH = '/__rusty/product/runtime/video-options';

/** A value of an option: a toggle's boolean, a choice's name, a range's number. */
export type VideoOptionValue = boolean | string | number;

export interface VideoOptionChoice {
  readonly value: string;
  readonly label: string;
}

interface VideoOptionBase {
  readonly id: string;
  readonly label: string;
  readonly group: string;
  readonly description: string;
}

/** One option of the Engine catalogue, with its state. */
export type EngineVideoOption = VideoOptionBase &
  (
    | { readonly kind: 'toggle' }
    | { readonly kind: 'choice'; readonly choices: readonly VideoOptionChoice[] }
    | { readonly kind: 'range'; readonly min: number; readonly max: number; readonly step: number; readonly unit: string }
  ) & {
    /** What draws. */
    readonly value: VideoOptionValue;
    /** What is asked of the device: the game's value or the player's choice. */
    readonly requested: VideoOptionValue;
    /** The game's own value, before the player's choice. */
    readonly gameDefault: VideoOptionValue;
    /** Whether the player chose this option. */
    readonly chosen: boolean;
    /** A change takes effect only when the game restarts. */
    readonly restart: boolean;
    /** What the setting costs, as measured. */
    readonly cost: string;
    /** Why the device does not draw it as asked, in words, or null. */
    readonly refused: string | null;
  };

export interface VideoOptionPreset {
  readonly id: string;
  readonly label: string;
}

/** The catalogue as the Engine serves it. */
export interface VideoOptionsCatalogue {
  readonly version: number;
  /** Whether choices are kept for the install (the game has a persistence root). */
  readonly stored: boolean;
  readonly presets: readonly VideoOptionPreset[];
  readonly options: readonly EngineVideoOption[];
}

/** A change the panel asks of the Engine. */
export type VideoOptionsChange =
  | { readonly choose: { readonly id: string; readonly value: VideoOptionValue } }
  | { readonly forget: string | null }
  | { readonly preset: string };

/** How the panel reaches the Engine. */
export interface VideoOptionsTransport {
  read(signal?: AbortSignal): Promise<VideoOptionsCatalogue>;
  change(change: VideoOptionsChange, signal?: AbortSignal): Promise<VideoOptionsCatalogue>;
}

export interface VideoOptionsHttpTransportOptions {
  /** The product host's origin; the page's own when omitted. */
  readonly origin?: string;
  readonly fetch?: typeof fetch;
}

/** The product host's route, same-origin by default. */
export function createVideoOptionsHttpTransport(
  options: VideoOptionsHttpTransportOptions = {},
): VideoOptionsTransport {
  const fetcher = options.fetch ?? globalThis.fetch.bind(globalThis);
  const url = `${options.origin ?? ''}${VIDEO_OPTIONS_PATH}`;
  const answer = async (response: Response): Promise<VideoOptionsCatalogue> => {
    const body: unknown = await response.json().catch(() => null);
    if (!response.ok) {
      throw new Error(errorDiagnostic(body) ?? `HTTP ${response.status}`);
    }
    return parseCatalogue(body);
  };
  return {
    async read(signal) {
      return answer(await fetcher(url, signal ? { signal } : {}));
    },
    async change(change, signal) {
      return answer(
        await fetcher(url, {
          method: 'POST',
          headers: { 'content-type': 'application/json' },
          body: JSON.stringify(change),
          ...(signal ? { signal } : {}),
        }),
      );
    },
  };
}

/** The product host's `{ error: { diagnostic } }`, when the body is one. */
function errorDiagnostic(body: unknown): string | null {
  if (typeof body !== 'object' || body === null || !('error' in body)) return null;
  const error: unknown = body.error;
  if (typeof error !== 'object' || error === null || !('diagnostic' in error)) return null;
  return typeof error.diagnostic === 'string' ? error.diagnostic : null;
}

/** Checks the served catalogue's shape, so a mismatched host fails plainly. */
export function parseCatalogue(body: unknown): VideoOptionsCatalogue {
  if (typeof body !== 'object' || body === null) {
    throw new Error('video options: the host sent no catalogue');
  }
  const catalogue = body as Partial<VideoOptionsCatalogue>;
  if (!Array.isArray(catalogue.options) || !Array.isArray(catalogue.presets)) {
    throw new Error('video options: the catalogue has no options');
  }
  return catalogue as VideoOptionsCatalogue;
}

/** A game's own option, shown beside the Engine's; the game applies it. */
export type ProductVideoOption = VideoOptionBase &
  (
    | { readonly kind: 'toggle'; readonly value: boolean }
    | { readonly kind: 'choice'; readonly choices: readonly VideoOptionChoice[]; readonly value: string }
    | {
        readonly kind: 'range';
        readonly min: number;
        readonly max: number;
        readonly step: number;
        readonly unit?: string;
        readonly value: number;
      }
  ) & {
    /** Called with the player's new value: the game applies and keeps it. */
    readonly onChange: (value: VideoOptionValue) => void;
  };

/** How a game trims and names the Engine's options. */
export interface VideoOptionsPresentation {
  /** Option ids not shown. Unknown ids are ignored, so a newer list works on an older pin. */
  readonly hide?: readonly string[];
  /** Labels by option id or group name. */
  readonly labels?: Readonly<Record<string, string>>;
  /** Hide the preset buttons. */
  readonly hidePresets?: boolean;
}

/** One row the panel draws. */
export interface VideoOptionRow {
  readonly source: 'engine' | 'product';
  readonly option: EngineVideoOption | ProductVideoOption;
  readonly label: string;
}

/** The rows by group, in catalogue order, product groups after. */
export interface VideoOptionGroup {
  readonly name: string;
  readonly label: string;
  readonly rows: readonly VideoOptionRow[];
}

/** Groups the catalogue's options and the game's for drawing. */
export function groupOptions(
  catalogue: VideoOptionsCatalogue,
  productOptions: readonly ProductVideoOption[],
  presentation: VideoOptionsPresentation = {},
): VideoOptionGroup[] {
  const hidden = new Set(presentation.hide ?? []);
  const labels = presentation.labels ?? {};
  const groups = new Map<string, VideoOptionRow[]>();
  const add = (row: VideoOptionRow): void => {
    const rows = groups.get(row.option.group) ?? [];
    rows.push(row);
    groups.set(row.option.group, rows);
  };
  for (const option of catalogue.options) {
    if (!hidden.has(option.id)) add({ source: 'engine', option, label: labels[option.id] ?? option.label });
  }
  for (const option of productOptions) {
    if (!hidden.has(option.id)) add({ source: 'product', option, label: labels[option.id] ?? option.label });
  }
  return [...groups].map(([name, rows]) => ({ name, label: labels[name] ?? name, rows }));
}

/** The value a row shows: an Engine option's requested value (what the player asked for), a product option's value. */
export function shownValue(row: VideoOptionRow): VideoOptionValue {
  return row.source === 'engine' ? (row.option as EngineVideoOption).requested : row.option.value;
}

/** Text for a range value, with its unit. */
export function formatRange(value: number, step: number, unit = ''): string {
  const decimals = Math.max(0, Math.min(3, Math.ceil(-Math.log10(step))));
  return `${value.toFixed(decimals)}${unit === '' ? '' : unit === '×' ? '×' : ` ${unit}`}`;
}
