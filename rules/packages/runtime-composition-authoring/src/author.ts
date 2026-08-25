import { RuntimeCompositionAuthoringError } from './error.js';
import {
  MAX_OPAQUE_JSON_NODES,
  compareUtf8,
  normalizeOpaqueJson,
  utf8Length,
  writeCanonicalJson,
} from './json.js';
import {
  PRODUCT_MODEL_CAPABILITY_CATALOG,
  PRODUCT_MODEL_CAPABILITY_TARGETS,
  PRODUCT_MODEL_FIELDS,
  PRODUCT_MODEL_IDENTITY,
  PRODUCT_MODEL_INPUT,
  PRODUCT_MODEL_LIMITS,
  PRODUCT_MODEL_SCHEDULE,
} from './generated.js';
import type { EngineCapabilityName } from './generated.js';
import type {
  CapabilityBinding,
  CompiledComposition,
  CompositionFragment,
  CompositionReplacement,
  GameplayDefinition,
  InputActionDraft,
  InputEdge,
  InputMapEntry,
  InputTrigger,
  JsonValue,
  KeyboardControl,
  ProductIntentDescriptor,
  ProductIntentDescriptorDraft,
  RuntimeCompositionArtifact,
  RuntimeCompositionDraft,
  ScheduleActionDraft,
  ScheduleCadence,
  ScheduleCompositionMode,
  ScheduleDraft,
  SchedulePhase,
  SchedulePhaseDeclaration,
  SchedulePlacement,
  ScheduleSystem,
  ScheduleEntryDraft,
  StandardPhase,
  Timeline,
  TimelineStep,
  TimelineStepDraft,
} from './types.js';

export const MAX_COMPILED_COMPOSITION_BYTES = PRODUCT_MODEL_LIMITS.maximumEncodedBytes;
export const MAX_INPUT_MAP_ENTRIES = PRODUCT_MODEL_LIMITS.maximumInputMapEntries;
export const MAX_INTENT_DESCRIPTORS = PRODUCT_MODEL_LIMITS.maximumIntentDescriptors;
export const MAX_INPUT_CHORD_CONTROLS = PRODUCT_MODEL_LIMITS.maximumInputChordControls;
export const MAX_SCHEDULE_ENTRIES = PRODUCT_MODEL_LIMITS.maximumScheduleEntries;
export const MAX_GAMEPLAY_DEFINITIONS = PRODUCT_MODEL_LIMITS.maximumGameplayDefinitions;
export const MAX_TIMELINES = PRODUCT_MODEL_LIMITS.maximumTimelines;
export const MAX_TIMELINE_STEPS = PRODUCT_MODEL_LIMITS.maximumTimelineSteps;
export const MAX_CAPABILITY_BINDINGS = PRODUCT_MODEL_LIMITS.maximumCapabilityBindings;
export const MAX_SCHEDULE_RESOURCE_DECLARATIONS = PRODUCT_MODEL_LIMITS.maximumScheduleAccessDeclarations;
export const MAX_SCHEDULE_DEPENDENCIES = PRODUCT_MODEL_LIMITS.maximumScheduleDependencies;

const IDENTITY = /^[a-z0-9](?:[a-z0-9]|[._-](?=[a-z0-9]))*$/;

/**
 * Admits a typed authoring draft into the exact current Rust-owned wire shape.
 * It materializes only frozen plain JSON data; it never evaluates game code.
 */
export function authorRuntimeComposition(draft: unknown): RuntimeCompositionArtifact {
  const source = admitDraft(draft, '$');
  const composition = admitCompiledComposition({
    product: source.product,
    intentDescriptors: source.intentDescriptors ?? [],
    inputMap: source.inputMap ?? [],
    schedule: source.schedule ?? schedule({}),
    gameplayDefinitions: source.gameplayDefinitions ?? [],
    timelines: source.timelines ?? [],
    capabilityBindings: source.capabilities,
  });
  const canonicalJson = `${writeCompiledComposition(composition)}\n`;
  const bytes = new TextEncoder().encode(canonicalJson);
  if (bytes.length > MAX_COMPILED_COMPOSITION_BYTES) {
    throw new RuntimeCompositionAuthoringError(
      'artifact-quota-exceeded', '$',
      `compiled composition exceeds ${String(MAX_COMPILED_COMPOSITION_BYTES)} bytes`,
    );
  }
  return Object.freeze({ composition, canonicalJson });
}

/** Returns a fresh UTF-8 byte view so callers cannot mutate the artifact. */
export function compiledCompositionBytes(artifact: RuntimeCompositionArtifact): Uint8Array {
  return new TextEncoder().encode(artifact.canonicalJson);
}

/** Admits a direct current-schema value for use by explicit composition transforms. */
export function admitCompiledComposition(value: unknown): CompiledComposition {
  const root = record(value, '$');
  known(root, PRODUCT_MODEL_FIELDS.compiledComposition, '$');
  const product = identity(requiredString(root, 'product', '$'), '$.product');
  const capabilityBindings = admitCapabilities(requiredArray(root, 'capabilityBindings', '$'), '$.capabilityBindings');
  const capabilityIds = new Set(capabilityBindings.map((entry) => entry.id));
  const budget = { nodes: 0 };
  const intentDescriptors = admitIntentDescriptors(requiredArray(root, 'intentDescriptors', '$'), '$.intentDescriptors', capabilityIds, budget);
  const intentKinds = new Map(intentDescriptors.map((entry) => [entry.id, entry.valueKind]));
  const definitions = admitDefinitions(requiredArray(root, 'gameplayDefinitions', '$'), '$.gameplayDefinitions', budget);
  const definitionIds = new Set(definitions.map((entry) => entry.id));
  const inputMap = admitInputMap(requiredArray(root, 'inputMap', '$'), '$.inputMap', intentKinds);
  const schedule = admitSchedule(requiredArray(root, 'schedule', '$'), '$.schedule', capabilityIds, definitionIds, budget);
  const timelines = admitTimelines(requiredArray(root, 'timelines', '$'), '$.timelines', capabilityIds, budget);
  return freezeComposition({ product, intentDescriptors, inputMap, schedule, gameplayDefinitions: definitions, timelines, capabilityBindings });
}

/** Creates an engine-owned capability reference without inventing a runtime registry. */
export function engineCapability(id: string, target: EngineCapabilityName): CapabilityBinding {
  const fullTarget = `engine.${target}`;
  if (!PRODUCT_MODEL_CAPABILITY_CATALOG.engine.some((descriptor) => descriptor.target === fullTarget)) {
    fail('unknown-engine-capability', '$.target', `Engine capability ${fullTarget} is not in the generated closed catalog`);
  }
  return capability(id, `engine.${identity(target, '$.target')}`);
}

/** Creates a downstream-kernel capability reference without evaluating it. */
export function kernelCapability(id: string, target: string): CapabilityBinding {
  return capability(id, `kernel.${identity(target, '$.target')}`);
}

