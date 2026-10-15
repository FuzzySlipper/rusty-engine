/** The `tunables.v1` description a product's `TunablesDebugModule` answers. */
export interface TunablesDescription {
  readonly contract: 'tunables.v1';
  readonly title: string;
  readonly choosers: readonly TunableChooser[];
  readonly timeline: TunableTimeline | null;
  readonly groups: readonly TunableGroup[];
  /** Command templates: `{name}` is filled from the call's values. */
  readonly commands: Readonly<Record<string, string>>;
  /** Shown as a warning when not empty. */
  readonly notice: string;
}

export interface TunableChooser {
  readonly id: string;
  readonly label: string;
  readonly value: string;
  readonly options: readonly string[];
}

export interface TunableTimeline {
  readonly keys: readonly { readonly index: number; readonly label: string; readonly time: number }[];
  readonly duration: number;
  readonly key: number;
  readonly speeds: readonly number[];
}

export interface TunableGroup {
  readonly id: string;
  readonly title: string;
  readonly file?: string;
  readonly controls: readonly TunableControl[];
}

export type TunableKind = 'slider' | 'toggle' | 'choice';

export interface TunableControl {
  readonly id: string;
  readonly label: string;
  readonly kind: TunableKind;
  readonly min: number;
  readonly max: number;
  readonly step: number;
  readonly value: number;
  readonly options?: readonly string[];
  readonly reshapes: boolean;
  /** Where `tunables.save` writes the value: a content-root-relative JSON file and a JSON pointer. */
  readonly source?: { readonly path: string; readonly pointer: string };
}

/** Fills `{name}` placeholders; a missing value fills nothing. */
export function fillTemplate(template: string, values: Readonly<Record<string, string | number>>): string {
  return template.replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''));
}

/** The decimals a step shows: 0.05 and 2.5 show two and one, 1 and 10 none. */
export function stepDecimals(step: number): number {
  if (!(step > 0) || Number.isInteger(step)) return 0;
  const [mantissa = '', exponent] = String(step).split('e-');
  const fraction = mantissa.split('.')[1]?.length ?? 0;
  return fraction + (exponent === undefined ? 0 : Number(exponent));
}

/** A value as the step spells it, so a saved slider writes 0.25, not 0.25000000372529. */
export function atStep(control: TunableControl, value: number): number {
  return Number(value.toFixed(stepDecimals(control.step)));
}

/** One control whose live value differs from its bound file's, as `tunables.diff` answers it. */
export interface TunableUnsaved {
  readonly id: string;
  readonly label: string;
  readonly pointer: string;
  readonly live: number;
  /** The file's value as a control value: what Reset sets back. */
  readonly saved: number;
  /** The JSON Save writes. */
  readonly save: unknown;
}

/** One bound file in a diff: its SHA-256 for `tunables.save` and its unsaved controls, or why it cannot be read. */
export interface TunablesDiffFile {
  readonly path: string;
  readonly sha256?: string;
  readonly error?: string;
  readonly unsaved: readonly TunableUnsaved[];
}

/** The `tunables.diff` answer. */
export interface TunablesDiff {
  readonly files: readonly TunablesDiffFile[];
}

/** The answer of `tunables.set-many` and `tunables.reset`: each control's value kept or refusal, then the description. */
export interface TunablesApplied {
  readonly results: readonly { readonly id: string; readonly value?: number; readonly refused?: string }[];
  readonly refused: number;
  readonly description: TunablesDescription;
}

export function parseDiff(message: string): TunablesDiff {
  const value = JSON.parse(message) as { files?: Partial<TunablesDiffFile>[] };
  return {
    files: (value.files ?? []).map((file) => ({ ...file, path: file.path ?? '', unsaved: file.unsaved ?? [] })),
  };
}

export function parseApplied(message: string): TunablesApplied {
  const value = JSON.parse(message) as Omit<TunablesApplied, 'description'> & { description: unknown };
  return {
    results: value.results ?? [],
    refused: value.refused ?? 0,
    description: parseDescription(JSON.stringify(value.description)),
  };
}

/** The refusals of an applied batch, one line each; empty when every control was applied. */
export function refusals(applied: TunablesApplied): string {
  return applied.results
    .filter((result) => result.refused !== undefined)
    .map((result) => `${result.id}: ${result.refused}`)
    .join('\n');
}

/** The unsaved controls of a diff by id, with the file each is bound to. */
export function unsavedControls(diff: TunablesDiff): Map<string, TunableUnsaved & { readonly path: string }> {
  const unsaved = new Map<string, TunableUnsaved & { readonly path: string }>();
  for (const file of diff.files) for (const control of file.unsaved) unsaved.set(control.id, { ...control, path: file.path });
  return unsaved;
}

/** A bound file's row: what it shows and which of its buttons work. */
export interface TunableFileState {
  readonly text: string;
  readonly canSave: boolean;
  readonly canReset: boolean;
}

