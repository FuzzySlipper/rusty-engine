import assert from 'node:assert/strict';
import test from 'node:test';

import {
  attributeBrowserFailureEvents,
  captureBlockers,
  compareBaseline,
  deduplicateFindings,
  drainEngineCheckpoint,
  engineFindings,
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

function diagnostic({
  sequence,
  severity,
  disposition,
  source = 'dev-host',
  code,
  message = code,
  fields = [],
}) {
  return { sequence, severity, disposition, source, code, message, fields };
}

function field(key, value) {
  return { key, value };
}

function baseline({ sequence = '2', attachmentId = 'fresh', replacesAttachmentId } = {}) {
  return diagnostic({
    sequence,
    severity: 'info',
    disposition: 'accepted',
    source: 'browser-host',
    code: 'BROWSER_HOST_STATUS',
    message: 'fresh browser baseline',
    fields: [
      field('attachment-id', attachmentId),
      ...(replacesAttachmentId === undefined ? [] : [field('replaces-attachment-id', replacesAttachmentId)]),
      field('baseline-established', 'true'),
      field('host-state', 'ready'),
      field('transport', 'open'),
      field('output', 'open'),
    ],
  });
}

function responseWarning({ sequence = '1', attachmentId = 'old', certainty = 'settled', requestPath = '/input' } = {}) {
  return diagnostic({
    sequence,
    severity: 'warning',
    disposition: 'resync-required',
    code: 'DEV_HOST_RESPONSE_WRITE_RESYNC',
    message: 'response delivery requires a fresh readout',
    fields: [
      field('attachment-id', attachmentId),
      field('request-path', requestPath),
      field('response-certainty', certainty),
    ],
  });
}

function findingsFor(events) {
  return deduplicateFindings(engineFindings(events));
}

function compatibleComparison() {
  return { status: 'compatible' };
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

test('resolves a settled response write only through a later exact fresh baseline', () => {
  const findings = findingsFor([
    responseWarning({ sequence: '10', attachmentId: 'attachment-a', certainty: 'settled' }),
    baseline({ sequence: '11', attachmentId: 'attachment-b', replacesAttachmentId: 'attachment-a' }),
  ]);
  const warning = findings.find((finding) => finding.code === 'DEV_HOST_RESPONSE_WRITE_RESYNC');
  assert.equal(warning?.recovery?.status, 'recovered');
  assert.equal(warning?.recovery?.scope, 'attachment');
  assert.equal(warning?.recovery?.method, 'fresh-baseline');
  assert.deepEqual(warning?.recovery?.evidence, {
    warningSequence: '10',
    recoverySequence: '11',
    recoveryCode: 'BROWSER_HOST_STATUS',
    attachmentId: 'attachment-a',
    recoveryAttachmentId: 'attachment-b',
    replacesAttachmentId: 'attachment-a',
    responseCertainty: 'settled',
    recoverySource: 'browser-host',
  });
  assert.equal(captureBlockers({ browser: { status: 'complete' }, engine: { status: 'complete' } }, compatibleComparison(), findings)
    .includes('Engine capture requires resynchronization'), false);
});

test('keeps a response write unresolved when the fresh baseline replaces another attachment', () => {
  const findings = findingsFor([
    responseWarning({ sequence: '10', attachmentId: 'attachment-a' }),
    baseline({ sequence: '11', attachmentId: 'attachment-c', replacesAttachmentId: 'attachment-b' }),
  ]);
  const warning = findings.find((finding) => finding.code === 'DEV_HOST_RESPONSE_WRITE_RESYNC');
  assert.equal(warning?.recovery?.status, 'unresolved');
  assert.equal(warning?.recovery?.reason, 'missing-matching-fresh-baseline');
  assert.match(captureBlockers({ browser: { status: 'complete' }, engine: { status: 'complete' } }, compatibleComparison(), findings).join('\n'), /resynchronization/);
});

test('does not resolve a queued-input response write even with an exact fresh baseline', () => {
  const findings = findingsFor([
    responseWarning({ sequence: '10', attachmentId: 'attachment-a', certainty: 'queued-input' }),
    baseline({ sequence: '11', attachmentId: 'attachment-b', replacesAttachmentId: 'attachment-a' }),
  ]);
  const warning = findings.find((finding) => finding.code === 'DEV_HOST_RESPONSE_WRITE_RESYNC');
  assert.equal(warning?.recovery?.status, 'unresolved');
  assert.equal(warning?.recovery?.reason, 'response-certainty-is-not-settled-or-observation');
  assert.match(captureBlockers({ browser: { status: 'complete' }, engine: { status: 'complete' } }, compatibleComparison(), findings).join('\n'), /resynchronization/);
});

test('does not treat the host missing-attachment sentinel as a correlation identity', () => {
  const findings = findingsFor([
    responseWarning({ sequence: '10', attachmentId: 'none' }),
    baseline({ sequence: '11', attachmentId: 'attachment-b', replacesAttachmentId: 'none' }),
  ]);
  const warning = findings.find((finding) => finding.code === 'DEV_HOST_RESPONSE_WRITE_RESYNC');
  assert.equal(warning?.recovery?.status, 'unresolved');
  assert.equal(warning?.recovery?.reason, 'missing-attachment-id');
});

test('keeps terminal diagnostics blocking after a delivery warning was recovered', () => {
  const findings = findingsFor([
    responseWarning({ sequence: '10', attachmentId: 'attachment-a' }),
    baseline({ sequence: '11', attachmentId: 'attachment-b', replacesAttachmentId: 'attachment-a' }),
    diagnostic({
      sequence: '12',
      severity: 'error',
      disposition: 'terminal',
      code: 'ENGINE_TERMINAL',
      message: 'runtime stopped',
    }),
  ]);
  const warning = findings.find((finding) => finding.code === 'DEV_HOST_RESPONSE_WRITE_RESYNC');
  assert.equal(warning?.recovery?.status, 'recovered');
  const blockers = captureBlockers({ browser: { status: 'complete' }, engine: { status: 'complete' } }, compatibleComparison(), findings);
  assert.match(blockers.join('\n'), /terminal/);
  assert.match(blockers.join('\n'), /Error finding/);
});

test('keeps missing, lagged, and dropped Engine capture blockers independent of recovery metadata', () => {
  const findings = findingsFor([
    responseWarning({ sequence: '10', attachmentId: 'attachment-a' }),
    baseline({ sequence: '11', attachmentId: 'attachment-b', replacesAttachmentId: 'attachment-a' }),
  ]);
  assert.equal(findings[0].recovery?.status, 'recovered');
  for (const status of ['incomplete', 'lagged', 'dropped', 'failed']) {
    const blockers = captureBlockers(
      { browser: { status: 'complete' }, engine: { status } },
      compatibleComparison(),
      findings,
    );
    assert.match(blockers.join('\n'), new RegExp(`Engine capture is ${status}`));
  }
});

test('resolves only the warning whose attachment the baseline replaces', () => {
  const findings = findingsFor([
    responseWarning({ sequence: '10', attachmentId: 'attachment-a', requestPath: '/input/a' }),
    responseWarning({ sequence: '11', attachmentId: 'attachment-b', requestPath: '/input/b', certainty: 'observation' }),
    baseline({ sequence: '12', attachmentId: 'attachment-c', replacesAttachmentId: 'attachment-a' }),
  ]);
  assert.equal(findings.length, 2);
  const first = findings.find((finding) => finding.attachmentId === 'attachment-a');
  const second = findings.find((finding) => finding.attachmentId === 'attachment-b');
  assert.equal(first?.recovery?.status, 'recovered');
  assert.equal(second?.recovery?.status, 'unresolved');
  assert.notEqual(first?.fingerprint, second?.fingerprint);
  assert.match(captureBlockers({ browser: { status: 'complete' }, engine: { status: 'complete' } }, compatibleComparison(), findings).join('\n'), /resynchronization/);
});

test('preserves a degraded browser status finding while recording same-attachment baseline recovery', () => {
  const findings = findingsFor([
    diagnostic({
      sequence: '20',
      severity: 'warning',
      disposition: 'degraded',
      source: 'browser-host',
      code: 'BROWSER_HOST_STATUS',
      message: 'browser host degraded',
      fields: [
        field('attachment-id', 'attachment-a'),
        field('host-state', 'degraded'),
        field('transport', 'open'),
        field('output', 'open'),
      ],
    }),
    baseline({ sequence: '21', attachmentId: 'attachment-a' }),
  ]);
  assert.equal(findings.length, 1);
  assert.equal(findings[0].code, 'BROWSER_HOST_STATUS');
  assert.equal(findings[0].recovery?.status, 'recovered');
  assert.equal(findings[0].recovery?.method, 'same-attachment-baseline');
  assert.equal(findings[0].recovery?.evidence.recoverySequence, '21');
});

test('does not merge findings from separate attachments or let one recovery clear the other', () => {
  const findings = findingsFor([
    responseWarning({ sequence: '10', attachmentId: 'attachment-a', requestPath: '/input' }),
    responseWarning({ sequence: '11', attachmentId: 'attachment-b', requestPath: '/input' }),
    baseline({ sequence: '12', attachmentId: 'attachment-c', replacesAttachmentId: 'attachment-a' }),
  ]);
  assert.equal(findings.length, 2);
  assert.equal(findings.find((finding) => finding.attachmentId === 'attachment-a')?.recovery?.status, 'recovered');
  assert.equal(findings.find((finding) => finding.attachmentId === 'attachment-b')?.recovery?.status, 'unresolved');
});

test('keeps repeated same-attachment warnings separate across a recovery boundary', () => {
  const findings = findingsFor([
    responseWarning({ sequence: '1', attachmentId: 'attachment-a', requestPath: '/input' }),
    baseline({ sequence: '2', attachmentId: 'attachment-b', replacesAttachmentId: 'attachment-a' }),
    responseWarning({ sequence: '3', attachmentId: 'attachment-a', requestPath: '/input' }),
  ]);
  assert.equal(findings.length, 2);
  assert.equal(findings.find((finding) => finding.sequence === '1')?.recovery?.status, 'recovered');
  assert.equal(findings.find((finding) => finding.sequence === '3')?.recovery?.status, 'unresolved');
  assert.match(captureBlockers({ browser: { status: 'complete' }, engine: { status: 'complete' } }, compatibleComparison(), findings).join('\n'), /resynchronization/);
});

test('attributes a console resource error to a matching response without duplicating it', () => {
  const events = attributeBrowserFailureEvents([
    {
      source: 'playwright',
      kind: 'console',
      severity: 'error',
      message: 'Failed to load resource: the server responded with a status of 404 (Not Found)',
      url: 'http://page.test/favicon.ico',
      status: 404,
    },
    {
      source: 'playwright',
      kind: 'resource-response',
      severity: 'error',
      code: 'PLAYWRIGHT_RESOURCE_RESPONSE',
      message: 'resource response returned HTTP 404',
      url: 'http://page.test/favicon.ico',
      status: 404,
    },
  ]);
  assert.equal(events.length, 1);
  assert.deepEqual(events[0].attributedResponse, { url: 'http://page.test/favicon.ico', status: 404 });
  assert.equal(events[0].url, 'http://page.test/favicon.ico');
  assert.equal(events[0].status, 404);
});

test('retains a URL-bearing failed response when the console error has no URL', () => {
  const events = attributeBrowserFailureEvents([
    {
      source: 'playwright',
      kind: 'console',
      severity: 'error',
      message: 'Failed to load resource: the server responded with a status of 404 (Not Found)',
    },
    {
      source: 'playwright',
      kind: 'resource-response',
      severity: 'error',
      code: 'PLAYWRIGHT_RESOURCE_RESPONSE',
      message: 'resource response returned HTTP 404',
      url: 'http://page.test/favicon.ico',
      status: 404,
    },
  ]);
  assert.equal(events.length, 2);
  assert.equal(events.some((event) => event.kind === 'resource-response' && event.url === 'http://page.test/favicon.ico' && event.status === 404), true);
});

test('preserves requestfailureURL and attributes an exact request failure once', () => {
  const events = attributeBrowserFailureEvents([
    {
      source: 'playwright',
      kind: 'console',
      severity: 'error',
      message: 'request failed: net::ERR_FAILED',
      url: 'http://page.test/data.json',
    },
    {
      source: 'playwright',
      kind: 'requestfailed',
      severity: 'error',
      code: 'PLAYWRIGHT_REQUEST_FAILED',
      message: 'request failed: net::ERR_FAILED',
      requestfailureURL: 'http://page.test/data.json',
      url: 'http://page.test/data.json',
    },
  ]);
  assert.equal(events.length, 1);
  assert.equal(events[0].requestfailureURL, 'http://page.test/data.json');
  assert.deepEqual(events[0].attributedRequestFailure, { url: 'http://page.test/data.json' });
});

test('does not suppress an unrelated console error merely because its URL matches a request failure', () => {
  const events = attributeBrowserFailureEvents([
    {
      source: 'playwright',
      kind: 'console',
      severity: 'error',
      message: 'application callback failed',
      url: 'http://page.test/data.json',
    },
    {
      source: 'playwright',
      kind: 'requestfailed',
      severity: 'error',
      code: 'PLAYWRIGHT_REQUEST_FAILED',
      message: 'request failed: net::ERR_FAILED',
      requestfailureURL: 'http://page.test/data.json',
      url: 'http://page.test/data.json',
    },
  ]);
  assert.equal(events.length, 2);
  assert.equal(events.some((event) => event.kind === 'requestfailed'), true);
  assert.equal(events.some((event) => event.kind === 'console'), true);
});
