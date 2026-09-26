// Renderer-neutral light descriptors adapted to retained Three.js lights.

import * as THREE from 'three';
import type { LightDescriptor, RenderHandle } from '@rusty-engine/render-contracts';

export type RendererLightShadowStatus = 'disabled' | 'active' | 'requested_unsupported';

export interface RendererLightReadout {
  readonly descriptor: LightDescriptor;
  readonly handle: RenderHandle;
  readonly parent: RenderHandle | null;
  readonly shadowStatus: RendererLightShadowStatus;
}

export function buildLight(descriptor: LightDescriptor, shadowsEnabled: boolean): THREE.Light {
  const color = new THREE.Color(...descriptor.color);
  let light: THREE.Light;
  switch (descriptor.kind) {
    case 'ambient':
      light = new THREE.AmbientLight(color, descriptor.intensity);
      break;
    case 'directional': {
      const directional = new THREE.DirectionalLight(color, descriptor.intensity);
      directional.add(directional.target);
      directional.target.position.set(...normalizedDirection(descriptor.direction));
      light = directional;
      break;
    }
    case 'point':
      light = new THREE.PointLight(
        color,
        descriptor.intensity,
        descriptor.range ?? 0,
        descriptor.decay,
      );
      light.position.set(...descriptor.position);
      break;
    case 'spot': {
      const spot = new THREE.SpotLight(
        color,
        descriptor.intensity,
        descriptor.range ?? 0,
        descriptor.outerAngleRadians,
        descriptor.penumbra,
        descriptor.decay,
      );
      spot.position.set(...descriptor.position);
      spot.add(spot.target);
      spot.target.position.set(...normalizedDirection(descriptor.direction));
      light = spot;
      break;
    }
  }
  light.visible = descriptor.enabled;
  applyShadowIntent(light, descriptor, shadowsEnabled);
  return light;
}

export function applyLightDescriptor(
  object: THREE.Object3D,
  descriptor: LightDescriptor,
  shadowsEnabled: boolean,
): void {
  const light = object as THREE.Light;
  light.color.setRGB(descriptor.color[0], descriptor.color[1], descriptor.color[2]);
  light.intensity = descriptor.intensity;
  light.visible = descriptor.enabled;
  if (descriptor.kind === 'directional') {
    (light as THREE.DirectionalLight).target.position.set(
      ...normalizedDirection(descriptor.direction),
    );
  } else if (descriptor.kind === 'point') {
    const point = light as THREE.PointLight;
    point.position.set(...descriptor.position);
    point.distance = descriptor.range ?? 0;
    point.decay = descriptor.decay;
  } else if (descriptor.kind === 'spot') {
    const spot = light as THREE.SpotLight;
    spot.position.set(...descriptor.position);
    spot.target.position.set(...normalizedDirection(descriptor.direction));
    spot.distance = descriptor.range ?? 0;
    spot.decay = descriptor.decay;
    spot.angle = descriptor.outerAngleRadians;
    spot.penumbra = descriptor.penumbra;
  }
  applyShadowIntent(light, descriptor, shadowsEnabled);
}

export function lightShadowStatus(
  descriptor: LightDescriptor,
  shadowsEnabled: boolean,
): RendererLightShadowStatus {
  if (!descriptor.enabled || descriptor.shadowIntent === 'disabled') {
    return 'disabled';
  }
  return shadowsEnabled && descriptor.kind !== 'ambient' ? 'active' : 'requested_unsupported';
}

export function projectionParentHandle(
  parent: THREE.Object3D | null,
  handles: ReadonlyMap<RenderHandle, { readonly object: THREE.Object3D }>,
): RenderHandle | null {
  if (parent === null) {
    return null;
  }
  for (const [handle, entry] of handles) {
    if (entry.object === parent) {
      return handle;
    }
  }
  return null;
}

export function disposeLight(object: THREE.Object3D): void {
  object.clear();
  object.removeFromParent();
}

function applyShadowIntent(
  light: THREE.Light,
  descriptor: LightDescriptor,
  shadowsEnabled: boolean,
): void {
  if (descriptor.kind !== 'ambient' && 'castShadow' in light) {
    (light as THREE.DirectionalLight | THREE.PointLight | THREE.SpotLight).castShadow =
      shadowsEnabled && descriptor.enabled && descriptor.shadowIntent === 'requested';
  }
}

function normalizedDirection(direction: readonly [number, number, number]): [number, number, number] {
  const normalized = new THREE.Vector3(...direction).normalize();
  return [normalized.x, normalized.y, normalized.z];
}
