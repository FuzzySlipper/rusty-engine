import type { RenderHandle } from '@rusty-engine/render-contracts';
import type {
  RendererAnimatedMeshSampleReadout,
  RendererSurface,
  RendererSurfaceCameraPose,
} from './surface.js';
import type { RendererSurfaceStatisticsSample } from './surface-statistics.js';

export const RUSTY_RENDERER_ANIMATED_MESH_CAPTURE_SCHEMA_VERSION = 1;
export const RUSTY_RENDERER_ANIMATED_MESH_CAPTURE_MAX_SAMPLES = 32;
export const RUSTY_RENDERER_ANIMATED_MESH_CAPTURE_MAX_PIXELS = 4_194_304;

export interface RendererAnimatedMeshCaptureRequest {
  readonly handle: RenderHandle;
  readonly clip: string;
  readonly normalizedTimes: readonly number[];
  readonly providerRevision: string;
  readonly overlaysIncluded?: boolean;
  readonly contactSheetColumns?: number;
}

export interface RendererAnimatedMeshCaptureImage {
  readonly fileName: string;
  readonly pngDataUrl: string;
  readonly sample: RendererAnimatedMeshSampleReadout;
  readonly statistics: RendererSurfaceStatisticsSample;
}

export interface RendererAnimatedMeshCaptureManifest {
  readonly schemaVersion: 1;
  readonly providerRevision: string;
  readonly asset: string;
  readonly contentHash: string | null;
  readonly clip: string;
  readonly camera: RendererSurfaceCameraPose;
  readonly projection: ReturnType<RendererSurface['cameraProjection']>;
  readonly viewport: {
    readonly width: number;
    readonly height: number;
  };
  readonly overlaysIncluded: boolean;
  readonly samples: readonly {
    readonly fileName: string;
    readonly normalizedTime: number;
    readonly durationSeconds: number;
    readonly assetBounds: RendererAnimatedMeshSampleReadout['assetBounds'];
    readonly sampledWorldBounds: RendererAnimatedMeshSampleReadout['sampledWorldBounds'];
    readonly sampledVertexCount: number;
    readonly boneCount: number;
    readonly diagnostics: RendererAnimatedMeshSampleReadout['diagnostics'];
    readonly statistics: RendererSurfaceStatisticsSample;
  }[];
}

export interface RendererAnimatedMeshCaptureResult {
  readonly manifest: RendererAnimatedMeshCaptureManifest;
  readonly manifestJson: string;
  readonly images: readonly RendererAnimatedMeshCaptureImage[];
  readonly contactSheetFileName: string;
  readonly contactSheetPngDataUrl: string;
}

/**
 * Pose, render, and encode exact samples through one already-mounted surface.
 * The capture stops automatic submission and intentionally leaves the surface
 * stopped so no ambient animation loop can perturb the fixed samples.
 */