/** Names an intent; it is a schema-checked identity, not an event bus route. */
export function intent(id: string): string { return identity(id, '$.intent'); }

/** Describes one typed intent with an optional legacy capability target. */
export function productIntent(draft: ProductIntentDescriptorDraft): ProductIntentDescriptor {
  return admitIntentDescriptor(draft, '$.productIntent', undefined, { nodes: 0 });
}

/** Builds one physical mapping into a previously described product intent. */
export function inputAction(draft: InputActionDraft): InputMapEntry {
  const source = record(draft, '$.inputAction');
  known(source, ['id', 'intent', 'trigger'], '$.inputAction');
  const trigger = admitInputTrigger(required(source, 'trigger', '$.inputAction'), '$.inputAction.trigger');
  return freezeInput({
    id: identity(requiredString(source, 'id', '$.inputAction'), '$.inputAction.id'),
    intent: identity(requiredString(source, 'intent', '$.inputAction'), '$.inputAction.intent'),
    trigger,
  });
}

const SCHEDULE_PHASES: readonly SchedulePhase[] = PRODUCT_MODEL_SCHEDULE.phases;

/** The implicit engine-owned anchors that products compose explicitly. */
export const Standard: Readonly<{ readonly [P in SchedulePhase]: StandardPhase }> = Object.freeze({
  input: Object.freeze({ kind: 'standard', phase: 'input' }),
  simulation: Object.freeze({ kind: 'standard', phase: 'simulation' }),
  consequences: Object.freeze({ kind: 'standard', phase: 'consequences' }),
  commit: Object.freeze({ kind: 'standard', phase: 'commit' }),
  projection: Object.freeze({ kind: 'standard', phase: 'projection' }),
});

/** Creates one build-time schedule system declaration. */
export function system(id: string, options: Omit<ScheduleActionDraft, 'id'> = {}): ScheduleSystem {
  const source = record(options, '$.system');
  known(source, ['capability', 'definition', 'after', 'reads', 'writes', 'cadence', 'payload'], '$.system');
  return freezeScheduleSystem({
    id: identity(id, '$.system.id'),
    capability: identity(source['capability'] === undefined ? id : requiredString(source, 'capability', '$.system'), '$.system.capability'),
    ...(source['definition'] === undefined ? {} : { definition: identity(requiredString(source, 'definition', '$.system'), '$.system.definition') }),
    after: identities(source['after'] === undefined ? [] : requiredArray(source, 'after', '$.system'), '$.system.after', MAX_SCHEDULE_DEPENDENCIES),
    reads: identities(source['reads'] === undefined ? [] : requiredArray(source, 'reads', '$.system'), '$.system.reads'),
    writes: identities(source['writes'] === undefined ? [] : requiredArray(source, 'writes', '$.system'), '$.system.writes'),
    cadence: admitCadence(source['cadence'] === undefined ? { everySteps: 1, offsetSteps: 0 } : required(source, 'cadence', '$.system'), '$.system.cadence'),
    payload: normalizeOpaqueJson(source['payload'] === undefined ? null : required(source, 'payload', '$.system'), '$.system.payload'),
  });
}

/** Creates an exact step cadence. No wall-clock or frequency conversion is admitted. */
export function cadence(value: number | ScheduleCadence | { readonly everySteps: number; readonly offsetSteps?: number }, offsetSteps = 0): ScheduleCadence {
  const source = typeof value === 'number' ? { everySteps: value, offsetSteps } : value;
  return admitCadence(source, '$.cadence');
}

/** Builds a phase-local append/prepend/extend/replace declaration. */
export function append(anchor: StandardPhase, ...systems: readonly ScheduleSystem[]): SchedulePhaseDeclaration {
  return composeSchedule(anchor, 'append', systems, '$.append.anchor');
}
export function prepend(anchor: StandardPhase, ...systems: readonly ScheduleSystem[]): SchedulePhaseDeclaration {
  return composeSchedule(anchor, 'prepend', systems, '$.prepend.anchor');
}
export function extend(anchor: StandardPhase, value: { readonly before: readonly ScheduleSystem[]; readonly after: readonly ScheduleSystem[] }): SchedulePhaseDeclaration {
  if (value === null || typeof value !== 'object') fail('invalid-schedule-mode', '$.extend', 'extend requires before and after system arrays');
  const source = record(value, '$.extend');
  known(source, ['before', 'after'], '$.extend');
  const phase = standardAnchor(anchor, '$.extend.anchor');
  return Object.freeze({
    phase,
    mode: 'extend' as const,
    before: freezeList(requiredArray(source, 'before', '$.extend').map((entry, index) => admitScheduleSystem(entry, `$.extend.before[${String(index)}]`))),
    after: freezeList(requiredArray(source, 'after', '$.extend').map((entry, index) => admitScheduleSystem(entry, `$.extend.after[${String(index)}]`))),
  });
}
export function replace(anchor: StandardPhase, ...systems: readonly ScheduleSystem[]): SchedulePhaseDeclaration {
  return composeSchedule(anchor, 'replace', systems, '$.replace.anchor');
}

/** Lowers a named phase map into the exact five-phase wire array. */
export function schedule(value: ScheduleDraft): readonly SchedulePhaseDeclaration[] {
  const source = record(value, '$.schedule');
  known(source, SCHEDULE_PHASES, '$.schedule');
  return freezeList(SCHEDULE_PHASES.map((phaseName) => {
    const declaration = source[phaseName];
    if (declaration === undefined) return composeSchedule(Standard[phaseName], 'append', [], `$.schedule.${phaseName}.anchor`);
    const normalized = admitSchedulePhase(declaration, `$.schedule.${phaseName}`);
    if (normalized.phase !== phaseName) fail('invalid-schedule-phase', `$.schedule.${phaseName}.phase`, `schedule map key ${phaseName} does not match declaration phase ${normalized.phase}`);
    return normalized;
  }));
}

/** Convenience alias for one system declaration during source migration. */
export function scheduleAction(draft: ScheduleEntryDraft): ScheduleSystem {
  return system(draft.id, draft);
}

/** Builds an append declaration for one named phase. */
export function phase(phaseId: SchedulePhase, actions: readonly ScheduleActionDraft[]): SchedulePhaseDeclaration {
  if (!SCHEDULE_PHASES.includes(phaseId)) fail('invalid-schedule-phase', '$.phase', `unknown schedule phase ${phaseId}`);
  const values = arrayData(actions, '$.phase.actions');
  return append(Standard[phaseId], ...values.map((entry, index) => {
    const action = record(entry, `$.phase.actions[${String(index)}]`);
    const id = requiredString(action, 'id', `$.phase.actions[${String(index)}]`);
    const { id: _ignoredId, ...options } = action;
    return system(id, options);
  }));
}

/** Declares opaque product data under one gameplay-definition identity. */
export function gameplayDefinition(id: string, payload: unknown): GameplayDefinition {
  return freezeDefinition({ id: identity(id, '$.gameplayDefinition.id'), payload: normalizeOpaqueJson(payload, '$.gameplayDefinition.payload') });
}

