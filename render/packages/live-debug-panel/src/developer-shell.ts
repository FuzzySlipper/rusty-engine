import type { LiveDebugTransport } from '@rusty-engine/live-debug-client';

import type { InspectionViewOptions } from './inspection-view-pane.js';
import type { TunablesPanelOptions } from './tunables-panel.js';

const STYLE_ID = 'rusty-developer-shell-style';

/**
 * The shell's look, overridable by the product: set the `--rusty-shell-*`
 * custom properties on `.rusty-developer-shell` or an ancestor; the panels
 * keep their own `--rusty-*` properties.
 */
const STYLE = `
.rusty-developer-shell {
  --_rs-background: var(--rusty-shell-background, rgb(10 14 20 / 92%));
  --_rs-foreground: var(--rusty-shell-foreground, #e9eef5);
  --_rs-border: var(--rusty-shell-border, #3a4656);
  --_rs-accent: var(--rusty-shell-accent, #7fb2ff);
  --_rs-width: var(--rusty-shell-width, 26rem);
  --_rs-font: var(--rusty-shell-font, 0.85rem/1.4 system-ui, sans-serif);
  position: absolute; top: 0; bottom: 0; width: var(--_rs-width); max-width: 100%;
  display: flex; flex-direction: column; box-sizing: border-box; gap: 0.4rem; padding: 0.4rem;
  color: var(--_rs-foreground); font: var(--_rs-font); overflow: hidden; z-index: 10;
}
.rusty-developer-shell[data-side=right] { right: 0; }
.rusty-developer-shell[data-side=left] { left: 0; }
/* The shell itself is see-through: an inspection view pane shows the view the
   Engine draws under it. The bar and the panels carry their own backgrounds. */
.rusty-developer-shell__bar {
  display: flex; flex-wrap: wrap; gap: 0.25rem; align-items: center; padding: 0.25rem;
  background: var(--_rs-background); border: 1px solid var(--_rs-border); border-radius: 0.3rem;
}
.rusty-developer-shell__bar button {
  font: inherit; color: inherit; background: transparent; cursor: pointer;
  border: 1px solid var(--_rs-border); border-radius: 0.3rem; padding: 0.05rem 0.45rem;
}
.rusty-developer-shell__bar button[aria-selected=true] { border-color: var(--_rs-accent); color: var(--_rs-accent); }
.rusty-developer-shell__close { margin-left: auto; }
.rusty-developer-shell__panels { flex: 1; min-height: 0; display: flex; flex-direction: column; gap: 0.4rem; overflow: auto; }
.rusty-developer-shell__panel { min-height: 0; display: flex; flex-direction: column; }
.rusty-developer-shell__panel[hidden] { display: none; }
.rusty-developer-shell[data-presentation=tabs] .rusty-developer-shell__panel { flex: 1; }
.rusty-developer-shell__panel--view { height: 16rem; flex: none; }
.rusty-developer-shell[data-presentation=tabs] .rusty-developer-shell__panel--view { height: auto; }
`;

/** Something a panel mount gives back. */
interface Disposable {
  dispose(): void;
}

/** One panel the shell docks: an Engine panel, or the product's own DOM. */
export type DeveloperShellPanel =
  | { readonly kind: 'console'; readonly title?: string }
  | { readonly kind: 'tunables'; readonly title?: string; readonly options?: TunablesPanelOptions }
  | { readonly kind: 'scene'; readonly title?: string }
  | { readonly kind: 'view'; readonly title?: string; readonly options: InspectionViewOptions }
  | { readonly kind: 'metrics'; readonly title?: string }
  | {
      readonly kind: 'product';
      readonly title: string;
      /** Fills `element`; the shell calls `dispose` when it closes. */
      mount(element: HTMLElement): Disposable;
    };

/** Configuration for a developer shell. */
export interface DeveloperShellOptions {
  readonly panels: readonly DeveloperShellPanel[];
  /** `dock` stacks every panel; `tabs` shows one at a time. `dock` when omitted. */
  readonly presentation?: 'dock' | 'tabs';
  /** The side of the host it docks to; `right` when omitted. */
  readonly side?: 'left' | 'right';
  /** Passed to the Engine panels; their same-origin transport when omitted. */
  readonly transport?: LiveDebugTransport;
}

/** A mounted developer shell. The product binds its key to `toggle`. */
export interface DeveloperShellMount {
  readonly isOpen: boolean;
  open(): Promise<void>;
  close(): void;
  toggle(): Promise<void>;
  /** Closes the shell for good. */
  dispose(): void;
}

/** The Engine panels' mount functions; the browser entry passes its own. */
export interface DeveloperShellMounts {
  console(element: HTMLElement): Promise<Disposable>;
  tunables(element: HTMLElement, options: TunablesPanelOptions): Disposable;
  scene(element: HTMLElement, transport?: LiveDebugTransport): Disposable;
  view(element: HTMLElement, options: InspectionViewOptions): Disposable;
  metrics(element: HTMLElement, transport?: LiveDebugTransport): Disposable;
}

