import assert from 'node:assert/strict';
import test from 'node:test';

import { captureUrl, dragOrbit, dragPan, focusDistance, inspectCommands, subjects, wheelZoom } from './inspection-view-model.js';

test('pane actions are inspect commands with compact numbers', () => {
  assert.equal(inspectCommands.create('a', 'inspect-a'), 'engine.inspect.view create a anchor inspect-a');
  assert.equal(inspectCommands.frame('a', 'underside', 'target grip'), 'engine.inspect.view frame a underside target grip');
  assert.equal(inspectCommands.orbit('a', ...dragOrbit(10, -5)), 'engine.inspect.view orbit a 4 -2');
  assert.equal(inspectCommands.pan('a', ...dragPan(100, 0, 2)), 'engine.inspect.view pan a -0.4 0');
  assert.ok(wheelZoom(1) > 1 && wheelZoom(-1) < 1);
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
