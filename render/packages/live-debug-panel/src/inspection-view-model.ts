/** The commands a view pane sends; every action of the pane is one of them. */
export const inspectCommands = {
  create: (id: string, anchor: string) => `engine.inspect.view create ${id} anchor ${anchor}`,
  close: (id: string) => `engine.inspect.view close ${id}`,
  select: (id: string) => `engine.inspect.view select ${id}`,
  frame: (id: string, framing: string, subject: string) =>
    `engine.inspect.view frame ${id} ${framing} ${subject}`,
  orbit: (id: string, yaw: number, pitch: number) =>
    `engine.inspect.view orbit ${id} ${round(yaw)} ${round(pitch)}`,
  pan: (id: string, right: number, up: number) => `engine.inspect.view pan ${id} ${round(right)} ${round(up)}`,
  zoom: (id: string, factor: number) => `engine.inspect.view zoom ${id} ${round(factor)}`,
  /** Frames the view and writes it beside a reference image, with an onion-skin panel. */
  compare: (id: string, framing: string, subject: string, reference: string) =>
    `engine.inspect.view compare ${id} ${framing} ${subject} ${reference} compare-${id}.png onion`,
  targets: () => 'engine.inspect.target list',
  showSetsIn: (id: string) => `engine.inspect.overlay show sets in ${id}`,
  hideSetsIn: (id: string) => `engine.inspect.overlay hide sets in ${id}`,
  overlays: () => 'engine.inspect.overlay list',
} as const;

/** The tool capture route for one view, with its overlays. */
export function captureUrl(id: string, width: number, height: number): string {
  return `/__rusty/product/runtime/frames/capture?view=${encodeURIComponent(id)}&overlays=1&width=${width}&height=${height}`;
}

/** Degrees of orbit per pixel dragged, metres of pan per pixel per metre away, zoom per wheel notch. */
export const POINTER = { orbitDegrees: 0.4, panPerPixel: 0.002, zoomStep: 1.12 } as const;

/** A drag of `dx`, `dy` pixels as an orbit: right turns the camera right about the focus, down raises it. */
export function dragOrbit(dx: number, dy: number): [number, number] {
  return [dx * POINTER.orbitDegrees, dy * POINTER.orbitDegrees];
}

/** A drag as a pan, scaled by the distance to the focus so near and far pan alike on screen. */
export function dragPan(dx: number, dy: number, distance: number): [number, number] {
  const scale = POINTER.panPerPixel * Math.max(distance, 0.05);
  return [-dx * scale, dy * scale];
}

/** A wheel delta as a zoom factor: away zooms out. */
export function wheelZoom(deltaY: number): number {
  return deltaY > 0 ? POINTER.zoomStep : 1 / POINTER.zoomStep;
}

/** The subjects a pane frames: the space before the player's camera, then the product's targets. */
export function subjects(targetsAnswer: string | null): { value: string; label: string; framing?: string }[] {
  const list: { value: string; label: string; framing?: string }[] = [{ value: 'camera', label: 'Player camera' }];
  if (targetsAnswer === null) return list;
  try {
    const parsed = JSON.parse(targetsAnswer) as { targets?: { name: string; framing?: string }[] };
    for (const target of parsed.targets ?? []) {
      list.push({ value: `target ${target.name}`, label: target.name, ...(target.framing ? { framing: target.framing } : {}) });
    }
  } catch {
    // Not a target list: frame the camera only.
  }
  return list;
}

/** A view's distance from its focus, from an `engine.inspect.view` listing. */
export function focusDistance(listing: string, id: string): number {
  const parsed = JSON.parse(listing) as {
    views?: { id: string; pose: { position: number[] }; focus: number[] }[];
  };
  const view = parsed.views?.find((candidate) => candidate.id === id);
  if (!view) return 1;
  const [x = 0, y = 0, z = 0] = view.pose.position;
  const [fx = 0, fy = 0, fz = 0] = view.focus;
  return Math.hypot(x - fx, y - fy, z - fz);
}

function round(value: number): string {
  return Number(value.toFixed(4)).toString();
}

/**
 * A drag's movement not yet sent. Moves add to it; `take` hands over what
 * accumulated (null when nothing did), so the last segment of a gesture is
 * sent on release rather than lost.
 */
export class DragGesture {
  #x: number;
  #y: number;
  #dx = 0;
  #dy = 0;

  constructor(x: number, y: number, readonly pan: boolean) {
    this.#x = x;
    this.#y = y;
  }

  move(x: number, y: number): void {
    this.#dx += x - this.#x;
    this.#dy += y - this.#y;
    this.#x = x;
    this.#y = y;
  }

  take(): [number, number] | null {
    if (this.#dx === 0 && this.#dy === 0) return null;
    const taken: [number, number] = [this.#dx, this.#dy];
    this.#dx = 0;
    this.#dy = 0;
    return taken;
  }
}

/** Which views show every overlay set, from an `engine.inspect.overlay` answer. */
export function setsIn(answer: string): readonly string[] {
  const parsed = JSON.parse(answer) as { setsIn?: string[] };
  return parsed.setsIn ?? [];
}

/**
 * A pane's overlay toggle: whether its own view shows every overlay set.
 * Its state is read back from the Engine after each change, so two panes'
 * toggles each show their own view, and releasing one leaves the others'.
 */
export class ViewOverlayToggle {
  #pressed = false;

  constructor(
    readonly id: string,
    readonly send: (command: string) => Promise<string>,
  ) {}

  get pressed(): boolean {
    return this.#pressed;
  }

  /** Reads whether the view shows every set now. */
  async sync(): Promise<boolean> {
    this.#pressed = setsIn(await this.send(inspectCommands.overlays())).includes(this.id);
    return this.#pressed;
  }

  async toggle(): Promise<boolean> {
    const answer = await this.send(this.#pressed ? inspectCommands.hideSetsIn(this.id) : inspectCommands.showSetsIn(this.id));
    this.#pressed = setsIn(answer).includes(this.id);
    return this.#pressed;
  }

  /** Stops the view showing every set, if this toggle turned it on. */
  async release(): Promise<void> {
    if (this.#pressed) await this.send(inspectCommands.hideSetsIn(this.id));
    this.#pressed = false;
  }
}
