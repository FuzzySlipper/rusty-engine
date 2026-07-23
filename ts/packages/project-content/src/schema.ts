export type Vec3 = readonly [number, number, number];
export type VoxelAddress = readonly [number, number, number];

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

export interface HealthDefinition {
  readonly max: number;
  readonly hitboxHalfExtents: Vec3;
}

export interface KinematicDefinition {
  readonly halfExtents: Vec3;
  readonly velocity: Vec3;
}

export interface NavigationDefinition {
  readonly goal: Vec3;
  readonly speedUnitsPerSecond: number;
  readonly maxVisited: number;
}

export interface PlayerInputBindingsDefinition {
  readonly moveForward: string;
  readonly moveBackward: string;
  readonly moveLeft: string;
  readonly moveRight: string;
  readonly mouseLook: string;
  readonly primaryFire: string;
}

export interface PlayerControllerDefinition {
  readonly moveSpeedUnitsPerSecond: number;
  readonly moveStepSeconds: number;
  readonly lookDegreesPerUnit: number;
  readonly initialYawDegrees: number;
  readonly initialPitchDegrees: number;
  readonly bindings: PlayerInputBindingsDefinition;
}

export interface WeaponDefinition {
  readonly damage: number;
  readonly maxDistance: number;
  readonly cooldownTicks: number;
  readonly ammoCapacity: number;
  readonly muzzleOffset: Vec3;
}

export interface VoxelCollisionDefinition {
  readonly voxelSize: number;
  readonly chunkSize: number;
  readonly solidVoxels: readonly VoxelAddress[];
}

export interface GeneratedVoxelEnvironmentDefinition {
  readonly seed: number;
  readonly voxelSize: number;
  readonly chunkSize: number;
  readonly width: number;
  readonly height: number;
  readonly length: number;
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
  readonly health?: HealthDefinition;
  readonly encounter?: EncounterDefinition;
  readonly kinematic?: KinematicDefinition;
  readonly navigation?: NavigationDefinition;
  readonly playerController?: PlayerControllerDefinition;
  readonly weapon?: WeaponDefinition;
}

export interface ProjectContent {
  readonly schemaVersion: 6;
  readonly entities: readonly EntityDefinition[];
  readonly voxelCollision?: VoxelCollisionDefinition;
  readonly generatedVoxelEnvironment?: GeneratedVoxelEnvironmentDefinition;
}
