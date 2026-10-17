import assert from 'node:assert/strict';
import test from 'node:test';

import { DeveloperShell, type DeveloperShellMounts } from './developer-shell.js';

/** A minimal DOM: enough elements for the shell's structure. */
class FakeElement {
  children: FakeElement[] = [];
  dataset: Record<string, string> = {};
  attributes = new Map<string, string>();
  hidden = false;
  className = '';
  textContent = '';
  type = '';
  id = '';
  parent: FakeElement | null = null;
  listeners = new Map<string, () => void>();
  constructor(readonly tagName: string, readonly ownerDocument: FakeDocument) {}
  append(...nodes: FakeElement[]): void {
    for (const node of nodes) {
      node.parent = this;
      this.children.push(node);
    }
  }
  remove(): void {
    if (this.parent) this.parent.children = this.parent.children.filter((child) => child !== this);
    this.parent = null;
  }
  setAttribute(name: string, value: string): void {
    this.attributes.set(name, value);
  }
  addEventListener(name: string, listener: () => void): void {
    this.listeners.set(name, listener);
  }
}

class FakeDocument {
  head: FakeElement;
  constructor() {
    this.head = new FakeElement('head', this);
  }
  createElement(tag: string): FakeElement {
    return new FakeElement(tag, this);
  }
  getElementById(): null {
    return null;
  }
}

function mounts(log: string[]): DeveloperShellMounts {
  const panel = (name: string) => () => {
    log.push(`mount ${name}`);
    return { dispose: () => log.push(`dispose ${name}`) };
  };
  return {
    console: async () => panel('console')(),
    tunables: panel('tunables'),
    scene: panel('scene'),
    view: panel('view'),
    metrics: panel('metrics'),
  };
}

test('a closed shell mounts nothing; opening mounts each panel and closing disposes them', async () => {
  const document = new FakeDocument();
  const host = document.createElement('div');
  const log: string[] = [];
  const shell = new DeveloperShell(
    host as unknown as HTMLElement,
    {
      panels: [
        { kind: 'console' },
        { kind: 'tunables' },
        { kind: 'product', title: 'Mine', mount: () => (log.push('mount mine'), { dispose: () => log.push('dispose mine') }) },
      ],
      presentation: 'tabs',
    },
    mounts(log),
  );
  assert.equal(shell.isOpen, false);
  assert.equal(host.children.length, 0);
  await shell.toggle();
  assert.equal(shell.isOpen, true);
  assert.deepEqual(log, ['mount console', 'mount tunables', 'mount mine']);
  const panels = host.children[0]!.children[1]!.children;
  assert.deepEqual(panels.map((panel) => panel.hidden), [false, true, true], 'tabs show one panel');
  await shell.toggle();
  assert.equal(shell.isOpen, false);
  assert.equal(host.children.length, 0);
  assert.deepEqual(log.slice(3), ['dispose console', 'dispose tunables', 'dispose mine']);
  shell.dispose();
  await shell.open();
  assert.equal(shell.isOpen, false, 'a disposed shell stays closed');
});

test("the shell's own Close button closes it and tells the product once", async () => {
  const document = new FakeDocument();
  const host = document.createElement('div');
  const log: string[] = [];
  let closed = 0;
  const shell = new DeveloperShell(
    host as unknown as HTMLElement,
    { panels: [{ kind: 'scene' }], onClose: () => closed++ },
    mounts(log),
  );
  shell.close();
  assert.equal(closed, 0, 'a closed shell closing again says nothing');
  await shell.open();
  const bar = host.children[0]!.children[0]!;
  const close = bar.children.find((child) => child.className === 'rusty-developer-shell__close')!;
  close.listeners.get('click')!();
  assert.equal(shell.isOpen, false);
  assert.equal(closed, 1);
  assert.deepEqual(log, ['mount scene', 'dispose scene']);
  await shell.open();
  shell.dispose();
  assert.equal(closed, 2, 'disposing an open shell closes it');
});