export function captureRendererAnimatedMesh(
  surface: RendererSurface,
  request: RendererAnimatedMeshCaptureRequest,
): RendererAnimatedMeshCaptureResult {
  validateCaptureRequest(surface.canvas, request);
  const columns = request.contactSheetColumns ?? Math.min(5, request.normalizedTimes.length);
  const rows = Math.ceil(request.normalizedTimes.length / columns);
  const contactSheet = surface.canvas.ownerDocument.createElement('canvas');
  contactSheet.width = surface.canvas.width * columns;
  contactSheet.height = surface.canvas.height * rows;
  const context = contactSheet.getContext('2d');
  if (context === null) {
    throw new Error('animated mesh capture requires a two-dimensional canvas context');
  }

  surface.stop();
  surface.renderOnce(0);
  const images: RendererAnimatedMeshCaptureImage[] = [];
  for (let index = 0; index < request.normalizedTimes.length; index += 1) {
    const normalizedTime = request.normalizedTimes[index] as number;
    const sample = surface.sampleAnimatedMesh(request.handle, request.clip, normalizedTime);
    const submission = surface.renderOnce(0);
    const fileName = captureFileName(sample.asset, request.clip, normalizedTime, index);
    images.push({
      fileName,
      pngDataUrl: surface.canvas.toDataURL('image/png'),
      sample,
      statistics: submission.statistics,
    });
    const x = (index % columns) * surface.canvas.width;
    const y = Math.floor(index / columns) * surface.canvas.height;
    context.drawImage(surface.canvas, x, y);
    context.fillStyle = 'rgba(0, 0, 0, 0.75)';
    context.fillRect(x, y, Math.min(surface.canvas.width, 180), 24);
    context.fillStyle = '#ffffff';
    context.font = '14px sans-serif';
    context.fillText(`${request.clip} ${(normalizedTime * 100).toFixed(0)}%`, x + 6, y + 17);
  }

  const first = images[0] as RendererAnimatedMeshCaptureImage;
  const manifest: RendererAnimatedMeshCaptureManifest = {
    schemaVersion: RUSTY_RENDERER_ANIMATED_MESH_CAPTURE_SCHEMA_VERSION,
    providerRevision: request.providerRevision,
    asset: first.sample.asset,
    contentHash: first.sample.contentHash,
    clip: request.clip,
    camera: surface.cameraPose(),
    projection: surface.cameraProjection(),
    viewport: { width: surface.canvas.width, height: surface.canvas.height },
    overlaysIncluded: request.overlaysIncluded ?? false,
    samples: images.map(({ fileName, sample, statistics }) => ({
      fileName,
      normalizedTime: sample.normalizedTime,
      durationSeconds: sample.durationSeconds,
      assetBounds: sample.assetBounds,
      sampledWorldBounds: sample.sampledWorldBounds,
      sampledVertexCount: sample.sampledVertexCount,
      boneCount: sample.boneCount,
      diagnostics: sample.diagnostics,
      statistics,
    })),
  };
  return {
    manifest,
    manifestJson: `${JSON.stringify(manifest, null, 2)}\n`,
    images,
    contactSheetFileName: `${safeFilePart(first.sample.asset)}-${safeFilePart(request.clip)}-contact-sheet.png`,
    contactSheetPngDataUrl: contactSheet.toDataURL('image/png'),
  };
}

function validateCaptureRequest(
  canvas: HTMLCanvasElement,
  request: RendererAnimatedMeshCaptureRequest,
): void {
  if (!/^[0-9a-f]{40}$/.test(request.providerRevision)) {
    throw new Error('animated mesh capture providerRevision must be an exact 40-character lowercase Git SHA');
  }
  if (
    request.normalizedTimes.length === 0
    || request.normalizedTimes.length > RUSTY_RENDERER_ANIMATED_MESH_CAPTURE_MAX_SAMPLES
    || request.normalizedTimes.some((value) => !Number.isFinite(value) || value < 0 || value > 1)
  ) {
    throw new Error(
      `animated mesh capture requires one to ${RUSTY_RENDERER_ANIMATED_MESH_CAPTURE_MAX_SAMPLES} normalized times`,
    );
  }
  const columns = request.contactSheetColumns ?? Math.min(5, request.normalizedTimes.length);
  if (!Number.isSafeInteger(columns) || columns < 1 || columns > request.normalizedTimes.length) {
    throw new Error('animated mesh capture contactSheetColumns is out of bounds');
  }
  if (
    canvas.width < 1
    || canvas.height < 1
    || canvas.width * canvas.height * request.normalizedTimes.length
      > RUSTY_RENDERER_ANIMATED_MESH_CAPTURE_MAX_PIXELS
  ) {
    throw new Error('animated mesh capture pixel quota exceeded');
  }
}

function captureFileName(asset: string, clip: string, normalizedTime: number, index: number): string {
  const percent = Math.round(normalizedTime * 100_000).toString().padStart(6, '0');
  return `${safeFilePart(asset)}-${safeFilePart(clip)}-${index.toString().padStart(2, '0')}-${percent}.png`;
}

function safeFilePart(value: string): string {
  const safe = value.toLowerCase().replace(/[^a-z0-9._-]+/g, '-').replace(/^-+|-+$/g, '');
  return safe.length > 0 ? safe : 'animated-mesh';
}
