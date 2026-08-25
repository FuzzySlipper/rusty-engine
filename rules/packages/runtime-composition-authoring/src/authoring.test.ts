import { strict as assert } from 'node:assert';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

import {
  RuntimeCompositionAuthoringError,
  admitCompiledComposition,
  append,
  appendComposition,
  authorRuntimeComposition,
  cadence,
  compiledCompositionBytes,
  engineCapability,
  extend,
  extendComposition,
  fragment,
  gameplayCatalog,
  gameplayDefinition,
  inputAction,
  kernelCapability,
  observePairs,
  phase,
  prepend,
  prependComposition,
  productIntent,
  replace,
  replaceComposition,
  schedule,
  Standard,
  system,
  timeline,
  timelineStep,
} from './index.js';
import type { SchedulePhaseDeclaration } from './types.js';

const fixtureUrl = new URL(
  '../../../../fixtures/product-model/minimum.compiled-composition.json',
  import.meta.url,
);
const canonicalNumbersFixtureUrl = new URL(
  '../../../../fixtures/product-model/canonical-numbers.expected.compiled-composition.json',
  import.meta.url,
);
const observePairsFixtureUrl = new URL(
  '../../../../fixtures/runtime-standard-capabilities/stealth.observe-pairs.compiled-composition.json',
  import.meta.url,
);

function expectError(action: () => unknown, code: RuntimeCompositionAuthoringError['code']): void {
  assert.throws(action, (error: unknown) => error instanceof RuntimeCompositionAuthoringError && error.code === code);
}

function declarations(): readonly SchedulePhaseDeclaration[] {
  return schedule({
    input: append(Standard.input),
    simulation: append(Standard.simulation, system('movement', {
      capability: 'movement.apply',
      reads: ['input.motion'],
      writes: ['state.transform'],
      payload: { kind: 'movement' },
    })),
    consequences: append(Standard.consequences),
    commit: append(Standard.commit),
    projection: append(Standard.projection, system('render-projection', {
      capability: 'projection.refresh',
      reads: ['entity-state.projection'],
      writes: ['render-frame.diff'],
      payload: null,
    })),
  });
}

function root(scheduleValue: readonly SchedulePhaseDeclaration[] = declarations()): Record<string, unknown> {
  return {
    product: 'example.product',
    capabilities: [
      kernelCapability('movement.apply', 'apply-movement'),
      engineCapability('projection.refresh', 'render.entity-project'),
    ],
    schedule: scheduleValue,
  };
}

test('typed Runtime Composition authoring emits the exact Rust-owned current fixture', async () => {
  const artifact = minimumArtifact();
  const fixture = await readFile(fixtureUrl, 'utf8');
  assert.equal(artifact.canonicalJson, fixture);
  assert.deepEqual(compiledCompositionBytes(artifact), new TextEncoder().encode(fixture));
  assert.ok(Object.isFrozen(artifact));
  assert.ok(Object.isFrozen(artifact.composition));
  assert.ok(Object.isFrozen(artifact.composition.schedule));
  const simulation = artifact.composition.schedule[1];
  assert.equal(simulation?.phase, 'simulation');
  if (simulation?.mode === 'append') {
    assert.equal(simulation.systems[0]?.reads[0], 'input.motion');
  }
  const projection = artifact.composition.schedule[4];
  if (projection?.mode === 'append') {
    assert.deepEqual(projection.systems[0]?.writes, ['render-frame.diff']);
  }
});

