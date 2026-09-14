import assert from 'node:assert/strict';
import test from 'node:test';

import {
  mountRustyApplicationWithEnvironment,
  type RustyApplicationUiContext,
} from './application-host.js';

void test('replacement canvas allocation failure retains the mounted surface and later disposes it', async () => {
  const previousAudioContext = globalThis.AudioContext;
  Object.defineProperty(globalThis, 'AudioContext', {
    configurable: true,
    value: FakeAudioContext,
  });
  try {
  const document = new FakeDocument();
  const root = document.createElement('div') as unknown as HTMLElement;
  let surfaceDisposals = 0;
  const host = await mountRustyApplicationWithEnvironment({
    root,
    mountUi: async () => undefined,
  }, {
    mountSurface: (canvas) => fakeSurface(canvas, () => { surfaceDisposals += 1; }) as never,
  });

  document.failNextCanvasCreation = true;
  const receipt = await host.renderer.replaceFrame({ schemaVersion: 1, ops: [] });
  assert.equal(receipt.applied, false);
  assert.equal(host.readout().state, 'ready');
  assert.equal(host.readout().contentRevision, 1);
  assert.equal(surfaceDisposals, 0, 'the active surface remains owned after candidate allocation fails');

  await host.dispose();
  assert.equal(surfaceDisposals, 1, 'normal disposal still releases the retained active surface');
  } finally {
    Object.defineProperty(globalThis, 'AudioContext', {
      configurable: true,
      value: previousAudioContext,
    });
  }
});

void test('a fail-atomic frame rejection keeps the application renderer usable for a later valid frame', async () => {
  const previousAudioContext = globalThis.AudioContext;
  Object.defineProperty(globalThis, 'AudioContext', { configurable: true, value: FakeAudioContext });
  try {
    const document = new FakeDocument();
    const receipts = [
      { applied: false, outcome: 'rejected_atomic' as const, diagnostics: [{ code: 'renderer_frame_rejected', message: 'stale handle' }] },
      { applied: true, outcome: 'applied' as const, diagnostics: [] },
    ];
    const host = await mountRustyApplicationWithEnvironment({
      root: document.createElement('div') as unknown as HTMLElement,
      mountUi: async () => undefined,
    }, {
      mountSurface: (canvas) => fakeSurface(canvas, () => undefined, () => receipts.shift()!) as never,
    });

    assert.equal(host.renderer.applyFrame({ schemaVersion: 1, ops: [] }).outcome, 'rejected_atomic');
    assert.equal(host.renderer.applyFrame({ schemaVersion: 1, ops: [] }).outcome, 'applied');
    assert.equal(host.readout().state, 'ready');
    await host.dispose();
  } finally {
    Object.defineProperty(globalThis, 'AudioContext', { configurable: true, value: previousAudioContext });
  }
});

void test('a terminal frame outcome closes the current renderer port instead of retrying it', async () => {
  const previousAudioContext = globalThis.AudioContext;
  Object.defineProperty(globalThis, 'AudioContext', { configurable: true, value: FakeAudioContext });
  try {
    const document = new FakeDocument();
    let applications = 0;
    const host = await mountRustyApplicationWithEnvironment({
      root: document.createElement('div') as unknown as HTMLElement,
      mountUi: async () => undefined,
    }, {
      mountSurface: (canvas) => fakeSurface(canvas, () => undefined, () => {
        applications += 1;
        return { applied: false, outcome: 'terminal' as const, diagnostics: [{ code: 'renderer_terminal', message: 'backend owner changed' }] };
      }) as never,
    });

    assert.equal(host.renderer.applyFrame({ schemaVersion: 1, ops: [] }).outcome, 'terminal');
    assert.equal(host.renderer.applyFrame({ schemaVersion: 1, ops: [] }).outcome, 'terminal');
    assert.equal(applications, 1, 'later frame output must not touch a terminal backend owner');
    await host.dispose();
  } finally {
    Object.defineProperty(globalThis, 'AudioContext', { configurable: true, value: previousAudioContext });
  }
});

