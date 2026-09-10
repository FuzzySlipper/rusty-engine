import * as THREE from 'three';
import type { RendererCompositionCamera } from '@rusty-engine/render-contracts';
import { applyRendererThreeCameraBasis, applyRendererThreeCameraPose } from './camera-pose.js';

interface Sample {
  readonly time: number;
  readonly position: THREE.Vector3;
  readonly orientation: THREE.Quaternion;
}

// A renderer storage budget, not a publication-rate or product-delay restriction.
// If the requested history exceeds it, hold the oldest available sample.
const HISTORY_CAPACITY = 64;

/** Renderer-only history; never feeds interpolated state back to simulation. */
export class CameraMotion {
  #samples: Sample[] = [];
  #motion: RendererCompositionCamera['motion'];
  #offset = 0;
  #cursor = -Infinity;
  #latest: Sample | undefined;
  #receivedAt = 0;
  #sampledAt = 0;

  receive(descriptor: RendererCompositionCamera, arrivalSeconds: number): void {
    const motion = descriptor.motion;
    if (motion !== undefined && motion.sampleId === this.#motion?.sampleId) return;
    const camera = new THREE.Camera();
    applyRendererThreeCameraPose(camera, descriptor.pose);
    if (descriptor.basis !== undefined) applyRendererThreeCameraBasis(camera, descriptor.basis);
    const next: Sample = {
      time: motion?.sampleTimeSeconds ?? 0,
      position: camera.position.clone(), orientation: camera.quaternion.clone(),
    };
    // A paused source clock (or delivery stall beyond the buffer) must not
    // leave the local clock permanently ahead, bypassing interpolation forever.
    const clockDiscontinuity = motion !== undefined && this.#latest !== undefined
      && arrivalSeconds - this.#receivedAt - (next.time - this.#latest.time) > 2 * motion.delaySeconds;
    const reset = clockDiscontinuity || motion === undefined || this.#motion === undefined || motion.cut
      || next.time <= (this.#latest?.time ?? -Infinity)
      || motion.interpolation !== this.#motion.interpolation
      || motion.delaySeconds !== this.#motion.delaySeconds;
    this.#motion = motion;
    this.#receivedAt = arrivalSeconds;
    this.#latest = next;
    if (reset) {
      this.#samples = [next];
      this.#offset = arrivalSeconds - next.time;
      this.#cursor = next.time;
    } else {
      // Relative clock mapping only: this is not a measurement of network latency.
      this.#offset = Math.min(this.#offset, arrivalSeconds - next.time);
      this.#samples.push(next);
      if (this.#samples.length > HISTORY_CAPACITY) this.#samples.shift();
    }
  }

  needsFrame(): boolean {
    return this.#motion !== undefined && this.#latest !== undefined
      && this.#cursor < this.#latest.time;
  }

  apply(camera: THREE.Camera, nowSeconds: number): void {
    const latest = this.#latest;
    if (latest === undefined) return;
    this.#sampledAt = nowSeconds;
    const motion = this.#motion;
    if (motion === undefined) {
      camera.position.copy(latest.position);
      camera.quaternion.copy(latest.orientation);
    } else {
      this.#cursor = Math.max(this.#cursor, Math.min(latest.time,
        nowSeconds - this.#offset - motion.delaySeconds));
      while (this.#samples.length > 2 && this.#samples[1]!.time <= this.#cursor) {
        this.#samples.shift();
      }
      const first = this.#samples[0]!;
      const second = this.#samples[1] ?? first;
      const fraction = second.time > first.time
        ? THREE.MathUtils.clamp((this.#cursor - first.time) / (second.time - first.time), 0, 1) : 1;
      camera.position.lerpVectors(first.position, second.position, fraction);
      if (motion.interpolation === 'pose') {
        camera.quaternion.slerpQuaternions(first.orientation, second.orientation, fraction);
      } else {
        camera.quaternion.copy(latest.orientation);
      }
    }
    camera.up.set(0, 1, 0).applyQuaternion(camera.quaternion);
    camera.updateMatrixWorld(true);
  }

  readout() {
    return Object.freeze({
      sampleId: this.#motion?.sampleId ?? null,
      sourceTimeSeconds: this.#motion?.sampleTimeSeconds ?? null,
      presentationTimeSeconds: this.#motion === undefined ? null : this.#cursor,
      receivedAtMs: this.#receivedAt * 1000,
      sampledAtMs: this.#sampledAt * 1000,
      retainedSampleCount: this.#samples.length,
    });
  }
}
