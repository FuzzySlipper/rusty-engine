import type { LiveDebugCommandDescriptor } from '@rusty-engine/live-debug-client';

export const LIVE_DEBUG_PANEL_MAX_TRANSCRIPT_ENTRIES = 128;

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

export function commandSummary(command: LiveDebugCommandDescriptor): string {
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
