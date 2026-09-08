#!/usr/bin/env node

import { execFileSync } from 'node:child_process';
import { readFile, writeFile } from 'node:fs/promises';
import os from 'node:os';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const SCHEMA_VERSION = 1;
const ARTIFACT = 'rusty-engine.performance-results';
const PREFIX = 'RUSTY_PERF ';
const COMPATIBILITY_FIELDS = ['renderer', 'vendor', 'browser', 'canvas', 'rendererClass', 'runtime'];

export async function capturePerformanceResults({ output, environment, logs, cwd = process.cwd() }) {
  if (typeof output !== 'string' || output.length === 0) throw new Error('--output is required');
  if (typeof environment !== 'string' || environment.length === 0) throw new Error('--environment is required');
  if (!Array.isArray(logs) || logs.length === 0) throw new Error('at least one log file is required');

  const records = [];
  for (const log of logs) {
    const path = resolve(log);
    const text = await readFile(path, 'utf8');
    for (const [index, line] of text.split(/\r?\n/u).entries()) {
      const trimmed = line.trim();
      if (!trimmed.startsWith(PREFIX)) continue;
      const json = trimmed.slice(PREFIX.length);
      let record;
      try {
        record = JSON.parse(json);
      } catch (cause) {
        throw new Error(`${path}:${String(index + 1)} has malformed RUSTY_PERF JSON: ${message(cause)}`);
      }
      validateRecord(record, `${path}:${String(index + 1)}`);
      records.push({ source: { path, line: index + 1 }, record });
    }
  }
  if (records.length === 0) throw new Error('no RUSTY_PERF JSON records were found');

  const artifact = {
    schemaVersion: SCHEMA_VERSION,
    artifact: ARTIFACT,
    capturedAt: new Date().toISOString(),
    environment,
    git: gitMetadata(cwd),
    host: hostMetadata(),
    records,
  };
  await writeFile(resolve(output), `${JSON.stringify(artifact, null, 2)}\n`, 'utf8');
  return artifact;
}

export function comparePerformanceResults(baseline, candidate, { failPercent } = {}) {
  validateArtifact(baseline, 'baseline');
  validateArtifact(candidate, 'candidate');
  if (failPercent !== undefined && (!Number.isFinite(failPercent) || failPercent < 0)) {
    throw new Error('--fail-percent must be a finite number greater than or equal to zero');
  }

  const environmentMismatches = compareEnvironment(baseline, candidate);
  const baselineGroups = groupRecords(baseline.records);
  const candidateGroups = groupRecords(candidate.records);
  const matching = [];
  const missingBaseline = [];
  const missingCandidate = [];
  const incompatibilities = [];

  for (const baselineGroup of baselineGroups.values()) {
    const candidateGroup = candidateGroups.get(groupKey(baselineGroup));
    if (candidateGroup === undefined) {
      missingCandidate.push(groupIdentity(baselineGroup));
      continue;
    }
    const compatibility = compareGroupCompatibility(baselineGroup, candidateGroup);
    if (compatibility.length > 0) {
      incompatibilities.push({ group: groupIdentity(baselineGroup), mismatches: compatibility });
      continue;
    }
    const summary = compareGroupMeasurements(baselineGroup, candidateGroup);
    if (summary.incompatibleReason !== undefined) {
      incompatibilities.push({ group: groupIdentity(baselineGroup), mismatches: [summary.incompatibleReason] });
      continue;
    }
    matching.push(summary);
  }
  for (const candidateGroup of candidateGroups.values()) {
    if (!baselineGroups.has(groupKey(candidateGroup))) missingBaseline.push(groupIdentity(candidateGroup));
  }

  const workloadMismatches = findWorkloadMismatches(baselineGroups, candidateGroups);
  const policyFailures = failPercent === undefined
    ? []
    : matching.flatMap((group) => group.metrics
      .filter((metric) => isRegression(metric) && (metric.changePercent === null || metric.changePercent > failPercent))
      .map((metric) => ({
        group: group.group,
        metric: metric.name,
        changePercent: metric.changePercent,
        ...(metric.changePercent === null ? { unboundedFromZeroBaseline: true } : {}),
      })));
  const reportOnlyIncreases = matching.flatMap((group) => group.metrics
    .filter(isRegression)
    .map((metric) => ({
      group: group.group,
      metric: metric.name,
      changePercent: metric.changePercent,
      ...(metric.changePercent === null ? { unboundedFromZeroBaseline: true } : {}),
    })));
  const comparisonComplete = environmentMismatches.length === 0
    && incompatibilities.length === 0
    && missingBaseline.length === 0
    && missingCandidate.length === 0;
  const status = !comparisonComplete
    ? 'incompatible'
    : policyFailures.length > 0
      ? 'policy-failed'
      : reportOnlyIncreases.length > 0
        ? 'report-only-increase'
        : 'compatible';

  return {
    schemaVersion: SCHEMA_VERSION,
    artifact: `${ARTIFACT}.comparison`,
    status,
    green: status === 'compatible',
    reportOnly: failPercent === undefined,
    ...(failPercent === undefined ? {} : { failPercent }),
    baseline: artifactIdentity(baseline),
    candidate: artifactIdentity(candidate),
    environmentMismatches,
    workloadMismatches,
    missing: { baseline: missingBaseline, candidate: missingCandidate },
    incompatibilities,
    matching,
    reportOnlyIncreases,
    policyFailures,
  };
}

