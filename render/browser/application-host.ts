import {
  mountRustyApplication,
  type RustyApplicationContent,
  type RustyApplicationHost,
  type RustyApplicationPresentationAspectBounds,
} from '@rusty-engine/application-host';
import { audioSignalHandle } from '@rusty-engine/render-contracts';
import characterUrl from '../../fixtures/render/assets/kenney-retro-character/character-medium.glb?url';

declare global {
  interface Window {
    __rustyApplicationHost?: RustyApplicationHost;
    __rustyApplicationMount?: (
      presentationAspectBounds?: RustyApplicationPresentationAspectBounds,
      includeRuntimeInput?: boolean,
    ) => Promise<RustyApplicationHost>;
    __rustyApplicationFailureProbe?: () => Promise<string>;
    __rustyApplicationBoundedFailureProbe?: () => Promise<string>;
    /** Browser-fixture-only gate for observing the normal bounded loading layer before UI mount. */
    __rustyApplicationLoadingGate?: {
      readonly mount: () => Promise<RustyApplicationHost>;
      readonly pending: () => boolean;
      readonly release: () => void;
    };
    __rustyApplicationInitialResourceFailureProbe?: () => Promise<string>;
    __rustyApplicationGameplayInputCount?: number;
    __rustyApplicationAudioReceipt?: unknown;
    __rustyApplicationAudioResume?: unknown;
    __rustyApplicationIndicatorReceipt?: unknown;
    __rustyApplicationResourceContent?: (corrupt?: boolean) => RustyApplicationContent;
    __rustyApplicationUiDisposed?: boolean;
    __rustyApplicationUiContextShape?: {
      readonly keys: readonly string[];
      readonly projectionKeys: readonly string[] | null;
      readonly intentsKeys: readonly string[] | null;
    };
    /** Browser-fixture resource URL for application-host retained animation proof. */
    __rustyApplicationRiggedFixtureUrl?: string;
  }
}

const TEXTURE_BYTES = new Uint8Array([
  137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,2,0,0,0,1,
  8,6,0,0,0,244,34,127,138,0,0,0,15,73,68,65,84,120,156,99,248,
  207,0,68,255,25,26,0,16,121,3,126,153,113,48,89,0,0,0,0,73,69,
  78,68,174,66,96,130,
]);
const TEXTURE_DIGEST = 'a58d5395a03945e56638dba7ae6158b2fdaf013610a798c059a6d88231a052ae';
const TEXTURE_CONTENT_HASH = `sha256:${TEXTURE_DIGEST}`;
const TEXTURE_RESOURCE = `texture-resource/${TEXTURE_DIGEST}`;
const VOXEL_FRAME_BYTES = new Uint8Array([
  137,80,78,71,13,10,26,10,0,0,0,13,73,72,68,82,0,0,0,8,0,0,0,8,
  8,6,0,0,0,196,15,190,139,0,0,0,22,73,68,65,84,24,211,99,60,208,
  224,240,159,1,15,96,98,32,0,134,135,2,0,149,226,2,143,136,199,46,
  79,0,0,0,0,73,69,78,68,174,66,96,130,
]);
const VOXEL_FRAME_DIGEST = 'd934c3fa8b5084fde1409345a36f4e849dc6db4caafeb5976ff9c2a3a116b4f8';
const VOXEL_FRAME_CONTENT_HASH = `sha256:${VOXEL_FRAME_DIGEST}`;
const VOXEL_FRAME_RESOURCE = `texture-resource/${VOXEL_FRAME_DIGEST}`;
const AUDIO_BYTES = new Uint8Array([
  82,73,70,70,44,0,0,0,87,65,86,69,102,109,116,32,16,0,0,0,1,0,
  1,0,64,31,0,0,128,62,0,0,2,0,16,0,100,97,116,97,8,0,0,0,0,0,
  224,46,0,0,32,209,
]);
const AUDIO_DIGEST = '642b803f562c2d0e589411a87ad8914cf147799aa196e62c11a7033fdfcf5513';
const AUDIO_CONTENT_HASH = `sha256:${AUDIO_DIGEST}`;
const AUDIO_RESOURCE = `audio-resource/${AUDIO_DIGEST}`;

window.__rustyApplicationResourceContent = resourceContent;
window.__rustyApplicationRiggedFixtureUrl = characterUrl;

