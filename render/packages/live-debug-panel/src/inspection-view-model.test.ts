import assert from 'node:assert/strict';
import test from 'node:test';

import {
  captureUrl,
  DragGesture,
  dragOrbit,
  dragPan,
  focusDistance,
  inspectCommands,
  subjects,
  ViewOverlayToggle,
  wheelZoom,
} from './inspection-view-model.js';

test('pane actions are inspect commands with compact numbers', () => {
  assert.equal(inspectCommands.create('a', 'inspect-a'), 'engine.inspect.view create a anchor inspect-a');
  assert.equal(inspectCommands.frame('a', 'underside', 'target grip'), 'engine.inspect.view frame a underside target grip');
  assert.equal(inspectCommands.orbit('a', ...dragOrbit(10, -5)), 'engine.inspect.view orbit a 4 -2');
  assert.equal(inspectCommands.pan('a', ...dragPan(100, 0, 2)), 'engine.inspect.view pan a -0.4 0');
  assert.ok(wheelZoom(1) > 1 && wheelZoom(-1) < 1);
  assert.equal(
    inspectCommands.compare('a', 'side', 'target grip', 'refs/pistol-side.png'),
    'engine.inspect.view compare a side target grip refs/pistol-side.png compare-a.png onion',
  );
  assert.equal(captureUrl('a b', 640, 360), '/__rusty/product/runtime/frames/capture?view=a%20b&overlays=1&width=640&height=360');
});

test('subjects list the camera, then the product targets with their framing', () => {
  assert.deepEqual(subjects(null), [{ value: 'camera', label: 'Player camera' }]);
  const listed = subjects('{"contract":"inspect.targets.v1","targets":[{"name":"grip","framing":"underside"},{"name":"muzzle"}]}');
  assert.deepEqual(listed.slice(1), [
    { value: 'target grip', label: 'grip', framing: 'underside' },
    { value: 'target muzzle', label: 'muzzle' },
  ]);
  assert.equal(subjects('not json').length, 1);
});

test('the focus distance comes from the view listing', () => {
  const listing = '{"views":[{"id":"a","pose":{"position":[3,4,0]},"focus":[0,0,0]}]}';
  assert.equal(focusDistance(listing, 'a'), 5);
  assert.equal(focusDistance(listing, 'b'), 1);
});

test('a gesture released within one send interval still sends its movement, and a trailing segment is not lost', () => {
  const quick = new DragGesture(0, 0, false);
  quick.move(20, 0);
  // Released before the timer: the release takes what moved.
  assert.deepEqual(quick.take(), [20, 0]);
  assert.equal(quick.take(), null);
  const long = new DragGesture(0, 0, true);
  long.move(20, 0);
  assert.deepEqual(long.take(), [20, 0], 'the timer sends the first 20 px');
  long.move(40, 5);
  assert.deepEqual(long.take(), [20, 5], 'the release sends the last segment');
  assert.ok(long.pan);
});

test('two panes toggle overlays in their own views, and releasing one leaves the other', async () => {
  // The Engine's per-view scope, as engine.inspect.overlay answers it.
  const views = new Set<string>();
  const engine = async (command: string): Promise<string> => {
    const words = command.split(' ');
    // engine.inspect.overlay show|hide sets in VIEW
    if (words[1] === 'show' && words[3] === 'in') views.add(words[4]!);
    if (words[1] === 'hide' && words[3] === 'in') views.delete(words[4]!);
    return JSON.stringify({ setsIn: [...views] });
  };
  const a = new ViewOverlayToggle('a', engine);
  const b = new ViewOverlayToggle('b', engine);
  assert.equal(await a.toggle(), true);
  assert.equal(await b.toggle(), true);
  assert.equal(await a.toggle(), false);
  assert.equal(b.pressed, true, 'turning a off leaves b');
  assert.deepEqual([...views], ['b']);
  assert.equal(await b.sync(), true);
  await b.release();
  assert.deepEqual([...views], [], 'closing b undoes only its own choice');
  await a.release();
  assert.equal(a.pressed, false);
});
