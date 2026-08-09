import type { RenderFrameDiff } from '@rusty-engine/render-contracts';
import {
  mountRendererInspectionSurface,
  type RendererMeshResourceDescriptor,
  type RendererTextureResourceDescriptor,
} from '@rusty-engine/renderer-host';
import { submitStudioViewportFrame } from '@rusty-engine/studio-viewport/submission';

interface Resource {
  readonly resource: string;
  readonly contentHash: string;
  readonly byteLength: number;
  readonly base64: string;
}

interface Metrics {
  readonly occupiedVoxels: number;
  readonly vertices: number;
  readonly triangles: number;
  readonly materialPartitions: number;
  readonly sampledCells: number;
  readonly qefRankDeficient: number;
  readonly qefFallbacks: number;
  readonly packedBytes: number;
  readonly retainedResourceCount: number;
  readonly boundsMin: readonly [number, number, number];
  readonly boundsMax: readonly [number, number, number];
}

interface Entry {
  readonly model: string;
  readonly sourcePath: string;
  readonly frame: number;
  readonly frameId: string;
  readonly mode: string;
  readonly texturedSource: boolean;
  readonly unsupportedReason: string | null;
  readonly buildMilliseconds: number;
  readonly metrics: Metrics | null;
  readonly projection: RenderFrameDiff | null;
  readonly resourceIds: readonly string[];
  readonly textureResourceIds: readonly string[];
}

interface Report {
  readonly schemaVersion: 1;
  readonly entries: readonly Entry[];
  readonly resources: readonly Resource[];
  readonly textureResources: readonly Resource[];
}

declare global {
  interface Window {
    renderVoxelSurfaceComparison(index: number): Promise<void>;
  }
}

const root = requiredElement('comparison');
const report = await fetch('/comparison.json').then(async (response) => {
  if (!response.ok) throw new Error(`comparison report unavailable: ${String(response.status)}`);
  return await response.json() as Report;
});
if (report.schemaVersion !== 1) throw new Error('unsupported comparison report');
let activeSurface: Awaited<ReturnType<typeof mountRendererInspectionSurface>> | null = null;

