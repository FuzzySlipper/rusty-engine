import assert from 'node:assert/strict';
import test from 'node:test';

import {
  LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES,
  appendLiveDebugTranscript,
  commandSummary,
  historyCommand,
  runtimeIncarnationLabel,
  updateAttributionLabel,
  workerUpdateLabel,
} from './live-debug-panel-model.js';

void test('transcript retains the most recent bounded command responses', () => {
  let entries = [] as ReturnType<typeof appendLiveDebugTranscript>;
  for (let index = 0; index <= LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES; index += 1) {
    entries = appendLiveDebugTranscript(entries, {
      command: `command.${String(index)}`,
      message: `response ${String(index)}`,
      succeeded: true,
    });
  }
  assert.equal(entries.length, LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES);
  assert.equal(entries[0]?.command, 'command.1');
  assert.equal(entries.at(-1)?.command, `command.${String(LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES)}`);
});

void test('history navigation restores older commands and clears after the newest entry', () => {
  const history = ['debug.world', 'debug.entity 42'];
  assert.deepEqual(historyCommand(history, null, -1), { cursor: 1, command: 'debug.entity 42' });
  assert.deepEqual(historyCommand(history, 1, -1), { cursor: 0, command: 'debug.world' });
  assert.deepEqual(historyCommand(history, 0, 1), { cursor: 1, command: 'debug.entity 42' });
  assert.deepEqual(historyCommand(history, 1, 1), { cursor: null, command: '' });
});

void test('catalog labels retain parameter names and types for command help', () => {
  assert.equal(commandSummary({
    name: 'debug.entity',
    description: 'Reads one entity.',
    parameters: [{ name: 'entityId', type: 'u64' }],
  }), 'debug.entity entityId: u64');
});

void test('worker and update labels retain the exact incarnation and separate phase meanings', () => {
  assert.equal(runtimeIncarnationLabel({ instanceId: '42', generation: '7', controlRevision: '9' }), 'runtime 42/7/9');
  assert.equal(workerUpdateLabel({
    workerPid: '4321',
    readout: {
      artifact: 'rusty.product.runtime-readout',
      runtime: { instanceId: '42', generation: '7', controlRevision: '9' },
      mode: 'realtime', state: 'running', admittedSimulationSteps: '12', admittedPresentations: '13',
      droppedRealtimeSteps: '0', clockRegressions: '0', scaledRemainder: null, lastObservedTimeNs: null, fault: null,
    },
    phases: { operationDurationUs: '180', outputConversionDurationUs: '25', outputEncodeWriteDurationUs: '35', inputQueueAgeUs: '8' },
    shellDeliveryIntervalUs: '240', shellOutputDecodeDurationUs: '7', shellOutputQueueDurationUs: '4', shellPublicationDurationUs: '6', ageMs: '3',
  }), 'Worker 4321 · runtime 42/7/9 · realtime/running · simulation 12 · age 3 ms');
  assert.equal(updateAttributionLabel({
    runtime: { instanceId: '42', generation: '7', controlRevision: '9' }, simulationStep: '12', admittedStepCount: '13', postCallbackDurationUs: '15', callbackDurationUs: '90',
    characterStepCalls: '0', characterStepDurationUs: '0', characterStepCastCount: '0', characterStepCandidateCount: '0', characterStepNarrowPhaseCount: '0',
    voxelResidencyCalls: '0', voxelResidencyDurationUs: '0', voxelScenePresentationCalls: '0', voxelScenePresentationDurationUs: '0',
  }), 'runtime 42/7/9 · simulation step 12 · admitted 13 · callback 90 us · post-callback 15 us');
});
