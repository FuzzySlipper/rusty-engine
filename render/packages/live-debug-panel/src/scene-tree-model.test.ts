import assert from 'node:assert/strict';
import test from 'node:test';

import { childrenCommand, objectFacts, objectLabel, parseObjectPage, rootsCommand } from './scene-tree-model.js';

const page = JSON.stringify({
  revision: 4,
  page: 0,
  pages: 1,
  total: 2,
  objects: [
    {
      depth: 0,
      handle: 7,
      kind: 'animatedMesh',
      layer: 'viewmodel',
      entity: 12,
      resource: 'animated-mesh/arms',
      world: { translation: [1, 2, 3], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      children: 1,
      changedRevision: 3,
    },
    {
      depth: 1,
      handle: 8,
      parent: 7,
      parentJoint: 'RightHand',
      kind: 'staticMesh',
      layer: 'viewmodel',
      label: 'coach gun',
      world: { translation: [0.00001, 0, 0], rotation: [0, 0, 0, 1], scale: [1, 1, 1] },
      bounds: { localMin: [0, 0, 0], localMax: [1, 1, 1], worldMin: [0, 0, 0], worldMax: [1, 1, 1] },
      children: 0,
      changedRevision: 4,
    },
  ],
});

test('a page parses and its rows name what each object is', () => {
  const parsed = parseObjectPage(page);
  assert.equal(parsed.total, 2);
  assert.equal(objectLabel(parsed.objects[0]!), '7 animatedMesh · entity 12');
  assert.equal(objectLabel(parsed.objects[1]!), '8 staticMesh · coach gun @RightHand');
  const facts = new Map(objectFacts(parsed.objects[1]!));
  assert.equal(facts.get('joint'), 'RightHand');
  assert.equal(facts.get('world'), '0.000, 0.000, 0.000');
  assert.equal(facts.get('bounds max'), '1.000, 1.000, 1.000');
  assert.equal(facts.get('changed'), 'revision 4');
  assert.throws(() => parseObjectPage('{}'));
});

test('tree commands list roots and one level of children, paged', () => {
  assert.equal(rootsCommand(), 'engine.scene.objects * 0');
  assert.equal(childrenCommand(7), 'engine.scene.objects 7 1');
  assert.equal(childrenCommand(7, 2), 'engine.scene.objects 7 1 --page 2');
});
