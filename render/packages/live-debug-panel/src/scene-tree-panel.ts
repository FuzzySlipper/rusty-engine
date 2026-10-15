import { createLiveDebugHttpTransport, type LiveDebugTransport } from '@rusty-engine/live-debug-client';

import {
  childrenCommand,
  objectCommand,
  objectFacts,
  objectLabel,
  parseObjectDetail,
  parseObjectPage,
  rootsCommand,
  type SceneObject,
  type SceneObjectDetail,
} from './scene-tree-model.js';

const STYLE_ID = 'rusty-scene-tree-style';

/**
 * The panel's look, overridable by the product: set the `--rusty-scene-tree-*`
 * custom properties on `.rusty-scene-tree` or an ancestor, or style its classes.
 */
const STYLE = `
.rusty-scene-tree {
  --_rs-background: var(--rusty-scene-tree-background, rgb(14 18 26 / 94%));
  --_rs-foreground: var(--rusty-scene-tree-foreground, #e9eef5);
  --_rs-muted: var(--rusty-scene-tree-muted, #9aa8b8);
  --_rs-accent: var(--rusty-scene-tree-accent, #7fb2ff);
  --_rs-border: var(--rusty-scene-tree-border, #3a4656);
  --_rs-failed: var(--rusty-scene-tree-failed, #f0b45a);
  --_rs-radius: var(--rusty-scene-tree-radius, 0.4rem);
  --_rs-font: var(--rusty-scene-tree-font, 0.85rem/1.4 system-ui, sans-serif);
  background: var(--_rs-background); color: var(--_rs-foreground); font: var(--_rs-font);
  border: 1px solid var(--_rs-border); border-radius: var(--_rs-radius);
  padding: 0.7rem 0.8rem; box-sizing: border-box; max-height: 100%; overflow: auto;
}
.rusty-scene-tree__header { display: flex; gap: 0.5rem; align-items: center; justify-content: space-between; }
.rusty-scene-tree__title { margin: 0; font-size: 1.05em; }
.rusty-scene-tree__revision { color: var(--_rs-muted); font-size: 0.85em; }
.rusty-scene-tree button {
  font: inherit; color: inherit; background: transparent; cursor: pointer;
  border: 1px solid var(--_rs-border); border-radius: var(--_rs-radius); padding: 0.1rem 0.45rem;
}
.rusty-scene-tree button:hover { border-color: var(--_rs-accent); }
.rusty-scene-tree__tree, .rusty-scene-tree__tree ul { list-style: none; margin: 0; padding: 0; }
.rusty-scene-tree__tree ul { padding-left: 1rem; }
.rusty-scene-tree__tree { margin: 0.5rem 0; }
.rusty-scene-tree__row { display: flex; gap: 0.25rem; align-items: center; }
.rusty-scene-tree__row button { border-color: transparent; text-align: left; }
.rusty-scene-tree__toggle { width: 1.6rem; padding: 0; }
.rusty-scene-tree__name { overflow-wrap: anywhere; }
.rusty-scene-tree__name[aria-selected=true] { border-color: var(--_rs-accent); color: var(--_rs-accent); }
.rusty-scene-tree__more { color: var(--_rs-muted); }
.rusty-scene-tree__detail { border-top: 1px solid var(--_rs-border); padding-top: 0.4rem; }
.rusty-scene-tree__facts { display: grid; grid-template-columns: max-content 1fr; gap: 0.1rem 0.6rem; margin: 0; }
.rusty-scene-tree__facts dt { color: var(--_rs-muted); }
.rusty-scene-tree__facts dd { margin: 0; overflow-wrap: anywhere; font-variant-numeric: tabular-nums; }
.rusty-scene-tree__description > summary { cursor: pointer; color: var(--_rs-muted); }
.rusty-scene-tree__description pre { white-space: pre-wrap; overflow-wrap: anywhere; font-size: 0.8em; margin: 0.3rem 0 0; }
.rusty-scene-tree__status { color: var(--_rs-muted); font-size: 0.85em; margin-top: 0.5rem; min-height: 1.2em; }
.rusty-scene-tree__status[data-failed=true] { color: var(--_rs-failed); }
`;

/** Configuration for one mounted scene tree panel. */
export interface SceneTreePanelOptions {
  /** Uses the same-origin live-debug HTTP transport when omitted. */
  readonly transport?: LiveDebugTransport;
}

/**
 * A tree over the committed scene (`engine.scene.objects`) and the selected
 * object's facts (`engine.scene.object`). It holds no product state: every
 * row is a reading, read again on Refresh.
 */
export class SceneTreePanel {
  readonly element: HTMLElement;
  readonly #document: Document;
  readonly #transport: LiveDebugTransport;
  readonly #abort = new AbortController();
  readonly #expanded = new Set<number>();
  #selected: number | null = null;
  readonly #tree: HTMLElement;
  readonly #detail: HTMLElement;
  readonly #revision: HTMLElement;
  readonly #status: HTMLElement;

