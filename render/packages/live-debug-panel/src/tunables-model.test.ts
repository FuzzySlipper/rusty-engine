import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  atStep,
  fillTemplate,
  LiveValues,
  parseApplied,
  parseDescription,
  parseDiff,
  refusals,
  stepDecimals,
  TunableSources,
  unsavedControls,
  type TunableControl,
  type TunablesDiff,
} from './tunables-model.js';

const slider: TunableControl = {
  id: 'marker.size', label: 'Size', kind: 'slider', min: 0, max: 2, step: 0.05, value: 0.8, reshapes: false,
  source: { path: 'tuning/marker.json', pointer: '/marker/size' },
};
const description = {
  contract: 'tunables.v1', title: 'T', choosers: [], timeline: null, notice: '', commands: { set: 'tunables.set {id} {value}' },
  groups: [{ id: 'g', title: 'G', controls: [slider] }],
};

test('values show at their step', () => {
  assert.equal(stepDecimals(0.05), 2);
  assert.equal(stepDecimals(1), 0);
  assert.equal(stepDecimals(1e-7), 7);
  assert.equal(stepDecimals(2.5), 1);
  assert.equal(atStep(slider, 0.25000000372529), 0.25);
  assert.equal(atStep({ ...slider, step: 2.5 }, 2.5), 2.5);
});

test('descriptions parse and fill their command templates', () => {
  const parsed = parseDescription(JSON.stringify(description));
  assert.equal(fillTemplate(parsed.commands['set'] ?? '', { id: 'marker.size', value: 1.5 }), 'tunables.set marker.size 1.5');
  assert.throws(() => parseDescription('{"contract":"other"}'));
});

test('a diff names each unsaved control with its file, and a file it cannot read', () => {
  const diff = parseDiff(JSON.stringify({
    page: { title: 'T', choosers: {}, key: null },
    files: [
      {
        path: 'tuning/marker.json', sha256: 'ab',
        unsaved: [{ id: 'marker.size', label: 'Size', pointer: '/marker/size', live: 1.25, saved: 0.8, save: 1.25 }],
      },
      { path: 'tuning/other.json', error: 'CONTENT_SOURCE_UNAVAILABLE: not in rusty dev' },
    ],
  }));
  assert.deepEqual(diff.files[1]?.unsaved, [], 'a file without a reading has nothing unsaved');
  const unsaved = unsavedControls(diff);
  assert.equal(unsaved.get('marker.size')?.path, 'tuning/marker.json');
  assert.equal(unsaved.get('marker.size')?.saved, 0.8);
});

test('an applied batch keeps its refusals and the description after it', () => {
  const applied = parseApplied(JSON.stringify({
    results: [{ id: 'marker.size', value: 2 }, { id: 'nope', refused: "No control 'nope' on this page." }],
    refused: 1,
    description,
  }));
  assert.equal(applied.description.groups[0]?.controls[0]?.id, 'marker.size');
  assert.equal(refusals(applied), "nope: No control 'nope' on this page.");
  assert.equal(refusals({ ...applied, results: [{ id: 'marker.size', value: 2 }] }), '');
});

const file = 'tuning/marker.json';
const compared = (sha256: string, ...ids: string[]): TunablesDiff => ({
  files: [{ path: file, sha256, unsaved: ids.map((id) => ({ id, label: id, pointer: `/${id}`, live: 1, saved: 0, save: 1 })) }],
});

test('comparisons after a set keep the SHA-256 Save names, so a change made elsewhere is refused', () => {
  const sources = new TunableSources();
  sources.adopt(compared('described'));
  // a is tuned here while b changes in the file elsewhere: the next comparison shows both.
  sources.compared(compared('edited elsewhere', 'a', 'b'));
  assert.equal(sources.baseline(file), 'described', 'Save still names the described file');
  assert.deepEqual(sources.state(file), { text: '2 unsaved, changed on disk', canSave: true, canReset: true });
  sources.saved(file, 'saved');
  sources.compared(compared('saved'));
  assert.deepEqual(sources.state(file), { text: 'saved', canSave: false, canReset: false });
  sources.adopt(compared('described again', 'b'));
  assert.equal(sources.baseline(file), 'described again', 'describing again takes the file as it is');
});

test('a comparison that fails leaves the files unknown, not saved', () => {
  const sources = new TunableSources();
  sources.adopt(compared('described', 'a'));
  sources.failed('connection lost');
  assert.equal(sources.error, 'connection lost');
  assert.deepEqual(sources.state(file), { text: 'unavailable', canSave: false, canReset: false });
  sources.compared(compared('described', 'a'));
  assert.deepEqual(sources.state(file), { text: '1 unsaved', canSave: true, canReset: true });
  sources.compared({ files: [{ path: file, error: 'CONTENT_SOURCE_UNAVAILABLE: not in rusty dev', unsaved: [] }] });
  assert.deepEqual(sources.state(file), { text: 'CONTENT_SOURCE_UNAVAILABLE: not in rusty dev', canSave: false, canReset: false });
});

/** A set the test answers when it chooses: kept value or refusal. */
function deferredSends() {
  const calls: { id: string; value: number; answer: (kept: number) => void; refuse: (error: Error) => void }[] = [];
  const send = (id: string, value: number) =>
    new Promise<number>((answer, refuse) => calls.push({ id, value, answer, refuse }));
  return { calls, send };
}

test('waiting for pending sets keeps only the value the product accepted', async () => {
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
