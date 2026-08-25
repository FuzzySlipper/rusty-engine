/** Values that can cross the authoring boundary into an opaque JSON payload. */
export interface JsonObject {
  readonly [key: string]: JsonValue;
}

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | readonly JsonValue[]
  | JsonObject;

export interface CapabilityBinding {
  readonly id: string;
  readonly target: string;
}

export interface InputMapEntry {
  readonly id: string;
  readonly intent: string;
  readonly trigger: InputTrigger;
}

export interface ProductIntentDescriptor {
  readonly id: string;
  readonly valueKind: IntentValueKind;
  /** Required exactly for a direct-UI-only product-payload intent. */
  readonly payloadContract?: string;
  /** Optional legacy Product Kernel linkage; VM-local intents omit it. */
  readonly capability?: string;
  readonly payload: JsonValue;
}

export type IntentValueKind = 'digital' | 'axis' | 'product-payload';
export type InputEdge = 'held' | 'pressed' | 'released';
export type KeyboardControl =
  | 'key-a' | 'key-b' | 'key-c' | 'key-d' | 'key-e' | 'key-f' | 'key-g' | 'key-h' | 'key-i'
  | 'key-j' | 'key-k' | 'key-l' | 'key-m' | 'key-n' | 'key-o' | 'key-p' | 'key-q' | 'key-r'
  | 'key-s' | 'key-t' | 'key-u' | 'key-v' | 'key-w' | 'key-x' | 'key-y' | 'key-z'
  | 'digit-0' | 'digit-1' | 'digit-2' | 'digit-3' | 'digit-4' | 'digit-5' | 'digit-6' | 'digit-7' | 'digit-8' | 'digit-9'
  | 'space' | 'enter' | 'escape' | 'shift-left' | 'shift-right' | 'control-left' | 'control-right' | 'alt-left' | 'alt-right';
export type PointerButton = 'primary' | 'secondary' | 'middle';
export type InputAxis = 'x' | 'y';
export type ControllerButton = `button-${0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 | 11 | 12 | 13 | 14 | 15}`;
export type ControllerAxis = `axis-${0 | 1 | 2 | 3}`;
export type InputTrigger =
  | { readonly kind: 'key'; readonly code: KeyboardControl; readonly edge: InputEdge; readonly chord?: readonly KeyboardControl[]; readonly context?: string }
  | { readonly kind: 'pointer-button'; readonly button: PointerButton; readonly edge: InputEdge; readonly context?: string }
  | { readonly kind: 'pointer-axis'; readonly axis: InputAxis; readonly context?: string }
  | { readonly kind: 'wheel'; readonly axis: InputAxis; readonly context?: string }
  | { readonly kind: 'controller-button'; readonly button: ControllerButton; readonly edge: InputEdge; readonly context?: string }
  | { readonly kind: 'controller-axis'; readonly axis: ControllerAxis; readonly context?: string };

export type SchedulePhase = 'input' | 'simulation' | 'consequences' | 'commit' | 'projection';
export type ScheduleCompositionMode = 'append' | 'prepend' | 'extend' | 'replace';
export type SchedulePlacement = 'append' | 'prepend' | 'extend-before' | 'extend-after' | 'replace';

export interface ScheduleCadence {
  readonly everySteps: number;
  readonly offsetSteps: number;
}

export interface ScheduleSystem {
  readonly id: string;
  readonly capability: string;
  readonly definition?: string;
  readonly after: readonly string[];
  readonly reads: readonly string[];
  readonly writes: readonly string[];
  readonly cadence: ScheduleCadence;
  readonly payload: JsonValue;
}

export interface ScheduleAppend {
  readonly phase: SchedulePhase;
  readonly mode: 'append';
  readonly systems: readonly ScheduleSystem[];
}

export interface SchedulePrepend {
  readonly phase: SchedulePhase;
  readonly mode: 'prepend';
  readonly systems: readonly ScheduleSystem[];
}

export interface ScheduleExtend {
  readonly phase: SchedulePhase;
  readonly mode: 'extend';
  readonly before: readonly ScheduleSystem[];
  readonly after: readonly ScheduleSystem[];
}

