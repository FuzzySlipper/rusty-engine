import assert from 'node:assert/strict';
import test from 'node:test';

import {
  captureBlockers,
  compareBaseline,
  deduplicateFindings,
  drainEngineCheckpoint,
  fingerprintFinding,
  normalizeMessage,
} from './capture-playtest-warning-delta.mjs';

const compatibility = Object.freeze({
  exerciseId: 'smoke-path',
  pageOrigin: 'http://127.0.0.1:4173',
  engineOrigin: 'http://127.0.0.1:9348',
});

function report(findings, capture = { browser: { status: 'complete' }, engine: { status: 'complete' } }) {
  return {
    schemaVersion: 1,
    captureProtocol: 'rusty-engine.playtest-warning-delta/v1',
    compatibility,
    run: { label: 'fixture' },
    capture,
    findings,
  };
}

test('normalizes dynamic text into stable deduplicated fingerprints with occurrence counts', () => {
  const findings = deduplicateFindings([
    { source: 'playwright', kind: 'console', severity: 'warning', message: 'retry 41 failed\nfor request 0x10' },
    { source: 'playwright', kind: 'console', severity: 'warning', message: 'retry 42 failed for request 0x20' },
  ]);
  assert.equal(normalizeMessage('  Retry\n41  '), 'Retry <n>');
  assert.equal(findings.length, 1);
  assert.equal(findings[0].occurrenceCount, 2);
  assert.equal(findings[0].fingerprint, fingerprintFinding(findings[0]));
});

test('compares only a named compatible baseline and reports new resolved and unchanged fingerprints', () => {
  const unchanged = deduplicateFindings([{ source: 'engine', kind: 'structured', severity: 'warning', code: 'OLD', disposition: 'degraded', message: 'old warning' }]);
  const added = deduplicateFindings([{ source: 'playwright', kind: 'pageerror', severity: 'error', message: 'new page error' }]);
  const resolved = deduplicateFindings([{ source: 'engine', kind: 'structured', severity: 'error', code: 'GONE', disposition: 'terminal', message: 'resolved error' }]);
  const comparison = compareBaseline(report([...unchanged, ...added]), report([...unchanged, ...resolved]));
  assert.equal(comparison.status, 'compatible');
  assert.deepEqual(comparison.counts, { new: 1, resolved: 1, unchanged: 1 });
  assert.equal(comparison.new[0].fingerprint, added[0].fingerprint);
  assert.equal(comparison.resolved[0].fingerprint, resolved[0].fingerprint);
});

test('rejects incompatible or incomplete baseline capture instead of calling it comparable', () => {
  const current = report([]);
  assert.deepEqual(compareBaseline(current, undefined), { status: 'unavailable', reason: 'no baseline was supplied' });
  assert.equal(compareBaseline(current, { ...report([]), compatibility: { ...compatibility, exerciseId: 'different' } }).status, 'incompatible');
  assert.equal(compareBaseline(current, report([], { browser: { status: 'complete' }, engine: { status: 'lagged' } })).status, 'incompatible');
});

test('drains a capped Engine checkpoint before the exercise cursor is set', async () => {
  const reads = [];
  const checkpoint = {
    events: Array.from({ length: 64 }, () => ({ severity: 'warning' })),
    floorSequence: '1', throughSequence: '130', nextCursor: '64', lagged: false,
    warningCount: '64', errorCount: '0', droppedCount: '0',
  };
  const batches = [
    { ...checkpoint, events: Array.from({ length: 64 }, () => ({ severity: 'warning' })), nextCursor: '128' },
    { ...checkpoint, events: [{ severity: 'warning' }, { severity: 'warning' }], nextCursor: '130' },
  ];
  const result = await drainEngineCheckpoint('http://engine.test', checkpoint, async (_origin, after) => {
    reads.push(after);
    return batches.shift();
  });
  assert.equal(result.status, 'complete');
  assert.equal(result.checkpointCursor, '130');
  assert.equal(result.reads, 3);
  assert.deepEqual(reads, ['64', '128']);
});

test('incomplete capture and unavailable comparison visibly block a clean claim', () => {
  const blockers = captureBlockers(
    { browser: { status: 'listener-failed' }, engine: { status: 'dropped' } },
    { status: 'unavailable' },
    [
      { source: 'engine', severity: 'error', disposition: 'terminal' },
      { source: 'engine', severity: 'warning', disposition: 'resync-required' },
    ],
  );
  assert.equal(blockers.length, 6);
  assert.match(blockers.join('\n'), /listener-failed/);
  assert.match(blockers.join('\n'), /dropped/);
  assert.match(blockers.join('\n'), /terminal/);
  assert.match(blockers.join('\n'), /Error finding/);
  assert.match(blockers.join('\n'), /resynchronization/);
});