test('observe-pairs authors the closed stealth pressure system with generated access and result contracts', async () => {
  const observe = engineCapability('observe-pairs', 'runtime.observe-pairs');
  const advanceAlert = kernelCapability('stealth.advance-alert', 'advance-alert');
  const artifact = authorRuntimeComposition({
    product: 'stealth.pressure',
    capabilities: [observe, advanceAlert],
    schedule: schedule({
      simulation: append(Standard.simulation, observePairs({
        id: 'stealth.detect',
        engineBinding: observe,
        operationBinding: advanceAlert,
        observerRole: 'stealth.vision',
        targetRole: 'stealth.target',
        quotas: { observers: 64, targets: 256, pairs: 1024, aggregates: 256 },
        cadence: { everySteps: 6, offsetSteps: 0 },
      })),
    }),
  });
  const fixture = await readFile(observePairsFixtureUrl, 'utf8');
  assert.equal(artifact.canonicalJson, fixture);
  const system = artifact.composition.schedule[1];
  assert.equal(system?.phase, 'simulation');
  if (system?.mode === 'append') {
    assert.deepEqual(system.systems[0]?.reads, ['entity-state.components', 'entity-state.transforms', 'engine-spatial.occlusion']);
    assert.deepEqual(system.systems[0]?.writes, ['runtime-mutation.operations']);
    assert.deepEqual(JSON.parse(JSON.stringify(system.systems[0]?.payload)), {
      kind: 'engine.runtime.observe-pairs.v1',
      observerRole: 'stealth.vision',
      targetRole: 'stealth.target',
      operationBinding: 'stealth.advance-alert',
      operationType: 'engine.runtime.observe-pairs.result.v1',
      quotas: { observers: 64, targets: 256, pairs: 1024, aggregates: 256 },
    });
  }
});

test('observe-pairs refuses unread expression, callback, visibility, reducer, and quota extensions', () => {
  const observe = engineCapability('observe-pairs', 'runtime.observe-pairs');
  const advanceAlert = kernelCapability('stealth.advance-alert', 'advance-alert');
  const safe = {
    id: 'stealth.detect', engineBinding: observe, operationBinding: advanceAlert,
    observerRole: 'stealth.vision', targetRole: 'stealth.target',
    quotas: { observers: 1, targets: 1, pairs: 1, aggregates: 1 }, cadence: 6,
  };
  let expressionRead = false;
  const expression = { ...safe } as Record<string, unknown>;
  Object.defineProperty(expression, 'expression', { enumerable: true, get: () => { expressionRead = true; return 'must-not-read'; } });
  expectError(() => observePairs(expression as unknown as Parameters<typeof observePairs>[0]), 'invalid-json-value');
  assert.equal(expressionRead, false);
  expectError(() => observePairs({ ...safe, visibility: 'thick-line' } as unknown as Parameters<typeof observePairs>[0]), 'unknown-field');
  expectError(() => observePairs({ ...safe, reducer: 'sum-by-field' } as unknown as Parameters<typeof observePairs>[0]), 'unknown-field');
  expectError(() => observePairs({ ...safe, observerRole: (() => 'stealth.vision') as unknown as string }), 'invalid-field-type');
  expectError(() => observePairs({ ...safe, targetRole: 'stealth.vision' }), 'duplicate-entry');
  expectError(() => observePairs({ ...safe, quotas: { observers: 65, targets: 1, pairs: 1, aggregates: 1 } }), 'quota-exceeded');
  expectError(() => observePairs({ ...safe, operationBinding: engineCapability('wrong', 'runtime.observe-pairs') }), 'invalid-capability-target');
});

test('typed W mapping resolves to one admitted move.forward descriptor', () => {
  const artifact = authorRuntimeComposition({
    product: 'example.product',
    capabilities: [kernelCapability('move.forward', 'move-forward')],
    intentDescriptors: [productIntent({
      id: 'move.forward', valueKind: 'digital', capability: 'move.forward', payload: { semantic: 'move-forward' },
    })],
    inputMap: [inputAction({
      id: 'w-forward', intent: 'move.forward',
      trigger: { kind: 'key', code: 'key-w', edge: 'held', context: 'gameplay' },
    })],
    schedule: schedule({}),
    gameplayDefinitions: [],
    timelines: [],
  });
  const wire = JSON.parse(artifact.canonicalJson) as { intentDescriptors: unknown; inputMap: unknown };
  assert.deepEqual(wire.intentDescriptors, [{
    id: 'move.forward', valueKind: 'digital', capability: 'move.forward', payload: { semantic: 'move-forward' },
  }]);
  assert.deepEqual(wire.inputMap, [{
    id: 'w-forward', intent: 'move.forward',
    trigger: { kind: 'key', code: 'key-w', edge: 'held', context: 'gameplay' },
  }]);
  expectError(() => admitCompiledComposition({
    ...artifact.composition,
    inputMap: [{ id: 'wrong-kind', intent: 'move.forward', trigger: { kind: 'pointer-axis', axis: 'x' } }],
  }), 'input-trigger-value-kind');
});