function groupRecords(records) {
  const groups = new Map();
  for (const entry of records) {
    const workload = workloadConfig(entry.record);
    const key = `${entry.record.lane}\u0000${canonicalJson(workload)}`;
    const existing = groups.get(key) ?? {
      lane: entry.record.lane,
      workload,
      records: [],
      compatibility: new Map(),
    };
    existing.records.push(entry);
    existing.compatibility.set(canonicalJson(recordCompatibility(entry.record)), recordCompatibility(entry.record));
    groups.set(key, existing);
  }
  return groups;
}

function compareGroupCompatibility(baseline, candidate) {
  const mismatches = [];
  if (baseline.compatibility.size !== 1) mismatches.push('baseline group has multiple renderer/browser/canvas configurations');
  if (candidate.compatibility.size !== 1) mismatches.push('candidate group has multiple renderer/browser/canvas configurations');
  if (baseline.compatibility.size === 1 && candidate.compatibility.size === 1
    && baseline.compatibility.keys().next().value !== candidate.compatibility.keys().next().value) {
    mismatches.push('renderer/vendor/browser/canvas configuration differs');
  }
  return mismatches;
}

function compareGroupMeasurements(baseline, candidate) {
  const baselineMetrics = measurements(baseline.records);
  const candidateMetrics = measurements(candidate.records);
  if (baselineMetrics === null || candidateMetrics === null) {
    return { group: groupIdentity(baseline), incompatibleReason: 'one or both groups do not provide finite median and p95 timing measurements' };
  }
  return {
    group: groupIdentity(baseline),
    baseline: { runs: baseline.records.length, metrics: summarizeMetrics(baselineMetrics) },
    candidate: { runs: candidate.records.length, metrics: summarizeMetrics(candidateMetrics) },
    metrics: ['median', 'p95'].map((name) => ({
      name,
      baseline: summarize(baselineMetrics[name]),
      candidate: summarize(candidateMetrics[name]),
      changePercent: percentChange(summarize(baselineMetrics[name]).median, summarize(candidateMetrics[name]).median),
    })),
  };
}

function measurements(records) {
  const result = { median: [], p95: [] };
  for (const { record } of records) {
    const values = metricValues(record);
    if (values === null) return null;
    result.median.push(values.median);
    result.p95.push(values.p95);
  }
  return result;
}

function metricValues(record) {
  const source = isPlainObject(record.metrics) ? record.metrics : record;
  if (!isTiming(source.median) || !isTiming(source.p95)) return null;
  return { median: source.median, p95: source.p95 };
}

function summarizeMetrics(metrics) {
  return { median: summarize(metrics.median), p95: summarize(metrics.p95) };
}

