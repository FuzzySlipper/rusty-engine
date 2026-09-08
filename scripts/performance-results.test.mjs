import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtemp, readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import { dirname, resolve } from 'node:path';
import test from 'node:test';

import { capturePerformanceResults, comparePerformanceResults } from './performance-results.mjs';

const SCRIPT = resolve('scripts/performance-results.mjs');

function perfRecord({
  lane = 'renderer-cpu-submission',
  workload = { id: 'terrain', version: 1, seed: 7 },
  run = 0,
  median = 10,
  p95 = 15,
  renderer = 'Mesa GPU',
  vendor = 'Mesa',
  browser = 'Chromium 149',
  canvas = { cssWidth: 640, cssHeight: 360, backingWidth: 640, backingHeight: 360 },
} = {}) {
  return {
    lane,
    workload,
    run,
    iterations: 200,
    samples: [median],
    metrics: { minimum: median - 1, median, p95, maximum: p95 + 1, mean: median },
    renderer,
    vendor,
    browser,
    canvas,
    pacing: { classification: 'timer-query' },
  };
}

async function fixtureDirectory() {
  return mkdtemp(resolve(os.tmpdir(), 'rusty-performance-results-'));
}

async function capture(directory, name, environment, records) {
  const log = resolve(directory, `${name}.log`);
  const output = resolve(directory, `${name}.json`);
  await writeFile(log, records.map((record) => `RUSTY_PERF ${JSON.stringify(record)}`).join('\n'), 'utf8');
  return capturePerformanceResults({ output, environment, logs: [log], cwd: dirname(SCRIPT) });
}

function cli(args) {
  return spawnSync(process.execPath, [SCRIPT, ...args], { encoding: 'utf8' });
}

test('capture writes versioned provenance and retains every raw run record', async () => {
  const directory = await fixtureDirectory();
  const log = resolve(directory, 'runs.log');
  const output = resolve(directory, 'results.json');
  const legacy = { lane: 'software-legacy', median: 2, p95: 3, iterations: 10 };
  await writeFile(log, [
    'ordinary runner output',
    `RUSTY_PERF ${JSON.stringify(perfRecord({ run: 1 }))}`,
    `RUSTY_PERF ${JSON.stringify(legacy)}`,
  ].join('\n'), 'utf8');

  const artifact = await capturePerformanceResults({
    output,
    environment: 'developer-linux',
    logs: [log],
    cwd: resolve('.'),
  });
  const saved = JSON.parse(await readFile(output, 'utf8'));
  assert.equal(artifact.schemaVersion, 1);
  assert.equal(saved.artifact, 'rusty-engine.performance-results');
  assert.equal(saved.environment, 'developer-linux');
  assert.equal(typeof saved.git.revision, 'string');
  assert.equal(typeof saved.git.dirty, 'boolean');
  assert.equal(saved.host.os.platform, process.platform);
  assert.equal(saved.host.node, process.version);
  assert.equal(saved.records.length, 2);
  assert.deepEqual(saved.records[0].record, perfRecord({ run: 1 }));
  assert.deepEqual(saved.records[1].record, legacy);
});

test('legacy records without a workload object compare as the empty workload configuration', async () => {
  const directory = await fixtureDirectory();
  const baseline = await capture(directory, 'baseline', 'local-software', [
    { lane: 'software-legacy', median: 2, p95: 3, iterations: 10 },
  ]);
  const candidate = await capture(directory, 'candidate', 'local-software', [
    { lane: 'software-legacy', median: 2, p95: 3, iterations: 20 },
  ]);
  const report = comparePerformanceResults(baseline, candidate);
  assert.equal(report.status, 'compatible');
  assert.deepEqual(report.matching[0].group, { lane: 'software-legacy', workload: {} });
});

test('compare reports per-run variability and a known increase without a default gate', async () => {
  const directory = await fixtureDirectory();
  const baseline = await capture(directory, 'baseline', 'local-gpu', [
    perfRecord({ run: 1, median: 10, p95: 15 }),
    perfRecord({ run: 2, median: 14, p95: 21 }),
  ]);
  const candidate = await capture(directory, 'candidate', 'local-gpu', [
    perfRecord({ run: 1, median: 15, p95: 22 }),
    perfRecord({ run: 2, median: 18, p95: 27 }),
  ]);
  const report = comparePerformanceResults(baseline, candidate);
  assert.equal(report.status, 'report-only-increase');
  assert.equal(report.green, false);
  assert.equal(report.reportOnly, true);
  assert.equal(report.matching.length, 1);
  assert.deepEqual(report.matching[0].baseline.metrics.median, {
    median: 14, p95: 14, minimum: 10, maximum: 14, spread: 4,
  });
  assert.equal(report.matching[0].metrics.find((metric) => metric.name === 'median')?.changePercent, 28.571428571);

  const baselineFile = resolve(directory, 'baseline.json');
  const candidateFile = resolve(directory, 'candidate.json');
  const defaultResult = cli(['compare', baselineFile, candidateFile]);
  assert.equal(defaultResult.status, 0, defaultResult.stderr);
  assert.equal(JSON.parse(defaultResult.stdout).status, 'report-only-increase');
  const gatedResult = cli(['compare', '--fail-percent', '20', baselineFile, candidateFile]);
  assert.equal(gatedResult.status, 1, gatedResult.stderr);
  assert.equal(JSON.parse(gatedResult.stdout).status, 'policy-failed');
});