test('VM-local typed intent snapshots without a Product Kernel capability', () => {
  const artifact = authorRuntimeComposition({
    product: 'example.product',
    capabilities: [],
    intentDescriptors: [productIntent({
      id: 'move.forward', valueKind: 'digital', payload: { semantic: 'move-forward' },
    })],
    inputMap: [inputAction({
      id: 'w-forward', intent: 'move.forward',
      trigger: { kind: 'key', code: 'key-w', edge: 'pressed', context: 'gameplay' },
    })],
    schedule: schedule({}),
    gameplayDefinitions: [],
    timelines: [],
  });
  assert.equal(
    artifact.canonicalJson,
    '{"product":"example.product","intentDescriptors":[{"id":"move.forward","valueKind":"digital","payload":{"semantic":"move-forward"}}],"inputMap":[{"id":"w-forward","intent":"move.forward","trigger":{"code":"key-w","context":"gameplay","edge":"pressed","kind":"key"}}],"schedule":[{"phase":"input","mode":"append","systems":[]},{"phase":"simulation","mode":"append","systems":[]},{"phase":"consequences","mode":"append","systems":[]},{"phase":"commit","mode":"append","systems":[]},{"phase":"projection","mode":"append","systems":[]}],"gameplayDefinitions":[],"timelines":[],"capabilityBindings":[]}\n',
  );
});

test('authoring materializes only detached plain data', () => {
  const artifact = minimumArtifact();
  const left = compiledCompositionBytes(artifact);
  left[0] = 0;
  const right = compiledCompositionBytes(artifact);
  assert.notEqual(right[0], 0);
  const parsed = JSON.parse(artifact.canonicalJson) as { schedule: readonly { phase: string }[] };
  assert.equal(parsed.schedule[1]?.phase, 'simulation');

  for (const value of [
    () => 1,
    undefined,
    1n,
    Symbol('symbol'),
    Number.NaN,
    Number.POSITIVE_INFINITY,
    9_007_199_254_740_992,
  ]) {
    expectError(() => productIntent({ id: 'bad', valueKind: 'digital', capability: 'camera.look', payload: value }), 'invalid-json-value');
  }
  const cyclic: Record<string, unknown> = {};
  cyclic['self'] = cyclic;
  expectError(() => gameplayDefinition('cyclic', cyclic), 'invalid-json-value');
  expectError(() => gameplayDefinition('class-value', new (class Value {})()), 'invalid-json-value');
  expectError(() => gameplayDefinition('sparse', new Array(1)), 'invalid-json-value');
});

test('canonical writer preserves the Rust number policy and bytewise opaque object key order', async () => {
  const artifact = authorRuntimeComposition({
    product: 'example.product',
    capabilities: [],
    intentDescriptors: [],
    inputMap: [],
    gameplayDefinitions: [gameplayDefinition('numeric', {
      '2': 'two', '10': 'ten', '1': 'one', small: 1e-6, tiny: 1.2e-6, negativeZero: -0,
    })],
    schedule: schedule({}),
    timelines: [],
  });
  const fixture = await readFile(canonicalNumbersFixtureUrl, 'utf8');
  assert.equal(artifact.canonicalJson, fixture);
});

