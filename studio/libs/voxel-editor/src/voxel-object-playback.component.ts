import {
  ChangeDetectionStrategy,
  Component,
  effect,
  input,
  output,
  signal,
} from '@angular/core';
import { FormsModule } from '@angular/forms';
import type {
  VoxelObjectAssetAuthoringReadout,
  VoxelObjectFrameSelection,
  VoxelObjectInstancePlaybackReadout,
  VoxelObjectInstanceReadout,
  VoxelObjectLoopMode,
  VoxelObjectPlaybackCommand,
} from '@rusty-engine/studio-adapter-client';

import type { VoxelEditorAction } from './voxel-editor-model.js';

@Component({
  selector: 'rusty-voxel-object-playback',
  standalone: true,
  imports: [FormsModule],
  templateUrl: './voxel-object-playback.component.html',
  styleUrl: './voxel-object-playback.component.css',
  changeDetection: ChangeDetectionStrategy.OnPush,
})
export class VoxelObjectPlaybackComponent {
  readonly instance = input<VoxelObjectInstanceReadout | null>(null);
  readonly asset = input<VoxelObjectAssetAuthoringReadout | null>(null);
  readonly playback = input<VoxelObjectInstancePlaybackReadout | null>(null);
  readonly busy = input(false);
  readonly action = output<VoxelEditorAction>();
  readonly playing = signal(false);

  clipId = '';
  frameIndex = 0;
  loopMode: VoxelObjectLoopMode = 'repeat';

  #instanceKey: string | null = null;

  constructor() {
    effect(() => {
      const entry = this.instance();
      const asset = this.asset();
      const nextKey = entry === null
        ? null
        : `${String(entry.ownerEntityId)}:${entry.sceneId}:${entry.instance.instanceId}`;
      if (nextKey !== this.#instanceKey) {
        this.#instanceKey = nextKey;
        this.playing.set(false);
        this.clipId = selectedInitialClip(entry, asset);
        this.frameIndex = selectedInitialFrame(entry, this.clipId);
        if (entry !== null && this.clipId !== '') {
          queueMicrotask(() => {
            if (this.#instanceKey === nextKey && !this.busy()) this.scrub(this.frameIndex);
          });
        }
      }

      const playback = this.playback();
      if (entry === null || playback?.instanceId !== entry.instance.instanceId) {
        this.playing.set(false);
        return;
      }
      if (playback.clipId !== null) this.clipId = playback.clipId;
      if (playback.clipFrame !== null) this.frameIndex = playback.clipFrame;
      this.loopMode = playback.loopMode;
      this.playing.set(playback.status === 'playing' && !playback.ended);
    });
  }

  chooseClip(clipId: string): void {
    this.clipId = clipId;
    this.frameIndex = 0;
    this.scrub(0);
  }

  chooseLoopMode(loopMode: VoxelObjectLoopMode): void {
    this.loopMode = loopMode;
    this.scrub();
  }

  frameMax(): number {
    const clip = this.asset()?.clips.find((candidate) => candidate.clipId === this.clipId);
    return Math.max(0, (clip?.frames.length ?? 1) - 1);
  }

  scrub(frameIndex = this.frameIndex): void {
    const entry = this.instance();
    const clip = this.asset()?.clips.find((candidate) => candidate.clipId === this.clipId);
    if (entry === null || clip === undefined) return;
    this.playing.set(false);
    this.frameIndex = Math.min(Math.max(0, integer(frameIndex)), this.frameMax());
    this.#emit(entry, {
      kind: 'scrub',
      clipId: clip.clipId,
      clipFrame: this.frameIndex,
      loopMode: this.loopMode,
    });
  }

  play(): void {
    const entry = this.instance();
    const playback = this.playback();
    if (
      entry === null
      || playback?.instanceId !== entry.instance.instanceId
      || playback.status !== 'paused'
    ) return;
    this.playing.set(true);
    this.#emit(entry, { kind: 'play' });
  }

  pause(): void {
    const entry = this.instance();
    const playback = this.playback();
    this.playing.set(false);
    if (entry === null || playback?.status !== 'playing') return;
    this.#emit(entry, { kind: 'pause' });
  }

  restore(): void {
    const entry = this.instance();
    this.playing.set(false);
    if (entry !== null) this.#emit(entry, { kind: 'stop' });
  }

  durableFrameLabel(frame: VoxelObjectFrameSelection): string {
    return frame.kind === 'default'
      ? 'default frame'
      : `${frame.clipId} frame ${String(frame.frameIndex)}`;
  }

  #emit(entry: VoxelObjectInstanceReadout, command: VoxelObjectPlaybackCommand): void {
    this.action.emit({
      kind: 'previewObjectInstance',
      sceneId: entry.sceneId,
      instanceId: entry.instance.instanceId,
      nowMicroseconds: Math.round(performance.now() * 1_000),
      command,
    });
  }
}

function selectedInitialClip(
  entry: VoxelObjectInstanceReadout | null,
  asset: VoxelObjectAssetAuthoringReadout | null,
): string {
  if (entry?.instance.frame.kind === 'clip') {
    const durableClip = entry.instance.frame.clipId;
    if (asset?.clips.some((clip) => clip.clipId === durableClip) === true) return durableClip;
  }
  return asset?.defaultClip ?? asset?.clips[0]?.clipId ?? '';
}

function selectedInitialFrame(entry: VoxelObjectInstanceReadout | null, clipId: string): number {
  return entry?.instance.frame.kind === 'clip' && entry.instance.frame.clipId === clipId
    ? entry.instance.frame.frameIndex
    : 0;
}

function integer(value: number): number {
  return Number.isFinite(value) ? Math.round(value) : 0;
}