async function renderComparison(index: number): Promise<void> {
  const entry = report.entries[index];
  if (entry === undefined) throw new Error(`comparison entry ${String(index)} is unavailable`);
  activeSurface?.dispose();
  activeSurface = null;
  root.dataset['status'] = 'loading';
  delete root.dataset['replacementMilliseconds'];
  delete root.dataset['retainedResources'];

  root.innerHTML = `
  <style>
    :root { color-scheme: dark; font-family: Inter, ui-sans-serif, system-ui, sans-serif; }
    body { margin: 0; background: #10141c; color: #eef3ff; }
    .card { width: 520px; padding: 18px; background: linear-gradient(145deg, #1b2432, #111722); }
    h1 { margin: 0 0 4px; font-size: 22px; letter-spacing: .02em; }
    .subtitle { margin: 0 0 12px; color: #9fb2ca; font-size: 14px; }
    canvas { display: block; width: 480px; height: 480px; border: 1px solid #53667a; background: #dce5ec; }
    dl { display: grid; grid-template-columns: 1fr 1fr; gap: 5px 16px; margin: 12px 0 0; font-size: 13px; }
    dt { color: #93a8c2; } dd { margin: 0; text-align: right; font-variant-numeric: tabular-nums; }
    .unsupported { height: 480px; display: grid; place-items: center; padding: 0 44px; border: 1px solid #744b4b; background: #27191d; color: #ffc7c7; text-align: center; }
  </style>
  <article class="card" data-entry="${String(index)}">
    <h1>${escapeHtml(entry.model)} · ${escapeHtml(entry.mode)}</h1>
    <p class="subtitle">${escapeHtml(entry.frameId)} · identical Studio inspection camera and neutral lighting</p>
    <div id="visual"></div>
    <dl id="metrics"></dl>
  </article>
`;

  const visual = requiredElement('visual');
  const metrics = requiredElement('metrics');
  if (entry.unsupportedReason !== null) {
    visual.innerHTML = `<div class="unsupported"><p><strong>Explicitly unsupported</strong><br />${escapeHtml(entry.unsupportedReason)}</p></div>`;
    metrics.innerHTML = metricRows([
      ['textured source', String(entry.texturedSource)],
      ['canonical voxels mutated', 'no'],
    ]);
    root.dataset['status'] = 'ready';
  } else {
    if (entry.projection === null || entry.metrics === null) throw new Error('supported entry is incomplete');
    const canvas = document.createElement('canvas');
    canvas.width = 480;
    canvas.height = 480;
    visual.append(canvas);
    const resources = new Map(report.resources.map((resource) => [resource.resource, resource]));
    const textureResources = new Map(
      report.textureResources.map((resource) => [resource.resource, resource]),
    );
    const selected = entry.resourceIds.map((id) => {
      const resource = resources.get(id);
      if (resource === undefined) throw new Error(`resource ${id} is unavailable`);
      return resource;
    });
    const selectedTextures = entry.textureResourceIds.map((id) => {
      const resource = textureResources.get(id);
      if (resource === undefined) throw new Error(`texture resource ${id} is unavailable`);
      return resource;
    });
    const surface = await mountRendererInspectionSurface(canvas, {
      autoStart: false,
      clearColor: 0xdce5ec,
      meshResourceManifest: {
        kind: 'rusty_renderer_mesh_resources.v1',
        resources: selected.map((resource) => ({
          resource: resource.resource,
          contentHash: resource.contentHash,
          byteLength: resource.byteLength,
        })),
      },
      pixelRatio: 1,
      resolveMeshResource: async (descriptor: RendererMeshResourceDescriptor) => {
        const resource = resources.get(descriptor.resource);
        if (resource === undefined) throw new Error(`resource ${descriptor.resource} is unavailable`);
        const bytes = Uint8Array.from(atob(resource.base64), (value) => value.charCodeAt(0));
        return bytes.buffer;
      },
      ...(selectedTextures.length === 0 ? {} : {
        textureResourceManifest: {
          kind: 'rusty_renderer_texture_resources.v1' as const,
          resources: selectedTextures.map((resource) => ({
            resource: resource.resource,
            contentHash: resource.contentHash,
            byteLength: resource.byteLength,
          })),
        },
        resolveTextureResource: async (descriptor: RendererTextureResourceDescriptor) => {
          const resource = textureResources.get(descriptor.resource);
          if (resource === undefined) {
            throw new Error(`texture resource ${descriptor.resource} is unavailable`);
          }
          const bytes = Uint8Array.from(atob(resource.base64), (value) => value.charCodeAt(0));
          return bytes.buffer;
        },
      }),
    });
    activeSurface = surface;
    const replacementStarted = performance.now();
    const submitted = submitStudioViewportFrame(
      surface,
      entry.projection,
      index + 1,
      'complete',
    );
    if (!submitted.receipt.applied) throw new Error('Studio viewport rejected comparison frame');
    surface.frameBounds({ min: entry.metrics.boundsMin, max: entry.metrics.boundsMax });
    surface.renderOnce(performance.now());
    const replacementMilliseconds = performance.now() - replacementStarted;
    const submission = submitted.event?.submission;
    metrics.innerHTML = metricRows([
      ['occupied voxels', integer(entry.metrics.occupiedVoxels)],
      ['vertices', integer(entry.metrics.vertices)],
      ['triangles', integer(entry.metrics.triangles)],
      ['material partitions', integer(entry.metrics.materialPartitions)],
      ['packed bytes', integer(entry.metrics.packedBytes)],
      ['build ms', entry.buildMilliseconds.toFixed(2)],
      ['replacement ms', replacementMilliseconds.toFixed(2)],
      ['retained mesh resources', integer(entry.metrics.retainedResourceCount)],
      ['draw calls', String(submission?.statistics.drawCallCount.value ?? 'unavailable')],
      ['QEF rank deficient', integer(entry.metrics.qefRankDeficient)],
      ['QEF fallbacks', integer(entry.metrics.qefFallbacks)],
    ]);
    root.dataset['replacementMilliseconds'] = replacementMilliseconds.toFixed(6);
    root.dataset['retainedResources'] = String(entry.metrics.retainedResourceCount);
    root.dataset['status'] = 'ready';
  }
}

window.renderVoxelSurfaceComparison = renderComparison;
const initialIndex = Number(new URLSearchParams(location.search).get('entry') ?? '0');
await renderComparison(initialIndex);

function requiredElement(id: string): HTMLElement {
  const element = document.getElementById(id);
  if (element === null) throw new Error(`#${id} is unavailable`);
  return element;
}

function metricRows(rows: readonly (readonly [string, string])[]): string {
  return rows.map(([label, value]) => `<dt>${escapeHtml(label)}</dt><dd>${escapeHtml(value)}</dd>`).join('');
}

function integer(value: number): string {
  return new Intl.NumberFormat('en-US').format(value);
}

function escapeHtml(value: string): string {
  return value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;').replaceAll('"', '&quot;');
}
