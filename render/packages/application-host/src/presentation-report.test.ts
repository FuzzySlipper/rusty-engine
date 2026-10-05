import assert from 'node:assert/strict';
import test from 'node:test';

import { anchorRect, createRustyApplicationPresentationReporter } from './presentation-report.js';

test('an anchored element is normalized to the surface, bottom-left based', () => {
  const surface = { left: 10, top: 20, width: 800, height: 400 };
  // The right half's upper quarter: 100 px down from the surface's top.
  const element = { left: 410, top: 20, width: 400, height: 100 };
  assert.deepEqual(anchorRect(surface, element), { x: 0.5, y: 0.75, width: 0.5, height: 0.25 });
});

test('a report reads its empty response, so the request finishes rather than aborts', async () => {
  let read = false;
  const original = globalThis.fetch;
  globalThis.fetch = (async () => ({
    ok: true,
    status: 204,
    arrayBuffer: async () => {
      read = true;
      return new ArrayBuffer(0);
    },
  })) as unknown as typeof fetch;
  try {
    const surface = {
      getBoundingClientRect: () => ({ left: 0, top: 0, width: 640, height: 360 }),
      ownerDocument: { defaultView: { devicePixelRatio: 1 } },
    } as unknown as HTMLElement;
    const reporter = createRustyApplicationPresentationReporter(surface, () => 1);
    reporter.tick();
    await new Promise((resolve) => setTimeout(resolve, 0));
    assert.equal(read, true);
    reporter.dispose();
  } finally {
    globalThis.fetch = original;
  }
});
