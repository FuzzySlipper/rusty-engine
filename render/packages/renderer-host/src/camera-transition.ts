import type {
  CameraBasis,
  CameraPose,
} from '@rusty-engine/render-contracts';
import type {
  RendererCameraSnapshot,
  RendererCameraTransitionReadout,
} from './host-types.js';

function clamp01(value: number): number {
  return Math.max(0, Math.min(1, value));
}

function sampleProgress(
  transition: RendererCameraTransitionReadout,
  elapsedMilliseconds: number,
): number {
  if (!Number.isFinite(elapsedMilliseconds)) {
    throw new TypeError('camera transition elapsedMilliseconds must be finite');
  }
  if (transition.durationMilliseconds <= 0) {
    throw new RangeError('camera transition durationMilliseconds must be positive');
  }
  const linear = clamp01(elapsedMilliseconds / transition.durationMilliseconds);
  return transition.easing === 'smoothStep'
    ? linear * linear * (3 - 2 * linear)
    : linear;
}

function interpolate(from: number, to: number, progress: number): number {
  return from + (to - from) * progress;
}

function interpolateAngle(from: number, to: number, progress: number): number {
  const shortestDelta = ((to - from + 540) % 360) - 180;
  return from + shortestDelta * progress;
}

function basisFromPose(pose: CameraPose): CameraBasis {
  const yaw = (pose.yawDegrees * Math.PI) / 180;
  const pitch = (pose.pitchDegrees * Math.PI) / 180;
  const cosPitch = Math.cos(pitch);
  return {
    forward: [Math.sin(yaw) * cosPitch, Math.sin(pitch), -Math.cos(yaw) * cosPitch],
    right: [Math.cos(yaw), 0, Math.sin(yaw)],
    up: [
      -Math.sin(yaw) * Math.sin(pitch),
      Math.cos(pitch),
      Math.cos(yaw) * Math.sin(pitch),
    ],
  };
}

/**
 * Samples a disposable renderer pose between two caller-owned snapshots.
 * The returned value is presentation state only.
 */
export function sampleCameraTransition(
  transition: RendererCameraTransitionReadout,
  elapsedMilliseconds: number,
): RendererCameraSnapshot {
  const progress = sampleProgress(transition, elapsedMilliseconds);
  if (progress === 0) return transition.from;
  if (progress === 1) return transition.to;
  const from = transition.from.pose;
  const to = transition.to.pose;
  const pose: CameraPose = {
    position: [
      interpolate(from.position[0], to.position[0], progress),
      interpolate(from.position[1], to.position[1], progress),
      interpolate(from.position[2], to.position[2], progress),
    ],
    yawDegrees: interpolateAngle(from.yawDegrees, to.yawDegrees, progress),
    pitchDegrees: interpolate(from.pitchDegrees, to.pitchDegrees, progress),
  };
  return { ...transition.to, pose, basis: basisFromPose(pose) };
}
