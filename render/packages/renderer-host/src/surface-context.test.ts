import assert from 'node:assert/strict';
import test from 'node:test';
import {
  installRendererSurfaceContextEventListeners,
  type RendererSurfaceContextEvent,
} from './surface.js';

class FakeCanvas {
  readonly listeners = new Map<string, Set<(event: Event) => void>>();

  addEventListener(type: string, listener: (event: Event) => void): void {
    const listeners = this.listeners.get(type) ?? new Set<(event: Event) => void>();
    listeners.add(listener);
    this.listeners.set(type, listeners);
  }

  removeEventListener(type: string, listener: (event: Event) => void): void {
    this.listeners.get(type)?.delete(listener);
  }

  dispatch(type: string, event: Event): void {
    for (const listener of this.listeners.get(type) ?? []) listener(event);
  }
}

test('renderer surface context events prevent loss and report one typed recovery episode', () => {
  const canvas = new FakeCanvas();
  const events: RendererSurfaceContextEvent[] = [];
  const remove = installRendererSurfaceContextEventListeners(
    canvas as unknown as Pick<HTMLCanvasElement, 'addEventListener' | 'removeEventListener'>,
    (event) => events.push(event),
  );
  let prevented = 0;
  const lost = {
    preventDefault: () => { prevented += 1; },
  } as unknown as Event;
  canvas.dispatch('webglcontextlost', lost);
  canvas.dispatch('webglcontextrestored', {} as Event);
  canvas.dispatch('webglcontextrestored', {} as Event);
  canvas.dispatch('webglcontextlost', lost);
  assert.equal(prevented, 1);
  assert.deepEqual(events, [
    {
      kind: 'lost',
      state: 'contextLost',
      diagnostic: 'WebGL context lost; automatic rendering is paused until remount',
    },
    {
      kind: 'restored',
      state: 'contextLost',
      diagnostic: 'WebGL context was restored; a fresh surface baseline is required before rendering resumes',
    },
  ]);
  remove();
  canvas.dispatch('webglcontextrestored', {} as Event);
  assert.equal(events.length, 2);
});