void test('mounted UI exclusively observes interface controller input and ownership transitions do not replay presses', async () => {
  const previousAudioContext = globalThis.AudioContext;
  Object.defineProperty(globalThis, 'AudioContext', { configurable: true, value: FakeAudioContext });
  try {
    const document = new FakeDocument();
    const buttons = Array.from({ length: 16 }, () => ({ value: 0, pressed: false }));
    const axes = [0, 0, 0, 0];
    document.gamepads = [{ connected: true, axes, buttons } as unknown as Gamepad];
    let uiContext: RustyApplicationUiContext | undefined;
    const observed: unknown[] = [];
    let unsubscribe = () => undefined as void;
    const binding = { runtime: { instanceId: '1', generation: '1', controlRevision: '1' }, context: 'gameplay' };
    const host = await mountRustyApplicationWithEnvironment({
      root: document.createElement('div') as unknown as HTMLElement,
      runtimeInput: { binding, selectedController: { index: 0 } },
      mountUi: (_root, context) => {
        uiContext = context;
        unsubscribe = context.input!.subscribe((observation) => {
          observed.push(observation);
          if (observation.fact.kind === 'controller-button' && observation.fact.edge === 'pressed') {
            if (observation.fact.button === 'button-1') context.ui.setInteractionMode('gameplay');
            else context.intents!.claim('menu.select', { kind: 'digital', active: true });
          }
        });
      },
    }, {
      mountSurface: (canvas) => {
        document.activeElement = canvas;
        return fakeSurface(canvas, () => undefined) as never;
      },
    });
    // The opening button is already held when the UI takes ownership.
    uiContext!.ui.setInteractionMode('gameplay');
    host.input!.drain();
    buttons[9] = { value: 1, pressed: true };
    host.input!.sampleController();
    host.input!.drain();
    uiContext!.ui.setInteractionMode('interface');
    host.input!.drain();
    host.input!.sampleController();
    assert.deepEqual(observed, [], 'opening a menu does not replay its held opening button');
    assert.deepEqual(host.input!.drain(), []);
    axes[1] = 0.7;
    buttons[0] = { value: 1, pressed: true };
    host.input!.sampleController();
    assert.deepEqual(observed, [
      { context: 'interface', fact: { kind: 'controller-axis', axis: 'axis-1', value: 0.7 } },
      { context: 'interface', fact: { kind: 'controller-button-value', button: 'button-0', value: 1 } },
      { context: 'interface', fact: { kind: 'controller-button', button: 'button-0', edge: 'pressed' } },
    ]);
    const claims = host.input!.drain();
    assert.equal(claims.length, 1);
    assert.ok(claims.every((entry) => 'intent' in entry && entry.intent === 'menu.select'),
      'UI commands use the existing claim lane; interface physical facts never reach gameplay');
    host.input!.rebaselineRuntime({ ...binding, runtime: { ...binding.runtime, controlRevision: '2' } });
    assert.deepEqual(host.input!.drain(), [], 'recovery in interface mode cannot replay held controls into gameplay');
    buttons[1] = { value: 1, pressed: true };
    buttons[2] = { value: 1, pressed: true };
    host.input!.sampleController();
    assert.equal(uiContext!.ui.interactionMode(), 'gameplay');
    assert.ok(host.input!.drain().every((entry) => 'fact' in entry && entry.fact.kind === 'clear'));
    host.input!.sampleController();
    assert.deepEqual(host.input!.drain(), [], 'closing sample does not leak held presses to gameplay');
    const before = observed.length;
    uiContext!.ui.setInteractionMode('interface');
    unsubscribe();
    buttons[0] = { value: 0, pressed: false };
    host.input!.sampleController();
    assert.equal(observed.length, before, 'unsubscribe removes observation');
    await host.dispose();
  } finally {
    Object.defineProperty(globalThis, 'AudioContext', { configurable: true, value: previousAudioContext });
  }
});