/**
 * Makes an opaque, deterministic catalog payload. Catalog meaning remains with
 * the product; this helper only guarantees it is safe JSON data.
 */
export function gameplayCatalog(entries: Readonly<Record<string, unknown>>): JsonValue {
  return normalizeOpaqueJson(entries, '$.gameplayCatalog');
}

/** Creates an ordered timeline declaration. Its steps are descriptive data, not a scheduler. */
export function timeline(id: string, steps: readonly TimelineStep[]): Timeline {
  return admitTimeline({ id, steps }, '$.timeline', undefined, { nodes: 0 });
}

/** Creates one ordered timeline step. */
export function timelineStep(draft: TimelineStepDraft): TimelineStep {
  const source = record(draft, '$.timelineStep');
  known(source, ['id', 'capability', 'payload'], '$.timelineStep');
  return freezeTimelineStep({
    id: identity(requiredString(source, 'id', '$.timelineStep'), '$.timelineStep.id'),
    capability: identity(requiredString(source, 'capability', '$.timelineStep'), '$.timelineStep.capability'),
    payload: normalizeOpaqueJson(required(source, 'payload', '$.timelineStep'), '$.timelineStep.payload'),
  });
}

/** Produces a normalized partial collection set for later composition. */
export function fragment(value: Partial<CompositionFragment>): CompositionFragment {
  const source = record(value, '$.fragment');
  known(source, ['intentDescriptors', 'inputMap', 'gameplayDefinitions', 'timelines', 'capabilityBindings'], '$.fragment');
  return Object.freeze({
    intentDescriptors: freezeList(optionalArray(source, 'intentDescriptors', '$.fragment').map((entry, index) => admitIntentDescriptor(entry, `$.fragment.intentDescriptors[${String(index)}]`, undefined, { nodes: 0 }))),
    inputMap: freezeList(optionalArray(source, 'inputMap', '$.fragment').map((entry, index) => admitInputMapEntry(entry, `$.fragment.inputMap[${String(index)}]`, undefined))),
    gameplayDefinitions: freezeList(optionalArray(source, 'gameplayDefinitions', '$.fragment').map((entry, index) => admitDefinition(entry, `$.fragment.gameplayDefinitions[${String(index)}]`, { nodes: 0 }))),
    timelines: freezeList(optionalArray(source, 'timelines', '$.fragment').map((entry, index) => admitTimeline(entry, `$.fragment.timelines[${String(index)}]`, undefined, { nodes: 0 }))),
    capabilityBindings: freezeList(optionalArray(source, 'capabilityBindings', '$.fragment').map((entry, index) => admitCapability(entry, `$.fragment.capabilityBindings[${String(index)}]`))),
  });
}

/** Appends each supplied fragment in argument order, preserving every collection order. */
export function appendComposition(base: CompiledComposition, ...fragments: readonly CompositionFragment[]): CompiledComposition {
  return composeFragments(base, fragments, false, false);
}

/** Prepends fragments in argument order; the first fragment becomes the earliest declaration. */
export function prependComposition(base: CompiledComposition, ...fragments: readonly CompositionFragment[]): CompiledComposition {
  return composeFragments(base, fragments, true, false);
}

/** Adds only new identities. It rejects collision rather than silently replacing authored data. */
export function extendComposition(base: CompiledComposition, ...fragments: readonly CompositionFragment[]): CompiledComposition {
  return composeFragments(base, fragments, false, true);
}

/** Replaces exactly the named whole collections. It cannot change the product identity. */
export function replaceComposition(base: CompiledComposition, replacement: CompositionReplacement): CompiledComposition {
  const source = record(replacement, '$.replacement');
  known(source, ['intentDescriptors', 'inputMap', 'schedule', 'gameplayDefinitions', 'timelines', 'capabilityBindings'], '$.replacement');
  if (Object.keys(source).length === 0) fail('invalid-operation', '$.replacement', 'replace requires at least one collection');
  return admitCompiledComposition({
    product: base.product,
    intentDescriptors: source['intentDescriptors'] === undefined ? base.intentDescriptors : source['intentDescriptors'],
    inputMap: source['inputMap'] === undefined ? base.inputMap : source['inputMap'],
    schedule: source['schedule'] === undefined ? base.schedule : source['schedule'],
    gameplayDefinitions: source['gameplayDefinitions'] === undefined ? base.gameplayDefinitions : source['gameplayDefinitions'],
    timelines: source['timelines'] === undefined ? base.timelines : source['timelines'],
    capabilityBindings: source['capabilityBindings'] === undefined ? base.capabilityBindings : source['capabilityBindings'],
  });
}

function composeFragments(base: CompiledComposition, fragments: readonly CompositionFragment[], prependFragments: boolean, requireNew: boolean): CompiledComposition {
  const normalizedBase = admitCompiledComposition(base);
  const normalized = fragments.map((entry, index) => fragment(entry));
  if (requireNew) {
    assertNewIdentities('intentDescriptors', normalizedBase.intentDescriptors, normalized.flatMap((entry) => entry.intentDescriptors));
    assertNewIdentities('inputMap', normalizedBase.inputMap, normalized.flatMap((entry) => entry.inputMap));
    assertNewIdentities('gameplayDefinitions', normalizedBase.gameplayDefinitions, normalized.flatMap((entry) => entry.gameplayDefinitions));
    assertNewIdentities('timelines', normalizedBase.timelines, normalized.flatMap((entry) => entry.timelines));
    assertNewIdentities('capabilityBindings', normalizedBase.capabilityBindings, normalized.flatMap((entry) => entry.capabilityBindings));
  }
  const merge = <T>(existing: readonly T[], additions: readonly T[]): readonly T[] =>
    prependFragments ? [...additions, ...existing] : [...existing, ...additions];
  return admitCompiledComposition({
    product: normalizedBase.product,
    intentDescriptors: merge(normalizedBase.intentDescriptors, normalized.flatMap((entry) => entry.intentDescriptors)),
    inputMap: merge(normalizedBase.inputMap, normalized.flatMap((entry) => entry.inputMap)),
    schedule: normalizedBase.schedule,
    gameplayDefinitions: merge(normalizedBase.gameplayDefinitions, normalized.flatMap((entry) => entry.gameplayDefinitions)),
    timelines: merge(normalizedBase.timelines, normalized.flatMap((entry) => entry.timelines)),
    capabilityBindings: merge(normalizedBase.capabilityBindings, normalized.flatMap((entry) => entry.capabilityBindings)),
  });
}

