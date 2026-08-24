import { strict as assert } from 'node:assert';
import { readFile } from 'node:fs/promises';
import { test } from 'node:test';

import {
  RuntimeCompositionAuthoringError,
  admitCompiledComposition,
  append,
  authorRuntimeComposition,
  cadence,
  compiledCompositionBytes,
  engineCapability,
  extend,
  fragment,
  gameplayCatalog,
  gameplayDefinition,
  inputAction,
  kernelCapability,
  phase,
  prepend,
  replace,
  timeline,
  timelineStep,
} from './index.js';

const fixtureUrl = new URL(
  '../../../../fixtures/product-model/minimum.compiled-composition.json',
  import.meta.url,
);
const canonicalNumbersFixtureUrl = new URL(
  '../../../../fixtures/product-model/canonical-numbers.expected.compiled-composition.json',
  import.meta.url,
);

test('typed Runtime Composition authoring emits the exact Rust-owned current fixture', async () => {
  const artifact = minimumArtifact();
  const fixture = await readFile(fixtureUrl, 'utf8');
  assert.equal(artifact.canonicalJson, fixture);
  assert.deepEqual(compiledCompositionBytes(artifact), new TextEncoder().encode(fixture));
  assert.ok(Object.isFrozen(artifact));
  assert.ok(Object.isFrozen(artifact.composition));
  assert.ok(Object.isFrozen(artifact.composition.schedule));
  assert.equal(artifact.composition.schedule[0]?.reads[0], 'input.motion');
  assert.equal(artifact.composition.schedule[1]?.writes.length, 0);
});

test('authoring materializes only detached plain data', () => {
  const artifact = minimumArtifact();
  const left = compiledCompositionBytes(artifact);
  left[0] = 0;
  const right = compiledCompositionBytes(artifact);
  assert.notEqual(right[0], 0);
  assert.equal(JSON.parse(artifact.canonicalJson).schedule[0].phase, 'simulation');

  for (const value of [
    () => 1,
    undefined,
    1n,
    Symbol('symbol'),
    Number.NaN,
    Number.POSITIVE_INFINITY,
    9_007_199_254_740_992,
  ]) {
    expectError(() => inputAction({ id: 'bad', intent: 'bad', capability: 'camera.look', payload: value }), 'invalid-json-value');
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
    inputMap: [],
    gameplayDefinitions: [gameplayDefinition('numeric', {
      '2': 'two', '10': 'ten', '1': 'one', small: 1e-6, tiny: 1.2e-6, negativeZero: -0,
    })],
    schedule: [],
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
  ((unknownCapability['inputMap'] as { capability: string }[])[0] as { capability: string }).capability = 'missing.capability';
  expectError(() => admitCompiledComposition(unknownCapability), 'unknown-capability');
  const missingReads = structuredClone(minimumArtifact().composition) as unknown as Record<string, unknown>;
  delete ((missingReads['schedule'] as Record<string, unknown>[])[0] as Record<string, unknown>)['reads'];
  expectError(() => admitCompiledComposition(missingReads), 'missing-field');
  const duplicateAccess = structuredClone(minimumArtifact().composition) as unknown as Record<string, unknown>;
  ((duplicateAccess['schedule'] as Record<string, unknown>[])[0] as Record<string, unknown>)['reads'] = ['state.transform', 'state.transform'];
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
    ((malformed['capabilityBindings'] as Record<string, unknown>[])[0] as Record<string, unknown>)['target'] = target;
    expectError(() => admitCompiledComposition(malformed), target.startsWith('engine.') ? 'invalid-identity' : 'invalid-capability-target');
  }
});

test('append and prepend have distinct ordered semantics while extend rejects accidental replacement', () => {
  const base = minimumArtifact().composition;
  const additions = fragment({
    inputMap: [inputAction({ id: 'fire', intent: 'fire', capability: 'movement.apply', payload: null })],
    schedule: phase('simulation', [{
      id: 'after-movement', capability: 'movement.apply', reads: ['state.transform'], writes: [], payload: { order: 2 },
    }]).schedule,
  });
  const appended = append(base, additions);
  const prepended = prepend(base, additions);
  assert.deepEqual(appended.inputMap.map((entry) => entry.id), ['look', 'fire']);
  assert.deepEqual(prepended.inputMap.map((entry) => entry.id), ['fire', 'look']);
  assert.deepEqual(appended.schedule.map((entry) => entry.id), ['movement', 'render-projection', 'after-movement']);
  assert.deepEqual(prepended.schedule.map((entry) => entry.id), ['after-movement', 'movement', 'render-projection']);
  expectError(() => extend(base, fragment({ inputMap: [inputAction({ id: 'look', intent: 'look', capability: 'camera.look', payload: null })] })), 'duplicate-entry');
});

test('replace changes only explicitly named whole collections and cadence does not fabricate an absent wire field', () => {
  const base = minimumArtifact().composition;
  const replaced = replace(base, {
    timelines: [timeline('outro', [timelineStep({ id: 'finish', capability: 'timeline.start', payload: { scene: 'ending' } })])],
  });
  assert.deepEqual(replaced.timelines.map((entry) => entry.id), ['outro']);
  assert.deepEqual(replaced.schedule, base.schedule);
  expectError(() => cadence({ every: 60 }), 'unrepresentable-cadence');
});

function minimumArtifact() {
  return authorRuntimeComposition(minimumDraft());
}

function minimumDraft() {
  return {
    product: 'example.product',
    capabilities: [
      engineCapability('camera.look', 'camera-look'),
      kernelCapability('movement.apply', 'apply-movement'),
      engineCapability('projection.refresh', 'refresh-projection'),
      kernelCapability('timeline.start', 'start-timeline'),
    ],
    inputMap: [
      inputAction({ id: 'look', intent: 'look', capability: 'camera.look', payload: { axes: ['x', 'y'] } }),
    ],
    schedule: [
      ...phase('simulation', [{
        id: 'movement', capability: 'movement.apply', definition: 'player',
        reads: ['input.motion', 'state.transform'], writes: ['state.transform'], payload: { order: 1 },
      }]).schedule,
      ...phase('projection', [{
        id: 'render-projection', capability: 'projection.refresh',
        reads: ['state.transform'], writes: [], payload: null,
      }]).schedule,
    ],
    gameplayDefinitions: [
      gameplayDefinition('player', gameplayCatalog({ kind: 'opaque-product-definition', stats: { health: 100 } })),
    ],
    timelines: [
      timeline('intro', [timelineStep({ id: 'start', capability: 'timeline.start', payload: { scene: 'opening' } })]),
    ],
  };
}

function expectError(action: () => unknown, code: RuntimeCompositionAuthoringError['code']): void {
  assert.throws(action, (error: unknown) => error instanceof RuntimeCompositionAuthoringError && error.code === code);
}
