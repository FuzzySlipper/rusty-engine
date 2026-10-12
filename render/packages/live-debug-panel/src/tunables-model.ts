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
  /** Where Save writes the value: a content-root-relative JSON file and a JSON pointer. */
  readonly source?: { readonly path: string; readonly pointer: string };
}

/** A bound source file as `engine.content.source.read` answered it. */
export interface SourceFile {
  readonly sha256: string;
  readonly document: unknown;
}

/** Fills `{name}` placeholders; a missing value fills nothing. */
export function fillTemplate(template: string, values: Readonly<Record<string, string | number>>): string {
  return template.replace(/\{(\w+)\}/g, (_, name: string) => String(values[name] ?? ''));
}

/** The value an RFC 6901 pointer names in a parsed document, or undefined. */
export function resolvePointer(document: unknown, pointer: string): unknown {
  if (pointer === '') return document;
  if (!pointer.startsWith('/')) return undefined;
  let value: unknown = document;
  for (const raw of pointer.slice(1).split('/')) {
    const token = raw.replace(/~1/g, '/').replace(/~0/g, '~');
    if (Array.isArray(value)) {
      if (!/^(0|[1-9][0-9]*)$/.test(token)) return undefined;
      value = value[Number(token)];
    } else if (value !== null && typeof value === 'object') {
      if (!Object.prototype.hasOwnProperty.call(value, token)) return undefined;
      value = (value as Record<string, unknown>)[token];
    } else {
      return undefined;
    }
  }
  return value;
}

/** The control value a source JSON value stands for, or undefined when it cannot be compared. */
export function controlValueOf(control: TunableControl, source: unknown): number | undefined {
  if (typeof source === 'number') return source;
  if (typeof source === 'boolean') return source ? 1 : 0;
  if (typeof source === 'string') {
    const index = control.options?.indexOf(source) ?? -1;
    return index >= 0 ? index : undefined;
  }
  return undefined;
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

/** The JSON value to save for a control's live value, in the source value's own type. */
export function sourceValueOf(control: TunableControl, value: number, previous: unknown): unknown {
  if (typeof previous === 'boolean' || (previous === undefined && control.kind === 'toggle')) return value >= 0.5;
  if (typeof previous === 'string' && control.options) {
    return control.options[Math.round(value)] ?? previous;
  }
  return atStep(control, value);
}

/** Whether a control's live value differs from what its source holds (floats at half a step). */
export function isUnsaved(control: TunableControl, value: number, source: unknown): boolean {
  const saved = controlValueOf(control, source);
  if (saved === undefined) return false;
  if (control.kind === 'toggle') return value >= 0.5 !== saved >= 0.5;
  if (control.kind === 'choice') return Math.round(value) !== Math.round(saved);
  const tolerance = control.step > 0 ? control.step / 2 : 0;
  return Math.abs(value - saved) > tolerance;
}

/** The files the description's controls are bound to, in first-seen order. */
export function boundFiles(description: TunablesDescription): string[] {
  const files: string[] = [];
  for (const group of description.groups) {
    for (const control of group.controls) {
      const path = control.source?.path;
      if (path !== undefined && !files.includes(path)) files.push(path);
    }
  }
  return files;
}

/** The RFC 6902 replace operations that save a file's unsaved controls. */
export function saveEdits(
  controls: readonly TunableControl[],
  values: ReadonlyMap<string, number>,
  file: SourceFile,
): { op: 'replace'; path: string; value: unknown }[] {
  const edits: { op: 'replace'; path: string; value: unknown }[] = [];
  for (const control of controls) {
    if (!control.source) continue;
    const value = values.get(control.id) ?? control.value;
    const source = resolvePointer(file.document, control.source.pointer);
    if (!isUnsaved(control, value, source)) continue;
    edits.push({ op: 'replace', path: control.source.pointer, value: sourceValueOf(control, value, source) });
  }
  return edits;
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
 * persists accepted values only, after the controls' sets settle.
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