function admitDraft(value: unknown, path: string): RuntimeCompositionDraft {
  const source = record(value, path);
  known(source, ['product', 'capabilities', 'intentDescriptors', 'inputMap', 'schedule', 'gameplayDefinitions', 'timelines'], path);
  return Object.freeze({
    product: identity(requiredString(source, 'product', path), `${path}.product`),
    capabilities: freezeList(requiredArray(source, 'capabilities', path).map((entry, index) => admitCapability(entry, `${path}.capabilities[${String(index)}]`))),
    ...(source['intentDescriptors'] === undefined ? {} : { intentDescriptors: freezeList(requiredArray(source, 'intentDescriptors', path).map((entry, index) => admitIntentDescriptor(entry, `${path}.intentDescriptors[${String(index)}]`, undefined, { nodes: 0 }))) }),
    ...(source['inputMap'] === undefined ? {} : { inputMap: freezeList(requiredArray(source, 'inputMap', path).map((entry, index) => admitInputMapEntry(entry, `${path}.inputMap[${String(index)}]`, undefined))) }),
    ...(source['schedule'] === undefined ? {} : { schedule: normalizeScheduleDraft(required(source, 'schedule', path), `${path}.schedule`) }),
    ...(source['gameplayDefinitions'] === undefined ? {} : { gameplayDefinitions: freezeList(requiredArray(source, 'gameplayDefinitions', path).map((entry, index) => admitDefinition(entry, `${path}.gameplayDefinitions[${String(index)}]`, { nodes: 0 }))) }),
    ...(source['timelines'] === undefined ? {} : { timelines: freezeList(requiredArray(source, 'timelines', path).map((entry, index) => admitTimeline(entry, `${path}.timelines[${String(index)}]`, undefined, { nodes: 0 }))) }),
  });
}

function admitCapabilities(values: readonly unknown[], path: string): readonly CapabilityBinding[] {
  quota(values.length, MAX_CAPABILITY_BINDINGS, path);
  const output = values.map((entry, index) => admitCapability(entry, `${path}[${String(index)}]`));
  unique(output, path);
  return freezeList(output);
}

function admitCapability(value: unknown, path: string): CapabilityBinding {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.capabilityBinding, path);
  const target = requiredString(source, 'target', path);
  const separator = PRODUCT_MODEL_CAPABILITY_TARGETS.separator;
  const separatorIndex = target.indexOf(separator);
  const namespace = separatorIndex < 0 ? '' : target.slice(0, separatorIndex);
  const local = separatorIndex < 0 ? '' : target.slice(separatorIndex + separator.length);
  if (!PRODUCT_MODEL_CAPABILITY_TARGETS.namespaces.includes(namespace as 'engine' | 'kernel')) {
    fail('invalid-capability-target', `${path}.target`, 'capability targets must use engine.<id> or kernel.<id>');
  }
  return Object.freeze({
    id: identity(requiredString(source, 'id', path), `${path}.id`),
    target: `${namespace}${separator}${identity(local, `${path}.target`)}`,
  });
}

function capability(id: string, target: string): CapabilityBinding {
  return Object.freeze({ id: identity(id, '$.capability.id'), target });
}

function admitIntentDescriptors(values: readonly unknown[], path: string, capabilities: ReadonlySet<string> | undefined, budget: JsonState): readonly ProductIntentDescriptor[] {
  quota(values.length, MAX_INTENT_DESCRIPTORS, path);
  const output = values.map((entry, index) => admitIntentDescriptor(entry, `${path}[${String(index)}]`, capabilities, budget));
  unique(output, path);
  return freezeList(output);
}

function admitIntentDescriptor(value: unknown, path: string, capabilities: ReadonlySet<string> | undefined, budget: JsonState): ProductIntentDescriptor {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.intentDescriptor, path);
  const capability = source['capability'] === undefined
    ? undefined
    : identity(requiredString(source, 'capability', path), `${path}.capability`);
  if (capability !== undefined && capabilities !== undefined && !capabilities.has(capability)) fail('unknown-capability', `${path}.capability`, `capability ${capability} is not declared`);
  const valueKind = requiredString(source, 'valueKind', path);
  if (!(PRODUCT_MODEL_INPUT.intentValueKinds as readonly string[]).includes(valueKind)) fail('invalid-input-value-kind', `${path}.valueKind`, 'intent valueKind must be digital, axis, or product-payload');
  const payloadContract = source['payloadContract'];
  if (valueKind === 'product-payload') {
    if (payloadContract === undefined) fail('product-payload-contract-required', `${path}.payloadContract`, 'product-payload intents require one stable payloadContract identity');
  } else if (payloadContract !== undefined) {
    fail('product-payload-contract-unexpected', `${path}.payloadContract`, 'payloadContract is valid only for product-payload intents');
  }
  return Object.freeze({
    id: identity(requiredString(source, 'id', path), `${path}.id`),
    valueKind: valueKind as ProductIntentDescriptor['valueKind'],
    ...(payloadContract === undefined ? {} : { payloadContract: identity(requiredString(source, 'payloadContract', path), `${path}.payloadContract`) }),
    ...(capability === undefined ? {} : { capability }),
    payload: normalizeWithBudget(required(source, 'payload', path), `${path}.payload`, budget),
  });
}

function admitInputMap(values: readonly unknown[], path: string, intents: ReadonlyMap<string, ProductIntentDescriptor['valueKind']>): readonly InputMapEntry[] {
  quota(values.length, MAX_INPUT_MAP_ENTRIES, path);
  const output = values.map((entry, index) => admitInputMapEntry(entry, `${path}[${String(index)}]`, intents));
  unique(output, path);
  return freezeList(output);
}

function admitInputMapEntry(value: unknown, path: string, intents: ReadonlyMap<string, ProductIntentDescriptor['valueKind']> | undefined): InputMapEntry {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.inputMap, path);
  const intent = identity(requiredString(source, 'intent', path), `${path}.intent`);
  const trigger = admitInputTrigger(required(source, 'trigger', path), `${path}.trigger`);
  const expected = intents?.get(intent);
  if (intents !== undefined && expected === undefined) fail('unknown-intent-descriptor', `${path}.intent`, `input mapping intent ${intent} is not declared`);
  if (expected === 'product-payload') fail('physical-product-payload-forbidden', `${path}.intent`, `input mapping cannot target direct-UI-only product-payload intent ${intent}`);
  if (expected !== undefined && expected !== triggerValueKind(trigger)) fail('input-trigger-value-kind', `${path}.trigger`, `input trigger produces ${triggerValueKind(trigger)} but intent ${intent} requires ${expected}`);
  return freezeInput({
    id: identity(requiredString(source, 'id', path), `${path}.id`),
    intent,
    trigger,
  });
}

