#!/usr/bin/env node

import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const SCHEMA_VERSION = 1;
const CAPTURE_PROTOCOL = 'rusty-engine.playtest-warning-delta/v1';
const MAX_MESSAGE_LENGTH = 512;
const MAX_ENGINE_READS = 32;
const DEFAULT_SETTLE_MS = 150;

export function normalizeMessage(value) {
  return String(value ?? '')
    .normalize('NFKC')
    .replace(/[\u0000-\u001f\u007f-\u009f]+/gu, ' ')
    .replace(/\s+/gu, ' ')
    .replace(/\b0x[0-9a-f]+\b/giu, '<hex>')
    .replace(/\b\d+\b/gu, '<n>')
    .trim()
    .slice(0, MAX_MESSAGE_LENGTH);
}

export function fingerprintFinding(finding) {
  const identity = [
    finding.source,
    finding.kind,
    finding.severity,
    finding.code ?? '',
    finding.disposition ?? '',
    finding.message,
  ].join('\u001f');
  return createHash('sha256').update(identity).digest('hex');
}

export function deduplicateFindings(findings) {
  const indexed = new Map();
  for (const candidate of findings) {
    const finding = Object.freeze({
      source: candidate.source,
      kind: candidate.kind,
      severity: candidate.severity,
      ...(candidate.code === undefined ? {} : { code: candidate.code }),
      ...(candidate.disposition === undefined ? {} : { disposition: candidate.disposition }),
      message: normalizeMessage(candidate.message),
    });
    if (finding.message.length === 0) continue;
    const fingerprint = fingerprintFinding(finding);
    const existing = indexed.get(fingerprint);
    if (existing === undefined) {
      indexed.set(fingerprint, { ...finding, fingerprint, occurrenceCount: 1 });
    } else {
      existing.occurrenceCount += 1;
    }
  }
  return [...indexed.values()].sort((left, right) => left.fingerprint.localeCompare(right.fingerprint));
}

export function compareBaseline(report, baseline, baselineLabel) {
  if (baseline === undefined) {
    return Object.freeze({ status: 'unavailable', reason: 'no baseline was supplied' });
  }
  const incompatibility = baselineIncompatibility(report, baseline);
  if (incompatibility !== undefined) {
    return Object.freeze({ status: 'incompatible', reason: incompatibility });
  }
  const current = new Map(report.findings.map((finding) => [finding.fingerprint, finding]));
  const previous = new Map(baseline.findings.map((finding) => [finding.fingerprint, finding]));
  const newlyObserved = [...current.values()].filter((finding) => !previous.has(finding.fingerprint));
  const resolved = [...previous.values()].filter((finding) => !current.has(finding.fingerprint));
  const unchanged = [...current.values()].filter((finding) => previous.has(finding.fingerprint));
  return Object.freeze({
    status: 'compatible',
    baseline: baselineLabel ?? baseline.run?.label ?? 'unnamed baseline',
    counts: Object.freeze({ new: newlyObserved.length, resolved: resolved.length, unchanged: unchanged.length }),
    new: newlyObserved,
    resolved,
    unchanged,
  });
}

export function captureBlockers(capture, comparison, findings = []) {
  const blockers = [];
  if (capture.browser.status !== 'complete') blockers.push(`Playwright listener capture is ${capture.browser.status}`);
  if (capture.engine.status !== 'complete') blockers.push(`Engine capture is ${capture.engine.status}`);
  if (comparison.status !== 'compatible') blockers.push(`baseline comparison is ${comparison.status}`);
  if (findings.some((finding) => finding.source === 'engine' && finding.disposition === 'terminal')) {
    blockers.push('Engine capture contains a terminal diagnostic');
  }
  if (findings.some((finding) => finding.severity === 'error')) {
    blockers.push('Capture contains an Error finding that requires disposition');
  }
  if (findings.some((finding) => finding.source === 'engine' && finding.disposition === 'resync-required')) {
    blockers.push('Engine capture requires resynchronization');
  }
  if (findings.some((finding) => finding.source === 'engine'
    && (finding.code === 'UNKNOWN_ENGINE_CODE' || finding.disposition === 'unknown'))) {
    blockers.push('Engine capture contains an unknown-provenance diagnostic');
  }
  return blockers;
}

