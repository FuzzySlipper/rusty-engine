import { createRequire } from "node:module";
import { fileURLToPath } from "node:url";

interface NativeAddon {
  createProjectDoorRuntime(initialStateJson: string): string;
  beginProjectInteraction(handle: number, actor: number, target: number): string;
  applyProjectDecisions(handle: number, decisionJson: string): string;
  advanceProjectTime(handle: number, ticks: number): string;
  readProjectRuntime(handle: number): string;
  saveProjectRuntime(handle: number): string;
  restoreProjectRuntime(snapshotJson: string): number;
  readBridgeStats(handle: number): string;
  closeProjectRuntime(handle: number): boolean;
}

let cached: NativeAddon | undefined;

export function loadNativeAddon(): NativeAddon {
  if (cached !== undefined) {
    return cached;
  }
  const addonPath = fileURLToPath(new URL("../native/game_bridge_napi.node", import.meta.url));
  const require = createRequire(import.meta.url);
  cached = require(addonPath) as NativeAddon;
  return cached;
}
