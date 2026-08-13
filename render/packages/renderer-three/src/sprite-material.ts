import * as THREE from 'three';

import type {
  SpriteInstanceDescriptor,
  SpriteMaterialDescriptor,
} from '@rusty-engine/render-contracts';

export interface SpriteMaterialTextures {
  readonly color: THREE.Texture | null;
  readonly normal: THREE.Texture | null;
  readonly depth: THREE.Texture | null;
}

export interface ResolvedSpriteMaterial {
  readonly descriptor: SpriteMaterialDescriptor;
  readonly material: THREE.MeshBasicMaterial | THREE.MeshStandardMaterial;
  readonly castShadow: boolean;
  readonly receiveShadow: boolean;
}

const DEFAULT_SPRITE_MATERIAL: SpriteMaterialDescriptor = Object.freeze({
  lighting: 'unlit',
  normalTexture: null,
  depthTexture: null,
  normalStrength: 1,
  normalBias: 0,
  alpha: Object.freeze({ kind: 'blend' }),
  shadow: 'none',
});

/** Resolve omitted legacy material facts without admitting arbitrary shader source. */
export function resolveSpriteMaterialDescriptor(
  sprite: Pick<SpriteInstanceDescriptor, 'material' | 'shading'>,
): SpriteMaterialDescriptor {
  if (sprite.material !== undefined) return sprite.material;
  if (sprite.shading === 'lit') {
    return { ...DEFAULT_SPRITE_MATERIAL, lighting: 'synthetic' };
  }
  if (sprite.shading === 'shadowed') {
    return { ...DEFAULT_SPRITE_MATERIAL, lighting: 'synthetic', shadow: 'castAndReceive' };
  }
  return DEFAULT_SPRITE_MATERIAL;
}

export function createSpriteMaterial(
  sprite: SpriteInstanceDescriptor,
  textures: SpriteMaterialTextures,
): ResolvedSpriteMaterial {
  const descriptor = resolveSpriteMaterialDescriptor(sprite);
  const variantKey = spriteMaterialVariantKey(sprite);
  const alpha = alphaState(descriptor, sprite);
  const common = {
    color: new THREE.Color(sprite.tint[0], sprite.tint[1], sprite.tint[2]),
    map: textures.color,
    opacity: sprite.tint[3],
    transparent: alpha.transparent,
    alphaTest: alpha.alphaTest,
    depthTest: sprite.depth !== 'depthTestOff',
    depthWrite: sprite.depth !== 'depthWriteOff' && alpha.depthWrite,
    side: THREE.DoubleSide,
    fog: true,
  } as const;

  let material: THREE.MeshBasicMaterial | THREE.MeshStandardMaterial;
  if (descriptor.lighting === 'unlit') {
    material = new THREE.MeshBasicMaterial(common);
  } else {
    const strength = effectiveNormalStrength(descriptor.normalStrength, descriptor.normalBias);
    material = new THREE.MeshStandardMaterial({
      ...common,
      roughness: 0.82,
      metalness: 0,
      normalMap: descriptor.lighting === 'authoredNormal' ? textures.normal : null,
      normalScale: new THREE.Vector2(strength, strength),
      bumpMap: descriptor.lighting === 'authoredDepth'
        ? textures.depth
        : descriptor.lighting === 'derivedGradient'
          ? textures.color
          : null,
      bumpScale: strength,
    });
    if (descriptor.lighting === 'synthetic') {
      installSyntheticNormal(material, descriptor.normalStrength, descriptor.normalBias);
    }
  }
  material.name = `rusty-sprite:${variantKey}`;
  material.userData['rustySpriteMaterialVariant'] = variantKey;
  material.userData['rustySpriteLighting'] = descriptor.lighting;
  material.userData['rustySpriteAlpha'] = descriptor.alpha.kind;
  material.userData['rustySpriteNormalStrength'] = descriptor.normalStrength;
  material.userData['rustySpriteNormalBias'] = descriptor.normalBias;
  return {
    descriptor,
    material,
    castShadow: descriptor.shadow === 'cast' || descriptor.shadow === 'castAndReceive',
    receiveShadow: descriptor.shadow === 'receive' || descriptor.shadow === 'castAndReceive',
  };
}

/**
 * Stable identity for shader-program-affecting sprite facts. Retained sprite
 * materials remain instance-owned; Three may reuse a compiled program for
 * equal variants without sharing mutable tint, opacity, or texture state.
 */