function summarize(values) {
  const sorted = [...values].sort((left, right) => left - right);
  const minimum = sorted[0];
  const maximum = sorted.at(-1);
  return {
    median: percentile(sorted, 0.5),
    p95: percentile(sorted, 0.95),
    minimum,
    maximum,
    spread: maximum - minimum,
  };
}

function percentile(sorted, fraction) {
  return sorted[Math.round((sorted.length - 1) * fraction)];
}

function percentChange(baseline, candidate) {
  if (baseline === 0) return candidate === 0 ? 0 : null;
  return Math.round((((candidate - baseline) / baseline) * 100) * 1_000_000_000) / 1_000_000_000;
}

function workloadConfig(record) {
  return record.workload === undefined ? {} : cloneJson(record.workload, 'workload');
}

function recordCompatibility(record) {
  const result = { unit: timingUnit(record) };
  for (const field of COMPATIBILITY_FIELDS) {
    if (record[field] !== undefined) result[field] = cloneJson(record[field], field);
  }
  return result;
}

function timingUnit(record) {
  if (typeof record.unit === 'string') return record.unit;
  if (isPlainObject(record.metrics) && typeof record.metrics.unit === 'string') return record.metrics.unit;
  return 'milliseconds';
}

function findWorkloadMismatches(baselineGroups, candidateGroups) {
  const byLane = new Map();
  for (const group of baselineGroups.values()) {
    const entry = byLane.get(group.lane) ?? { baseline: [], candidate: [] };
    entry.baseline.push(group.workload);
    byLane.set(group.lane, entry);
  }
  for (const group of candidateGroups.values()) {
    const entry = byLane.get(group.lane) ?? { baseline: [], candidate: [] };
    entry.candidate.push(group.workload);
    byLane.set(group.lane, entry);
  }
  return [...byLane.entries()]
    .map(([lane, groups]) => ({
      lane,
      baseline: sortWorkloads(groups.baseline),
      candidate: sortWorkloads(groups.candidate),
    }))
    .filter((groups) => groups.baseline.length > 0 && groups.candidate.length > 0
      && canonicalJson(groups.baseline) !== canonicalJson(groups.candidate));
}

function sortWorkloads(workloads) {
  return [...workloads].sort((left, right) => canonicalJson(left).localeCompare(canonicalJson(right)));
}

function compareEnvironment(baseline, candidate) {
  const mismatches = [];
  if (baseline.environment !== candidate.environment) mismatches.push('environment label differs');
  if (canonicalJson(baseline.host) !== canonicalJson(candidate.host)) mismatches.push('host CPU/OS/Node metadata differs');
  return mismatches;
}

function artifactIdentity(artifact) {
  return {
    environment: artifact.environment,
    revision: artifact.git.revision,
    dirty: artifact.git.dirty,
    capturedAt: artifact.capturedAt,
  };
}

function groupIdentity(group) {
  return { lane: group.lane, workload: group.workload };
}

function groupKey(group) {
  return `${group.lane}\u0000${canonicalJson(group.workload)}`;
}

function validateRecord(value, location) {
  if (!isPlainObject(value) || typeof value.lane !== 'string' || value.lane.length === 0) {
    throw new Error(`${location} RUSTY_PERF record must have a non-empty string lane`);
  }
  if (value.workload !== undefined) cloneJson(value.workload, `${location} workload`);
  for (const field of COMPATIBILITY_FIELDS) {
    if (value[field] !== undefined) cloneJson(value[field], `${location} ${field}`);
  }
}

function validateArtifact(value, name) {
  if (!isPlainObject(value) || value.schemaVersion !== SCHEMA_VERSION || value.artifact !== ARTIFACT
    || typeof value.environment !== 'string' || !isPlainObject(value.git) || !isPlainObject(value.host)
    || !Array.isArray(value.records) || value.records.length === 0) {
    throw new Error(`${name} is not a ${ARTIFACT}/v${String(SCHEMA_VERSION)} artifact`);
  }
  for (const [index, entry] of value.records.entries()) {
    if (!isPlainObject(entry) || !isPlainObject(entry.source)) throw new Error(`${name} record ${String(index)} has no source`);
    validateRecord(entry.record, `${name} record ${String(index)}`);
  }
}

