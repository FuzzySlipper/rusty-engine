import {
  entityId,
  renderHandle,
  type Geometry,
  type Material,
  type RenderDiff,
  type RenderFrameDiff,
  type RenderHandle,
  type RenderNode,
  type Transform,
} from "@asha/contracts";

export interface RuntimeProjectionNode {
  readonly id: number;
  readonly name: string;
  readonly asset: string;
  readonly translation: readonly [number, number, number] | null;
  readonly visible: boolean;
}

export interface RuntimeEnemyState {
  readonly id: number;
  readonly name: string;
  readonly state: "alive" | "defeated";
}

export interface RuntimeBrowserState {
  readonly tick: number;
  readonly worldRevision: number;
  readonly projection: readonly RuntimeProjectionNode[];
  readonly doorState: "closed" | "open";
  readonly encounterState: "active" | "cleared";
  readonly motionState: "moving" | "blocked";
  readonly enemies: readonly RuntimeEnemyState[];
  readonly lastEvents: readonly string[];
}

const FLOOR_HANDLE = renderHandle(900_000);
const ENTITY_HANDLE_OFFSET = 100_000;

/** Stateful adapter from whole Rust projection readouts to retained renderer diffs. */
export class RuntimeProjectionAdapter {
  readonly #known = new Map<number, RuntimeProjectionNode>();
  #floorCreated = false;

  apply(state: RuntimeBrowserState): RenderFrameDiff {
    const ops: RenderDiff[] = [];
    if (!this.#floorCreated) {
      ops.push({
        op: "create",
        handle: FLOOR_HANDLE,
        parent: null,
        node: primitiveNode(
          "loading-bay-floor",
          null,
          "cube",
          [0, -0.15, 5],
          [14, 0.3, 18],
          { color: [0.11, 0.16, 0.17, 1], wireframe: false },
        ),
      });
      this.#floorCreated = true;
    }

    const incoming = new Set<number>();
    for (const node of state.projection) {
      incoming.add(node.id);
      const known = this.#known.get(node.id);
      if (known === undefined) {
        ops.push({
          op: "create",
          handle: entityHandle(node.id),
          parent: null,
          node: projectedNode(node),
        });
      } else if (!sameProjectionNode(known, node)) {
        const next = projectedNode(node);
        ops.push({
          op: "update",
          handle: entityHandle(node.id),
          transform: next.transform,
          material: next.material,
          visible: next.visible,
          metadata: next.metadata,
        });
      }
      this.#known.set(node.id, node);
    }

    for (const id of [...this.#known.keys()]) {
      if (!incoming.has(id)) {
        ops.push({ op: "destroy", handle: entityHandle(id) });
        this.#known.delete(id);
      }
    }
    return { ops };
  }

  get trackedEntityCount(): number {
    return this.#known.size;
  }
}

export function entityHandle(id: number): RenderHandle {
  if (!Number.isSafeInteger(id) || id < 0 || id > Number.MAX_SAFE_INTEGER - ENTITY_HANDLE_OFFSET) {
    throw new RangeError("projection entity id is outside the browser-safe integer range");
  }
  return renderHandle(ENTITY_HANDLE_OFFSET + id);
}

function projectedNode(node: RuntimeProjectionNode): RenderNode {
  const door = node.asset.includes("door");
  const probe = node.asset.includes("spatial-probe");
  const wall = node.asset.includes("voxel-wall");
  const scale: readonly [number, number, number] = door
    ? [2.4, 3.4, 0.55]
    : probe
      ? [0.5, 0.5, 0.5]
      : wall
        ? [1, 1, 1]
        : [1.1, 1.8, 1.1];
  const authored = node.translation ?? [0, 0, 0];
  const translation: readonly [number, number, number] = [
    authored[0],
    authored[1] + (probe || wall ? 0 : scale[1] / 2),
    authored[2],
  ];
  const color: Material = door
    ? { color: [0.9, 0.55, 0.16, 1], wireframe: false }
    : probe
      ? { color: [0.26, 0.85, 0.68, 1], wireframe: false }
      : wall
        ? { color: [0.22, 0.38, 0.43, 1], wireframe: false }
        : { color: [0.82, 0.18, 0.14, 1], wireframe: false };
  return primitiveNode(
    node.name,
    node.id,
    probe ? "sphere" : "cube",
    translation,
    scale,
    color,
    node.visible,
  );
}

function primitiveNode(
  label: string,
  source: number | null,
  shape: Exclude<Geometry["shape"], "line">,
  translation: readonly [number, number, number],
  scale: readonly [number, number, number],
  material: Material,
  visible = true,
): RenderNode {
  return {
    geometry: { shape },
    material,
    transform: identityTransform(translation, scale),
    visible,
    layer: "scene",
    metadata: { source: source === null ? null : entityId(source), tags: [], label },
  };
}

function identityTransform(
  translation: readonly [number, number, number],
  scale: readonly [number, number, number],
): Transform {
  return { translation, rotation: [0, 0, 0, 1], scale };
}

function sameProjectionNode(left: RuntimeProjectionNode, right: RuntimeProjectionNode): boolean {
  return (
    left.name === right.name &&
    left.asset === right.asset &&
    left.visible === right.visible &&
    JSON.stringify(left.translation) === JSON.stringify(right.translation)
  );
}
