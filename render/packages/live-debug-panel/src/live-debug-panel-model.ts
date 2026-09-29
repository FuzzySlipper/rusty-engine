import type {
  ProductDevDebugCommandDescriptor,
  ProductDevRuntimeBinding,
  ProductDevUpdateAttribution,
} from '@rusty-engine/live-debug-client';

export const LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES = 128;

/** The optional panel's DOM-only placement; it has no product-state meaning. */
export type LiveDebugPanelPresentation = 'inline' | 'dock' | 'overlay';

export interface LiveDebugTranscriptEntry {
  readonly command: string;
  readonly message: string;
  readonly succeeded: boolean;
}

export function appendLiveDebugTranscript(
  entries: readonly LiveDebugTranscriptEntry[],
  entry: LiveDebugTranscriptEntry,
): readonly LiveDebugTranscriptEntry[] {
  return [...entries, entry].slice(-LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES);
}

export function commandSummary(command: ProductDevDebugCommandDescriptor): string {
  const parameters = command.parameters.map((parameter) => `${parameter.name}: ${parameter.type}`);
  return parameters.length === 0 ? command.name : `${command.name} ${parameters.join(' ')}`;
}

export function historyCommand(
  history: readonly string[],
  cursor: number | null,
  direction: -1 | 1,
): { readonly cursor: number | null; readonly command: string } {
  if (history.length === 0) return { cursor: null, command: '' };
  const current = cursor ?? history.length;
  const next = Math.min(history.length, Math.max(0, current + direction));
  return next === history.length
    ? { cursor: null, command: '' }
    : { cursor: next, command: history[next] ?? '' };
}

/** Formats the exact runtime incarnation without assigning any game meaning to it. */
export function runtimeIncarnationLabel(runtime: ProductDevRuntimeBinding | null): string {
  return runtime === null
    ? 'runtime unavailable'
    : `runtime ${runtime.instanceId}/${runtime.generation}/${runtime.controlRevision}`;
}

/** Labels a completed C# callback and its separate Rust post-callback work. */
export function updateAttributionLabel(sample: ProductDevUpdateAttribution): string {
  return `${runtimeIncarnationLabel(sample.runtime)} · simulation step ${sample.simulationStep} · admitted ${sample.admittedStepCount} · callback ${sample.callbackDurationUs} us · post-callback ${sample.postCallbackDurationUs} us`;
}
