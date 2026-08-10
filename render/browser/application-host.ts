import {
  mountRustyApplication,
  type RustyApplicationContent,
  type RustyApplicationHost,
} from '@rusty-engine/application-host';

declare global {
  interface Window {
    __rustyApplicationHost?: RustyApplicationHost;
    __rustyApplicationMount?: () => Promise<RustyApplicationHost>;
    __rustyApplicationFailureProbe?: () => Promise<string>;
    __rustyApplicationInitialResourceFailureProbe?: () => Promise<string>;
    __rustyApplicationGameplayInputCount?: number;
    __rustyApplicationResourceContent?: (corrupt?: boolean) => RustyApplicationContent;
    __rustyApplicationUiDisposed?: boolean;
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

window.__rustyApplicationResourceContent = resourceContent;

function resourceContent(corrupt = false): RustyApplicationContent {
  const bytes = TEXTURE_BYTES.slice();
  if (corrupt) bytes[bytes.length - 1] = bytes[bytes.length - 1]! ^ 0xff;
  return {
    frame: resourceBackedFrame(),
    resources: [{
      identity: TEXTURE_RESOURCE,
      contentHash: TEXTURE_CONTENT_HASH,
      mediaType: 'image/png',
      bytes,
    }],
  };
}

const root = document.querySelector<HTMLElement>('#application');
if (root === null) throw new Error('application root is missing');

window.__rustyApplicationMount = () =>
  mountRustyApplication({
    root,
    initialInteractionMode: 'gameplay',
    renderer: { initialContent: resourceContent() },
    mountUi: (uiRoot, context) => {
      const gameplay = document.createElement('div');
      gameplay.id = 'gameplay-zone';
      gameplay.textContent = 'Gameplay surface';
      const toolbar = document.createElement('div');
      toolbar.id = 'toolbar';
      const button = document.createElement('button');
      button.id = 'interface-button';
      button.textContent = 'Interface action';
      const input = document.createElement('input');
      input.id = 'text-entry';
      input.setAttribute('aria-label', 'Text entry');
      const modal = document.createElement('section');
      modal.id = 'modal';
      modal.setAttribute('role', 'dialog');
      modal.hidden = true;
      modal.textContent = 'Modal content';
      toolbar.append(button, input, modal);
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
