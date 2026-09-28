import { renderHandle, type RendererViewComposition } from '@rusty-engine/render-contracts';
import { mountRendererBrowserSurface } from '@rusty-engine/renderer-three';

type Pixel = readonly [number, number, number, number];

interface PassReadout {
  readonly drawCallCount: number;
  readonly center: Pixel;
  readonly viewmodel: Pixel;
}

declare global {
  interface Window {
    __rustyViewPassProof?: {
      fallback: () => PassReadout;
      fullPrimaryView: () => PassReadout;
      insetPrimaryView: () => PassReadout;
    };
  }
}

const CLEAR_COLOR = 0x123456;
const CAMERA_POSE = { position: [0, 0, 5] as const, pitchDegrees: 0, yawDegrees: 0 };
const VIEWMODEL_SAMPLE = [0.75, 0.25] as const;

const canvas = document.querySelector<HTMLCanvasElement>('#view-pass');
if (canvas === null) throw new Error('view pass canvas is missing');
const surface = mountRendererBrowserSurface(canvas, {
  autoStart: false,
  clearColor: CLEAR_COLOR,
  pixelRatio: 1,
  frame: {
    schemaVersion: 1,
    ops: [
      {
        op: 'create', handle: renderHandle(1), parent: null,
        node: {
          geometry: { kind: 'cube' },
          material: { color: [1, 0.04, 0.02, 1], wireframe: false },
          transform: { translation: [0, 0, 0], rotation: [0, 0, 0, 1], scale: [2, 2, 2] },
          visible: true, layer: 'scene',
          metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'world-red' },
        },
      },
      {
        op: 'create', handle: renderHandle(2), parent: null,
        node: {
          geometry: { kind: 'cube' },
          material: { color: [0.02, 1, 0.04, 1], wireframe: false },
          transform: { translation: [0.5, -0.5, -1.5], rotation: [0, 0, 0, 1], scale: [0.3, 0.3, 0.3] },
          visible: true, layer: 'viewmodel',
          metadata: { sourceEntity: null, sourceSceneNode: null, tags: [], label: 'viewmodel-green' },
        },
      },
    ],
  },
});
surface.setCameraPose(CAMERA_POSE);

const gl = canvas.getContext('webgl2');
if (gl === null) throw new Error('WebGL2 context is unavailable');

function pixel(u: number, v: number): Pixel {
  const out = new Uint8Array(4);
  gl!.readPixels(
    Math.floor(u * canvas!.width), Math.floor(v * canvas!.height), 1, 1,
    gl!.RGBA, gl!.UNSIGNED_BYTE, out,
  );
  return [out[0]!, out[1]!, out[2]!, out[3]!];
}

function composition(viewport: RendererViewComposition['views'][number]['viewport']): RendererViewComposition {
  return {
    schemaVersion: 1,
    cameras: [{
      id: 'camera.view-pass',
      pose: CAMERA_POSE,
      projection: { kind: 'perspective', fovYDegrees: 60, near: 0.1, far: 50 },
    }],
    targets: [],
    views: [{
      id: 'view.view-pass', cameraId: 'camera.view-pass', order: 10,
      target: { kind: 'primary' }, viewport,
    }],
    presentations: [],
  };
}

function submit(): PassReadout {
  // Read pixels in the same task as the submission, before presentation.
  const statistics = surface.renderOnce(0, 0);
  return {
    drawCallCount: statistics.drawCallCount,
    center: pixel(0.5, 0.5),
    viewmodel: pixel(VIEWMODEL_SAMPLE[0], VIEWMODEL_SAMPLE[1]),
  };
}

window.__rustyViewPassProof = {
  fallback: () => submit(),
  fullPrimaryView: () => {
    surface.configureViews(composition({ x: 0, y: 0, width: 1, height: 1 }));
    return submit();
  },
  insetPrimaryView: () => {
    surface.configureViews(composition({ x: 0.6, y: 0.6, width: 0.4, height: 0.4 }));
    return submit();
  },
};
