import assert from 'node:assert/strict';
import test from 'node:test';
import { resolveRendererStoredEditorCamera } from './stored-editor-camera.js';

function forwardFromPose(yawDegrees: number, pitchDegrees: number): readonly [number, number, number] {
  const yaw = yawDegrees * Math.PI / 180;
  const pitch = pitchDegrees * Math.PI / 180;
  const cosPitch = Math.cos(pitch);
  return [Math.sin(yaw) * cosPitch, Math.sin(pitch), -Math.cos(yaw) * cosPitch];
}

void test('stored editor camera resolver owns canonical axis-aligned pose and basis semantics', () => {
  const result = resolveRendererStoredEditorCamera({
    position: [0, 0, 5],
    target: [0, 0, 0],
    up: [0, 1, 0],
    projection: { fovYDegrees: 55, near: 0.05, far: 1000 },
  });

  assert.deepEqual(result, {
    ok: true,
    camera: {
      source: 'stored_editor',
      pose: { position: [0, 0, 5], yawDegrees: 0, pitchDegrees: 0 },
      basis: {
        forward: [0, 0, -1],
        right: [1, 0, 0],
        up: [0, 1, 0],
      },
      projection: { fovYDegrees: 55, near: 0.05, far: 1000 },
    },
  });
});

void test('stored editor camera resolver deterministically normalizes a non-axis-aligned look-at input', () => {
  const result = resolveRendererStoredEditorCamera({
    position: [4, 4, 8],
    target: [0, 0, 0],
    up: [0, 1, 0],
    projection: { fovYDegrees: 60, near: 0.1, far: 500 },
  });

  assert.equal(result.ok, true);
  if (!result.ok) return;
  assert.deepEqual(result.camera.pose.position, [4, 4, 8]);
  assert.ok(Math.abs(result.camera.pose.yawDegrees - -26.565_051_177) < 0.000_001);
  assert.ok(Math.abs(result.camera.pose.pitchDegrees - -24.094_842_552) < 0.000_001);
  assertVectorClose(result.camera.basis.forward, [-0.408_248_29, -0.408_248_29, -0.816_496_58]);
  assertVectorClose(
    forwardFromPose(result.camera.pose.yawDegrees, result.camera.pose.pitchDegrees),
    result.camera.basis.forward,
  );
  assertVectorClose(result.camera.basis.right, [0.894_427_19, 0, -0.447_213_6]);
  assertVectorClose(result.camera.basis.up, [-0.182_574_19, 0.912_870_93, -0.365_148_37]);
});

void test('stored editor camera resolver rejects malformed inputs without returning partial camera state', () => {
  const valid = {
    position: [0, 0, 5] as const,
    target: [0, 0, 0] as const,
    up: [0, 1, 0] as const,
    projection: { fovYDegrees: 55, near: 0.05, far: 1000 },
  };
  const cases = [
    {
      expectedCode: 'non_finite_camera_input',
      input: { ...valid, position: [Number.NaN, 0, 5] as const },
    },
    {
      expectedCode: 'coincident_position_target',
      input: { ...valid, target: valid.position },
    },
    {
      expectedCode: 'collinear_camera_up',
      input: { ...valid, up: [0, 0, -1] as const },
    },
    {
      expectedCode: 'invalid_projection',
      input: { ...valid, projection: { fovYDegrees: 55, near: 10, far: 1 } },
    },
  ] as const;

  for (const fixture of cases) {
    const result = resolveRendererStoredEditorCamera(fixture.input);
    assert.equal(result.ok, false);
    if (result.ok) continue;
    assert.equal(result.diagnostic.code, fixture.expectedCode);
    assert.equal('camera' in result, false);
  }
});

function assertVectorClose(
  actual: readonly [number, number, number],
  expected: readonly [number, number, number],
): void {
  for (let index = 0; index < 3; index += 1) {
    assert.ok(Math.abs((actual[index] ?? 0) - (expected[index] ?? 0)) < 0.000_001);
  }
}
