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
  readonly capability: string;
  readonly payload: JsonValue;
}

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
  readonly inputMap?: readonly InputMapEntry[];
  readonly schedule?: readonly ScheduleEntry[];
  readonly gameplayDefinitions?: readonly GameplayDefinition[];
  readonly timelines?: readonly Timeline[];
}

/** A partial collection set intended for an explicit composition operation. */
export interface CompositionFragment {
  readonly inputMap: readonly InputMapEntry[];
  readonly schedule: readonly ScheduleEntry[];
  readonly gameplayDefinitions: readonly GameplayDefinition[];
  readonly timelines: readonly Timeline[];
  readonly capabilityBindings: readonly CapabilityBinding[];
}

/** Replaces exactly the listed whole collections; omitted collections remain untouched. */
export interface CompositionReplacement {
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
