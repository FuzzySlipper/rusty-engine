import assert from 'node:assert/strict';
import test from 'node:test';

import { renderHandle, type RenderFrameDiff } from '@rusty-engine/render-contracts';
import {
  type RendererEditorViewportChannelReceipt,
  type RendererSurfaceSubmissionSample,
} from '@rusty-engine/renderer-host';

import {
  submitStudioViewportFrame,
  type StudioViewportFrameUpdateKind,
} from './viewport-submission.js';

const COMPLETE_FRAME: RenderFrameDiff = {
  schemaVersion: 1,
  ops: [{
    op: 'defineVoxelObject',
    asset: {
      asset: 'voxel-object/submission-proof',
      contentHash: 'sha256:submission-proof',
      meshes: [{
        payload: {
          layout: {
            vertexCount: 3,
            indexCount: 3,
            indexWidth: 'u32',
            attributes: [
              { name: 'position', components: 3, kind: 'f32' },
              { name: 'normal', components: 3, kind: 'f32' },
            ],
          },
          groups: [{ materialSlot: 7, start: 0, count: 3 }],
          bounds: { min: [0, 0, 0], max: [1, 1, 0] },
          source: {
            kind: 'inline',
            positions: [0, 0, 0, 1, 0, 0, 0, 1, 0],
            normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
            indices: [0, 1, 2],
          },
          provenance: 'voxelObject',
        },
      }],
      frames: [{ id: 'default', mesh: 0 }],
      materialSlots: [{ slot: 7, material: 'material/submission-proof' }],
    },
  }],
};

const INCREMENTAL_FRAME: RenderFrameDiff = {
  schemaVersion: 1,
  ops: [{
    op: 'setVoxelObjectFrame',
    handle: renderHandle(41),
    frame: 0,
  }],
};

function submission(renderSequence: number): RendererSurfaceSubmissionSample {
  const available = (scope: 'liveResident' | 'perSubmission', value: number) =>
    Object.freeze({ scope, status: 'available' as const, value });
  const sample: RendererSurfaceSubmissionSample = {
    schemaVersion: 1,
    renderSequence,
    source: 'explicit',
    sourceTimeMs: renderSequence,
    frameIntervalMs: renderSequence === 1 ? null : 1,
    frameIntervalStatus: renderSequence === 1 ? 'firstFrame' : 'available',
    backendSubmissionDurationMs: 0.25,
    backendSubmissionDurationStatus: 'available',
    statistics: Object.freeze({
      schemaVersion: 1,
      drawCallCount: available('perSubmission', renderSequence),
      renderHandleCount: available('liveResident', renderSequence),
      geometryResourceCount: available('liveResident', renderSequence),
      materialResourceCount: available('liveResident', renderSequence),
      textureResourceCount: available('liveResident', 0),
      animatedInstanceCount: available('liveResident', 0),
      triangleCount: available('perSubmission', renderSequence * 2),
    }),
  };
  return Object.freeze(sample);
}

function receipt(applied: boolean): RendererEditorViewportChannelReceipt {
  const value: RendererEditorViewportChannelReceipt = {
    applied,
    channel: 'authored',
    diagnostics: applied
      ? []
      : [{
          channel: 'authored',
          code: 'invalid_frame',
          message: 'fixture rejected',
          recoverable: true,
        }],
    generation: applied ? 1 : 0,
    snapshotHash: applied ? 'accepted' : 'unchanged',
  };
  return Object.freeze(value);
}

class SubmissionSurface {
  readonly frames: Array<{
    readonly frame: RenderFrameDiff;
    readonly method: 'apply' | 'replace';
  }> = [];
  renderCount = 0;
  readCount = 0;
  latest = submission(7);
  reject = false;

  applyAuthoredFrame(frame: RenderFrameDiff): RendererEditorViewportChannelReceipt {
    this.frames.push({ frame, method: 'apply' });
    return receipt(!this.reject);
  }

  replaceFrame(frame: RenderFrameDiff): RendererEditorViewportChannelReceipt {
    this.frames.push({ frame, method: 'replace' });
    return receipt(!this.reject);
  }

  renderOnce(): void {
    this.renderCount += 1;
    this.latest = submission(this.latest.renderSequence + 1);
  }

  submission(): RendererSurfaceSubmissionSample {
    this.readCount += 1;
    return this.latest;
  }
}

test('Studio associates complete incremental and presentation generations with the new submission', () => {
  const cases: ReadonlyArray<{
    readonly updateKind: StudioViewportFrameUpdateKind;
    readonly method: 'apply' | 'replace';
    readonly frame: RenderFrameDiff;
  }> = [
    { updateKind: 'complete', method: 'replace', frame: COMPLETE_FRAME },
    { updateKind: 'incremental', method: 'apply', frame: INCREMENTAL_FRAME },
    { updateKind: 'presentation', method: 'replace', frame: COMPLETE_FRAME },
  ];

  for (const [index, fixture] of cases.entries()) {
    const surface = new SubmissionSurface();
    const stale = surface.latest;
    const generation = 40 + index;
    const result = submitStudioViewportFrame(
      surface,
      fixture.frame,
      generation,
      fixture.updateKind,
    );

    assert.equal(result.receipt.applied, true);
    assert.equal(surface.frames[0]?.method, fixture.method);
    assert.equal(surface.frames[0]?.frame, fixture.frame);
    assert.equal(surface.renderCount, 1);
    assert.equal(surface.readCount, 1);
    assert.equal(result.event?.generation, generation);
    assert.equal(result.event?.updateKind, fixture.updateKind);
    assert.equal(result.event?.submission.renderSequence, stale.renderSequence + 1);
    assert.notEqual(result.event?.submission, stale);
    assert.equal(Object.isFrozen(result), true);
    assert.equal(Object.isFrozen(result.event), true);
    assert.equal(Object.isFrozen(result.event?.submission), true);
  }
});

test('a rejected frame publishes no submission observation and leaves the prior sample unread', () => {
  const surface = new SubmissionSurface();
  surface.reject = true;
  const previous = surface.latest;

  const result = submitStudioViewportFrame(surface, COMPLETE_FRAME, 9, 'complete');

  assert.equal(result.receipt.applied, false);
  assert.equal(result.event, null);
  assert.equal(surface.renderCount, 0);
  assert.equal(surface.readCount, 0);
  assert.equal(surface.latest, previous);
});
