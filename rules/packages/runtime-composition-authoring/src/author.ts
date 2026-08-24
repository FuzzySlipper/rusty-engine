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
  ScheduleEntry,
  ScheduleEntryDraft,
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
    schedule: source.schedule ?? [],
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

/** Describes one typed intent and its one admitted capability target. */
export function productIntent(draft: ProductIntentDescriptorDraft): ProductIntentDescriptor {
  return admitIntentDescriptor(draft, '$.productIntent', undefined, { nodes: 0 });
}

/** Builds one physical mapping into a previously described product intent. */
export function inputAction(draft: InputActionDraft): InputMapEntry {
  const source = record(draft, '$.inputAction');
  known(source, ['id', 'intent', 'trigger'], '$.inputAction');
  return freezeInput({
    id: identity(requiredString(source, 'id', '$.inputAction'), '$.inputAction.id'),
    intent: identity(requiredString(source, 'intent', '$.inputAction'), '$.inputAction.intent'),
    trigger: admitInputTrigger(required(source, 'trigger', '$.inputAction'), '$.inputAction.trigger'),
  });
}

/** Builds a phase-local schedule fragment; array order is intentionally retained. */
export function phase(phaseId: string, actions: readonly ScheduleActionDraft[]): CompositionFragment {
  const phaseName = identity(phaseId, '$.phase');
  const values = arrayData(actions, '$.phase.actions');
  return fragment({ schedule: values.map((action, index) => scheduleEntryForPhase(action, phaseName, `$.phase.actions[${String(index)}]`)) });
}

