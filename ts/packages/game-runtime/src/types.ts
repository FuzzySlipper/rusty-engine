export type JsonPrimitive = string | number | boolean | null;
export type JsonValue = JsonPrimitive | readonly JsonValue[] | { readonly [key: string]: JsonValue };

export interface ProjectStateRecord {
  readonly instanceId: string;
  readonly behaviorType: string;
  readonly version: number;
  readonly revision: number;
  readonly payload: JsonValue;
}

export interface HostCollisionView {
  readonly enabled: boolean;
  readonly staticCollider: boolean;
}

export interface HostRenderableView {
  readonly visible: boolean;
  readonly asset: string;
}

export interface HostEntityView {
  readonly entity: number;
  readonly name: string;
  readonly lifecycle: "active" | "disabled";
  readonly translation: readonly [number, number, number] | null;
  readonly collision: HostCollisionView | null;
  readonly renderable: HostRenderableView | null;
}

export type HostInputEvent =
  | {
      readonly kind: "interaction";
      readonly actor: number;
      readonly target: number;
    }
  | {
      readonly kind: "message";
      readonly messageId: string;
      readonly messageKind: string;
      readonly payload: JsonValue;
    };

export interface ProjectInvocation {
  readonly invocationId: number;
  readonly behavior: {
    readonly instanceId: string;
    readonly behaviorType: string;
    readonly version: number;
  };
  readonly events: readonly HostInputEvent[];
  readonly owner: HostEntityView;
  readonly related: readonly HostEntityView[];
  readonly state: ProjectStateRecord;
}

export interface ProjectInvocationWave {
  readonly schemaVersion: 1;
  readonly tick: number;
  readonly expectedWorldRevision: number;
  readonly invocations: readonly ProjectInvocation[];
}

export type ProjectWorldCommand =
  | {
      readonly op: "setTranslation";
      readonly entity: number;
      readonly translation: readonly [number, number, number];
    }
  | {
      readonly op: "setCollisionEnabled";
      readonly entity: number;
      readonly enabled: boolean;
    }
  | {
      readonly op: "setVisible";
      readonly entity: number;
      readonly visible: boolean;
    };

export type ProjectScheduleRequest =
  | {
      readonly op: "upsert";
      readonly messageId: string;
      readonly dueAfterTicks: number;
      readonly messageKind: string;
      readonly payload: JsonValue;
    }
  | {
      readonly op: "cancel";
      readonly messageId: string;
    };

export interface ProjectFact {
  readonly kind: string;
  readonly version: number;
  readonly payload: JsonValue;
}

export interface ProjectDecision {
  readonly invocationId: number;
  readonly commands: readonly ProjectWorldCommand[];
  readonly stateUpdate: {
    readonly expectedRevision: number;
    readonly version: number;
    readonly payload: JsonValue;
  } | null;
  readonly schedules: readonly ProjectScheduleRequest[];
  readonly facts: readonly ProjectFact[];
}

export interface ProjectDecisionBatch {
  readonly schemaVersion: 1;
  readonly expectedWorldRevision: number;
  readonly decisions: readonly ProjectDecision[];
}

export type HostEngineFact =
  | {
      readonly kind: "translationChanged";
      readonly entity: number;
      readonly before: readonly [number, number, number];
      readonly after: readonly [number, number, number];
      readonly revision: number;
    }
  | {
      readonly kind: "collisionChanged";
      readonly entity: number;
      readonly before: boolean;
      readonly after: boolean;
      readonly revision: number;
    }
  | {
      readonly kind: "visibilityChanged";
      readonly entity: number;
      readonly before: boolean;
      readonly after: boolean;
      readonly revision: number;
    };

export interface HostProjectionNode {
  readonly entity: number;
  readonly name: string;
  readonly asset: string;
  readonly translation: readonly [number, number, number] | null;
  readonly visible: boolean;
}

export interface ProjectApplyReceipt {
  readonly tick: number;
  readonly revisionBefore: number;
  readonly revisionAfter: number;
  readonly engineFacts: readonly HostEngineFact[];
  readonly projectFacts: readonly ProjectFact[];
  readonly stateRecords: readonly ProjectStateRecord[];
  readonly pendingMessageCount: number;
  readonly projection: readonly HostProjectionNode[];
}

export interface ProjectRuntimeReadout {
  readonly tick: number;
  readonly worldRevision: number;
  readonly stateRecords: readonly ProjectStateRecord[];
  readonly pendingMessageCount: number;
  readonly pendingInvocation: boolean;
  readonly projectFacts: readonly {
    readonly tick: number;
    readonly instanceId: string;
    readonly fact: ProjectFact;
  }[];
  readonly projection: readonly HostProjectionNode[];
}

export interface BridgeStats {
  readonly gameplayCalls: number;
  readonly bytesIn: number;
  readonly bytesOut: number;
}

export interface ProjectDoorIds {
  readonly actor: number;
  readonly switch: number;
  readonly door: number;
}
