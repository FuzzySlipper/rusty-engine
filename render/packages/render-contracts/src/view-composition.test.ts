import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  MAX_RENDERER_COMPOSITION_CAMERAS,
  MAX_RENDERER_TARGET_DIMENSION,
  RendererViewCompositionValidationError,
  validateRendererViewComposition,
  type RendererViewComposition,
} from './view-composition.js';

type DeepMutable<T> = T extends readonly (infer Item)[]
  ? DeepMutable<Item>[]
  : T extends object
    ? { -readonly [Key in keyof T]: DeepMutable<T[Key]> }
    : T;

function mutableComposition(): DeepMutable<RendererViewComposition> {
  return structuredClone(composition()) as unknown as DeepMutable<RendererViewComposition>;
}

function validateMutable(input: DeepMutable<RendererViewComposition>): RendererViewComposition {
  return validateRendererViewComposition(input as unknown as RendererViewComposition);
}

function composition(): RendererViewComposition {
  return {
    schemaVersion: 1,
    cameras: [{
      id: 'camera.minimap',
      pose: { position: [0, 12, 0], pitchDegrees: -90, yawDegrees: 0 },
      projection: { kind: 'orthographic', verticalSize: 24, near: 0.1, far: 50 },
    }],
    targets: [{
      id: 'target.minimap', revision: 1, width: 256, height: 256,
      color: 'rgba8_srgb', depth: 'depth24', sampling: 'nearest',
    }],
    views: [{
      id: 'view.minimap', cameraId: 'camera.minimap', order: 10,
      target: { kind: 'offscreen', targetId: 'target.minimap', targetRevision: 1 },
      viewport: { x: 0, y: 0, width: 1, height: 1 },
    }],
    presentations: [{
      id: 'presentation.minimap', sourceTargetId: 'target.minimap',
      sourceTargetRevision: 1, order: 20,
      destination: { kind: 'primary', viewport: { x: 0.72, y: 0.68, width: 0.25, height: 0.28 } },
    }],
  };
}

void test('admits a bounded orthographic offscreen view presented on the primary surface', () => {
  assert.equal(validateRendererViewComposition(composition()).views.length, 1);
});

void test('rejects stale target references, feedback destinations, and duplicate producers', () => {
  const stale = mutableComposition();
  stale.presentations[0]!.sourceTargetRevision = 2;
  assert.throws(() => validateMutable(stale), /admitted target revision/u);

  const feedback = structuredClone(composition()) as unknown as {
    presentations: Array<{ destination: { kind: string } }>;
  };
  feedback.presentations[0]!.destination.kind = 'offscreen';
  assert.throws(() => validateRendererViewComposition(feedback as never), /feedback/u);

  const duplicate = mutableComposition();
  duplicate.views.push({ ...duplicate.views[0]!, id: 'view.duplicate' });
  assert.throws(() => validateMutable(duplicate), /producing view/u);
});

void test('enforces exact camera and target boundaries before publication', () => {
  const exact = mutableComposition();
  while (exact.cameras.length < MAX_RENDERER_COMPOSITION_CAMERAS) {
    const index = exact.cameras.length;
    exact.cameras.push({ ...exact.cameras[0]!, id: `camera.boundary-${String(index)}` });
  }
  exact.targets[0]!.width = MAX_RENDERER_TARGET_DIMENSION;
  assert.equal(validateMutable(exact).cameras.length, MAX_RENDERER_COMPOSITION_CAMERAS);

  exact.cameras.push({ ...exact.cameras[0]!, id: 'camera.one-over' });
  assert.throws(
    () => validateMutable(exact),
    RendererViewCompositionValidationError,
  );
});

void test('rejects non-finite transforms and aggregate target pixel exhaustion', () => {
  const nonFinite = mutableComposition();
  nonFinite.cameras[0]!.pose.position[0] = Number.NaN;
  assert.throws(() => validateMutable(nonFinite), /must be finite/u);

  const exhausted = mutableComposition();
  exhausted.targets = Array.from({ length: 3 }, (_, index) => ({
    ...exhausted.targets[0]!,
    id: `target.large-${String(index)}`,
    width: MAX_RENDERER_TARGET_DIMENSION,
    height: MAX_RENDERER_TARGET_DIMENSION,
  }));
  exhausted.views = [];
  exhausted.presentations = [];
  assert.throws(() => validateMutable(exhausted), /aggregate pixels/u);
});

void test('rejects every omitted view and presentation viewport coordinate', () => {
  for (const coordinate of ['x', 'y', 'width', 'height'] as const) {
    const missingViewCoordinate = structuredClone(composition()) as unknown as {
      views: Array<{ viewport: Partial<Record<typeof coordinate, number>> }>;
    };
    delete missingViewCoordinate.views[0]!.viewport[coordinate];
    assert.throws(
      () => validateRendererViewComposition(missingViewCoordinate as never),
      new RegExp(`composition\\.views\\[0\\]\\.viewport\\.${coordinate} must be finite`, 'u'),
    );

    const missingPresentationCoordinate = structuredClone(composition()) as unknown as {
      presentations: Array<{
        destination: { viewport: Partial<Record<typeof coordinate, number>> };
      }>;
    };
    delete missingPresentationCoordinate.presentations[0]!.destination.viewport[coordinate];
    assert.throws(
      () => validateRendererViewComposition(missingPresentationCoordinate as never),
      new RegExp(
        `composition\\.presentations\\[0\\]\\.destination\\.viewport\\.${coordinate} must be finite`,
        'u',
      ),
    );
  }
});
