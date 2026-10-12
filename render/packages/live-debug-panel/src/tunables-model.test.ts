import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  atStep,
  boundFiles,
  controlValueOf,
  fillTemplate,
  isUnsaved,
  LiveValues,
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

test('a fractional step above one keeps its fraction when shown and saved', () => {
  const quarter: TunableControl = { ...slider, min: 0, max: 10, step: 2.5, value: 2.5 };
  assert.equal(stepDecimals(2.5), 1);
  assert.equal(atStep(quarter, 2.5), 2.5);
  assert.equal(isUnsaved(quarter, 2.5, 0), true);
  assert.deepEqual(saveEdits([quarter], new Map([['marker.size', 2.5]]), { sha256: 'x', document: { marker: { size: 0 } } }), [
    { op: 'replace', path: '/marker/size', value: 2.5 },
  ]);
  assert.equal(isUnsaved(quarter, 2.5, 2.5), false);
});

/** A set the test answers when it chooses: kept value or refusal. */
function deferredSends() {
  const calls: { id: string; value: number; answer: (kept: number) => void; refuse: (error: Error) => void }[] = [];
  const send = (id: string, value: number) =>
    new Promise<number>((answer, refuse) => calls.push({ id, value, answer, refuse }));
  return { calls, send };
}

test('saving waits for pending sets and keeps only the value the product accepted', async () => {
  const { calls, send } = deferredSends();
  const live = new LiveValues(send);
  live.reset([['marker.size', 0.8]]);
  const set = live.set('marker.size', 9);
  assert.equal(live.shown('marker.size'), 9, 'the request shows while it is in flight');
  let settled = false;
  const saving = live.settled(['marker.size']).then(() => (settled = true));
  await Promise.resolve();
  assert.equal(settled, false, 'a save waits for the set to answer');
  calls[0]!.answer(2);
  await set;
  await saving;
  assert.equal(live.accepted.get('marker.size'), 2);
  assert.equal(live.shown('marker.size'), 2, 'the control shows the clamped value');
  const file = { sha256: 'x', document: { marker: { size: 0.8 } } };
  assert.deepEqual(saveEdits([slider], live.accepted, file), [{ op: 'replace', path: '/marker/size', value: 2 }]);
});

test('a newer request replaces a waiting one and a refusal keeps the accepted value', async () => {
  const { calls, send } = deferredSends();
  const live = new LiveValues(send);
  live.reset([['marker.size', 0.8]]);
  const first = live.set('marker.size', 1);
  void live.set('marker.size', 1.2);
  void live.set('marker.size', 1.5);
  calls[0]!.answer(1);
  await new Promise((resolve) => setTimeout(resolve, 0));
  assert.deepEqual(calls.map((call) => call.value), [1, 1.5], 'only the latest waiting value is sent');
  calls[1]!.answer(1.5);
  await first;
  assert.equal(live.accepted.get('marker.size'), 1.5);
  const refused = live.set('marker.size', 1.9);
  calls[2]!.refuse(new Error('CSHARP_DEBUG: refused'));
  await assert.rejects(refused, /refused/);
  assert.equal(live.accepted.get('marker.size'), 1.5);
  assert.equal(live.shown('marker.size'), 1.5, 'a refused request no longer shows');
});