test('compare refuses to pool different host and browser configurations', async () => {
  const directory = await fixtureDirectory();
  const baseline = await capture(directory, 'baseline', 'local-gpu', [perfRecord()]);
  const candidate = await capture(directory, 'candidate', 'local-gpu', [perfRecord({ renderer: 'Different GPU' })]);
  candidate.host = { ...candidate.host, cpu: { ...candidate.host.cpu, models: ['Different CPU'] } };

  const report = comparePerformanceResults(baseline, candidate);
  assert.equal(report.status, 'incompatible');
  assert.deepEqual(report.environmentMismatches, ['host CPU/OS/Node metadata differs']);
  assert.equal(report.incompatibilities[0].mismatches.includes('renderer/vendor/browser/canvas configuration differs'), true);
});

test('compare reports missing and differently configured workloads instead of treating them as green', async () => {
  const directory = await fixtureDirectory();
  const baseline = await capture(directory, 'baseline', 'local-gpu', [perfRecord({ workload: { id: 'terrain', version: 1 } })]);
  const candidate = await capture(directory, 'candidate', 'local-gpu', [
    perfRecord({ workload: { id: 'terrain', version: 2 } }),
    perfRecord({ lane: 'renderer-frame-interval', workload: { id: 'terrain', version: 1 } }),
  ]);
  const report = comparePerformanceResults(baseline, candidate);
  assert.equal(report.status, 'incompatible');
  assert.equal(report.green, false);
  assert.equal(report.missing.baseline.length, 2);
  assert.equal(report.missing.candidate.length, 1);
  assert.deepEqual(report.workloadMismatches, [{
    lane: 'renderer-cpu-submission',
    baseline: [{ id: 'terrain', version: 1 }],
    candidate: [{ id: 'terrain', version: 2 }],
  }]);
});

test('workload-order changes do not create an incompatibility', async () => {
  const directory = await fixtureDirectory();
  const first = perfRecord({ workload: { id: 'terrain', version: 1 } });
  const second = perfRecord({ workload: { id: 'caves', version: 1 } });
  const baseline = await capture(directory, 'baseline', 'local-gpu', [first, second]);
  const candidate = await capture(directory, 'candidate', 'local-gpu', [second, first]);
  const report = comparePerformanceResults(baseline, candidate);
  assert.equal(report.status, 'compatible');
  assert.deepEqual(report.workloadMismatches, []);
});

test('floating timestamp noise rounds to no change while a small measured increase remains visible', async () => {
  const directory = await fixtureDirectory();
  const baseline = await capture(directory, 'baseline', 'local-gpu', [
    perfRecord({ median: 33.13999999999942, p95: 34.13999999999942 }),
  ]);
  const noisyCandidate = await capture(directory, 'noisy', 'local-gpu', [
    perfRecord({ median: 33.14000000000033, p95: 34.14000000000033 }),
  ]);
  const noisyReport = comparePerformanceResults(baseline, noisyCandidate);
  assert.equal(noisyReport.status, 'compatible');
  assert.deepEqual(noisyReport.reportOnlyIncreases, []);
  assert.deepEqual(noisyReport.matching[0].metrics.map((metric) => metric.changePercent), [0, 0]);

  const measuredCandidate = await capture(directory, 'measured', 'local-gpu', [
    perfRecord({ median: 33.140001, p95: 34.140001 }),
  ]);
  const measuredReport = comparePerformanceResults(baseline, measuredCandidate);
  assert.equal(measuredReport.status, 'report-only-increase');
  assert.equal(measuredReport.reportOnlyIncreases.length, 2);
  assert.equal(measuredReport.reportOnlyIncreases.every(
    (increase) => typeof increase.changePercent === 'number' && increase.changePercent > 0,
  ), true);
});

test('compare rejects an empty performance artifact', async () => {
  const directory = await fixtureDirectory();
  const candidate = await capture(directory, 'candidate', 'local-gpu', [perfRecord()]);
  assert.throws(
    () => comparePerformanceResults({ ...candidate, records: [] }, candidate),
    /not a rusty-engine\.performance-results\/v1 artifact/u,
  );
});

test('capture rejects malformed and empty RUSTY_PERF input', async () => {
  const directory = await fixtureDirectory();
  const malformed = resolve(directory, 'malformed.log');
  const empty = resolve(directory, 'empty.log');
  await writeFile(malformed, 'RUSTY_PERF {not-json}\n', 'utf8');
  await writeFile(empty, 'runner produced no measurements\n', 'utf8');

  await assert.rejects(
    capturePerformanceResults({ output: resolve(directory, 'bad.json'), environment: 'local', logs: [malformed] }),
    /malformed RUSTY_PERF JSON/u,
  );
  const captureResult = cli(['capture', '--output', resolve(directory, 'empty.json'), '--environment', 'local', empty]);
  assert.equal(captureResult.status, 2);
  assert.match(captureResult.stderr, /no RUSTY_PERF JSON records/u);
});
