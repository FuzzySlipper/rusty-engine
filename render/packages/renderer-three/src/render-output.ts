import { zlibSync } from 'fflate';
import * as THREE from 'three';
import { GLTFExporter } from 'three/examples/jsm/exporters/GLTFExporter.js';
import * as SkeletonUtils from 'three/examples/jsm/utils/SkeletonUtils.js';
import type { RenderFrameDiff, RenderHandle } from '@rusty-engine/render-contracts';
import { synchronizeCameraRelativeViewmodelCamera } from './viewmodel-camera.js';
import {
  ThreeRenderer,
  type ThreeRendererAnimationClipSource,
  type ThreeRendererIsolatedCaptureScene,
} from './three-renderer.js';

const OUTPUT_VERTEX_SHADER = /* glsl */`
varying vec2 vUv;
void main() {
	vUv = uv;
	gl_Position = projectionMatrix * modelViewMatrix * vec4( position, 1.0 );
}
`;

const OUTPUT_FRAGMENT_SHADER = /* glsl */`
uniform sampler2D tColor;
uniform float exposure;
uniform bool useAces;
varying vec2 vUv;
${THREE.ShaderChunk.tonemapping_pars_fragment}

void main() {
	vec4 color = texture2D( tColor, vUv );
	float alpha = clamp( color.a, 0.0, 1.0 );
	vec3 linearColor = alpha > 0.0 ? color.rgb / alpha : vec3( 0.0 );
	if ( useAces ) linearColor = ACESFilmicToneMapping( linearColor );
	else linearColor *= exposure;
	vec4 encoded = sRGBTransferOETF( vec4( max( linearColor, vec3( 0.0 ) ), alpha ) );
	gl_FragColor = vec4( clamp( encoded.rgb, 0.0, 1.0 ), alpha );
}
`;

export interface RenderOutputPose {
  readonly handle: RenderHandle;
  readonly clip: string;
  readonly normalizedTime: number;
}

export interface RenderOutputCaptureOptions {
  readonly width: number;
  readonly height: number;
  /** Normalized linear RGBA channels, matching Engine native color semantics. */
  readonly background: readonly [number, number, number, number];
  readonly useCameraBackground?: boolean;
  readonly exposure?: number;
  readonly toneMapping?: THREE.ToneMapping;
  readonly samples?: number;
  readonly pose?: RenderOutputPose | null;
}

/** A concrete feature in the selected scene cannot be represented by glTF 2.0. */
export class RenderOutputUnsupportedError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'RenderOutputUnsupportedError';
  }
}

interface SavedRendererState {
  readonly renderTarget: THREE.WebGLRenderTarget | null;
  readonly activeCubeFace: number;
  readonly activeMipmapLevel: number;
  readonly viewport: THREE.Vector4;
  readonly scissor: THREE.Vector4;
  readonly scissorTest: boolean;
  readonly clearColor: THREE.Color;
  readonly clearAlpha: number;
  readonly autoClear: boolean;
  readonly autoClearColor: boolean;
  readonly autoClearDepth: boolean;
  readonly autoClearStencil: boolean;
  readonly toneMapping: THREE.ToneMapping;
  readonly toneMappingExposure: number;
  readonly outputColorSpace: string;
  readonly localClippingEnabled: boolean;
  readonly xrEnabled: boolean;
}

/**
 * Reusable Engine output executor. It reconstructs each output from a complete
 * retained frame, so output never borrows the visible renderer's canvas.
 */
