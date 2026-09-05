import type { LiveDebugCommandDescriptor, LiveDebugRuntimeBinding, LiveDebugUpdateAttribution, LiveDebugWorkerUpdateSnapshot } from './live-debug-client.js';
export declare const LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES = 128;
/** The optional panel's DOM-only placement; it has no product-state meaning. */
export type LiveDebugPanelPresentation = 'inline' | 'dock' | 'overlay';
export interface LiveDebugTranscriptEntry {
    readonly command: string;
    readonly message: string;
    readonly succeeded: boolean;
}
export declare function appendLiveDebugTranscript(entries: readonly LiveDebugTranscriptEntry[], entry: LiveDebugTranscriptEntry): readonly LiveDebugTranscriptEntry[];
export declare function commandSummary(command: LiveDebugCommandDescriptor): string;
export declare function historyCommand(history: readonly string[], cursor: number | null, direction: -1 | 1): {
    readonly cursor: number | null;
    readonly command: string;
};
/** Formats the exact runtime incarnation without assigning any game meaning to it. */
export declare function runtimeIncarnationLabel(runtime: LiveDebugRuntimeBinding | null): string;
/** Summarizes one worker-local publication without comparing its clock to shell clocks. */
export declare function workerUpdateLabel(update: LiveDebugWorkerUpdateSnapshot): string;
/** Labels a completed C# callback and its separate Rust post-callback work. */
export declare function updateAttributionLabel(sample: LiveDebugUpdateAttribution): string;