/**
 * What the panel knows of its bound files: the latest comparison, or why it
 * failed, and per file the SHA-256 Save names. That baseline moves only when
 * the panel describes again, saves or resets, so the comparisons after each
 * set never make a change made elsewhere savable over: Save is refused
 * `CONTENT_SOURCE_STALE` until the panel describes again.
 */
export class TunableSources {
  #diff: TunablesDiff = { files: [] };
  #error: string | null = null;
  readonly #baseline = new Map<string, string>();

  /** A comparison made with a description: Save names its files' SHA-256 from now on. */
  adopt(diff: TunablesDiff): void {
    this.compared(diff);
    this.#baseline.clear();
    for (const file of diff.files) if (file.sha256 !== undefined) this.#baseline.set(file.path, file.sha256);
  }

  /** A comparison after a change: marks what is unsaved, keeps what Save names. */
  compared(diff: TunablesDiff): void {
    this.#diff = diff;
    this.#error = null;
  }

  /** A comparison that failed: the files' state is unknown until one answers. */
  failed(error: string): void {
    this.#error = error;
  }

  /** A save answered the file's new SHA-256. */
  saved(path: string, sha256: string): void {
    this.#baseline.set(path, sha256);
  }

  get diff(): TunablesDiff {
    return this.#diff;
  }

  get error(): string | null {
    return this.#error;
  }

  /** The SHA-256 Save names for a file, or undefined when no description compared it. */
  baseline(path: string): string | undefined {
    return this.#baseline.get(path);
  }

  /** The unsaved controls of the latest comparison that answered. */
  unsaved(): Map<string, TunableUnsaved & { readonly path: string }> {
    return unsavedControls(this.#diff);
  }

  state(path: string): TunableFileState {
    const unavailable = { text: 'unavailable', canSave: false, canReset: false };
    if (this.#error !== null) return unavailable;
    const file = this.#diff.files.find((candidate) => candidate.path === path);
    if (file === undefined) return unavailable;
    if (file.error !== undefined) return { text: file.error, canSave: false, canReset: false };
    const changed = file.unsaved.length;
    const baseline = this.#baseline.get(path);
    const elsewhere = baseline !== undefined && file.sha256 !== baseline;
    const count = changed === 0 ? 'saved' : `${changed} unsaved`;
    return {
      text: elsewhere ? `${changed === 0 ? '' : `${count}, `}changed on disk` : count,
      canSave: changed > 0 && baseline !== undefined,
      canReset: changed > 0,
    };
  }
}

/** Parses a describe answer, refusing anything that is not a `tunables.v1` description. */
export function parseDescription(message: string): TunablesDescription {
  const value = JSON.parse(message) as Partial<TunablesDescription>;
  if (value.contract !== 'tunables.v1' || !Array.isArray(value.groups)) {
    throw new Error('The describe command did not answer a tunables.v1 description.');
  }
  return {
    contract: 'tunables.v1',
    title: value.title ?? '',
    choosers: value.choosers ?? [],
    timeline: value.timeline ?? null,
    groups: value.groups,
    commands: value.commands ?? {},
    notice: value.notice ?? '',
  };
}

/**
 * Live control values: the value each control shows, and the value the
 * product accepted (from the description or a set's answer). One set per
 * control is in flight; a newer request replaces one still waiting. Save
 * and Reset wait for the controls' sets to settle.
 */
export class LiveValues {
  readonly #send: (id: string, value: number) => Promise<number>;
  readonly #accepted = new Map<string, number>();
  readonly #requested = new Map<string, number>();
  readonly #waiting = new Map<string, number>();
  readonly #running = new Map<string, Promise<void>>();

  /** `send` applies one value and answers the value the product kept. */
  constructor(send: (id: string, value: number) => Promise<number>) {
    this.#send = send;
  }

  /** Takes the accepted values of a new description. */
  reset(values: Iterable<readonly [string, number]>): void {
    this.#accepted.clear();
    this.#requested.clear();
    for (const [id, value] of values) this.#accepted.set(id, value);
  }

  /** What the control shows: the latest request, else the accepted value. */
  shown(id: string): number | undefined {
    return this.#requested.get(id) ?? this.#accepted.get(id);
  }

  get accepted(): ReadonlyMap<string, number> {
    return this.#accepted;
  }

  /**
   * Requests a value. Settles when the control has no set in flight; a
   * refusal rejects, and the control shows its accepted value again.
   */
  set(id: string, value: number): Promise<void> {
    this.#requested.set(id, value);
    const running = this.#running.get(id);
    if (running) {
      this.#waiting.set(id, value);
      return running;
    }
    const run = (async () => {
      try {
        for (let next: number | undefined = value; next !== undefined; next = this.#waiting.get(id)) {
          this.#waiting.delete(id);
          this.#accepted.set(id, await this.#send(id, next));
        }
      } finally {
        this.#waiting.delete(id);
        this.#requested.delete(id);
        this.#running.delete(id);
      }
    })();
    this.#running.set(id, run);
    return run;
  }

  /** Waits until none of these controls has a set in flight or waiting. */
  async settled(ids: Iterable<string>): Promise<void> {
    await Promise.allSettled([...ids].map((id) => this.#running.get(id)));
  }
}