function admitInputTrigger(value: unknown, path: string): InputTrigger {
  const source = record(value, path);
  const kind = requiredString(source, 'kind', path);
  const context = source['context'] === undefined ? undefined : identity(requiredString(source, 'context', path), `${path}.context`);
  const copyContext = context === undefined ? {} : { context };
  if (kind === 'key') {
    known(source, ['kind', 'code', 'edge', 'chord', 'context'], path);
    const code = listedString(source, 'code', path, PRODUCT_MODEL_INPUT.keyboardControls);
    const edge = listedString(source, 'edge', path, PRODUCT_MODEL_INPUT.edges);
    const chord = source['chord'] === undefined ? [] : requiredArray(source, 'chord', path).map((entry, index) => listed(entry, `${path}.chord[${String(index)}]`, PRODUCT_MODEL_INPUT.keyboardControls));
    if (chord.length > MAX_INPUT_CHORD_CONTROLS) fail('input-chord-quota-exceeded', `${path}.chord`, `input chords contain at most ${String(MAX_INPUT_CHORD_CONTROLS)} controls`);
    if (new Set(chord).size !== chord.length) fail('duplicate-input-chord-control', `${path}.chord`, 'input chord controls must be unique');
    return Object.freeze({ kind: 'key' as const, code: code as KeyboardControl, edge: edge as InputEdge, ...(chord.length === 0 ? {} : { chord: freezeList(chord as KeyboardControl[]) }), ...copyContext }) as InputTrigger;
  }
  if (kind === 'pointer-button') {
    known(source, ['kind', 'button', 'edge', 'context'], path);
    return Object.freeze({ kind, button: listedString(source, 'button', path, PRODUCT_MODEL_INPUT.pointerButtons) as never, edge: listedString(source, 'edge', path, PRODUCT_MODEL_INPUT.edges) as never, ...copyContext }) as InputTrigger;
  }
  if (kind === 'pointer-axis' || kind === 'wheel') {
    known(source, ['kind', 'axis', 'context'], path);
    return Object.freeze({ kind, axis: listedString(source, 'axis', path, PRODUCT_MODEL_INPUT.axes) as never, ...copyContext }) as InputTrigger;
  }
  if (kind === 'controller-button') {
    known(source, ['kind', 'button', 'edge', 'context'], path);
    return Object.freeze({ kind, button: listedString(source, 'button', path, PRODUCT_MODEL_INPUT.controllerButtons) as never, edge: listedString(source, 'edge', path, PRODUCT_MODEL_INPUT.edges) as never, ...copyContext }) as InputTrigger;
  }
  if (kind === 'controller-axis') {
    known(source, ['kind', 'axis', 'context'], path);
    return Object.freeze({ kind, axis: listedString(source, 'axis', path, PRODUCT_MODEL_INPUT.controllerAxes) as never, ...copyContext }) as InputTrigger;
  }
  fail('invalid-input-trigger', `${path}.kind`, 'input trigger kind is not in the generated closed grammar');
}

function listedString(source: Readonly<Record<string, unknown>>, name: string, path: string, values: readonly string[]): string {
  return listed(requiredString(source, name, path), `${path}.${name}`, values);
}

function listed(value: unknown, path: string, values: readonly string[]): string {
  if (typeof value !== 'string' || !values.includes(value)) fail('invalid-input-trigger', path, 'value is not in the generated closed input grammar');
  return value;
}

function triggerValueKind(trigger: InputTrigger): ProductIntentDescriptor['valueKind'] {
  return trigger.kind === 'key' || trigger.kind === 'pointer-button' || trigger.kind === 'controller-button' ? 'digital' : 'axis';
}

function normalizeScheduleDraft(value: unknown, path: string): readonly SchedulePhaseDeclaration[] {
  if (Array.isArray(value)) {
    return admitSchedule(arrayData(value, path), path, undefined, undefined, { nodes: 0 });
  }
  return schedule(value as ScheduleDraft);
}

function admitSchedule(values: readonly unknown[], path: string, capabilities: ReadonlySet<string> | undefined, definitions: ReadonlySet<string> | undefined, budget: JsonState): readonly SchedulePhaseDeclaration[] {
  if (values.length !== PRODUCT_MODEL_SCHEDULE.phases.length) fail('invalid-schedule-phase', path, `schedule must declare exactly ${String(PRODUCT_MODEL_SCHEDULE.phases.length)} phases`);
  const output = values.map((entry, index) => admitSchedulePhase(entry, `${path}[${String(index)}]`, capabilities, definitions, budget));
  const totalSystems = output.reduce((total, phase) => total + schedulePhaseSystems(phase).length, 0);
  quota(totalSystems, MAX_SCHEDULE_ENTRIES, path);
  for (const [index, entry] of output.entries()) {
    if (entry.phase !== PRODUCT_MODEL_SCHEDULE.phases[index]) fail('invalid-schedule-phase', `${path}[${String(index)}].phase`, `schedule phases must use canonical order; expected ${PRODUCT_MODEL_SCHEDULE.phases[index]}`);
  }
  const locations = new Map<string, { readonly phase: number; readonly placement: SchedulePlacement; readonly system: ScheduleSystem }>();
  const allSystems = output.flatMap((entry, phase) => schedulePhaseSystems(entry).map(({ system, placement }) => {
    if (locations.has(system.id)) fail('duplicate-entry', `${path}[${String(phase)}].${placementPath(placement, system.id)}.id`, `duplicate identity ${system.id}`);
    const location = { phase, placement, system };
    locations.set(system.id, location);
    return location;
  }));
  for (const location of allSystems) {
    for (const [dependencyIndex, dependency] of location.system.after.entries()) {
      const dependencyLocation = locations.get(dependency);
      const dependencyPath = `${path}[${String(location.phase)}].${placementPath(location.placement, location.system.id)}.after[${String(dependencyIndex)}]`;
      if (dependency === location.system.id) fail('schedule-dependency-cycle', dependencyPath, `system ${location.system.id} cannot depend on itself`);
      if (dependencyLocation === undefined) fail('unknown-schedule-dependency', dependencyPath, `system ${location.system.id} depends on undeclared system ${dependency}`);
      if (dependencyLocation.phase !== location.phase) fail('schedule-cross-phase-dependency', dependencyPath, 'schedule dependencies must remain within one phase');
      if (dependencyLocation.placement !== location.placement) fail('schedule-placement-dependency', dependencyPath, 'schedule dependencies cannot cross a composition placement partition');
    }
  }
  validateScheduleCycles(allSystems, path);
  validateScheduleAccessAmbiguity(allSystems, path);
  return freezeList(output);
}

function schedulePhaseSystems(phase: SchedulePhaseDeclaration): readonly { readonly system: ScheduleSystem; readonly placement: SchedulePlacement }[] {
  if (phase.mode === 'extend') {
    return [
      ...phase.before.map((system) => ({ system, placement: 'extend-before' as const })),
      ...phase.after.map((system) => ({ system, placement: 'extend-after' as const })),
    ];
  }
  return phase.systems.map((system) => ({ system, placement: phase.mode as SchedulePlacement }));
}

function placementPath(placement: SchedulePlacement, id: string): string {
  return placement === 'extend-before' ? `before[${id}]` : placement === 'extend-after' ? `after[${id}]` : `systems[${id}]`;
}