const TITLES: Readonly<Record<DeveloperShellPanel['kind'], string>> = {
  console: 'Console',
  tunables: 'Tunables',
  scene: 'Scene',
  view: 'View',
  metrics: 'Renderer',
  product: 'Product',
};

/**
 * Docks the Engine's developer panels (and the product's own) beside play
 * without pausing it. Closed, it holds no panel and sends nothing: opening
 * mounts each panel, closing disposes them all. It holds no product state.
 */
export class DeveloperShell implements DeveloperShellMount {
  readonly #host: HTMLElement;
  readonly #options: DeveloperShellOptions;
  readonly #mounts: DeveloperShellMounts;
  #element: HTMLElement | null = null;
  #mounted: Disposable[] = [];
  #selected = 0;
  #disposed = false;

  constructor(host: HTMLElement, options: DeveloperShellOptions, mounts: DeveloperShellMounts) {
    this.#host = host;
    this.#options = options;
    this.#mounts = mounts;
  }

  get isOpen(): boolean {
    return this.#element !== null;
  }

  async open(): Promise<void> {
    if (this.#disposed || this.#element) return;
    const document = this.#host.ownerDocument;
    installStyle(document);
    const shell = document.createElement('aside');
    shell.className = 'rusty-developer-shell';
    shell.dataset['side'] = this.#options.side ?? 'right';
    shell.dataset['presentation'] = this.#options.presentation ?? 'dock';
    shell.setAttribute('data-rusty-ui-interactive', '');
    shell.setAttribute('aria-label', 'Developer shell');
    const bar = document.createElement('div');
    bar.className = 'rusty-developer-shell__bar';
    const panels = document.createElement('div');
    panels.className = 'rusty-developer-shell__panels';
    shell.append(bar, panels);
    this.#element = shell;
    this.#host.append(shell);
    const tabs = this.#options.presentation === 'tabs';
    const slots: HTMLElement[] = [];
    const buttons: HTMLButtonElement[] = [];
    const show = (index: number): void => {
      this.#selected = index;
      slots.forEach((slot, at) => (slot.hidden = tabs && at !== index));
      buttons.forEach((button, at) => button.setAttribute('aria-selected', String(at === index)));
    };
    this.#options.panels.forEach((panel, index) => {
      const slot = document.createElement('section');
      slot.className = `rusty-developer-shell__panel rusty-developer-shell__panel--${panel.kind}`;
      slot.setAttribute('aria-label', panel.title ?? TITLES[panel.kind]);
      panels.append(slot);
      slots.push(slot);
      if (tabs) {
        const button = document.createElement('button');
        button.type = 'button';
        button.textContent = panel.title ?? TITLES[panel.kind];
        button.addEventListener('click', () => show(index));
        bar.append(button);
        buttons.push(button);
      }
    });
    const close = document.createElement('button');
    close.type = 'button';
    close.className = 'rusty-developer-shell__close';
    close.textContent = 'Close';
    close.addEventListener('click', () => this.close());
    bar.append(close);
    show(Math.min(this.#selected, Math.max(0, slots.length - 1)));
    for (const [index, panel] of this.#options.panels.entries()) {
      const mounted = await this.#mount(panel, slots[index]!);
      // Closed while a panel loaded: release it at once.
      if (this.#element !== shell) {
        mounted.dispose();
        return;
      }
      this.#mounted.push(mounted);
    }
  }

  close(): void {
    const element = this.#element;
    this.#element = null;
    for (const mounted of this.#mounted.splice(0)) mounted.dispose();
    element?.remove();
  }

  async toggle(): Promise<void> {
    if (this.isOpen) this.close();
    else await this.open();
  }

  dispose(): void {
    this.#disposed = true;
    this.close();
  }

  async #mount(panel: DeveloperShellPanel, element: HTMLElement): Promise<Disposable> {
    const transport = this.#options.transport;
    switch (panel.kind) {
      case 'console':
        return this.#mounts.console(element);
      case 'tunables':
        return this.#mounts.tunables(element, { ...(transport ? { transport } : {}), ...panel.options });
      case 'scene':
        return this.#mounts.scene(element, transport);
      case 'view':
        return this.#mounts.view(element, { ...(transport ? { transport } : {}), ...panel.options });
      case 'metrics':
        return this.#mounts.metrics(element, transport);
      case 'product':
        return panel.mount(element);
    }
  }
}

function installStyle(document: Document): void {
  if (document.getElementById(STYLE_ID)) return;
  const style = document.createElement('style');
  style.id = STYLE_ID;
  style.textContent = STYLE;
  document.head.append(style);
}