test('descriptor-first authoring rejects executable and ambiguous action arrays before reading them', () => {
  let getterRead = false;
  const getterAction = {};
  Object.defineProperty(getterAction, 'id', {
    enumerable: true,
    get: () => { getterRead = true; return 'unexpected'; },
  });
  expectError(() => phase('simulation', [getterAction as unknown as Parameters<typeof phase>[1][number]]), 'invalid-json-value');
  assert.equal(getterRead, false);

  class Action {
    public readonly id = 'class-action';
    public readonly capability = 'movement.apply';
    public readonly reads = [];
    public readonly writes = [];
    public readonly payload = null;
  }
  expectError(() => phase('simulation', [new Action()]), 'invalid-field-type');
  expectError(() => phase('simulation', [(() => null) as unknown as Parameters<typeof phase>[1][number]]), 'invalid-field-type');

  const accessorArray: unknown[] = [];
  Object.defineProperty(accessorArray, '0', { enumerable: true, get: () => ({}) });
  accessorArray.length = 1;
  expectError(() => phase('simulation', accessorArray as Parameters<typeof phase>[1]), 'invalid-json-value');

  const extraPropertyArray = [{ id: 'safe', capability: 'movement.apply', reads: [], writes: [], payload: null }];
  Object.defineProperty(extraPropertyArray, '4294967295', { enumerable: true, value: 'not-an-index' });
  expectError(() => phase('simulation', extraPropertyArray), 'invalid-json-value');

  let overriddenMapCalled = false;
  class ExecutableArray extends Array<Parameters<typeof phase>[1][number]> {
    public override map(): never {
      overriddenMapCalled = true;
      throw new Error('must not execute');
    }
  }
  const subclass = new ExecutableArray();
  subclass.push({ id: 'subclass', capability: 'movement.apply', reads: [], writes: [], payload: null });
  expectError(() => phase('simulation', subclass), 'invalid-json-value');
  assert.equal(overriddenMapCalled, false);

  const executablePayload = [1];
  Object.defineProperty(executablePayload, 'run', { enumerable: true, value: () => 'must not persist' });
  expectError(() => gameplayDefinition('executable-array', executablePayload), 'invalid-json-value');
});

test('composition admission rejects unknown fields, unsafe references, and missing current schedule access declarations', () => {
  expectError(
    () => authorRuntimeComposition({ ...minimumDraft(), schemaVersion: 1 }),
    'unknown-field',
  );
  const unknownCapability = structuredClone(minimumArtifact().composition) as unknown as Record<string, unknown>;
  ((unknownCapability['intentDescriptors'] as { capability: string }[])[0] as { capability: string })['capability'] = 'missing.capability';
  expectError(() => admitCompiledComposition(unknownCapability), 'unknown-capability');
  const missingReads = structuredClone(minimumArtifact().composition) as unknown as Record<string, unknown>;
  const missingReadsPhase = (missingReads['schedule'] as Record<string, unknown>[])[1]!;
  const missingReadsSystem = (missingReadsPhase['systems'] as Record<string, unknown>[])[0]!;
  delete missingReadsSystem['reads'];
  expectError(() => admitCompiledComposition(missingReads), 'missing-field');
  const duplicateAccess = structuredClone(minimumArtifact().composition) as unknown as Record<string, unknown>;
  ((((duplicateAccess['schedule'] as Record<string, unknown>[])[1] as Record<string, unknown>)['systems'] as Record<string, unknown>[])[0] as Record<string, unknown>)['reads'] = ['state.transform', 'state.transform'];
  expectError(() => admitCompiledComposition(duplicateAccess), 'duplicate-entry');
});

test('capability targets split only at the Rust-owned namespace separator', () => {
  const direct = minimumArtifact().composition;
  const dotted = structuredClone(direct) as unknown as Record<string, unknown>;
  const bindings = dotted['capabilityBindings'] as Record<string, unknown>[];
  const firstBinding = bindings[0];
  const secondBinding = bindings[1];
  assert.ok(firstBinding !== undefined);
  assert.ok(secondBinding !== undefined);
  firstBinding['target'] = 'engine.camera.look';
  secondBinding['target'] = 'kernel.movement.apply';
  const admitted = admitCompiledComposition(dotted);
  assert.equal(admitted.capabilityBindings[0]?.target, 'engine.camera.look');
  assert.equal(admitted.capabilityBindings[1]?.target, 'kernel.movement.apply');
  for (const target of ['browser.camera.look', 'engine', 'engine.', '.camera.look', 'engine..camera']) {
    const malformed = structuredClone(direct) as unknown as Record<string, unknown>;
    (malformed['capabilityBindings'] as Record<string, unknown>[])[0]!['target'] = target;
    expectError(() => admitCompiledComposition(malformed), target.startsWith('engine.') ? 'invalid-identity' : 'invalid-capability-target');
  }
});