export class RendererOutputExecutor {
  readonly #webgl: THREE.WebGLRenderer;
  readonly #retained: ThreeRenderer;
  #target: THREE.WebGLRenderTarget | null = null;
  #sceneTarget: THREE.WebGLRenderTarget | null = null;
  #targetSamples = -1;
  #captureTail: Promise<void> = Promise.resolve();
  #captureActive = false;
  #disposed = false;
  readonly #conversionScene = new THREE.Scene();
  readonly #conversionCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0, 1);
  readonly #conversionGeometry = new THREE.PlaneGeometry(2, 2);
  readonly #conversionMaterial = new THREE.ShaderMaterial({
    uniforms: {
      tColor: { value: null },
      exposure: { value: 1 },
      useAces: { value: false },
      toneMappingExposure: { value: 1 },
    },
    vertexShader: OUTPUT_VERTEX_SHADER,
    fragmentShader: OUTPUT_FRAGMENT_SHADER,
    depthTest: false,
    depthWrite: false,
    blending: THREE.NoBlending,
    toneMapped: false,
  });

  constructor(webglRenderer: THREE.WebGLRenderer, retainedRenderer: ThreeRenderer) {
    this.#webgl = webglRenderer;
    this.#retained = retainedRenderer;
    this.#conversionScene.name = 'render-output-conversion';
    this.#conversionCamera.position.z = 1;
    this.#conversionScene.add(new THREE.Mesh(this.#conversionGeometry, this.#conversionMaterial));
  }

  /** Render one frozen retained frame into an isolated RGBA8 PNG. */
  capture(
    frame: RenderFrameDiff,
    camera: THREE.Camera,
    options: RenderOutputCaptureOptions,
  ): Promise<Uint8Array> {
    if (this.#disposed) return Promise.reject(new Error('renderer output executor is disposed'));
    const result = this.#captureTail.then(() => this.#captureNow(frame, camera, options));
    this.#captureTail = result.then(() => undefined, () => undefined);
    return result;
  }

  /** Export the selected retained hierarchy as a self-contained binary GLB. */
  async exportGlb(
    frame: RenderFrameDiff,
    rootHandle: RenderHandle,
    includeAnimations: boolean,
  ): Promise<Uint8Array> {
    if (this.#disposed) throw new Error('renderer output executor is disposed');
    const isolated = this.#retained.createIsolatedCaptureScene(frame);
    try {
      const sourceObject = isolated.objectFor(rootHandle);
      const sourceScene = isolated.sceneFor(rootHandle);
      if (sourceObject === undefined || sourceScene === undefined) {
        throw new RangeError(`exportGlb: retained handle ${String(rootHandle)} is not in the captured frame`);
      }

      const clonedHierarchy = cloneSelectedHierarchy(sourceScene, sourceObject);
      validateGltfHierarchy(clonedHierarchy.root);

      const clips = includeAnimations
        ? prepareAnimationClips(
          isolated.animationClipSourcesInSubtree?.(rootHandle) ?? [],
          clonedHierarchy.cloneFor,
        )
        : [];
      const exported = await new GLTFExporter().parseAsync(clonedHierarchy.root, {
        binary: true,
        onlyVisible: false,
        animations: clips,
      });
      if (!(exported instanceof ArrayBuffer)) {
        throw new Error('GLTFExporter returned JSON when binary GLB output was requested');
      }
      return new Uint8Array(exported);
    } finally {
      isolated.dispose();
    }
  }

  /** Release the reusable render target after any active synchronous readback. */
  dispose(): void {
    if (this.#disposed) return;
    this.#disposed = true;
    if (!this.#captureActive) this.#disposeTarget();
  }

  async #captureNow(
    frame: RenderFrameDiff,
    camera: THREE.Camera,
    options: RenderOutputCaptureOptions,
  ): Promise<Uint8Array> {
    if (this.#disposed) throw new Error('renderer output executor is disposed');
    const { width, height, background, samples, exposure, toneMapping } = validateCaptureOptions(options, this.#webgl);
    const isolated = this.#retained.createIsolatedCaptureScene(frame);
    this.#captureActive = true;
    let readback: Uint8Array | null = null;
    try {
      if (options.pose !== undefined && options.pose !== null) {
        if (isolated.sampleAnimatedMesh === undefined) {
          throw new Error('capture: isolated renderer does not support deterministic animated pose sampling');
        }
        isolated.sampleAnimatedMesh(options.pose.handle, options.pose.clip, options.pose.normalizedTime);
      }

      isolated.setViewportSize?.(width, height);
      const worldCamera = cloneCameraForCapture(camera, width, height);
      const viewmodelCamera = new THREE.PerspectiveCamera();
      synchronizeCameraRelativeViewmodelCamera(worldCamera, viewmodelCamera, width / height);
      const viewmodelScene = isolated.viewmodelScene ?? new THREE.Scene();

      if (options.useCameraBackground !== true) isolated.scene.background = null;
      viewmodelScene.background = null;
      isolated.prepareSpritesForCamera?.(worldCamera, isolated.scene);
      isolated.prepareStaticInstanceBatches?.(worldCamera);
      isolated.prepareSpritesForCamera?.(viewmodelCamera, viewmodelScene);

      const { sceneTarget, outputTarget } = this.#ensureTargets(width, height, samples);
      const state = saveRendererState(this.#webgl);
      try {
        this.#webgl.setRenderTarget(sceneTarget);
        // Render-target viewports are already expressed in physical pixels and
        // are installed by setRenderTarget. WebGLRenderer.setViewport instead
        // scales by the canvas pixel ratio, which can shrink an offscreen
        // capture to a small lower-left sub-rectangle.
        this.#webgl.setScissorTest(false);
        const clearColor = new THREE.Color().setRGB(
          background[0], background[1], background[2], THREE.LinearSRGBColorSpace,
        );
        // WebGLBackground/WebGLState premultiply clear RGB when the shared
        // renderer's context uses premultiplied alpha; keep NativeColor linear.
        this.#webgl.setClearColor(clearColor, background[3]);
        this.#webgl.toneMapping = THREE.NoToneMapping;
        this.#webgl.toneMappingExposure = 1;
        this.#webgl.outputColorSpace = THREE.LinearSRGBColorSpace;
        this.#webgl.xr.enabled = false;
        this.#webgl.autoClear = false;
        this.#webgl.clear(true, true, true);
        this.#webgl.render(isolated.scene, worldCamera);
        this.#webgl.clearDepth();
        this.#webgl.render(viewmodelScene, viewmodelCamera);

        this.#conversionMaterial.uniforms['tColor']!.value = sceneTarget.texture;
        this.#conversionMaterial.uniforms['exposure']!.value = exposure;
        this.#conversionMaterial.uniforms['toneMappingExposure']!.value = exposure;
        this.#conversionMaterial.uniforms['useAces']!.value = toneMapping === THREE.ACESFilmicToneMapping;
        this.#webgl.setRenderTarget(outputTarget);
        this.#webgl.setScissorTest(false);
        this.#webgl.render(this.#conversionScene, this.#conversionCamera);

        readback = new Uint8Array(width * height * 4);
        this.#webgl.readRenderTargetPixels(outputTarget, 0, 0, width, height, readback);
      } finally {
        restoreRendererState(this.#webgl, state);
      }
    } finally {
      isolated.dispose();
      this.#captureActive = false;
      if (this.#disposed) this.#disposeTarget();
    }

    if (readback === null) throw new Error('capture readback did not complete');
    return encodeRgbaPng(readback, width, height);
  }

  #ensureTargets(
    width: number,
    height: number,
    samples: number,
  ): { readonly sceneTarget: THREE.WebGLRenderTarget; readonly outputTarget: THREE.WebGLRenderTarget } {
    if (!this.#webgl.extensions.has('EXT_color_buffer_float')
      && !this.#webgl.extensions.has('EXT_color_buffer_half_float')) {
      throw new Error('capture requires a renderable half-float color buffer for exposure and tone mapping');
    }
    if (this.#sceneTarget === null || this.#targetSamples !== samples) {
      this.#sceneTarget?.dispose();
      this.#target?.dispose();
      this.#sceneTarget = new THREE.WebGLRenderTarget(width, height, {
        format: THREE.RGBAFormat,
        type: THREE.HalfFloatType,
        depthBuffer: true,
        stencilBuffer: false,
        colorSpace: THREE.LinearSRGBColorSpace,
        minFilter: THREE.NearestFilter,
        magFilter: THREE.NearestFilter,
      });
      this.#sceneTarget.samples = samples;
      this.#target = new THREE.WebGLRenderTarget(width, height, {
        format: THREE.RGBAFormat,
        type: THREE.UnsignedByteType,
        depthBuffer: false,
        stencilBuffer: false,
        colorSpace: THREE.LinearSRGBColorSpace,
      });
      this.#target.samples = 0;
      this.#target.texture.colorSpace = THREE.LinearSRGBColorSpace;
      this.#sceneTarget.texture.colorSpace = THREE.LinearSRGBColorSpace;
      this.#targetSamples = samples;
    } else if (this.#sceneTarget.width !== width || this.#sceneTarget.height !== height) {
      this.#sceneTarget.setSize(width, height);
      this.#target?.setSize(width, height);
    }
    if (this.#target === null) {
      throw new Error('capture output target was not initialized');
    }
    this.#sceneTarget.texture.colorSpace = THREE.LinearSRGBColorSpace;
    this.#target.texture.colorSpace = THREE.LinearSRGBColorSpace;
    return { sceneTarget: this.#sceneTarget, outputTarget: this.#target };
  }

  #disposeTarget(): void {
    this.#target?.dispose();
    this.#sceneTarget?.dispose();
    this.#target = null;
    this.#sceneTarget = null;
    this.#targetSamples = -1;
    this.#conversionGeometry.dispose();
    this.#conversionMaterial.dispose();
  }
}

function validateCaptureOptions(
  options: RenderOutputCaptureOptions,
  renderer: THREE.WebGLRenderer,
): { width: number; height: number; background: RenderOutputCaptureOptions['background']; samples: number; exposure: number; toneMapping: THREE.ToneMapping } {
  const { width, height } = options;
  if (!Number.isSafeInteger(width) || width < 1 || width > renderer.capabilities.maxTextureSize
    || !Number.isSafeInteger(height) || height < 1 || height > renderer.capabilities.maxTextureSize) {
    throw new RangeError(`capture dimensions must be positive integers no larger than ${String(renderer.capabilities.maxTextureSize)}`);
  }
  const pixelBytes = width * height * 4;
  if (!Number.isSafeInteger(pixelBytes) || pixelBytes > 0xffff_ffff) {
    throw new RangeError('capture RGBA byte length exceeds PNG and typed-array limits');
  }
  if (options.background.length !== 4
    || options.background.some((channel) => !Number.isFinite(channel) || channel < 0 || channel > 1)) {
    throw new RangeError('capture background must contain four finite normalized RGBA channels');
  }
  const samples = options.samples ?? 0;
  const maximumSamples = renderer.capabilities.maxSamples;
  if (!Number.isSafeInteger(samples) || samples < 0 || samples > maximumSamples) {
    throw new RangeError(`capture samples must be an integer in 0..=${String(maximumSamples)}`);
  }
  validateHalfFloatSampleCount(renderer, samples);
  const exposure = options.exposure ?? renderer.toneMappingExposure;
  if (!Number.isFinite(exposure) || exposure < 0) {
    throw new RangeError('capture exposure must be finite and non-negative');
  }
  const toneMapping = options.toneMapping ?? renderer.toneMapping;
  if (toneMapping !== THREE.NoToneMapping && toneMapping !== THREE.ACESFilmicToneMapping) {
    throw new RangeError('capture supports NoToneMapping and ACESFilmicToneMapping');
  }
  return {
    width,
    height,
    background: options.background,
    samples,
    exposure,
    toneMapping,
  };
}

function validateHalfFloatSampleCount(renderer: THREE.WebGLRenderer, samples: number): void {
  if (samples === 0) return;
  if (!renderer.capabilities.isWebGL2) {
    throw new RangeError('capture multisampling requires WebGL 2');
  }
  const gl = renderer.getContext() as WebGL2RenderingContext;
  let supported: number[];
  try {
    supported = Array.from(
      gl.getInternalformatParameter(gl.RENDERBUFFER, gl.RGBA16F, gl.SAMPLES) as Int32Array,
    );
  } catch (cause) {
    throw new Error(`capture could not query RGBA16F multisample support: ${errorMessage(cause)}`);
  }
  if (!supported.includes(samples)) {
    const list = supported.length === 0 ? 'none' : supported.join(', ');
    throw new RangeError(
      `capture requested ${String(samples)} samples, but the RGBA16F render target supports only ${list}`,
    );
  }
}

function saveRendererState(renderer: THREE.WebGLRenderer): SavedRendererState {
  return {
    renderTarget: renderer.getRenderTarget(),
    activeCubeFace: renderer.getActiveCubeFace(),
    activeMipmapLevel: renderer.getActiveMipmapLevel(),
    viewport: renderer.getViewport(new THREE.Vector4()),
    scissor: renderer.getScissor(new THREE.Vector4()),
    scissorTest: renderer.getScissorTest(),
    clearColor: renderer.getClearColor(new THREE.Color()),
    clearAlpha: renderer.getClearAlpha(),
    autoClear: renderer.autoClear,
    autoClearColor: renderer.autoClearColor,
    autoClearDepth: renderer.autoClearDepth,
    autoClearStencil: renderer.autoClearStencil,
    toneMapping: renderer.toneMapping,
    toneMappingExposure: renderer.toneMappingExposure,
    outputColorSpace: renderer.outputColorSpace,
    localClippingEnabled: renderer.localClippingEnabled,
    xrEnabled: renderer.xr.enabled,
  };
}

function restoreRendererState(renderer: THREE.WebGLRenderer, state: SavedRendererState): void {
  renderer.setRenderTarget(state.renderTarget, state.activeCubeFace, state.activeMipmapLevel);
  renderer.setViewport(state.viewport);
  renderer.setScissor(state.scissor);
  renderer.setScissorTest(state.scissorTest);
  renderer.setClearColor(state.clearColor, state.clearAlpha);
  renderer.autoClear = state.autoClear;
  renderer.autoClearColor = state.autoClearColor;
  renderer.autoClearDepth = state.autoClearDepth;
  renderer.autoClearStencil = state.autoClearStencil;
  renderer.toneMapping = state.toneMapping;
  renderer.toneMappingExposure = state.toneMappingExposure;
  renderer.outputColorSpace = state.outputColorSpace;
  renderer.localClippingEnabled = state.localClippingEnabled;
  renderer.xr.enabled = state.xrEnabled;
}

function cloneCameraForCapture(camera: THREE.Camera, width: number, height: number): THREE.Camera {
  const ancestors: THREE.Object3D[] = [];
  for (let object: THREE.Object3D | null = camera; object !== null; object = object.parent) {
    ancestors.push(object);
  }
  ancestors.reverse();
  const worldMatrix = new THREE.Matrix4();
  for (const object of ancestors) {
    const localMatrix = object.matrixAutoUpdate
      ? new THREE.Matrix4().compose(object.position, object.quaternion, object.scale)
      : object.matrix;
    worldMatrix.multiply(localMatrix);
  }

  const copy = camera.clone() as THREE.Camera;
  copy.parent = null;
  copy.matrixAutoUpdate = false;
  copy.matrix.copy(worldMatrix);
  copy.matrixWorld.copy(worldMatrix);
  copy.matrixWorldInverse.copy(worldMatrix).invert();
  copy.matrixWorldNeedsUpdate = false;
  if (copy instanceof THREE.PerspectiveCamera) {
    copy.aspect = width / height;
    copy.updateProjectionMatrix();
  }
  return copy;
}

function cloneSelectedHierarchy(
  scene: THREE.Scene,
  selected: THREE.Object3D,
): {
  readonly root: THREE.Scene;
  readonly selected: THREE.Object3D;
  readonly cloneFor: (source: THREE.Object3D) => THREE.Object3D | undefined;
} {
  const sourceNodes: THREE.Object3D[] = [];
  const clonedNodes: THREE.Object3D[] = [];
  scene.traverse((object) => sourceNodes.push(object));
  const root = SkeletonUtils.clone(scene) as THREE.Scene;
  root.traverse((object) => clonedNodes.push(object));
  if (sourceNodes.length !== clonedNodes.length) {
    throw new Error('exportGlb: could not preserve the retained hierarchy while cloning');
  }
  const cloneBySource = new Map(sourceNodes.map((object, index) => [object, clonedNodes[index] as THREE.Object3D]));
  const sourceByClone = new Map(clonedNodes.map((object, index) => [object, sourceNodes[index] as THREE.Object3D]));
  const selectedClone = cloneBySource.get(selected);
  if (selectedClone === undefined) throw new Error('exportGlb: selected handle is not in its retained scene');

  const path = new Set<THREE.Object3D>();
  for (let object: THREE.Object3D | null = selected; object !== null; object = object.parent) {
    path.add(object);
    if (object === scene) break;
  }
  if (!path.has(scene)) throw new Error('exportGlb: selected handle has no path to its retained scene');

  const prune = (source: THREE.Object3D, clone: THREE.Object3D): void => {
    if (source !== selected && !isDescendantOf(source, selected)) {
      const keep = source.children.find((child) => path.has(child));
      for (let index = clone.children.length - 1; index >= 0; index--) {
        const cloneChild = clone.children[index] as THREE.Object3D;
        const sourceChild = sourceByClone.get(cloneChild);
        if (sourceChild !== undefined && sourceChild === keep) {
          prune(sourceChild, cloneChild);
        } else {
          clone.remove(cloneChild);
        }
      }
    }
  };
  prune(scene, root);
  return { root, selected: selectedClone, cloneFor: (source) => cloneBySource.get(source) };
}

function isDescendantOf(object: THREE.Object3D, ancestor: THREE.Object3D): boolean {
  for (let candidate: THREE.Object3D | null = object; candidate !== null; candidate = candidate.parent) {
    if (candidate === ancestor) return true;
  }
  return false;
}

function prepareAnimationClips(
  sources: readonly ThreeRendererAnimationClipSource[],
  cloneFor: (source: THREE.Object3D) => THREE.Object3D | undefined,
): THREE.AnimationClip[] {
  return sources.flatMap((source) => {
    const animationRoot = cloneFor(source.object);
    if (animationRoot === undefined) {
      throw new Error(`exportGlb: animated handle ${String(source.handle)} is outside the exported hierarchy`);
    }
    return source.clips.map((sourceClip) => {
      const clip = sourceClip.clone();
      clip.tracks = clip.tracks.map((track) => {
        let binding: ReturnType<typeof THREE.PropertyBinding.parseTrackName>;
        try {
          binding = THREE.PropertyBinding.parseTrackName(track.name);
        } catch (cause) {
          throw new RenderOutputUnsupportedError(
            `exportGlb: animation clip ${sourceClip.name} contains an unsupported track ${track.name}: ${errorMessage(cause)}`,
          );
        }
        if (!['position', 'quaternion', 'scale', 'morphTargetInfluences'].includes(binding.propertyName)) {
          throw new RenderOutputUnsupportedError(
            `exportGlb: animation clip ${sourceClip.name} targets unsupported property ${binding.propertyName}`,
          );
        }
        if (binding.objectName !== undefined && binding.objectName !== 'bones') {
          throw new RenderOutputUnsupportedError(
            `exportGlb: animation clip ${sourceClip.name} uses unsupported track object ${binding.objectName}`,
          );
        }
        const sourceMatches: THREE.Object3D[] = [];
        if (binding.nodeName === null || binding.nodeName.length === 0) {
          sourceMatches.push(source.object);
        } else {
          source.object.traverse((object) => {
            if (object.uuid === binding.nodeName || object.name === binding.nodeName) sourceMatches.push(object);
          });
        }
        const sourceTarget = sourceMatches.find((object) => object.uuid === binding.nodeName) ?? sourceMatches[0];
        if (sourceTarget === undefined) {
          throw new RenderOutputUnsupportedError(
            `exportGlb: animation clip ${sourceClip.name} targets missing node ${binding.nodeName}`,
          );
        }
        if (sourceMatches.length > 1 && sourceTarget.uuid !== binding.nodeName) {
          throw new RenderOutputUnsupportedError(
            `exportGlb: animation clip ${sourceClip.name} targets ambiguous node name ${binding.nodeName}`,
          );
        }
        if (binding.objectName === 'bones'
          && (!(sourceTarget instanceof THREE.SkinnedMesh)
            || binding.objectIndex === undefined
            || sourceTarget.skeleton.getBoneByName(binding.objectIndex) === undefined)) {
          throw new RenderOutputUnsupportedError(
            `exportGlb: animation clip ${sourceClip.name} targets missing bone ${binding.objectIndex ?? '(unnamed)'}`,
          );
        }
        const target = cloneFor(sourceTarget);
        if (target === undefined) {
          throw new RenderOutputUnsupportedError(
            `exportGlb: animation clip ${sourceClip.name} targets node outside the retained clone`,
          );
        }
        track.name = formatTrackName(target.uuid, binding);
        return track;
      });
      return clip;
    });
  });
}

function formatTrackName(
  uuid: string,
  binding: ReturnType<typeof THREE.PropertyBinding.parseTrackName>,
): string {
  const objectBinding = binding.objectName === undefined
    ? ''
    : `.${binding.objectName}${binding.objectIndex === undefined ? '' : `[${binding.objectIndex}]`}`;
  const propertyIndex = binding.propertyIndex === undefined ? '' : `[${binding.propertyIndex}]`;
  return `${uuid}${objectBinding}.${binding.propertyName}${propertyIndex}`;
}

function validateGltfHierarchy(root: THREE.Object3D): void {
  root.traverse((object) => {
    if (object instanceof THREE.Sprite) {
      throw new RenderOutputUnsupportedError(`exportGlb: Sprite ${object.name || object.uuid} is not representable as a glTF node`);
    }
    if (object.onBeforeRender !== THREE.Object3D.prototype.onBeforeRender
      || object.onAfterRender !== THREE.Object3D.prototype.onAfterRender) {
      throw new RenderOutputUnsupportedError(`exportGlb: custom render callback on ${object.name || object.uuid} is not represented in glTF`);
    }
    const candidate = object as THREE.Object3D & {
      customDepthMaterial?: THREE.Material | null;
      customDistanceMaterial?: THREE.Material | null;
    };
    if (candidate.customDepthMaterial != null || candidate.customDistanceMaterial != null) {
      throw new RenderOutputUnsupportedError(`exportGlb: custom shadow material on ${object.name || object.uuid} is not represented in glTF`);
    }
    if (object instanceof THREE.SkinnedMesh) {
      const missingBone = object.skeleton.bones.find((bone) => !isDescendantOf(bone, root));
      if (missingBone !== undefined) {
        throw new RenderOutputUnsupportedError(
          `exportGlb: skin ${object.name || object.uuid} references bone ${missingBone.name || missingBone.uuid} outside the exported hierarchy`,
        );
      }
    }
    if (object instanceof THREE.Mesh || object instanceof THREE.Line || object instanceof THREE.Points) {
      const materialValues = Array.isArray(object.material) ? object.material : [object.material];
      for (const material of materialValues) validateGltfMaterial(object, material);
    }
  });
}

function validateGltfMaterial(object: THREE.Object3D, material: THREE.Material): void {
  const flags = material as THREE.Material & {
    readonly isShaderMaterial?: boolean;
    readonly isRawShaderMaterial?: boolean;
    readonly isMeshStandardMaterial?: boolean;
    readonly isMeshBasicMaterial?: boolean;
  };
  if (flags.isShaderMaterial || flags.isRawShaderMaterial) {
    throw new RenderOutputUnsupportedError(
      `exportGlb: ShaderMaterial ${material.name || material.uuid} on ${object.name || object.uuid} cannot be represented in glTF`,
    );
  }
  if (!flags.isMeshStandardMaterial && !flags.isMeshBasicMaterial) {
    throw new RenderOutputUnsupportedError(
      `exportGlb: material ${material.name || material.uuid} on ${object.name || object.uuid} has properties glTF would discard`,
    );
  }
  if (material.onBeforeCompile !== THREE.Material.prototype.onBeforeCompile
    || material.customProgramCacheKey !== THREE.Material.prototype.customProgramCacheKey) {
    throw new RenderOutputUnsupportedError(
      `exportGlb: custom shader hook on material ${material.name || material.uuid} is not represented in glTF`,
    );
  }
}

function encodeRgbaPng(bottomUpPixels: Uint8Array, width: number, height: number): Uint8Array {
  const rowBytes = width * 4;
  const scanlines = new Uint8Array(height * (rowBytes + 1));
  for (let row = 0; row < height; row++) {
    const sourceOffset = (height - 1 - row) * rowBytes;
    const targetOffset = row * (rowBytes + 1);
    scanlines[targetOffset] = 0;
    scanlines.set(bottomUpPixels.subarray(sourceOffset, sourceOffset + rowBytes), targetOffset + 1);
  }
  const compressed = zlibSync(scanlines);
  const png = new Uint8Array(70 + compressed.byteLength);
  png.set([137, 80, 78, 71, 13, 10, 26, 10], 0);
  let offset = 8;
  const header = new Uint8Array(13);
  const headerView = new DataView(header.buffer);
  headerView.setUint32(0, width, false);
  headerView.setUint32(4, height, false);
  header.set([8, 6, 0, 0, 0], 8);
  offset = writePngChunk(png, offset, 'IHDR', header);
  offset = writePngChunk(png, offset, 'sRGB', Uint8Array.of(0));
  offset = writePngChunk(png, offset, 'IDAT', compressed);
  writePngChunk(png, offset, 'IEND', new Uint8Array());
  return png;
}

function writePngChunk(output: Uint8Array, offset: number, type: string, data: Uint8Array): number {
  const typeBytes = Uint8Array.from(type, (character) => character.charCodeAt(0));
  const view = new DataView(output.buffer, output.byteOffset, output.byteLength);
  view.setUint32(offset, data.byteLength, false);
  output.set(typeBytes, offset + 4);
  output.set(data, offset + 8);
  view.setUint32(offset + 8 + data.byteLength, pngCrc32(typeBytes, data), false);
  return offset + 12 + data.byteLength;
}

function pngCrc32(type: Uint8Array, data: Uint8Array): number {
  let crc = 0xffff_ffff;
  for (const byte of type) crc = updateCrc32(crc, byte);
  for (const byte of data) crc = updateCrc32(crc, byte);
  return (crc ^ 0xffff_ffff) >>> 0;
}

function updateCrc32(crc: number, byte: number): number {
  let value = crc ^ byte;
  for (let bit = 0; bit < 8; bit++) {
    value = (value & 1) === 1 ? 0xedb8_8320 ^ (value >>> 1) : value >>> 1;
  }
  return value >>> 0;
}

function errorMessage(cause: unknown): string {
  return cause instanceof Error ? cause.message : String(cause);
}