function resourceContent(corrupt = false): RustyApplicationContent {
  const bytes = TEXTURE_BYTES.slice();
  if (corrupt) bytes[bytes.length - 1] = bytes[bytes.length - 1]! ^ 0xff;
  return {
    frame: resourceBackedFrame(),
    resources: [
      {
        identity: TEXTURE_RESOURCE,
        contentHash: TEXTURE_CONTENT_HASH,
        mediaType: 'image/png',
        bytes,
      },
      {
        identity: AUDIO_RESOURCE,
        contentHash: AUDIO_CONTENT_HASH,
        mediaType: 'audio/wav',
        bytes: AUDIO_BYTES.slice(),
      },
      {
        identity: VOXEL_FRAME_RESOURCE,
        contentHash: VOXEL_FRAME_CONTENT_HASH,
        mediaType: 'image/png',
        bytes: VOXEL_FRAME_BYTES.slice(),
      },
    ],
  };
}

const root = document.querySelector<HTMLElement>('#application');
if (root === null) throw new Error('application root is missing');

window.__rustyApplicationMount = (presentationAspectBounds, includeRuntimeInput = true) =>
  mountRustyApplication({
    root,
    ...(presentationAspectBounds === undefined ? {} : { presentationAspectBounds }),
    initialInteractionMode: 'gameplay',
    ...(includeRuntimeInput ? {
      runtimeInput: {
        binding: {
          runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
          context: 'gameplay.default',
        },
        maximumPointerDelta: 32,
        maximumWheelDelta: 64,
        selectedController: { index: 0 },
      },
    } : {}),
    uiProjection: {
      expectedStream: 'product.hud',
      expectedContract: 'product.hud.v1',
    },
    renderer: {
      initialContent: resourceContent(),
      fog: { color: 0xff00ff, near: 0, far: 0.25 },
      resolveIndicatorEntityPosition: (entity) => entity === 42 ? [0, 0, 0] : null,
    },
    mountUi: (uiRoot, context) => {
      window.__rustyApplicationUiContextShape = {
        keys: Object.keys(context).sort(),
        projectionKeys: context.projection === undefined ? null : Object.keys(context.projection).sort(),
        intentsKeys: context.intents === undefined ? null : Object.keys(context.intents).sort(),
      };
      const gameplay = document.createElement('div');
      gameplay.id = 'gameplay-zone';
      gameplay.textContent = 'Gameplay surface';
      const toolbar = document.createElement('div');
      toolbar.id = 'toolbar';
      toolbar.style.zIndex = '1';
      const button = document.createElement('button');
      button.id = 'interface-button';
      button.textContent = 'Interface action';
      const input = document.createElement('input');
      input.id = 'text-entry';
      input.setAttribute('aria-label', 'Text entry');
      const audioButton = document.createElement('button');
      audioButton.id = 'audio-button';
      audioButton.textContent = 'Play audio proof';
      const inputClaimButton = document.createElement('button');
      inputClaimButton.id = 'input-claim-button';
      inputClaimButton.textContent = 'Claim UI intent';
      const modal = document.createElement('section');
      modal.id = 'modal';
      modal.setAttribute('role', 'dialog');
      modal.hidden = true;
      modal.textContent = 'Modal content';
      const nativeModal = document.createElement('dialog');
      nativeModal.id = 'native-modal';
      nativeModal.setAttribute('aria-modal', 'true');
      nativeModal.hidden = true;
      nativeModal.textContent = 'Native dialog content';
      const ariaModal = document.createElement('section');
      ariaModal.id = 'aria-modal-section';
      ariaModal.setAttribute('aria-modal', 'true');
      ariaModal.hidden = true;
      ariaModal.textContent = 'ARIA modal content';
      toolbar.append(button, audioButton, inputClaimButton, input, modal, nativeModal, ariaModal);
      uiRoot.append(gameplay, toolbar);
      window.__rustyApplicationGameplayInputCount = 0;
      const onMouseDown = (event: MouseEvent): void => {
        if (context.ui.allowsGameplayInput(event)) {
          window.__rustyApplicationGameplayInputCount =
            (window.__rustyApplicationGameplayInputCount ?? 0) + 1;
        }
      };
      window.addEventListener('mousedown', onMouseDown);
      button.addEventListener('click', () => {
        context.ui.setInteractionMode('interface');
      });
      audioButton.addEventListener('click', async () => {
        const renderer = window.__rustyApplicationHost?.renderer;
        if (renderer === undefined) throw new Error('application host is not ready for audio proof');
        window.__rustyApplicationAudioResume = await renderer.resumeAudio();
        window.__rustyApplicationAudioReceipt = await renderer.applyPresentation({
          schemaVersion: 1,
          ops: [{
            domain: 'audio',
            meta: { sequence: 0 },
            op: {
              op: 'emit',
              signalHandle: audioSignalHandle(1),
              signalId: 'application-host-browser-proof',
              descriptor: {
                clip: { asset: 'audio/application-host-proof', contentHash: AUDIO_CONTENT_HASH },
                bus: 'sfx',
                volume: 0.1,
                pitch: 1,
                looping: false,
                spatialBlend: 0,
                attenuation: 1,
                pan: 0,
                emitter: { kind: 'global2d' },
              },
            },
          }],
        });
      });
      inputClaimButton.addEventListener('click', () => {
        context.intents?.claim('ui.confirm', { kind: 'digital', active: true });
      });
      return {
        dispose: () => {
          window.removeEventListener('mousedown', onMouseDown);
          window.__rustyApplicationUiDisposed = true;
        },
      };
    },
  });