class FakeAudioContext {
  readonly currentTime = 0;
  readonly destination = { connect: () => undefined, disconnect: () => undefined };
  readonly listener = {};
  readonly state = 'running';
  createGain(): { readonly gain: { setValueAtTime: () => void }; connect: () => void; disconnect: () => void } {
    return {
      gain: { setValueAtTime: () => undefined },
      connect: () => undefined,
      disconnect: () => undefined,
    };
  }
  close(): Promise<void> { return Promise.resolve(); }
  resume(): Promise<void> { return Promise.resolve(); }
}

class FakeDocument {
  gamepads: (Gamepad | null)[] = [];
  activeElement: Element | null = null;
  readonly defaultView = {
    navigator: { getGamepads: () => this.gamepads },
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
  };
  pointerLockElement: Element | null = null;
  failNextCanvasCreation = false;

  createElement(name: string): FakeElement {
    if (name === 'canvas' && this.failNextCanvasCreation) {
      this.failNextCanvasCreation = false;
      throw new Error('injected canvas allocation failure');
    }
    return new FakeElement(this, name);
  }

  addEventListener(): void {}
  removeEventListener(): void {}
}

class FakeElement {
  readonly childNodes: FakeElement[] = [];
  readonly dataset: Record<string, string> = {};
  readonly style: { cssText: string; [key: string]: string } = { cssText: '' };
  readonly ownerDocument: FakeDocument;
  parent: FakeElement | null = null;
  textContent: string | null = null;
  tabIndex = -1;

  constructor(document: FakeDocument, readonly tagName: string) {
    this.ownerDocument = document;
  }

  append(...children: FakeElement[]): void {
    for (const child of children) {
      child.parent?.removeChild(child);
      child.parent = this;
      this.childNodes.push(child);
    }
  }

  remove(): void {
    this.parent?.removeChild(this);
  }

  replaceWith(candidate: FakeElement): void {
    const parent = this.parent;
    if (parent === null) throw new Error('canvas has no parent');
    const index = parent.childNodes.indexOf(this);
    if (index < 0) throw new Error('canvas parent is inconsistent');
    candidate.parent?.removeChild(candidate);
    parent.childNodes[index] = candidate;
    candidate.parent = parent;
    this.parent = null;
  }

  setAttribute(): void {}
  addEventListener(): void {}
  removeEventListener(): void {}
  focus(): void {}
  querySelector(): null { return null; }

  private removeChild(child: FakeElement): void {
    const index = this.childNodes.indexOf(child);
    if (index >= 0) this.childNodes.splice(index, 1);
    child.parent = null;
  }
}

function fakeSurface(
  canvas: HTMLCanvasElement,
  dispose: () => void,
  applyFrame: () => unknown = () => ({ applied: true, outcome: 'applied', diagnostics: [] }),
): unknown {
  return {
    canvas,
    animationProjection: { subscribeNaturalCompletions: () => () => undefined },
    projectWorldPoint: () => ({ x: 0, y: 0, visible: false }),
    createParticleSink: () => ({ dispose: () => undefined }),
    createGhostPlatePresentation: () => ({ dispose: () => undefined }),
    applyFrame,
    setPresentationHosts: () => undefined,
    viewCompositionReadout: () => ({
      schemaVersion: 1,
      cameras: [], targets: [], views: [], presentations: [],
    }),
    cameraPose: () => ({ position: [0, 0, 0], pitchDegrees: 0, yawDegrees: 0 }),
    configureViews: () => ({ applied: true, diagnostics: [] }),
    setCameraPose: () => undefined,
    renderOnce: () => undefined,
    pointerLocked: () => false,
    releaseInput: () => undefined,
    dispose,
  };
}