test('engine capability helper accepts only the generated closed Engine names', () => {
  assert.equal(
    engineCapability('projection', 'render.entity-project').target,
    'engine.render.entity-project',
  );
  expectError(
    () => engineCapability('stale', 'render.stale-project' as never),
    'unknown-engine-capability',
  );
});

test('whole-composition append and prepend preserve ordered collections while fragments stay schedule-free', () => {
  const base = minimumArtifact().composition;
  const additions = fragment({
    inputMap: [inputAction({ id: 'fire', intent: 'look', trigger: { kind: 'pointer-axis', axis: 'x' } })],
  });
  const appended = appendComposition(base, additions);
  const prepended = prependComposition(base, additions);
  assert.deepEqual(appended.inputMap.map((entry) => entry.id), ['look', 'fire']);
  assert.deepEqual(prepended.inputMap.map((entry) => entry.id), ['fire', 'look']);
  assert.deepEqual(appended.schedule, base.schedule);
  assert.deepEqual(prepended.schedule, base.schedule);
  expectError(() => extendComposition(base, fragment({ inputMap: [inputAction({ id: 'look', intent: 'look', trigger: { kind: 'pointer-axis', axis: 'x' } })] })), 'duplicate-entry');
});

test('whole-composition replace changes only explicitly named collections and cadence is representable', () => {
  const base = minimumArtifact().composition;
  const replaced = replaceComposition(base, {
    timelines: [timeline('outro', [timelineStep({ id: 'finish', capability: 'timeline.start', payload: { scene: 'ending' } })])],
  });
  assert.deepEqual(replaced.timelines.map((entry) => entry.id), ['outro']);
  assert.deepEqual(replaced.schedule, base.schedule);
  assert.deepEqual(cadence({ everySteps: 60, offsetSteps: 0 }), { everySteps: 60, offsetSteps: 0 });
});

test('schedule DSL exposes closed Standard anchors, explicit composition, and step cadence', () => {
  assert.deepEqual(declarations().map((entry) => entry.phase), ['input', 'simulation', 'consequences', 'commit', 'projection']);
  const simulation = declarations()[1];
  assert.equal(simulation?.mode, 'append');
  if (simulation?.mode === 'append') assert.equal(simulation.systems[0]?.id, 'movement');
  assert.deepEqual(cadence(3), { everySteps: 3, offsetSteps: 0 });
  assert.deepEqual(cadence({ everySteps: 4, offsetSteps: 2 }), { everySteps: 4, offsetSteps: 2 });
  assert.ok(Object.isFrozen(Standard.input));
  expectError(() => cadence(0), 'invalid-schedule-cadence');
  expectError(() => cadence({ everySteps: 2, offsetSteps: 2 }), 'invalid-schedule-cadence');
});

test('authoring lowers to the exact five-phase wire and canonical bytes', () => {
  const artifact = authorRuntimeComposition(root());
  assert.ok(Object.isFrozen(artifact.composition.schedule));
  assert.equal(artifact.composition.schedule.length, 5);
  const wire = JSON.parse(artifact.canonicalJson) as Record<string, unknown>;
  assert.equal((wire['schedule'] as Record<string, unknown>[])[1]?.['mode'], 'append');
  assert.match(artifact.canonicalJson, /"cadence":\{"everySteps":1,"offsetSteps":0\}/);
  assert.equal(wire['schemaVersion'], undefined);
});