  constructor(document: Document, options: SceneTreePanelOptions) {
    this.#document = document;
    this.#transport = options.transport ?? createLiveDebugHttpTransport();
    installStyle(document);
    this.element = document.createElement('section');
    this.element.className = 'rusty-scene-tree';
    this.element.setAttribute('data-rusty-ui-interactive', '');
    const header = this.#make('div', 'rusty-scene-tree__header');
    this.#revision = this.#make('span', 'rusty-scene-tree__revision');
    const refresh = this.#make('button', '', 'Refresh') as HTMLButtonElement;
    refresh.type = 'button';
    refresh.addEventListener('click', () => void this.refresh());
    header.append(this.#make('h2', 'rusty-scene-tree__title', 'Scene'), this.#revision, refresh);
    this.#tree = this.#make('ul', 'rusty-scene-tree__tree');
    this.#tree.setAttribute('role', 'tree');
    this.#detail = this.#make('div', 'rusty-scene-tree__detail');
    this.#status = this.#make('p', 'rusty-scene-tree__status');
    this.#status.setAttribute('role', 'status');
    this.element.append(header, this.#tree, this.#detail, this.#status);
  }

  /** Reads the roots, every expanded node's children and the selection again. */
  async refresh(): Promise<void> {
    try {
      const tree = this.#make('ul', 'rusty-scene-tree__tree');
      tree.setAttribute('role', 'tree');
      await this.#fill(tree, null, 0);
      this.#tree.replaceChildren(...tree.children);
      if (this.#selected !== null) await this.#select(this.#selected);
      this.#report('', false);
    } catch (error) {
      this.#report(error, true);
    }
  }

  dispose(): void {
    this.#abort.abort();
    this.element.remove();
  }

  /** Lists `parent`'s children (the roots for null) into `list`, one page. */
  async #fill(list: HTMLElement, parent: number | null, page: number): Promise<void> {
    const answer = parseObjectPage(
      await this.#send(parent === null ? rootsCommand(page) : childrenCommand(parent, page)),
    );
    this.#revision.textContent = `revision ${answer.revision}`;
    for (const object of answer.objects) {
      if (object.handle === parent) continue;
      list.append(await this.#row(object));
    }
    if (answer.page + 1 < answer.pages) {
      const more = this.#make('button', 'rusty-scene-tree__more', `More (${answer.total} in all)`) as HTMLButtonElement;
      more.type = 'button';
      const item = this.#make('li', '');
      item.append(more);
      more.addEventListener('click', () => {
        item.remove();
        void this.#fill(list, parent, answer.page + 1).catch((error: unknown) => this.#report(error, true));
      });
      list.append(item);
    }
  }

  async #row(object: SceneObject): Promise<HTMLElement> {
    const item = this.#make('li', '');
    item.setAttribute('role', 'treeitem');
    const row = this.#make('div', 'rusty-scene-tree__row');
    const toggle = this.#make('button', 'rusty-scene-tree__toggle', object.children > 0 ? '▸' : '') as HTMLButtonElement;
    toggle.type = 'button';
    toggle.disabled = object.children === 0;
    const name = this.#make('button', 'rusty-scene-tree__name', objectLabel(object)) as HTMLButtonElement;
    name.type = 'button';
    name.dataset['handle'] = String(object.handle);
    name.setAttribute('aria-selected', String(object.handle === this.#selected));
    name.addEventListener('click', () => void this.#select(object.handle).catch((error: unknown) => this.#report(error, true)));
    row.append(toggle, name);
    item.append(row);
    const children = this.#make('ul', '');
    children.setAttribute('role', 'group');
    item.append(children);
    const show = async (open: boolean): Promise<void> => {
      toggle.textContent = open ? '▾' : '▸';
      item.setAttribute('aria-expanded', String(open));
      children.replaceChildren();
      if (open) await this.#fill(children, object.handle, 0);
    };
    toggle.addEventListener('click', () => {
      const open = !this.#expanded.has(object.handle);
      if (open) this.#expanded.add(object.handle);
      else this.#expanded.delete(object.handle);
      void show(open).catch((error: unknown) => this.#report(error, true));
    });
    if (object.children > 0 && this.#expanded.has(object.handle)) await show(true);
    return item;
  }

  async #select(handle: number): Promise<void> {
    this.#selected = handle;
    for (const name of this.element.querySelectorAll<HTMLElement>('.rusty-scene-tree__name')) {
      name.setAttribute('aria-selected', String(name.dataset['handle'] === String(handle)));
    }
    let detail: SceneObjectDetail;
    try {
      detail = parseObjectDetail(await this.#send(objectCommand(handle)));
    } catch (error) {
      this.#selected = null;
      this.#detail.replaceChildren();
      throw error;
    }
    this.#revision.textContent = `revision ${detail.revision}`;
    const facts = this.#make('dl', 'rusty-scene-tree__facts');
    for (const [label, value] of objectFacts(detail)) {
      facts.append(this.#make('dt', '', label), this.#make('dd', '', value));
    }
    const description = this.#make('details', 'rusty-scene-tree__description');
    const committed = { description: detail.description, pose: detail.pose, materialOverride: detail.materialOverride, materialParameters: detail.materialParameters };
    description.append(this.#make('summary', '', 'Committed description'), this.#make('pre', '', JSON.stringify(committed, null, 2)));
    this.#detail.replaceChildren(facts, description);
  }

  async #send(command: string): Promise<string> {
    const result = await this.#transport.execute(command, this.#abort.signal);
    if (!result.succeeded) throw new Error(result.message);
    return result.message;
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