window.__rustyApplicationHost = await window.__rustyApplicationMount();
window.__rustyApplicationHost.renderer.setCameraPose({
  position: [0, 0, 3],
  pitchDegrees: 0,
  yawDegrees: 0,
});
window.__rustyApplicationHost.renderer.renderOnce();
window.__rustyApplicationIndicatorReceipt =
  await window.__rustyApplicationHost.renderer.applyPresentation({
    schemaVersion: 1,
    ops: [{
      domain: 'billboard',
      meta: { sequence: 0 },
      op: {
        op: 'create',
        handle: 41,
        descriptor: {
          anchor: { kind: 'entityAttached', entity: 42, offset: [0, 0.9, 0] },
          content: {
            kind: 'structured',
            indicator: {
              label: {
                localizationKey: 'actor.ranger.name',
                fallbackText: 'Ranger',
              },
              icon: {
                asset: 'texture/application-host-proof',
                contentHash: TEXTURE_CONTENT_HASH,
              },
              accessibleLabel: {
                localizationKey: 'actor.ranger.indicator',
                fallbackText: 'Ranger status',
              },
              meters: [{
                id: 'health',
                accessibleLabel: {
                  localizationKey: 'resource.health',
                  fallbackText: 'Health',
                },
                current: 72,
                min: 0,
                max: 100,
                preview: 64,
                fillDirection: 'leftToRight',
                segments: 10,
                fill: [0.16, 0.72, 0.28, 1],
                previewFill: [0.95, 0.72, 0.12, 1],
                back: [0.04, 0.04, 0.04, 0.9],
                border: [0, 0, 0, 1],
              }, {
                id: 'stamina',
                accessibleLabel: {
                  localizationKey: 'resource.stamina',
                  fallbackText: 'Stamina',
                },
                current: 44,
                min: 0,
                max: 60,
                preview: null,
                fillDirection: 'leftToRight',
                segments: 6,
                fill: [0.2, 0.55, 0.95, 1],
                previewFill: [0.4, 0.7, 1, 1],
                back: [0.04, 0.04, 0.04, 0.9],
                border: [0, 0, 0, 1],
              }],
              statusCues: [{
                id: 'interact',
                label: {
                  localizationKey: 'prompt.open',
                  fallbackText: 'Open',
                },
                icon: {
                  asset: 'texture/application-host-proof',
                  contentHash: TEXTURE_CONTENT_HASH,
                },
              }],
              widthPixels: 192,
              spacingPixels: 6,
              alignment: 'center',
              style: {
                opacity: 0.96,
                backing: [0, 0, 0, 0.58],
                border: [0.2, 0.2, 0.2, 1],
                radiusPixels: 6,
              },
            },
          },
          font: { kind: 'system', family: 'sans-serif' },
          heightPixels: 20,
          color: [1, 1, 1, 1],
          background: [0, 0, 0, 0],
          maxDistance: 80,
          layer: 'occluded',
          visible: true,
          layout: {
            priority: 100,
            sizing: {
              kind: 'distanceScaled',
              referenceDistance: 12,
              minScale: 0.75,
              maxScale: 1.25,
            },
            safeArea: {
              topPixels: 12,
              rightPixels: 12,
              bottomPixels: 12,
              leftPixels: 12,
            },
            edgeBehavior: 'clamp',
            overlapBehavior: 'suppress',
          },
        },
      },
    }],
  });