function hostMetadata() {
  const cpus = os.cpus();
  return {
    os: { platform: process.platform, release: os.release(), arch: process.arch },
    cpu: {
      count: cpus.length,
      models: [...new Set(cpus.map((cpu) => cpu.model))].sort(),
    },
    node: process.version,
  };
}

function gitMetadata(cwd) {
  const revision = git(['rev-parse', 'HEAD'], cwd);
  const status = git(['status', '--porcelain'], cwd);
  return { revision, dirty: status === null ? null : status.length > 0 };
}

function git(args, cwd) {
  try {
    return execFileSync('git', args, { cwd, encoding: 'utf8', stdio: ['ignore', 'pipe', 'ignore'] }).trim();
  } catch {
    return null;
  }
}

function cloneJson(value, name) {
  if (value === null || typeof value === 'boolean' || typeof value === 'string') return value;
  if (typeof value === 'number') {
    if (!Number.isFinite(value)) throw new Error(`${name} must contain only finite JSON numbers`);
    return value;
  }
  if (Array.isArray(value)) return value.map((entry) => cloneJson(entry, name));
  if (!isPlainObject(value)) throw new Error(`${name} must be JSON data`);
  const result = {};
  for (const key of Object.keys(value).sort()) result[key] = cloneJson(value[key], name);
  return result;
}

function canonicalJson(value) {
  return JSON.stringify(cloneJson(value, 'configuration'));
}

function isPlainObject(value) {
  if (value === null || typeof value !== 'object' || Array.isArray(value)) return false;
  const prototype = Object.getPrototypeOf(value);
  return prototype === Object.prototype || prototype === null;
}

function isTiming(value) {
  return typeof value === 'number' && Number.isFinite(value) && value >= 0;
}

function isRegression(metric) {
  return metric.changePercent === null
    ? metric.candidate.median > metric.baseline.median
    : metric.changePercent > 0;
}

function message(cause) {
  return cause instanceof Error ? cause.message : String(cause);
}

async function main(arguments_) {
  const [command, ...args] = arguments_;
  if (command === 'capture') {
    const { output, environment, files } = parseCaptureArguments(args);
    await capturePerformanceResults({ output, environment, logs: files });
    return 0;
  }
  if (command === 'compare') {
    const { baseline, candidate, failPercent } = parseCompareArguments(args);
    const report = comparePerformanceResults(
      JSON.parse(await readFile(resolve(baseline), 'utf8')),
      JSON.parse(await readFile(resolve(candidate), 'utf8')),
      { failPercent },
    );
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
    if (report.status === 'incompatible') return 2;
    if (report.status === 'policy-failed') return 1;
    return 0;
  }
  throw new Error('usage: performance-results.mjs capture --output FILE --environment LABEL LOG... | compare [--fail-percent NUMBER] BASE CANDIDATE');
}

function parseCaptureArguments(args) {
  let output;
  let environment;
  const files = [];
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === '--output') output = args[++index];
    else if (args[index] === '--environment') environment = args[++index];
    else if (args[index]?.startsWith('--')) throw new Error(`unknown capture option ${args[index]}`);
    else files.push(args[index]);
  }
  return { output, environment, files };
}

function parseCompareArguments(args) {
  let failPercent;
  const files = [];
  for (let index = 0; index < args.length; index += 1) {
    if (args[index] === '--fail-percent') {
      const raw = args[++index];
      failPercent = Number(raw);
      if (raw === undefined || !Number.isFinite(failPercent) || failPercent < 0) {
        throw new Error('--fail-percent must be a finite number greater than or equal to zero');
      }
    } else if (args[index]?.startsWith('--')) throw new Error(`unknown compare option ${args[index]}`);
    else files.push(args[index]);
  }
  if (files.length !== 2) throw new Error('compare requires BASE and CANDIDATE artifacts');
  return { baseline: files[0], candidate: files[1], failPercent };
}

if (process.argv[1] !== undefined && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  main(process.argv.slice(2)).then((exitCode) => {
    process.exitCode = exitCode;
  }).catch((cause) => {
    process.stderr.write(`performance-results: ${message(cause)}\n`);
    process.exitCode = 2;
  });
}
