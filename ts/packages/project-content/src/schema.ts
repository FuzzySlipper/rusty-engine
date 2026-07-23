export type Vec3 = readonly [number, number, number];

export interface CollisionDefinition {
  readonly enabled: boolean;
  readonly staticCollider: boolean;
}

export interface RenderableDefinition {
  readonly asset: string;
  readonly visible: boolean;
}

export interface DoorDefinition {
  readonly openTranslation: Vec3;
  readonly autoCloseAfterTicks: number | null;
}

export interface SwitchDefinition {
  readonly controls: readonly number[];
}

export interface EncounterDefinition {
  readonly members: readonly number[];
  readonly exit: number;
}

export interface EntityDefinition {
  readonly id: number;
  readonly name: string;
  readonly translation?: Vec3;
  readonly collision?: CollisionDefinition;
  readonly renderable?: RenderableDefinition;
  readonly door?: DoorDefinition;
  readonly switch?: SwitchDefinition;
  readonly enemy?: true;
  readonly encounter?: EncounterDefinition;
}

export interface ProjectContent {
  readonly schemaVersion: 1;
  readonly entities: readonly EntityDefinition[];
}