/** Builds one schedule entry when the caller needs to interleave multiple phases manually. */
export function scheduleAction(draft: ScheduleEntryDraft): ScheduleEntry {
  return scheduleEntry(draft, '$.scheduleAction');
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

/**
 * Current Compiled Composition has no cadence field. Refuse rather than
 * inventing a timing contract; #7256 owns schedule ordering/conflict policy.
 */
export function cadence(_value: unknown): never {
  throw new RuntimeCompositionAuthoringError(
    'unrepresentable-cadence', '$.cadence',
    'the current Compiled Composition schema has no cadence field; keep timing policy downstream until an admitted schema exists',
  );
}

/** Produces a normalized partial collection set for later composition. */
export function fragment(value: Partial<CompositionFragment>): CompositionFragment {
  const source = record(value, '$.fragment');
  known(source, ['intentDescriptors', 'inputMap', 'schedule', 'gameplayDefinitions', 'timelines', 'capabilityBindings'], '$.fragment');
  return Object.freeze({
    intentDescriptors: freezeList(optionalArray(source, 'intentDescriptors', '$.fragment').map((entry, index) => admitIntentDescriptor(entry, `$.fragment.intentDescriptors[${String(index)}]`, undefined, { nodes: 0 }))),
    inputMap: freezeList(optionalArray(source, 'inputMap', '$.fragment').map((entry, index) => admitInputMapEntry(entry, `$.fragment.inputMap[${String(index)}]`, undefined))),
    schedule: freezeList(optionalArray(source, 'schedule', '$.fragment').map((entry, index) => admitScheduleEntry(entry, `$.fragment.schedule[${String(index)}]`, undefined, undefined, { nodes: 0 }))),
    gameplayDefinitions: freezeList(optionalArray(source, 'gameplayDefinitions', '$.fragment').map((entry, index) => admitDefinition(entry, `$.fragment.gameplayDefinitions[${String(index)}]`, { nodes: 0 }))),
    timelines: freezeList(optionalArray(source, 'timelines', '$.fragment').map((entry, index) => admitTimeline(entry, `$.fragment.timelines[${String(index)}]`, undefined, { nodes: 0 }))),
    capabilityBindings: freezeList(optionalArray(source, 'capabilityBindings', '$.fragment').map((entry, index) => admitCapability(entry, `$.fragment.capabilityBindings[${String(index)}]`))),
  });
}

/** Appends each supplied fragment in argument order, preserving every collection order. */
export function append(base: CompiledComposition, ...fragments: readonly CompositionFragment[]): CompiledComposition {
  return composeFragments(base, fragments, false, false);
}

/** Prepends fragments in argument order; the first fragment becomes the earliest declaration. */
export function prepend(base: CompiledComposition, ...fragments: readonly CompositionFragment[]): CompiledComposition {
  return composeFragments(base, fragments, true, false);
}

/** Adds only new identities. It rejects collision rather than silently replacing authored data. */
export function extend(base: CompiledComposition, ...fragments: readonly CompositionFragment[]): CompiledComposition {
  return composeFragments(base, fragments, false, true);
}

/** Replaces exactly the named whole collections. It cannot change the product identity. */
export function replace(base: CompiledComposition, replacement: CompositionReplacement): CompiledComposition {
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
    assertNewIdentities('schedule', normalizedBase.schedule, normalized.flatMap((entry) => entry.schedule));
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
    schedule: merge(normalizedBase.schedule, normalized.flatMap((entry) => entry.schedule)),
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
    ...(source['schedule'] === undefined ? {} : { schedule: freezeList(requiredArray(source, 'schedule', path).map((entry, index) => admitScheduleEntry(entry, `${path}.schedule[${String(index)}]`, undefined, undefined, { nodes: 0 }))) }),
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
  const capability = identity(requiredString(source, 'capability', path), `${path}.capability`);
  if (capabilities !== undefined && !capabilities.has(capability)) fail('unknown-capability', `${path}.capability`, `capability ${capability} is not declared`);
  const valueKind = requiredString(source, 'valueKind', path);
  if (!(PRODUCT_MODEL_INPUT.intentValueKinds as readonly string[]).includes(valueKind)) fail('invalid-input-value-kind', `${path}.valueKind`, 'intent valueKind must be digital or axis');
  return Object.freeze({
    id: identity(requiredString(source, 'id', path), `${path}.id`),
    valueKind: valueKind as ProductIntentDescriptor['valueKind'],
    capability,
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

function admitSchedule(values: readonly unknown[], path: string, capabilities: ReadonlySet<string>, definitions: ReadonlySet<string>, budget: JsonState): readonly ScheduleEntry[] {
  quota(values.length, MAX_SCHEDULE_ENTRIES, path);
  const output = values.map((entry, index) => admitScheduleEntry(entry, `${path}[${String(index)}]`, capabilities, definitions, budget));
  unique(output, path);
  return freezeList(output);
}

function admitScheduleEntry(value: unknown, path: string, capabilities: ReadonlySet<string> | undefined, definitions: ReadonlySet<string> | undefined, budget: JsonState): ScheduleEntry {
  const source = record(value, path);
  known(source, PRODUCT_MODEL_FIELDS.schedule, path);
  const capabilityId = identity(requiredString(source, 'capability', path), `${path}.capability`);
  if (capabilities !== undefined && !capabilities.has(capabilityId)) fail('unknown-capability', `${path}.capability`, `capability ${capabilityId} is not declared`);
  const definition = source['definition'] === undefined ? undefined : identity(requiredString(source, 'definition', path), `${path}.definition`);
  if (definition !== undefined && definitions !== undefined && !definitions.has(definition)) fail('unknown-definition', `${path}.definition`, `gameplay definition ${definition} is not declared`);
  return freezeSchedule({
    id: identity(requiredString(source, 'id', path), `${path}.id`),
    phase: identity(requiredString(source, 'phase', path), `${path}.phase`),
    capability: capabilityId,
    ...(definition === undefined ? {} : { definition }),
    reads: identities(requiredArray(source, 'reads', path), `${path}.reads`),
    writes: identities(requiredArray(source, 'writes', path), `${path}.writes`),
    payload: normalizeWithBudget(required(source, 'payload', path), `${path}.payload`, budget),
  });
}

function scheduleEntry(draft: ScheduleEntryDraft, path: string): ScheduleEntry {
  return admitScheduleEntry(draft, path, undefined, undefined, { nodes: 0 });
}

function scheduleEntryForPhase(value: unknown, phaseName: string, path: string): ScheduleEntry {
  const source = record(value, path);
  known(source, ['id', 'capability', 'definition', 'reads', 'writes', 'payload'], path);
  return admitScheduleEntry({
    id: required(source, 'id', path),
    phase: phaseName,
    capability: required(source, 'capability', path),
    ...(Object.hasOwn(source, 'definition') ? { definition: source['definition'] } : {}),
    reads: required(source, 'reads', path),
    writes: required(source, 'writes', path),
    payload: required(source, 'payload', path),
  }, path, undefined, undefined, { nodes: 0 });
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

function identities(values: readonly unknown[], path: string): readonly string[] {
  quota(values.length, MAX_SCHEDULE_RESOURCE_DECLARATIONS, path);
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
function freezeSchedule(value: ScheduleEntry): ScheduleEntry { return Object.freeze({ ...value, reads: freezeList(value.reads), writes: freezeList(value.writes) }); }
function freezeDefinition(value: GameplayDefinition): GameplayDefinition { return Object.freeze({ ...value }); }
function freezeTimeline(value: Timeline): Timeline { return Object.freeze({ id: value.id, steps: freezeList(value.steps) }); }
function freezeTimelineStep(value: TimelineStep): TimelineStep { return Object.freeze({ ...value }); }
function freezeList<T>(value: readonly T[]): readonly T[] { return Object.freeze(Array.from(value)); }

function writeCompiledComposition(value: CompiledComposition): string {
  return `{"product":${JSON.stringify(value.product)},"intentDescriptors":[${value.intentDescriptors.map(writeIntentDescriptor).join(',')}],"inputMap":[${value.inputMap.map(writeInputMapEntry).join(',')}],"schedule":[${value.schedule.map(writeScheduleEntry).join(',')}],"gameplayDefinitions":[${value.gameplayDefinitions.map(writeGameplayDefinition).join(',')}],"timelines":[${value.timelines.map(writeTimeline).join(',')}],"capabilityBindings":[${value.capabilityBindings.map(writeCapabilityBinding).join(',')}]}`;
}
function writeIntentDescriptor(value: ProductIntentDescriptor): string {
  return `{"id":${JSON.stringify(value.id)},"valueKind":${JSON.stringify(value.valueKind)},"capability":${JSON.stringify(value.capability)},"payload":${writeCanonicalJson(value.payload)}}`;
}
function writeInputMapEntry(value: InputMapEntry): string {
  return `{"id":${JSON.stringify(value.id)},"intent":${JSON.stringify(value.intent)},"trigger":${writeCanonicalJson(value.trigger as unknown as JsonValue)}}`;
}
function writeScheduleEntry(value: ScheduleEntry): string {
  return `{"id":${JSON.stringify(value.id)},"phase":${JSON.stringify(value.phase)},"capability":${JSON.stringify(value.capability)}${value.definition === undefined ? '' : `,"definition":${JSON.stringify(value.definition)}`},"reads":[${value.reads.map((entry) => JSON.stringify(entry)).join(',')}],"writes":[${value.writes.map((entry) => JSON.stringify(entry)).join(',')}],"payload":${writeCanonicalJson(value.payload)}}`;
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