test('direct current-schema admission is strict about phase set and system fields', () => {
  const artifact = authorRuntimeComposition(root());
  const malformed = JSON.parse(artifact.canonicalJson) as Record<string, unknown>;
  malformed['schedule'] = (malformed['schedule'] as unknown[]).slice(0, 4);
  expectError(() => admitCompiledComposition(malformed), 'invalid-schedule-phase');
  const unknown = JSON.parse(artifact.canonicalJson) as Record<string, unknown>;
  (unknown['schedule'] as Record<string, unknown>[])[1]!['unexpected'] = true;
  expectError(() => admitCompiledComposition(unknown), 'unknown-field');
  expectError(() => authorRuntimeComposition({ ...root(), schedule: { input: append(Standard.simulation) } }), 'invalid-schedule-phase');
  class ScheduleArray extends Array<SchedulePhaseDeclaration> {}
  expectError(() => authorRuntimeComposition({ ...root(), schedule: new ScheduleArray(...declarations()) }), 'invalid-json-value');
  const getterArray = declarations().slice();
  Object.defineProperty(getterArray, 0, { enumerable: true, get: () => declarations()[0] });
  expectError(() => authorRuntimeComposition({ ...root(), schedule: getterArray }), 'invalid-json-value');
});

test('composition operations preserve explicit phase provenance', () => {
  const before = prepend(Standard.input, system('before-input'));
  const extended = extend(Standard.simulation, { before: [system('before-simulation')], after: [system('after-simulation')] });
  const replaced = replace(Standard.projection, system('projection-only'));
  assert.equal(before.mode, 'prepend');
  if (extended.mode === 'extend') {
    assert.deepEqual(extended.before.map((entry) => entry.id), ['before-simulation']);
    assert.deepEqual(extended.after.map((entry) => entry.id), ['after-simulation']);
  }
  if (replaced.mode === 'replace') assert.deepEqual(replaced.systems.map((entry) => entry.id), ['projection-only']);
});

test('capability and identity checks remain build-time and payloads are immutable', () => {
  const artifact = authorRuntimeComposition(root());
  const simulation = artifact.composition.schedule[1];
  const movement = simulation?.mode === 'extend' ? undefined : simulation?.systems[0];
  assert.ok(movement);
  assert.ok(Object.isFrozen(movement));
  assert.ok(Object.isFrozen(movement.reads));
  expectError(() => authorRuntimeComposition({ ...root(), capabilities: [] }), 'unknown-capability');
  const bad = JSON.parse(artifact.canonicalJson) as Record<string, unknown>;
  ((((bad['schedule'] as Record<string, unknown>[])[1]!['systems'] as Record<string, unknown>[])[0]!))['after'] = ['missing'];
  expectError(() => admitCompiledComposition(bad), 'unknown-schedule-dependency');
});

function minimumArtifact() {
  return authorRuntimeComposition(minimumDraft());
}

function minimumDraft() {
  return {
    product: 'example.product',
    capabilities: [
      kernelCapability('camera.look', 'camera-look'),
      kernelCapability('movement.apply', 'apply-movement'),
      engineCapability('projection.refresh', 'render.entity-project'),
      kernelCapability('timeline.start', 'start-timeline'),
    ],
    intentDescriptors: [
      productIntent({ id: 'look', valueKind: 'axis', capability: 'camera.look', payload: { axes: ['x', 'y'] } }),
    ],
    inputMap: [
      inputAction({ id: 'look', intent: 'look', trigger: { kind: 'pointer-axis', axis: 'x' } }),
    ],
    schedule: schedule({
      simulation: phase('simulation', [{
        id: 'movement', capability: 'movement.apply', definition: 'player',
        reads: ['input.motion', 'state.transform'], writes: ['state.transform'], payload: { order: 1 },
      }]),
      projection: phase('projection', [{
        id: 'render-projection', capability: 'projection.refresh',
        reads: ['entity-state.projection'], writes: ['render-frame.diff'], payload: null,
      }]),
    }),
    gameplayDefinitions: [
      gameplayDefinition('player', gameplayCatalog({ kind: 'opaque-product-definition', stats: { health: 100 } })),
    ],
    timelines: [
      timeline('intro', [timelineStep({ id: 'start', capability: 'timeline.start', payload: { scene: 'opening' } })]),
    ],
  };
}
