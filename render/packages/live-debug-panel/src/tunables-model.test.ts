import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  atStep,
  boundFiles,
  controlValueOf,
  fillTemplate,
  isUnsaved,
  parseDescription,
  resolvePointer,
  saveEdits,
  sourceValueOf,
  stepDecimals,
  type TunableControl,
} from './tunables-model.js';

const slider: TunableControl = {
  id: 'marker.size', label: 'Size', kind: 'slider', min: 0, max: 2, step: 0.05, value: 0.8, reshapes: false,
  source: { path: 'tuning/marker.json', pointer: '/marker/size' },
};
const toggle: TunableControl = {
  id: 'marker.spin', label: 'Spin', kind: 'toggle', min: 0, max: 1, step: 1, value: 1, reshapes: false,
  source: { path: 'tuning/marker.json', pointer: '/marker/spin' },
};
const choice: TunableControl = {
  id: 'marker.color', label: 'Colour', kind: 'choice', min: 0, max: 2, step: 1, value: 0, reshapes: false,
  options: ['orange', 'teal', 'violet'], source: { path: 'tuning/marker.json', pointer: '/marker/color' },
};
const document = { marker: { size: 0.8, spin: true, color: 'orange', 'a/b': [3, 4] } };

test('pointers resolve object members, escaped keys and array indices', () => {
  assert.equal(resolvePointer(document, '/marker/size'), 0.8);
  assert.equal(resolvePointer(document, '/marker/a~1b/1'), 4);
  assert.equal(resolvePointer(document, ''), document);
  for (const pointer of ['/marker/none', '/marker/a~1b/01', '/marker/size/x', 'marker']) {
    assert.equal(resolvePointer(document, pointer), undefined, pointer);
  }
});

test('source values compare in the control kind, floats at half a step', () => {
  assert.equal(isUnsaved(slider, 0.81, 0.8), false);
  assert.equal(isUnsaved(slider, 0.85, 0.8), true);
  assert.equal(isUnsaved(toggle, 1, true), false);
  assert.equal(isUnsaved(toggle, 0, true), true);
  assert.equal(isUnsaved(choice, 1, 'orange'), true);
  assert.equal(isUnsaved(choice, 0, 'orange'), false);
  assert.equal(isUnsaved(choice, 0, 'unknown'), false, 'an unknown source value is not compared');
  assert.equal(controlValueOf(choice, 'violet'), 2);
});

test('saved values keep the source type and the step spelling', () => {
  assert.equal(stepDecimals(0.05), 2);
  assert.equal(stepDecimals(1), 0);
  assert.equal(stepDecimals(1e-7), 7);
  assert.equal(atStep(slider, 0.25000000372529), 0.25);
  assert.equal(sourceValueOf(toggle, 0, true), false);
  assert.equal(sourceValueOf(choice, 2, 'orange'), 'violet');
  assert.equal(sourceValueOf(slider, 1.2500001, 0.8), 1.25);
});

test('save edits replace only the unsaved controls of one file', () => {
  const values = new Map([['marker.size', 1.25], ['marker.spin', 1], ['marker.color', 1]]);
  assert.deepEqual(saveEdits([slider, toggle, choice], values, { sha256: 'x', document }), [
    { op: 'replace', path: '/marker/size', value: 1.25 },
    { op: 'replace', path: '/marker/color', value: 'teal' },
  ]);
});

test('descriptions parse and name their bound files once', () => {
  const description = parseDescription(JSON.stringify({
    contract: 'tunables.v1', title: 'T', choosers: [], timeline: null, notice: '', commands: { set: 'tunables.set {id} {value}' },
    groups: [{ id: 'g', title: 'G', controls: [slider, toggle, { ...slider, id: 'free', source: undefined }] }],
  }));
  assert.deepEqual(boundFiles(description), ['tuning/marker.json']);
  assert.equal(fillTemplate(description.commands['set'] ?? '', { id: 'marker.size', value: 1.5 }), 'tunables.set marker.size 1.5');
  assert.throws(() => parseDescription('{"contract":"other"}'));
});