export function spriteMaterialVariantKey(
  sprite: Pick<SpriteInstanceDescriptor, 'depth' | 'material' | 'shading'>,
): string {
  const descriptor = resolveSpriteMaterialDescriptor(sprite);
  return [
    descriptor.lighting,
    descriptor.alpha.kind,
    sprite.depth,
    descriptor.lighting === 'authoredNormal' ? 'normal-map' : 'no-normal-map',
    descriptor.lighting === 'authoredDepth' || descriptor.lighting === 'derivedGradient'
      ? 'bump-map'
      : 'no-bump-map',
  ].join(':');
}

export function updateSpriteMaterialTint(
  material: THREE.MeshBasicMaterial | THREE.MeshStandardMaterial,
  sprite: SpriteInstanceDescriptor,
): void {
  const descriptor = resolveSpriteMaterialDescriptor(sprite);
  const alpha = alphaState(descriptor, sprite);
  material.color.setRGB(sprite.tint[0], sprite.tint[1], sprite.tint[2]);
  material.opacity = sprite.tint[3];
  material.transparent = alpha.transparent;
  material.alphaTest = alpha.alphaTest;
  material.depthWrite = sprite.depth !== 'depthWriteOff' && alpha.depthWrite;
  material.needsUpdate = true;
}

function alphaState(
  descriptor: SpriteMaterialDescriptor,
  sprite: Pick<SpriteInstanceDescriptor, 'tint'>,
): { readonly transparent: boolean; readonly alphaTest: number; readonly depthWrite: boolean } {
  if (descriptor.alpha.kind === 'opaque') {
    return { transparent: sprite.tint[3] < 1, alphaTest: 0, depthWrite: true };
  }
  if (descriptor.alpha.kind === 'mask') {
    return { transparent: sprite.tint[3] < 1, alphaTest: descriptor.alpha.cutoff, depthWrite: true };
  }
  return { transparent: true, alphaTest: 0, depthWrite: false };
}

function effectiveNormalStrength(strength: number, bias: number): number {
  return strength * THREE.MathUtils.clamp(1 - bias * 0.5, 0.5, 1.5);
}

function installSyntheticNormal(
  material: THREE.MeshStandardMaterial,
  strength: number,
  bias: number,
): void {
  material.defines = { ...material.defines, USE_UV: '' };
  material.onBeforeCompile = (shader) => {
    shader.uniforms['rustySpriteNormalStrength'] = { value: strength };
    shader.uniforms['rustySpriteNormalBias'] = { value: bias };
    shader.fragmentShader = shader.fragmentShader
      .replace(
        '#include <common>',
        `#include <common>\nuniform float rustySpriteNormalStrength;\nuniform float rustySpriteNormalBias;`,
      )
      .replace(
        '#include <normal_fragment_maps>',
        `
        vec2 rustySpriteXY = (vUv * 2.0 - 1.0) * rustySpriteNormalStrength;
        float rustySpriteRadius = max(0.001, 1.0 + rustySpriteNormalBias);
        rustySpriteXY /= rustySpriteRadius;
        float rustySpriteZ = sqrt(max(0.001, 1.0 - min(dot(rustySpriteXY, rustySpriteXY), 0.999)));
        vec3 rustyQ0 = dFdx(-vViewPosition);
        vec3 rustyQ1 = dFdy(-vViewPosition);
        vec2 rustySt0 = dFdx(vUv);
        vec2 rustySt1 = dFdy(vUv);
        vec3 rustyQ1Perp = cross(rustyQ1, normal);
        vec3 rustyQ0Perp = cross(normal, rustyQ0);
        vec3 rustyT = rustyQ1Perp * rustySt0.x + rustyQ0Perp * rustySt1.x;
        vec3 rustyB = rustyQ1Perp * rustySt0.y + rustyQ0Perp * rustySt1.y;
        float rustyInvMax = inversesqrt(max(dot(rustyT, rustyT), dot(rustyB, rustyB)));
        mat3 rustyTbn = mat3(rustyT * rustyInvMax, rustyB * rustyInvMax, normal);
        normal = normalize(rustyTbn * vec3(rustySpriteXY, rustySpriteZ));
        `,
      );
  };
  material.customProgramCacheKey = () => 'rusty-sprite-synthetic-v1';
}