window.__rustyApplicationFailureProbe = async () => {
  await window.__rustyApplicationHost?.dispose();
  try {
    await mountRustyApplication({
      root,
      mountUi: () => {
        throw new Error('trusted UI mount rejected');
      },
    });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return 'unexpected success';
};

window.__rustyApplicationBoundedFailureProbe = async () => {
  await window.__rustyApplicationHost?.dispose();
  try {
    await mountRustyApplication({
      root,
      presentationAspectBounds: { minimum: 4 / 3, maximum: 16 / 9 },
      mountUi: () => {
        throw new Error('bounded trusted UI mount rejected');
      },
    });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return 'unexpected success';
};

let releaseLoadingGate: (() => void) | null = null;
let loadingGatePending = false;
window.__rustyApplicationLoadingGate = {
  mount: async () => {
    if (releaseLoadingGate !== null || loadingGatePending) {
      throw new Error('bounded loading gate is already active');
    }
    loadingGatePending = true;
    const gate = new Promise<void>((resolve) => {
      releaseLoadingGate = resolve;
    });
    try {
      return await mountRustyApplication({
        root,
        presentationAspectBounds: { minimum: 4 / 3, maximum: 16 / 9 },
        renderer: { initialContent: resourceContent() },
        mountUi: async (uiRoot) => {
          await gate;
          const content = document.createElement('div');
          content.textContent = 'Bounded loading gate released';
          uiRoot.append(content);
        },
      });
    } finally {
      loadingGatePending = false;
      releaseLoadingGate = null;
    }
  },
  pending: () => loadingGatePending,
  release: () => {
    if (releaseLoadingGate === null) throw new Error('bounded loading gate is not active');
    const release = releaseLoadingGate;
    releaseLoadingGate = null;
    release();
  },
};

window.__rustyApplicationInitialResourceFailureProbe = async () => {
  await window.__rustyApplicationHost?.dispose();
  try {
    await mountRustyApplication({
      root,
      renderer: { initialContent: resourceContent(true) },
      mountUi: () => {
        throw new Error('UI must not mount before resource admission');
      },
    });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return 'unexpected success';
};

function resourceBackedFrame(): RustyApplicationContent['frame'] {
  return {
    schemaVersion: 1,
    ops: [
      {
        op: 'defineTexture',
        texture: {
          id: 'texture/application-host-proof',
          width: 2,
          height: 1,
          filter: 'nearest',
          wrap: 'clamp',
          contentHash: TEXTURE_CONTENT_HASH,
          version: 1,
          payload: {
            encoding: 'pngRgba8',
            colorSpace: 'srgb',
            contentHash: TEXTURE_CONTENT_HASH,
            byteLength: TEXTURE_BYTES.byteLength,
            source: { kind: 'resource', resource: TEXTURE_RESOURCE },
          },
        },
      },
      ...(['color', 'depth', 'normal', 'coverage'] as const).map((channel) => ({
        op: 'defineTexture' as const,
        texture: {
          id: `texture/application-host-voxel-${channel}`,
          width: 8,
          height: 8,
          filter: 'nearest' as const,
          wrap: 'clamp' as const,
          contentHash: VOXEL_FRAME_CONTENT_HASH,
          version: 1,
          payload: {
            encoding: 'pngRgba8' as const,
            colorSpace: channel === 'color' ? 'srgb' as const : 'linear' as const,
            contentHash: VOXEL_FRAME_CONTENT_HASH,
            byteLength: VOXEL_FRAME_BYTES.byteLength,
            source: { kind: 'resource' as const, resource: VOXEL_FRAME_RESOURCE },
          },
        },
      })),
      {
        op: 'defineMaterial',
        material: {
          schemaVersion: 3,
          id: 'material/application-host-proof',
          color: [1, 1, 1, 1],
          texture: 'texture/application-host-proof',
          roughness: 1,
          textureTint: [1, 1, 1, 1],
          emissionColor: [0, 0, 0],
          emissionIntensity: 0,
          uvStrategy: 'planar',
        },
      },
      {
        op: 'defineStaticMesh',
        asset: {
          asset: 'mesh/application-host-proof',
          payload: {
            layout: {
              vertexCount: 4,
              indexCount: 6,
              indexWidth: 'u32',
              attributes: [
                { name: 'position', components: 3, kind: 'f32' },
                { name: 'normal', components: 3, kind: 'f32' },
                { name: 'uv', components: 2, kind: 'f32' },
              ],
            },
            groups: [{ materialSlot: 0, start: 0, count: 6 }],
            bounds: { min: [-1, -0.5, 0], max: [1, 0.5, 0] },
            source: {
              kind: 'inline',
              positions: [-1,-0.5,0, 1,-0.5,0, 1,0.5,0, -1,0.5,0],
              normals: [0,0,1, 0,0,1, 0,0,1, 0,0,1],
              uvs: [0,0, 1,0, 1,1, 0,1],
              indices: [0,1,2, 0,2,3],
            },
            provenance: 'staticAsset',
          },
          materialSlots: [{ slot: 0, material: 'material/application-host-proof' }],
          collision: { kind: 'visualOnly' },
        },
      },
      {
        op: 'createStaticMeshInstance',
        handle: 1,
        parent: null,
        instance: {
          asset: 'mesh/application-host-proof',
          transform: {
            translation: [0, 0, 0],
            rotation: [0, 0, 0, 1],
            scale: [1, 1, 1],
          },
          visible: true,
          materialOverrides: [],
          metadata: {
            sourceEntity: null,
            sourceSceneNode: null,
            tags: [],
            label: 'application-host-resource-proof',
          },
        },
      },
    ],
  };
}