function validateScheduleCycles(
  systems: readonly { readonly system: ScheduleSystem; readonly placement: SchedulePlacement }[],
  path: string,
): void {
  const byId = new Map(systems.map((entry) => [entry.system.id, entry.system]));
  const visiting = new Set<string>();
  const visited = new Set<string>();
  const visit = (id: string): void => {
    if (visited.has(id)) return;
    if (!visiting.add(id)) fail('schedule-dependency-cycle', path, `schedule dependency graph contains a cycle involving ${id}`);
    const systemValue = byId.get(id);
    if (systemValue !== undefined) for (const dependency of systemValue.after) visit(dependency);
    visiting.delete(id);
    visited.add(id);
  };
  for (const entry of systems) visit(entry.system.id);
}

function validateScheduleAccessAmbiguity(
  systems: readonly { readonly system: ScheduleSystem; readonly placement: SchedulePlacement }[],
  path: string,
): void {
  const byId = new Map(systems.map((entry) => [entry.system.id, entry]));
  const reaches = (before: string, target: string): boolean => {
    const seen = new Set<string>();
    const pending = [target];
    while (pending.length > 0) {
      const current = pending.pop() as string;
      if (!seen.add(current)) continue;
      const entry = byId.get(current);
      if (entry?.system.after.includes(before)) return true;
      if (entry !== undefined) pending.push(...entry.system.after);
    }
    return false;
  };
  for (let leftIndex = 0; leftIndex < systems.length; leftIndex += 1) {
    for (let rightIndex = leftIndex + 1; rightIndex < systems.length; rightIndex += 1) {
      const left = systems[leftIndex]!;
      const right = systems[rightIndex]!;
      if (left.placement !== right.placement) continue;
      const conflict = left.system.writes.some((write) => right.system.writes.includes(write) || right.system.reads.includes(write))
        || right.system.writes.some((write) => left.system.reads.includes(write));
      if (conflict && !reaches(left.system.id, right.system.id) && !reaches(right.system.id, left.system.id)) {
        fail('schedule-access-ambiguity', path, `systems ${left.system.id} and ${right.system.id} conflict without an explicit dependency`);
      }
    }
  }
}

function admitSchedulePhase(value: unknown, path: string, capabilities?: ReadonlySet<string>, definitions?: ReadonlySet<string>, budget: JsonState = { nodes: 0 }): SchedulePhaseDeclaration {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.schedule, path);
  const phase = listedString(source, 'phase', path, PRODUCT_MODEL_SCHEDULE.phases);
  const mode = listedString(source, 'mode', path, PRODUCT_MODEL_SCHEDULE.modes) as ScheduleCompositionMode;
  if (mode === 'extend') {
    known(source, ['phase', 'mode', 'before', 'after'], path);
    return Object.freeze({
      phase: phase as SchedulePhase,
      mode,
      before: freezeList(requiredArray(source, 'before', path).map((entry, index) => admitScheduleSystem(entry, `${path}.before[${String(index)}]`, capabilities, definitions, budget))),
      after: freezeList(requiredArray(source, 'after', path).map((entry, index) => admitScheduleSystem(entry, `${path}.after[${String(index)}]`, capabilities, definitions, budget))),
    });
  }
  known(source, ['phase', 'mode', 'systems'], path);
  return Object.freeze({
    phase: phase as SchedulePhase,
    mode,
    systems: freezeList(requiredArray(source, 'systems', path).map((entry, index) => admitScheduleSystem(entry, `${path}.systems[${String(index)}]`, capabilities, definitions, budget))),
  }) as SchedulePhaseDeclaration;
}

function admitScheduleSystem(value: unknown, path: string, capabilities?: ReadonlySet<string>, definitions?: ReadonlySet<string>, budget: JsonState = { nodes: 0 }): ScheduleSystem {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.scheduleSystem, path);
  const capabilityId = identity(requiredString(source, 'capability', path), `${path}.capability`);
  if (capabilities !== undefined && !capabilities.has(capabilityId)) fail('unknown-capability', `${path}.capability`, `capability ${capabilityId} is not declared`);
  const definition = source['definition'] === undefined ? undefined : identity(requiredString(source, 'definition', path), `${path}.definition`);
  if (definition !== undefined && definitions !== undefined && !definitions.has(definition)) fail('unknown-definition', `${path}.definition`, `gameplay definition ${definition} is not declared`);
  return freezeScheduleSystem({
    id: identity(requiredString(source, 'id', path), `${path}.id`),
    capability: capabilityId,
    ...(definition === undefined ? {} : { definition }),
    after: identities(requiredArray(source, 'after', path), `${path}.after`, MAX_SCHEDULE_DEPENDENCIES),
    reads: identities(requiredArray(source, 'reads', path), `${path}.reads`),
    writes: identities(requiredArray(source, 'writes', path), `${path}.writes`),
    cadence: admitCadence(required(source, 'cadence', path), `${path}.cadence`),
    payload: normalizeWithBudget(required(source, 'payload', path), `${path}.payload`, budget),
  });
}

function admitCadence(value: unknown, path: string): ScheduleCadence {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.scheduleCadence, path);
  const everySteps = integer(required(source, 'everySteps', path), `${path}.everySteps`);
  const offsetSteps = integer(required(source, 'offsetSteps', path), `${path}.offsetSteps`);
  if (everySteps <= 0 || offsetSteps < 0 || offsetSteps >= everySteps) fail('invalid-schedule-cadence', path, 'cadence requires everySteps > 0 and 0 <= offsetSteps < everySteps');
  return Object.freeze({ everySteps, offsetSteps });
}

function integer(value: unknown, path: string): number {
  if (!Number.isSafeInteger(value) || (value as number) < 0 || (value as number) > 4_294_967_295) fail('invalid-schedule-cadence', path, 'cadence steps must be an unsigned 32-bit integer');
  return value as number;
}

function composeSchedule(anchor: StandardPhase, mode: ScheduleCompositionMode, values: readonly ScheduleSystem[], anchorPath: string): SchedulePhaseDeclaration {
  const phase = standardAnchor(anchor, anchorPath);
  const systems = values.map((entry, index) => admitScheduleSystem(entry, `$.${mode}.systems[${String(index)}]`));
  if (mode === 'append') return Object.freeze({ phase, mode: 'append' as const, systems: freezeList(systems) });
  if (mode === 'prepend') return Object.freeze({ phase, mode: 'prepend' as const, systems: freezeList(systems) });
  if (mode === 'replace') return Object.freeze({ phase, mode: 'replace' as const, systems: freezeList(systems) });
  throw new Error('extend uses its before/after overload');
}

function standardAnchor(value: StandardPhase, path: string): SchedulePhase {
  const source = record(value, path);
  if (!Object.isFrozen(value)) fail('invalid-schedule-phase', path, 'schedule composition anchors must be frozen Standard.<phase> values');
  known(source, ['kind', 'phase'], path);
  if (source['kind'] !== 'standard') fail('invalid-schedule-phase', `${path}.kind`, 'schedule composition anchors must be Standard.<phase> values');
  const phase = source['phase'];
  if (typeof phase !== 'string' || !SCHEDULE_PHASES.includes(phase as SchedulePhase)) fail('invalid-schedule-phase', `${path}.phase`, 'schedule composition anchor phase is not in the closed phase catalog');
  return phase as SchedulePhase;
}

