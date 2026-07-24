import type {
  TelemetryOverlayDescriptor,
  TelemetryOverlayHandle,
} from '@rusty-engine/render-contracts';
import type { LiveTelemetrySnapshot } from './host-types.js';
import type {
  RendererParticleBillboard,
  RendererParticleBillboardSink,
} from './particle-host.js';
import type { RendererTelemetryOverlaySink } from './telemetry-host.js';

export interface RendererDomProjection {
  readonly xPixels: number;
  readonly yPixels: number;
  readonly insideViewport: boolean;
}

export interface RendererDomParticleSinkOptions {
  readonly container: HTMLElement;
  readonly createElement?: () => HTMLDivElement;
  readonly pixelsPerWorldUnit?: number;
  readonly projectWorld: (
    position: readonly [number, number, number],
  ) => RendererDomProjection;
}

/** Browser overlay realization for the bounded particle simulation host. */
export class RendererDomParticleBillboardSink implements RendererParticleBillboardSink {
  readonly #container: HTMLElement;
  readonly #createElement: () => HTMLDivElement;
  readonly #pixelsPerWorldUnit: number;
  readonly #projectWorld: RendererDomParticleSinkOptions['projectWorld'];
  readonly #elements = new Map<number, HTMLDivElement>();

  constructor(options: RendererDomParticleSinkOptions) {
    if (!Number.isFinite(options.pixelsPerWorldUnit ?? 24) || (options.pixelsPerWorldUnit ?? 24) <= 0) {
      throw new RangeError('pixelsPerWorldUnit must be finite and greater than zero');
    }
    this.#container = options.container;
    this.#createElement = options.createElement ?? (() => document.createElement('div'));
    this.#pixelsPerWorldUnit = options.pixelsPerWorldUnit ?? 24;
    this.#projectWorld = options.projectWorld;
  }

  create(particle: RendererParticleBillboard): void {
    if (this.#elements.has(particle.id)) {
      throw new Error(`particle billboard ${String(particle.id)} already exists`);
    }
    const element = this.#createElement();
    element.dataset['rustyParticleId'] = String(particle.id);
    element.style.position = 'absolute';
    element.style.pointerEvents = 'none';
    element.style.backgroundRepeat = 'no-repeat';
    element.style.transform = 'translate(-50%, -50%)';
    element.style.willChange = 'left, top, width, height, opacity';
    this.#container.appendChild(element);
    this.#elements.set(particle.id, element);
    this.#updateElement(element, particle);
  }

  update(particle: RendererParticleBillboard): void {
    const element = this.#elements.get(particle.id);
    if (element === undefined) {
      throw new Error(`particle billboard ${String(particle.id)} does not exist`);
    }
    this.#updateElement(element, particle);
  }

  destroy(id: number): void {
    const element = this.#elements.get(id);
    if (element === undefined) return;
    element.remove();
    this.#elements.delete(id);
  }

  dispose(): void {
    for (const element of this.#elements.values()) element.remove();
    this.#elements.clear();
  }

  get activeCount(): number {
    return this.#elements.size;
  }

  #updateElement(element: HTMLDivElement, particle: RendererParticleBillboard): void {
    const projected = this.#projectWorld(particle.position);
    const size = Math.max(1, particle.size * this.#pixelsPerWorldUnit);
    const boundedFrame = Math.max(0, Math.min(particle.frameCount - 1, particle.frameIndex));
    const framePosition = particle.frameCount <= 1
      ? 0
      : (boundedFrame / (particle.frameCount - 1)) * 100;
    element.style.display = projected.insideViewport ? 'block' : 'none';
    element.style.left = `${projected.xPixels}px`;
    element.style.top = `${projected.yPixels}px`;
    element.style.width = `${size}px`;
    element.style.height = `${size}px`;
    element.style.opacity = String(Math.max(0, Math.min(1, particle.color[3])));
    element.style.backgroundColor = rgba(particle.color);
    element.style.backgroundImage = `url("${cssUrl(particle.spriteUrl)}")`;
    element.style.backgroundSize = `${String(particle.frameCount * 100)}% 100%`;
    element.style.backgroundPosition = `${String(framePosition)}% 0`;
  }
}

export interface RendererDomTelemetryOverlaySinkOptions {
  readonly container: HTMLElement;
  readonly createElement?: () => HTMLPreElement;
}

/** Default readable DOM realization for telemetry overlay descriptors. */
export class RendererDomTelemetryOverlaySink implements RendererTelemetryOverlaySink {
  readonly #container: HTMLElement;
  readonly #createElement: () => HTMLPreElement;
  readonly #elements = new Map<number, HTMLPreElement>();

