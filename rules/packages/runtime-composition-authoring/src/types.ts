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
  readonly capability: string;
  readonly payload: JsonValue;
}

export type IntentValueKind = 'digital' | 'axis';
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

export interface ScheduleEntry {
  readonly id: string;
  readonly phase: string;
  readonly capability: string;
  readonly definition?: string;
  readonly reads: readonly string[];
  readonly writes: readonly string[];
  readonly payload: JsonValue;
}

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
  readonly schedule: readonly ScheduleEntry[];
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
  readonly schedule?: readonly ScheduleEntry[];
  readonly gameplayDefinitions?: readonly GameplayDefinition[];
  readonly timelines?: readonly Timeline[];
}

/** A partial collection set intended for an explicit composition operation. */
export interface CompositionFragment {
  readonly intentDescriptors: readonly ProductIntentDescriptor[];
  readonly inputMap: readonly InputMapEntry[];
  readonly schedule: readonly ScheduleEntry[];
  readonly gameplayDefinitions: readonly GameplayDefinition[];
  readonly timelines: readonly Timeline[];
  readonly capabilityBindings: readonly CapabilityBinding[];
}

/** Replaces exactly the listed whole collections; omitted collections remain untouched. */
export interface CompositionReplacement {
  readonly intentDescriptors?: readonly ProductIntentDescriptor[];
  readonly inputMap?: readonly InputMapEntry[];
  readonly schedule?: readonly ScheduleEntry[];
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
  readonly capability: string;
  readonly payload: unknown;
}

export interface ScheduleActionDraft {
  readonly id: string;
  readonly capability: string;
  readonly definition?: string;
  readonly reads: readonly string[];
  readonly writes: readonly string[];
  readonly payload: unknown;
}

export interface ScheduleEntryDraft extends ScheduleActionDraft {
  readonly phase: string;
}

export interface TimelineStepDraft {
  readonly id: string;
  readonly capability: string;
  readonly payload: unknown;
}