function admitDefinitions(values: readonly unknown[], path: string, budget?: JsonState): readonly GameplayDefinition[] {
  quota(values.length, MAX_GAMEPLAY_DEFINITIONS, path);
  const activeBudget = budget ?? { nodes: 0 };
  const output = values.map((entry, index) => admitDefinition(entry, `${path}[${String(index)}]`, activeBudget));
  unique(output, path);
  return freezeList(output);
}

function admitDefinition(value: unknown, path: string, budget: JsonState): GameplayDefinition {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.gameplayDefinition, path);
  return freezeDefinition({ id: identity(requiredString(source, 'id', path), `${path}.id`), payload: normalizeWithBudget(required(source, 'payload', path), `${path}.payload`, budget) });
}

function admitTimelines(values: readonly unknown[], path: string, capabilities: ReadonlySet<string>, budget: JsonState): readonly Timeline[] {
  quota(values.length, MAX_TIMELINES, path);
  const output = values.map((entry, index) => admitTimeline(entry, `${path}[${String(index)}]`, capabilities, budget));
  unique(output, path);
  return freezeList(output);
}

function admitTimeline(value: unknown, path: string, capabilities: ReadonlySet<string> | undefined, budget: JsonState): Timeline {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.timeline, path);
  const steps = requiredArray(source, 'steps', path);
  quota(steps.length, MAX_TIMELINE_STEPS, `${path}.steps`);
  const output = steps.map((entry, index) => admitTimelineStep(entry, `${path}.steps[${String(index)}]`, capabilities, budget));
  unique(output, `${path}.steps`);
  return freezeTimeline({ id: identity(requiredString(source, 'id', path), `${path}.id`), steps: output });
}

function admitTimelineStep(value: unknown, path: string, capabilities: ReadonlySet<string> | undefined, budget: JsonState): TimelineStep {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.timelineStep, path);
  const capabilityId = identity(requiredString(source, 'capability', path), `${path}.capability`);
  if (capabilities !== undefined && !capabilities.has(capabilityId)) fail('unknown-capability', `${path}.capability`, `capability ${capabilityId} is not declared`);
  return freezeTimelineStep({
    id: identity(requiredString(source, 'id', path), `${path}.id`), capability: capabilityId,
    payload: normalizeWithBudget(required(source, 'payload', path), `${path}.payload`, budget),
  });
}

interface JsonState { nodes: number; }
function normalizeWithBudget(value: unknown, path: string, budget: JsonState): JsonValue {
  // normalizeOpaqueJson owns plain-data, cycle, depth and per-payload limits.
  // Count the resulting tree into Rust's aggregate composition budget.
  const normalized = normalizeOpaqueJson(value, path);
  budget.nodes += countJsonNodes(normalized);
  if (budget.nodes > MAX_OPAQUE_JSON_NODES) {
    fail('json-node-quota-exceeded', path, `opaque payloads exceed ${String(MAX_OPAQUE_JSON_NODES)} nodes`);
  }
  return normalized;
}

function countJsonNodes(value: JsonValue): number {
  if (Array.isArray(value)) return 1 + value.reduce((total, child) => total + countJsonNodes(child), 0);
  if (value !== null && typeof value === 'object') return 1 + Object.values(value as Readonly<Record<string, JsonValue>>).reduce<number>((total, child) => total + countJsonNodes(child), 0);
  return 1;
}

function identities(values: readonly unknown[], path: string, maximum = MAX_SCHEDULE_RESOURCE_DECLARATIONS): readonly string[] {
  quota(values.length, maximum, path);
  const output = values.map((value, index) => identity(string(value, `${path}[${String(index)}]`), `${path}[${String(index)}]`));
  unique(output.map((id) => ({ id })), path);
  return freezeList(output);
}

function unique<T extends { readonly id: string }>(values: readonly T[], path: string): void {
  const seen = new Set<string>();
  for (const [index, value] of values.entries()) {
    if (seen.has(value.id)) fail('duplicate-entry', `${path}[${String(index)}].id`, `duplicate identity ${value.id}`);
    seen.add(value.id);
  }
}

function assertNewIdentities<T extends { readonly id: string }>(name: string, existing: readonly T[], additions: readonly T[]): void {
  const knownIds = new Set(existing.map((entry) => entry.id));
  for (const [index, entry] of additions.entries()) {
    if (knownIds.has(entry.id)) fail('duplicate-entry', `$.${name}[${String(index)}].id`, `extend cannot replace existing identity ${entry.id}`);
    knownIds.add(entry.id);
  }
}

function freezeComposition(value: CompiledComposition): CompiledComposition {
  return Object.freeze({
    product: value.product,
    intentDescriptors: freezeList(value.intentDescriptors),
    inputMap: freezeList(value.inputMap), schedule: freezeList(value.schedule),
    gameplayDefinitions: freezeList(value.gameplayDefinitions), timelines: freezeList(value.timelines),
    capabilityBindings: freezeList(value.capabilityBindings),
  });
}
function freezeInput(value: InputMapEntry): InputMapEntry { return Object.freeze({ ...value }); }
function freezeScheduleSystem(value: ScheduleSystem): ScheduleSystem {
  return Object.freeze({
    ...value,
    after: freezeList(value.after),
    reads: freezeList(value.reads),
    writes: freezeList(value.writes),
    cadence: Object.freeze({ ...value.cadence }),
  });
}
function freezeDefinition(value: GameplayDefinition): GameplayDefinition { return Object.freeze({ ...value }); }
function freezeTimeline(value: Timeline): Timeline { return Object.freeze({ id: value.id, steps: freezeList(value.steps) }); }
function freezeTimelineStep(value: TimelineStep): TimelineStep { return Object.freeze({ ...value }); }
function freezeList<T>(value: readonly T[]): readonly T[] { return Object.freeze(Array.from(value)); }