function baselineIncompatibility(report, baseline) {
  if (baseline === null || typeof baseline !== 'object') return 'baseline is not an object';
  if (baseline.schemaVersion !== SCHEMA_VERSION || baseline.captureProtocol !== CAPTURE_PROTOCOL) {
    return 'baseline schema or capture protocol differs';
  }
  if (!sameJson(baseline.compatibility, report.compatibility)) return 'baseline compatibility identity differs';
  if (!Array.isArray(baseline.findings)) return 'baseline findings are missing';
  if (baseline.capture?.browser?.status !== 'complete' || baseline.capture?.engine?.status !== 'complete') {
    return 'baseline capture was incomplete';
  }
  return undefined;
}

function sameJson(left, right) {
  return left !== null && right !== null
    && typeof left === 'object' && typeof right === 'object'
    && left.exerciseId === right.exerciseId
    && left.pageOrigin === right.pageOrigin
    && left.engineOrigin === right.engineOrigin;
}

function parseArguments(argv) {
  const options = { settleMs: DEFAULT_SETTLE_MS };
  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index];
    if (argument === '--help') return { help: true };
    if (!argument.startsWith('--')) throw new Error(`unexpected argument: ${argument}`);
    const key = argument.slice(2).replace(/-([a-z])/gu, (_, letter) => letter.toUpperCase());
    const value = argv[index + 1];
    if (value === undefined || value.startsWith('--')) throw new Error(`missing value for ${argument}`);
    index += 1;
    options[key] = value;
  }
  if (typeof options.url !== 'string') throw new Error('--url is required');
  if (typeof options.exerciseId !== 'string' || options.exerciseId.length === 0) {
    throw new Error('--exercise-id is required');
  }
  options.url = new URL(options.url).toString();
  options.engineOrigin = options.engineOrigin === undefined ? undefined : normalizeOrigin(options.engineOrigin);
  options.engineCursor = options.engineCursor === undefined ? undefined : canonicalCursor(options.engineCursor, '--engine-cursor');
  options.settleMs = boundedInteger(options.settleMs, '--settle-ms', 0, 5_000);
  return options;
}

function normalizeOrigin(value) {
  const url = new URL(value);
  if (!['http:', 'https:'].includes(url.protocol) || url.pathname !== '/' || url.search !== '' || url.hash !== '') {
    throw new Error('--engine-origin must be an http(s) origin without a path, query, or fragment');
  }
  return url.origin;
}

function canonicalCursor(value, name) {
  if (!/^(?:0|[1-9]\d*)$/u.test(value)) throw new Error(`${name} must be canonical unsigned decimal text`);
  return value;
}

function boundedInteger(value, name, minimum, maximum) {
  const number = Number(value);
  if (!Number.isInteger(number) || number < minimum || number > maximum) {
    throw new Error(`${name} must be an integer from ${minimum} to ${maximum}`);
  }
  return number;
}

async function loadExercise(value) {
  if (value === undefined) return async ({ page, url }) => page.goto(url, { waitUntil: 'networkidle' });
  const module = await import(pathToFileURL(resolve(value)).href);
  const exercise = module.exercise ?? module.default;
  if (typeof exercise !== 'function') throw new Error('--exercise must export an async exercise(context) function');
  return exercise;
}

async function readEngineBatch(origin, after) {
  const response = await fetch(`${origin}/__rusty/product/runtime/diagnostics/read`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(after === undefined ? {} : { after }),
  });
  if (!response.ok) throw new Error(`Engine diagnostics read returned HTTP ${response.status}`);
  const batch = await response.json();
  validateEngineBatch(batch);
  return batch;
}

function validateEngineBatch(batch) {
  if (batch === null || typeof batch !== 'object' || !Array.isArray(batch.events)
    || typeof batch.lagged !== 'boolean') throw new Error('Engine diagnostics response has an invalid shape');
  for (const key of ['floorSequence', 'throughSequence', 'nextCursor', 'warningCount', 'errorCount', 'droppedCount']) {
    if (typeof batch[key] !== 'string' || !/^(?:0|[1-9]\d*)$/u.test(batch[key])) {
      throw new Error(`Engine diagnostics ${key} is not canonical cursor text`);
    }
  }
}

