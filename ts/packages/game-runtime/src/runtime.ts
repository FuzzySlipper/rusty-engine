import { loadNativeAddon } from "./native-addon.js";
import type {
  BridgeStats,
  JsonValue,
  ProjectApplyReceipt,
  ProjectDecisionBatch,
  ProjectDoorIds,
  ProjectInvocationWave,
  ProjectRuntimeReadout,
} from "./types.js";

interface CreateReceipt extends ProjectDoorIds {
  readonly handle: number;
}

function parseJson<T>(input: string): T {
  return JSON.parse(input) as T;
}

export class ProjectDoorRuntime {
  readonly ids: ProjectDoorIds;
  readonly #handle: number;

  private constructor(handle: number, ids: ProjectDoorIds) {
    this.#handle = handle;
    this.ids = ids;
  }

  static create(initialState: JsonValue): ProjectDoorRuntime {
    const receipt = parseJson<CreateReceipt>(
      loadNativeAddon().createProjectDoorRuntime(JSON.stringify(initialState)),
    );
    return new ProjectDoorRuntime(receipt.handle, {
      actor: receipt.actor,
      switch: receipt.switch,
      door: receipt.door,
    });
  }

  static restore(snapshot: string, ids: ProjectDoorIds): ProjectDoorRuntime {
    const handle = loadNativeAddon().restoreProjectRuntime(snapshot);
    return new ProjectDoorRuntime(handle, ids);
  }

  beginInteraction(actor = this.ids.actor, target = this.ids.switch): ProjectInvocationWave {
    return parseJson<ProjectInvocationWave>(
      loadNativeAddon().beginProjectInteraction(this.#handle, actor, target),
    );
  }

  apply(decisions: ProjectDecisionBatch): ProjectApplyReceipt {
    return parseJson<ProjectApplyReceipt>(
      loadNativeAddon().applyProjectDecisions(this.#handle, JSON.stringify(decisions)),
    );
  }

  advanceBy(ticks: number): ProjectInvocationWave | null {
    return parseJson<ProjectInvocationWave | null>(
      loadNativeAddon().advanceProjectTime(this.#handle, ticks),
    );
  }

  readout(): ProjectRuntimeReadout {
    return parseJson<ProjectRuntimeReadout>(loadNativeAddon().readProjectRuntime(this.#handle));
  }

  save(): string {
    return loadNativeAddon().saveProjectRuntime(this.#handle);
  }

  bridgeStats(): BridgeStats {
    return parseJson<BridgeStats>(loadNativeAddon().readBridgeStats(this.#handle));
  }

  close(): boolean {
    return loadNativeAddon().closeProjectRuntime(this.#handle);
  }
}