function writeCompiledComposition(value: CompiledComposition): string {
  return `{"product":${JSON.stringify(value.product)},"intentDescriptors":[${value.intentDescriptors.map(writeIntentDescriptor).join(',')}],"inputMap":[${value.inputMap.map(writeInputMapEntry).join(',')}],"schedule":[${value.schedule.map(writeSchedulePhase).join(',')}],"gameplayDefinitions":[${value.gameplayDefinitions.map(writeGameplayDefinition).join(',')}],"timelines":[${value.timelines.map(writeTimeline).join(',')}],"capabilityBindings":[${value.capabilityBindings.map(writeCapabilityBinding).join(',')}]}`;
}
function writeIntentDescriptor(value: ProductIntentDescriptor): string {
  const payloadContract = value.payloadContract === undefined
    ? ''
    : `,"payloadContract":${JSON.stringify(value.payloadContract)}`;
  const capability = value.capability === undefined
    ? ''
    : `,"capability":${JSON.stringify(value.capability)}`;
  return `{"id":${JSON.stringify(value.id)},"valueKind":${JSON.stringify(value.valueKind)}${payloadContract}${capability},"payload":${writeCanonicalJson(value.payload)}}`;
}
function writeInputMapEntry(value: InputMapEntry): string {
  return `{"id":${JSON.stringify(value.id)},"intent":${JSON.stringify(value.intent)},"trigger":${writeCanonicalJson(value.trigger as unknown as JsonValue)}}`;
}
function writeSchedulePhase(value: SchedulePhaseDeclaration): string {
  if (value.mode === 'extend') {
    return `{"phase":${JSON.stringify(value.phase)},"mode":"extend","before":[${value.before.map(writeScheduleSystem).join(',')}],"after":[${value.after.map(writeScheduleSystem).join(',')}]}`;
  }
  return `{"phase":${JSON.stringify(value.phase)},"mode":${JSON.stringify(value.mode)},"systems":[${value.systems.map(writeScheduleSystem).join(',')}]}`;
}
function writeScheduleSystem(value: ScheduleSystem): string {
  return `{"id":${JSON.stringify(value.id)},"capability":${JSON.stringify(value.capability)}${value.definition === undefined ? '' : `,"definition":${JSON.stringify(value.definition)}`},"after":[${value.after.map((entry) => JSON.stringify(entry)).join(',')}],"reads":[${value.reads.map((entry) => JSON.stringify(entry)).join(',')}],"writes":[${value.writes.map((entry) => JSON.stringify(entry)).join(',')}],"cadence":{"everySteps":${String(value.cadence.everySteps)},"offsetSteps":${String(value.cadence.offsetSteps)}},"payload":${writeCanonicalJson(value.payload)}}`;
}
function writeGameplayDefinition(value: GameplayDefinition): string {
  return `{"id":${JSON.stringify(value.id)},"payload":${writeCanonicalJson(value.payload)}}`;
}
function writeTimeline(value: Timeline): string {
  return `{"id":${JSON.stringify(value.id)},"steps":[${value.steps.map(writeTimelineStep).join(',')}]}`;
}
function writeTimelineStep(value: TimelineStep): string {
  return `{"id":${JSON.stringify(value.id)},"capability":${JSON.stringify(value.capability)},"payload":${writeCanonicalJson(value.payload)}}`;
}
function writeCapabilityBinding(value: CapabilityBinding): string {
  return `{"id":${JSON.stringify(value.id)},"target":${JSON.stringify(value.target)}}`;
}

function identity(value: string, path: string): string {
  if (utf8Length(value) > PRODUCT_MODEL_IDENTITY.maximumBytes || !IDENTITY.test(value)) fail('invalid-identity', path, 'identities must be 1..=128 lowercase ASCII segments with only single dots, underscores, or hyphens between alphanumerics');
  return value;
}

function record(value: unknown, path: string): Readonly<Record<string, unknown>> {
  if (typeof value !== 'object' || value === null || Array.isArray(value) || (Object.getPrototypeOf(value) !== Object.prototype && Object.getPrototypeOf(value) !== null)) {
    fail('invalid-field-type', path, 'expected a plain object');
  }
  const descriptors = Object.getOwnPropertyDescriptors(value);
  for (const key of Reflect.ownKeys(descriptors)) {
    if (typeof key === 'symbol') fail('invalid-json-value', path, 'authoring objects cannot contain symbol keys');
    const descriptor = descriptors[key];
    if (descriptor === undefined || !descriptor.enumerable || !('value' in descriptor)) fail('invalid-json-value', `${path}.${key}`, 'authoring objects cannot contain accessors or non-enumerable fields');
    if (descriptor.value === undefined) fail('invalid-json-value', `${path}.${key}`, 'authoring objects cannot contain undefined fields');
  }
  return value as Readonly<Record<string, unknown>>;
}
function known(source: Readonly<Record<string, unknown>>, fields: readonly string[], path: string): void {
  for (const key of Object.keys(source)) if (!fields.includes(key)) fail('unknown-field', `${path}.${key}`, `unknown field ${key}`);
}
function required(source: Readonly<Record<string, unknown>>, field: string, path: string): unknown {
  if (!Object.hasOwn(source, field)) fail('missing-field', `${path}.${field}`, `missing required field ${field}`);
  return source[field];
}
function requiredString(source: Readonly<Record<string, unknown>>, field: string, path: string): string { return string(required(source, field, path), `${path}.${field}`); }
function optionalArray(source: Readonly<Record<string, unknown>>, field: string, path: string): readonly unknown[] { return source[field] === undefined ? [] : requiredArray(source, field, path); }
function requiredArray(source: Readonly<Record<string, unknown>>, field: string, path: string): readonly unknown[] {
  const value = required(source, field, path);
  return arrayData(value, `${path}.${field}`);
}
function arrayData(value: unknown, path: string): readonly unknown[] {
  if (!Array.isArray(value)) fail('invalid-field-type', path, 'expected an array');
  if (Object.getPrototypeOf(value) !== Array.prototype) fail('invalid-json-value', path, 'authoring arrays must use the ordinary Array prototype');
  for (let index = 0; index < value.length; index += 1) {
    if (!Object.hasOwn(value, index)) fail('invalid-json-value', `${path}[${String(index)}]`, 'authoring arrays cannot contain holes or undefined entries');
  }
  for (const key of Reflect.ownKeys(value)) {
    if (key === 'length') continue;
    if (typeof key === 'symbol' || !isArrayIndex(key)) fail('invalid-json-value', path, 'authoring arrays cannot contain non-index properties');
    const descriptor = Object.getOwnPropertyDescriptor(value, key);
    if (descriptor === undefined || !descriptor.enumerable || !('value' in descriptor)) fail('invalid-json-value', `${path}[${key}]`, 'authoring arrays cannot contain accessors or non-enumerable entries');
    if (descriptor.value === undefined) fail('invalid-json-value', `${path}[${key}]`, 'authoring arrays cannot contain undefined entries');
  }
  return value;
}
function isArrayIndex(key: string): boolean {
  if (key !== '0' && !/^[1-9][0-9]*$/.test(key)) return false;
  const number = Number(key);
  return Number.isSafeInteger(number) && number < 4_294_967_295 && String(number) === key;
}
function string(value: unknown, path: string): string { if (typeof value !== 'string') fail('invalid-field-type', path, 'expected a string'); return value; }
function quota(actual: number, maximum: number, path: string): void { if (actual > maximum) fail('quota-exceeded', path, `contains ${String(actual)} entries; maximum is ${String(maximum)}`); }
function fail(code: RuntimeCompositionAuthoringError['code'], path: string, message: string): never { throw new RuntimeCompositionAuthoringError(code, path, message); }