export interface ScheduleReplace {
  readonly phase: SchedulePhase;
  readonly mode: 'replace';
  readonly systems: readonly ScheduleSystem[];
}

export type SchedulePhaseDeclaration = ScheduleAppend | SchedulePrepend | ScheduleExtend | ScheduleReplace;

/** A named implicit Standard.<phase> anchor used by the schedule DSL. */
export interface StandardPhase {
  readonly kind: 'standard';
  readonly phase: SchedulePhase;
}

/** A source-level schedule map. `schedule()` lowers it to five declarations. */
export type ScheduleDraft = Partial<Record<SchedulePhase, SchedulePhaseDeclaration>>;

/** Legacy name retained only as a type alias for source migration diagnostics. */
export type ScheduleEntry = ScheduleSystem;

export interface GameplayDefinition {
  readonly id: string;
  readonly payload: JsonValue;
}

export interface TimelineStep {
  readonly id: string;
  readonly capability: string;
  readonly payload: JsonValue;
}

export interface Timeline {
  readonly id: string;
  readonly steps: readonly TimelineStep[];
}

/** The current Rust-owned Compiled Composition wire shape. No version field exists. */
export interface CompiledComposition {
  readonly product: string;
  readonly intentDescriptors: readonly ProductIntentDescriptor[];
  readonly inputMap: readonly InputMapEntry[];
  readonly schedule: readonly SchedulePhaseDeclaration[];
  readonly gameplayDefinitions: readonly GameplayDefinition[];
  readonly timelines: readonly Timeline[];
  readonly capabilityBindings: readonly CapabilityBinding[];
}

/** Ergonomic source shape: authoring calls the capability collection by its role. */
export interface RuntimeCompositionDraft {
  readonly product: string;
  readonly capabilities: readonly CapabilityBinding[];
  readonly intentDescriptors?: readonly ProductIntentDescriptor[];
  readonly inputMap?: readonly InputMapEntry[];
  readonly schedule?: readonly SchedulePhaseDeclaration[] | ScheduleDraft;
  readonly gameplayDefinitions?: readonly GameplayDefinition[];
  readonly timelines?: readonly Timeline[];
}

/** A partial collection set intended for an explicit composition operation. */
export interface CompositionFragment {
  readonly intentDescriptors: readonly ProductIntentDescriptor[];
  readonly inputMap: readonly InputMapEntry[];
  readonly gameplayDefinitions: readonly GameplayDefinition[];
  readonly timelines: readonly Timeline[];
  readonly capabilityBindings: readonly CapabilityBinding[];
}

/** Replaces exactly the listed whole collections; omitted collections remain untouched. */
export interface CompositionReplacement {
  readonly intentDescriptors?: readonly ProductIntentDescriptor[];
  readonly inputMap?: readonly InputMapEntry[];
  readonly schedule?: readonly SchedulePhaseDeclaration[];
  readonly gameplayDefinitions?: readonly GameplayDefinition[];
  readonly timelines?: readonly Timeline[];
  readonly capabilityBindings?: readonly CapabilityBinding[];
}

export interface RuntimeCompositionArtifact {
  readonly composition: CompiledComposition;
  readonly canonicalJson: string;
}

export interface InputActionDraft {
  readonly id: string;
  readonly intent: string;
  readonly trigger: InputTrigger;
}

export interface ProductIntentDescriptorDraft {
  readonly id: string;
  readonly valueKind: IntentValueKind;
  readonly payloadContract?: string;
  readonly capability?: string;
  readonly payload: unknown;
}

export interface ScheduleActionDraft {
  readonly id: string;
  readonly capability?: string;
  readonly definition?: string;
  readonly after?: readonly string[];
  readonly reads?: readonly string[];
  readonly writes?: readonly string[];
  readonly cadence?: ScheduleCadence;
  readonly payload?: unknown;
}

export interface ScheduleEntryDraft extends ScheduleActionDraft {
  readonly phase?: SchedulePhase;
}

export interface TimelineStepDraft {
  readonly id: string;
  readonly capability: string;
  readonly payload: unknown;
}