export async function drainEngineCheckpoint(origin, checkpoint, readBatch = readEngineBatch) {
  try {
    let batch = checkpoint;
    let lagged = batch.lagged;
    let droppedCount = BigInt(batch.droppedCount);
    let reads = 1;
    while (batch.nextCursor !== batch.throughSequence && reads < MAX_ENGINE_READS) {
      batch = await readBatch(origin, batch.nextCursor);
      reads += 1;
      lagged ||= batch.lagged;
      droppedCount = BigInt(batch.droppedCount);
    }
    const incomplete = batch.nextCursor !== batch.throughSequence;
    const status = lagged ? 'lagged' : droppedCount > 0n ? 'dropped' : incomplete ? 'incomplete' : 'complete';
    return {
      status,
      batch: { ...batch, lagged, droppedCount: droppedCount.toString() },
      initialCursor: checkpoint.nextCursor,
      checkpointCursor: batch.nextCursor,
      reads,
    };
  } catch (error) {
    return {
      status: 'failed',
      error: normalizeMessage(error instanceof Error ? error.message : error),
    };
  }
}

async function captureEngine(origin, checkpoint, checkpointMetadata) {
  try {
    let after = checkpoint.nextCursor;
    const batches = [];
    for (let attempt = 0; attempt < MAX_ENGINE_READS; attempt += 1) {
      const batch = await readEngineBatch(origin, after);
      batches.push(batch);
      if (batch.lagged || batch.nextCursor === batch.throughSequence) break;
      after = batch.nextCursor;
    }
    const final = batches.at(-1);
    const exhausted = final !== undefined && final.nextCursor !== final.throughSequence && !final.lagged;
    const lagged = checkpoint.lagged || batches.some((batch) => batch.lagged);
    const droppedCount = BigInt(final?.droppedCount ?? checkpoint.droppedCount);
    const postExerciseStatus = lagged ? 'lagged' : droppedCount > 0n ? 'dropped' : exhausted ? 'incomplete' : 'complete';
    const status = checkpointMetadata.status === 'complete' ? postExerciseStatus : checkpointMetadata.status;
    return {
      status,
      origin,
      checkpointCursor: checkpoint.nextCursor,
      finalCursor: final?.nextCursor ?? checkpoint.nextCursor,
      reads: batches.length + 1,
      lagged,
      droppedCount: (final?.droppedCount ?? checkpoint.droppedCount),
      checkpoint: {
        status: checkpointMetadata.status,
        initialCursor: checkpointMetadata.initialCursor,
        checkpointCursor: checkpointMetadata.checkpointCursor,
        reads: checkpointMetadata.reads,
      },
      events: batches.flatMap((batch) => batch.events),
    };
  } catch (error) {
    return { status: 'failed', origin, error: normalizeMessage(error instanceof Error ? error.message : error), events: [] };
  }
}

function engineFindings(events) {
  return events
    .filter((event) => event?.severity === 'warning' || event?.severity === 'error')
    .map((event) => ({
      source: 'engine',
      kind: 'structured',
      severity: event.severity,
      code: typeof event.code === 'string' ? event.code : 'UNKNOWN_ENGINE_CODE',
      disposition: typeof event.disposition === 'string' ? event.disposition : 'unknown',
      message: event.message,
    }));
}

function checkoutMetadata() {
  try {
    return {
      gitRevision: execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim(),
      gitDirty: execFileSync('git', ['status', '--porcelain'], { encoding: 'utf8' }).trim().length > 0,
    };
  } catch {
    return {};
  }
}