  constructor(options: RendererDomTelemetryOverlaySinkOptions) {
    this.#container = options.container;
    this.#createElement = options.createElement ?? (() => document.createElement('pre'));
  }

  render(
    handle: TelemetryOverlayHandle,
    descriptor: TelemetryOverlayDescriptor,
    snapshot: LiveTelemetrySnapshot | null,
  ): void {
    const rawHandle = handle as number;
    let element = this.#elements.get(rawHandle);
    if (element === undefined) {
      element = this.#createElement();
      element.dataset['rustyTelemetryHandle'] = String(rawHandle);
      element.style.position = 'absolute';
      element.style.zIndex = '31000';
      element.style.pointerEvents = 'none';
      element.style.margin = '12px';
      element.style.padding = '8px 10px';
      element.style.borderRadius = '4px';
      element.style.background = 'rgba(8, 12, 16, 0.82)';
      element.style.color = '#d9f2ff';
      element.style.font = '12px/1.35 ui-monospace, SFMono-Regular, Menlo, monospace';
      this.#container.appendChild(element);
      this.#elements.set(rawHandle, element);
    }
    positionTelemetryElement(element, descriptor.corner);
    element.style.display = descriptor.visible ? 'block' : 'none';
    element.textContent = telemetryText(descriptor, snapshot);
  }

  destroy(handle: TelemetryOverlayHandle): void {
    const rawHandle = handle as number;
    const element = this.#elements.get(rawHandle);
    if (element === undefined) return;
    element.remove();
    this.#elements.delete(rawHandle);
  }

  dispose(): void {
    for (const element of this.#elements.values()) element.remove();
    this.#elements.clear();
  }

  get activeCount(): number {
    return this.#elements.size;
  }
}

function positionTelemetryElement(
  element: HTMLPreElement,
  corner: TelemetryOverlayDescriptor['corner'],
): void {
  element.style.top = corner.startsWith('top') ? '0' : '';
  element.style.bottom = corner.startsWith('bottom') ? '0' : '';
  element.style.left = corner.endsWith('Left') ? '0' : '';
  element.style.right = corner.endsWith('Right') ? '0' : '';
}

function telemetryText(
  descriptor: TelemetryOverlayDescriptor,
  snapshot: LiveTelemetrySnapshot | null,
): string {
  if (snapshot === null) return `${descriptor.title}\nwaiting for telemetry`;
  const metrics = snapshot.metrics.map((metric) => (
    `${metric.counter}: ${formatMetric(metric.value)} ${metric.unit}`
  ));
  const diagnostics = snapshot.diagnostics.map((diagnostic) => `! ${diagnostic.message}`);
  return [descriptor.title, ...metrics, ...diagnostics].join('\n');
}

function formatMetric(value: number): string {
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}

function rgba(value: readonly [number, number, number, number]): string {
  return `rgba(${Math.round(value[0] * 255)}, ${Math.round(value[1] * 255)}, ${Math.round(value[2] * 255)}, ${value[3]})`;
}

function cssUrl(value: string): string {
  return value.replaceAll('\\', '\\\\').replaceAll('"', '\\"');
}
