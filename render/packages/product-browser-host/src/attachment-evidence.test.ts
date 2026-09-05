import assert from 'node:assert/strict';
import test from 'node:test';
import { createBrowserAttachmentEvidence } from './attachment-evidence.js';

const baseline = {
  runtime: { instanceId: '1', generation: '2', controlRevision: '3' },
  nextInputSequence: '7',
  publicationFrontiers: [{ stream: 'presentation-world', revision: 9 }],
};

function fixture(reload: boolean) {
  const values = new Map([['tab', 'prior-attachment']]);
  let next = 0;
  const evidence = createBrowserAttachmentEvidence({ key: 'tab', reload,
    storage: { getItem: (key) => values.get(key) ?? null, setItem: (key, value) => { values.set(key, value); } },
    newId: () => `attachment-${String(++next)}` });
  return { evidence, values };
}

void test('reload correlation requires renderer confirmation of the same staged epoch', () => {
  const { evidence, values } = fixture(true);
  evidence.begin(1);
  evidence.stage(1, baseline);
  assert.equal(evidence.read().replaces, 'prior-attachment');
  assert.equal(evidence.read().baseline, undefined);
  evidence.confirm(2);
  assert.equal(values.get('tab'), 'prior-attachment');
  evidence.confirm(1);
  assert.deepEqual(evidence.read().baseline, baseline);
  assert.equal(values.get('tab'), evidence.read().id);
});

void test('new navigation or duplicated tab does not claim another attachment recovery', () => {
  const { evidence } = fixture(false);
  evidence.begin(1);
  evidence.stage(1, baseline);
  evidence.confirm(1);
  assert.equal(evidence.read().replaces, undefined);
});

void test('superseded recovery cannot confirm or overwrite the last realized predecessor', () => {
  const { evidence } = fixture(false);
  evidence.begin(1); evidence.stage(1, baseline); evidence.confirm(1);
  const previous = evidence.read().id;
  evidence.begin(2); evidence.stage(2, baseline);
  evidence.begin(3); evidence.confirm(2);
  assert.equal(evidence.read().baseline, undefined);
  assert.equal(evidence.read().replaces, previous);
  evidence.stage(3, baseline); evidence.confirm(3);
  assert.deepEqual(evidence.read().baseline, baseline);
});
