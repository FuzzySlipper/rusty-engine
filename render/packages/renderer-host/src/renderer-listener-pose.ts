import type {
  RendererCompositionCamera,
  RendererViewComposition,
} from '@rusty-engine/render-contracts';
import type { RendererAudioListenerPose } from './audio-host.js';

export interface RendererListenerFallbackCamera {
  readonly pose: {
    readonly position: readonly [number, number, number];
    readonly pitchDegrees: number;
    readonly yawDegrees: number;
  };
  readonly basis?: {
    readonly forward: readonly [number, number, number];
    readonly up: readonly [number, number, number];
  };
}

/**
 * Resolves the listener from the renderer's displayed primary view. A
 * composition with no primary view deliberately falls back to the controls
 * snapshot so clearing the active product camera cannot retain a stale pose.
 */
export function resolveRendererAudioListenerPose(
  composition: Pick<RendererViewComposition, 'cameras' | 'views'>,
  fallback: RendererListenerFallbackCamera,
): RendererAudioListenerPose {
  const primaryView = composition.views
    .filter((view) => view.target.kind === 'primary')
    .sort(compareRendererViews)[0];
  const camera = primaryView === undefined
    ? undefined
    : composition.cameras.find((candidate) => candidate.id === primaryView.cameraId);
  return listenerPoseFromCamera(camera ?? fallback);
}

function compareRendererViews(
  left: RendererViewComposition['views'][number],
  right: RendererViewComposition['views'][number],
): number {
  return left.order - right.order || left.id.localeCompare(right.id);
}

function listenerPoseFromCamera(
  camera: Pick<RendererCompositionCamera, 'pose' | 'basis'> | RendererListenerFallbackCamera,
): RendererAudioListenerPose {
  const { pose } = camera;
  if (camera.basis !== undefined) {
    return {
      position: [...pose.position] as [number, number, number],
      forward: normalize(camera.basis.forward),
      up: normalize(camera.basis.up),
    };
  }
  const yaw = degreesToRadians(pose.yawDegrees);
  const pitch = degreesToRadians(pose.pitchDegrees);
  return {
    position: [...pose.position] as [number, number, number],
    forward: [Math.sin(yaw) * Math.cos(pitch), Math.sin(pitch), -Math.cos(yaw) * Math.cos(pitch)],
    up: [-Math.sin(yaw) * Math.sin(pitch), Math.cos(pitch), Math.cos(yaw) * Math.sin(pitch)],
  };
}

function normalize(vector: readonly [number, number, number]): [number, number, number] {
  const length = Math.hypot(vector[0], vector[1], vector[2]);
  return [vector[0] / length, vector[1] / length, vector[2] / length];
}

function degreesToRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}