async function runCapture(options) {
  const startedAt = new Date().toISOString();
  const browserEvents = [];
  let browser = { status: 'pending' };
  let engineCapture = { status: options.engineOrigin === undefined ? 'not-configured' : 'pending', events: [] };
  let runnerError;
  let browserInstance;
  let browserVersion;
  let page;
  const consoleListener = (message) => {
    try {
      if (message.type() === 'warning' || message.type() === 'error') {
        browserEvents.push({ source: 'playwright', kind: 'console', severity: message.type(), message: message.text() });
      }
    } catch (error) {
      browser = { status: 'listener-failed', error: normalizeMessage(error instanceof Error ? error.message : error) };
    }
  };
  const pageErrorListener = (error) => {
    try {
      browserEvents.push({ source: 'playwright', kind: 'pageerror', severity: 'error', message: error.message });
    } catch (listenerError) {
      browser = { status: 'listener-failed', error: normalizeMessage(listenerError instanceof Error ? listenerError.message : listenerError) };
    }
  };
  try {
    const require = createRequire(new URL('../render/package.json', import.meta.url));
    const { chromium } = require('@playwright/test');
    browserInstance = await chromium.launch({ headless: true });
    browserVersion = browserInstance.version();
    page = await browserInstance.newPage();
    page.on('console', consoleListener);
    page.on('pageerror', pageErrorListener);
    browser = { status: 'complete' };
    const exercise = await loadExercise(options.exercise);
    const initialEngineBatch = options.engineOrigin === undefined
      ? undefined
      : await readEngineBatch(options.engineOrigin, options.engineCursor).catch((error) => error);
    let engineCheckpoint;
    if (initialEngineBatch instanceof Error) {
      engineCapture = { status: 'failed', origin: options.engineOrigin, error: normalizeMessage(initialEngineBatch.message), events: [] };
    } else if (initialEngineBatch !== undefined) {
      engineCheckpoint = await drainEngineCheckpoint(options.engineOrigin, initialEngineBatch);
      if (engineCheckpoint.status === 'failed') {
        engineCapture = { status: 'failed', origin: options.engineOrigin, error: engineCheckpoint.error, events: [] };
      }
    }
    await exercise(Object.freeze({ page, url: options.url }));
    if (options.settleMs > 0) await new Promise((done) => setTimeout(done, options.settleMs));
    if (options.engineOrigin !== undefined && engineCapture.status !== 'failed') {
      engineCapture = await captureEngine(options.engineOrigin, engineCheckpoint.batch, engineCheckpoint);
    }
  } catch (error) {
    runnerError = normalizeMessage(error instanceof Error ? error.message : error);
    if (browser.status === 'pending' || browser.status === 'complete') browser = { status: 'failed', error: runnerError };
  } finally {
    page?.off('console', consoleListener);
    page?.off('pageerror', pageErrorListener);
    await browserInstance?.close();
  }
  const findings = deduplicateFindings([...browserEvents, ...engineFindings(engineCapture.events)]);
  const report = {
    schemaVersion: SCHEMA_VERSION,
    captureProtocol: CAPTURE_PROTOCOL,
    compatibility: {
      exerciseId: options.exerciseId,
      pageOrigin: new URL(options.url).origin,
      engineOrigin: options.engineOrigin ?? null,
    },
    run: {
      startedAt,
      completedAt: new Date().toISOString(),
      url: options.url,
      exerciseId: options.exerciseId,
      nodeVersion: process.version,
      ...checkoutMetadata(),
      ...(browserVersion === undefined ? {} : { browserVersion }),
      ...(options.exercise === undefined ? {} : { exercise: options.exercise }),
      ...(runnerError === undefined ? {} : { error: runnerError }),
    },
    capture: {
      browser,
      engine: withoutEvents(engineCapture),
    },
    findings,
  };
  return report;
}

function withoutEvents(capture) {
  const { events, ...metadata } = capture;
  return metadata;
}

async function main() {
  const options = parseArguments(process.argv.slice(2));
  if (options.help) {
    console.log('Usage: node scripts/capture-playtest-warning-delta.mjs --url <url> --exercise-id <id> [--exercise <module.mjs>] [--engine-origin <origin>] [--engine-cursor <canonical-u64>] [--baseline <report.json>] [--output <report.json>] [--settle-ms <0-5000>]');
    return;
  }
  const report = await runCapture(options);
  let baseline;
  if (options.baseline !== undefined) {
    try {
      baseline = JSON.parse(await readFile(options.baseline, 'utf8'));
    } catch (error) {
      baseline = { incompatibleBaseline: normalizeMessage(error instanceof Error ? error.message : error) };
    }
  }
  const comparison = options.baseline === undefined
    ? compareBaseline(report, undefined)
    : baseline?.incompatibleBaseline === undefined
      ? compareBaseline(report, baseline, options.baseline)
      : { status: 'incompatible', reason: `baseline could not be read: ${baseline.incompatibleBaseline}` };
  const blockers = captureBlockers(report.capture, comparison, report.findings);
  const finalReport = { ...report, comparison, cleanClaimEligible: blockers.length === 0, blockers };
  const encoded = `${JSON.stringify(finalReport, null, 2)}\n`;
  if (options.output !== undefined) await writeFile(options.output, encoded, 'utf8');
  process.stdout.write(encoded);
  if (report.run.error !== undefined) process.exitCode = 1;
}

if (import.meta.url === pathToFileURL(process.argv[1]).href) {
  main().catch((error) => {
    console.error(`capture-playtest-warning-delta: ${error instanceof Error ? error.message : String(error)}`);
    process.exitCode = 2;
  });
}
