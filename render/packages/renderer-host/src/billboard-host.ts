import type {
  BillboardAnchor,
  BillboardContent,
  BillboardDescriptor,
  BillboardFontRef,
  BillboardHandle,
  BillboardPatch,
  BillboardProjectionOp,
  PresentationFrameDiff,
  PresentationOp,
} from '@rusty-engine/render-contracts';
import type {
  BillboardProjectionDiagnostic,
  BillboardProjectionReadout,
} from './host-types.js';

type Vec3 = readonly [number, number, number];
type BillboardPresentationOp = Extract<PresentationOp, { readonly domain: 'billboard' }>;

export interface RendererBillboardResource {
  readonly bytes: ArrayBuffer;
  readonly url?: string;
}

export type RendererBillboardResourceResolver = (
  asset: string,
) => Promise<RendererBillboardResource | null>;

export type RendererBillboardEntityPositionResolver = (entity: number) => Vec3 | null;

export interface RendererBillboardScreenProjection {
  readonly xPixels: number;
  readonly yPixels: number;
  readonly depth: number;
  readonly distance: number;
  readonly insideViewport: boolean;
  readonly occluded: boolean;
}

export type RendererBillboardWorldProjector = (
  position: Vec3,
) => RendererBillboardScreenProjection;

export type RendererBillboardLocalizer = (
  key: string,
  fallback: string,
  argumentsByName: Readonly<Record<string, string>>,
) => string;

export interface RendererBillboardElementStyle {
  backgroundColor: string;
  backgroundImage: string;
  backgroundPosition: string;
  backgroundRepeat: string;
  backgroundSize: string;
  borderRadius: string;
  color: string;
  display: string;
  fontFamily: string;
  fontSize: string;
  left: string;
  lineHeight: string;
  pointerEvents: string;
  position: string;
  top: string;
  transform: string;
  whiteSpace: string;
  zIndex: string;
}

export interface RendererBillboardElement {
  readonly style: RendererBillboardElementStyle;
  textContent: string | null;
  setAttribute(name: string, value: string): void;
  remove(): void;
}

export interface RendererBillboardContainerPort {
  appendChild(element: RendererBillboardElement): unknown;
}

export type RendererBillboardContainer = RendererBillboardContainerPort | HTMLElement;

export type RendererBillboardElementFactory = () => RendererBillboardElement;

export type RendererBillboardFontLoader = (
  family: string,
  bytes: ArrayBuffer,
) => Promise<void>;

export interface RendererBillboardHostOptions {
  readonly container: RendererBillboardContainer;
  readonly createElement?: RendererBillboardElementFactory;
  readonly loadFont?: RendererBillboardFontLoader;
  readonly localize?: RendererBillboardLocalizer;
  readonly projectWorld: RendererBillboardWorldProjector;
  readonly resolveEntityPosition: RendererBillboardEntityPositionResolver;
  readonly resolveResource?: RendererBillboardResourceResolver;
}

export interface RendererBillboardFrameReceipt {
  readonly applied: number;
  readonly diagnostics: readonly BillboardProjectionDiagnostic[];
  readonly readout: BillboardProjectionReadout;
}

interface ActiveBillboard {
  descriptor: BillboardDescriptor;
  readonly element: RendererBillboardElement;
}

export class RendererBillboardHost {
  readonly #container: RendererBillboardContainer;
  readonly #createElement: RendererBillboardElementFactory;
  readonly #loadFont: RendererBillboardFontLoader;
  readonly #localize: RendererBillboardLocalizer;
  readonly #projectWorld: RendererBillboardWorldProjector;
  readonly #resolveEntityPosition: RendererBillboardEntityPositionResolver;
  readonly #resolveResource: RendererBillboardResourceResolver;
  readonly #active = new Map<BillboardHandle, ActiveBillboard>();
  readonly #loadedFonts = new Set<string>();
  readonly #loadedIcons = new Set<string>();
  readonly #iconUrls = new Map<string, string>();
  readonly #diagnostics: BillboardProjectionDiagnostic[] = [];
  #culledBillboards = 0;

  constructor(options: RendererBillboardHostOptions) {
    this.#container = options.container;
    this.#createElement = options.createElement ?? createBrowserBillboardElement;
    this.#loadFont = options.loadFont ?? loadBrowserFont;
    this.#localize = options.localize ?? defaultLocalizer;
    this.#projectWorld = options.projectWorld;
    this.#resolveEntityPosition = options.resolveEntityPosition;
    this.#resolveResource = options.resolveResource ?? (async () => null);
  }

  async applyPresentation(frame: PresentationFrameDiff): Promise<RendererBillboardFrameReceipt> {
    const diagnostics: BillboardProjectionDiagnostic[] = [];
    let applied = 0;
    for (const operation of frame.ops) {
      if (operation.domain !== 'billboard') {
        continue;
      }
      const diagnostic = await this.#applyOperation(operation);
      if (diagnostic === null) {
        applied += 1;
      } else {
        diagnostics.push(diagnostic);
        this.#diagnostics.push(diagnostic);
      }
    }
    diagnostics.push(...this.refreshLayout());
    return { applied, diagnostics, readout: this.readout() };
  }

  refreshLayout(): readonly BillboardProjectionDiagnostic[] {
    const diagnostics: BillboardProjectionDiagnostic[] = [];
    let culled = 0;
    for (const [handle, active] of this.#active) {
      const position = this.#resolveAnchor(active.descriptor.anchor);
      if (position === null) {
        active.element.style.display = 'none';
        culled += 1;
        diagnostics.push(
          this.#diagnostic(
            'anchorMissing',
            0,
            handle,
            'billboard entity anchor is unavailable',
          ),
        );
        continue;
      }
      const projection = this.#projectWorld(position);
      const hidden = !active.descriptor.visible
        || !projection.insideViewport
        || projection.distance > active.descriptor.maxDistance
        || (active.descriptor.layer === 'occluded' && projection.occluded);
      active.element.style.display = hidden ? 'none' : 'block';
      if (hidden) {
        culled += 1;
        continue;
      }
      active.element.style.left = `${projection.xPixels}px`;
      active.element.style.top = `${projection.yPixels}px`;
      active.element.style.zIndex = billboardZIndex(active.descriptor.layer, projection.depth);
    }
    this.#culledBillboards = culled;
    this.#diagnostics.push(...diagnostics);
    return diagnostics;
  }

  readout(): BillboardProjectionReadout {
    return {
      activeBillboards: this.#active.size,
      loadedFonts: this.#loadedFonts.size,
      loadedIcons: this.#loadedIcons.size,
      culledBillboards: this.#culledBillboards,
      diagnostics: [...this.#diagnostics],
    };
  }

  cleanup(): void {
    for (const active of this.#active.values()) {
      active.element.remove();
    }
    this.#active.clear();
    this.#culledBillboards = 0;
  }

  dispose(): void {
    this.cleanup();
    this.#loadedFonts.clear();
    this.#loadedIcons.clear();
    this.#iconUrls.clear();
    this.#diagnostics.length = 0;
  }

  async #applyOperation(
    operation: BillboardPresentationOp,
  ): Promise<BillboardProjectionDiagnostic | null> {
    try {
      switch (operation.op.op) {
        case 'create':
          return await this.#create(operation.meta, operation.op);
        case 'update':
          return await this.#update(operation.meta, operation.op);
        case 'destroy':
          return this.#destroy(operation.meta, operation.op);
      }
    } catch (error) {
      return this.#diagnostic(
        classifyBillboardHostError(error),
        operation.meta.sequence,
        operation.op.handle,
        error instanceof Error ? error.message : String(error),
      );
    }
  }

  async #create(
    meta: BillboardPresentationOp['meta'],
    op: Extract<BillboardProjectionOp, { readonly op: 'create' }>,
  ): Promise<BillboardProjectionDiagnostic | null> {
    if (this.#active.has(op.handle)) {
      return this.#diagnostic(
        'duplicateHandle',
        meta.sequence,
        op.handle,
        'billboard handle is already active',
      );
    }
    await this.#prepareResources(op.descriptor);
    const element = this.#createElement();
    element.setAttribute('data-rusty-billboard-handle', String(op.handle as number));
    this.#applyElementDescriptor(element, op.descriptor);
    appendBillboardElement(this.#container, element);
    this.#active.set(op.handle, {
      descriptor: op.descriptor,
      element,
    });
    return null;
  }

  async #update(
    meta: BillboardPresentationOp['meta'],
    op: Extract<BillboardProjectionOp, { readonly op: 'update' }>,
  ): Promise<BillboardProjectionDiagnostic | null> {
    const active = this.#active.get(op.handle);
    if (active === undefined) {
      return this.#diagnostic(
        'unknownHandle',
        meta.sequence,
        op.handle,
        'billboard handle is not active',
      );
    }
    const descriptor = applyBillboardPatch(active.descriptor, op.patch);
    await this.#prepareResources(descriptor);
    this.#applyElementDescriptor(active.element, descriptor);
    active.descriptor = descriptor;
    return null;
  }

  #destroy(
    meta: BillboardPresentationOp['meta'],
    op: Extract<BillboardProjectionOp, { readonly op: 'destroy' }>,
  ): BillboardProjectionDiagnostic | null {
    const active = this.#active.get(op.handle);
    if (active === undefined) {
      return this.#diagnostic(
        'unknownHandle',
        meta.sequence,
        op.handle,
        'billboard handle is not active',
      );
    }
    active.element.remove();
    this.#active.delete(op.handle);
    return null;
  }

  async #prepareResources(descriptor: BillboardDescriptor): Promise<void> {
    await this.#prepareFont(descriptor.font);
    if (descriptor.content.kind === 'icon') {
      await this.#prepareIcon(descriptor.content);
    }
  }

  async #prepareFont(font: BillboardFontRef): Promise<void> {
    if (font.kind === 'system') {
      return;
    }
    const cacheKey = `${font.asset}:${font.contentHash}`;
    if (this.#loadedFonts.has(cacheKey)) {
      return;
    }
    const resource = await this.#resolveResource(font.asset);
    if (resource === null) {
      throw new RendererBillboardResourceError('fontLoadFailed', `font resource ${font.asset} is unavailable`);
    }
    await validateResourceHash(resource.bytes, font.contentHash);
    await this.#loadFont(font.family, resource.bytes);
    this.#loadedFonts.add(cacheKey);
  }

  async #prepareIcon(content: Extract<BillboardContent, { readonly kind: 'icon' }>): Promise<void> {
    const cacheKey = `${content.texture.asset}:${content.texture.contentHash}`;
    if (this.#loadedIcons.has(cacheKey)) {
      return;
    }
    const resource = await this.#resolveResource(content.texture.asset);
    if (resource === null || resource.url === undefined) {
      throw new RendererBillboardResourceError(
        'iconLoadFailed',
        `icon resource ${content.texture.asset} is unavailable or has no host URL`,
      );
    }
    await validateResourceHash(resource.bytes, content.texture.contentHash);
    this.#loadedIcons.add(cacheKey);
    this.#iconUrls.set(cacheKey, resource.url);
  }

  #applyElementDescriptor(element: RendererBillboardElement, descriptor: BillboardDescriptor): void {
    element.style.position = 'absolute';
    element.style.pointerEvents = 'none';
    element.style.transform = 'translate(-50%, -100%)';
    element.style.whiteSpace = 'nowrap';
    element.style.borderRadius = '4px';
    element.style.lineHeight = '1.2';
    element.style.fontFamily = descriptor.font.family;
    element.style.fontSize = `${descriptor.heightPixels}px`;
    element.style.color = rgba(descriptor.color);
    element.style.backgroundColor = rgba(descriptor.background);
    element.style.backgroundImage = '';
    element.style.backgroundPosition = 'center';
    element.style.backgroundRepeat = 'no-repeat';
    element.style.backgroundSize = 'contain';
    element.setAttribute('data-rusty-billboard-layer', descriptor.layer);
    element.textContent = this.#contentText(descriptor.content);
    if (descriptor.content.kind === 'icon') {
      element.setAttribute('role', 'img');
      element.setAttribute('aria-label', element.textContent);
      const cacheKey = `${descriptor.content.texture.asset}:${descriptor.content.texture.contentHash}`;
      const iconUrl = this.#iconUrls.get(cacheKey);
      if (iconUrl !== undefined) {
        element.style.backgroundImage = `url("${iconUrl}")`;
      }
    } else {
      element.setAttribute('role', 'status');
    }
  }

  #contentText(content: BillboardContent): string {
    if (content.kind === 'text') {
      return this.#localize(
        content.localizationKey,
        content.fallbackText,
        Object.fromEntries(content.arguments.map((argument) => [argument.name, argument.value])),
      );
    }
    if (content.kind === 'value') {
      const label = this.#localize(content.labelKey, content.fallbackLabel, {});
      const unit = content.unitKey === null
        ? (content.fallbackUnit ?? '')
        : this.#localize(content.unitKey, content.fallbackUnit ?? '', {});
      return `${label}: ${content.value}${unit === '' ? '' : ` ${unit}`}`;
    }
    return this.#localize(content.altKey, content.fallbackAlt, {});
  }

  #resolveAnchor(anchor: BillboardAnchor): Vec3 | null {
    if (anchor.kind === 'world') {
      return anchor.position;
    }
    const position = this.#resolveEntityPosition(anchor.entity);
    if (position === null) {
      return null;
    }
    return [
      position[0] + anchor.offset[0],
      position[1] + anchor.offset[1],
      position[2] + anchor.offset[2],
    ];
  }

  #diagnostic(
    code: BillboardProjectionDiagnostic['code'],
    sequence: number,
    handle: BillboardHandle,
    message: string,
  ): BillboardProjectionDiagnostic {
    return { code, sequence, handle, message };
  }
}

function applyBillboardPatch(
  descriptor: BillboardDescriptor,
  patch: BillboardPatch,
): BillboardDescriptor {
  return {
    anchor: patch.anchor ?? descriptor.anchor,
    content: patch.content ?? descriptor.content,
    font: patch.font ?? descriptor.font,
    heightPixels: patch.heightPixels ?? descriptor.heightPixels,
    color: patch.color ?? descriptor.color,
    background: patch.background ?? descriptor.background,
    maxDistance: patch.maxDistance ?? descriptor.maxDistance,
    layer: patch.layer ?? descriptor.layer,
    visible: patch.visible ?? descriptor.visible,
  };
}

function rgba(value: readonly [number, number, number, number]): string {
  return `rgba(${Math.round(value[0] * 255)}, ${Math.round(value[1] * 255)}, ${Math.round(value[2] * 255)}, ${value[3]})`;
}

function billboardZIndex(layer: BillboardDescriptor['layer'], depth: number): string {
  if (layer === 'alwaysOnTop') {
    return '30000';
  }
  const boundedDepth = Math.max(0, Math.min(1, depth));
  return String(20000 - Math.round(boundedDepth * 10000));
}

function defaultLocalizer(
  _key: string,
  fallback: string,
  argumentsByName: Readonly<Record<string, string>>,
): string {
  return Object.entries(argumentsByName).reduce(
    (text, [name, value]) => text.replaceAll(`{${name}}`, value),
    fallback,
  );
}

function createBrowserBillboardElement(): RendererBillboardElement {
  if (globalThis.document === undefined) {
    throw new Error('billboard DOM host is unavailable');
  }
  return globalThis.document.createElement('div') as unknown as RendererBillboardElement;
}

function appendBillboardElement(
  container: RendererBillboardContainer,
  element: RendererBillboardElement,
): void {
  if (globalThis.HTMLElement !== undefined && container instanceof globalThis.HTMLElement) {
    container.appendChild(element as unknown as Node);
    return;
  }
  (container as RendererBillboardContainerPort).appendChild(element);
}

async function loadBrowserFont(family: string, bytes: ArrayBuffer): Promise<void> {
  if (globalThis.FontFace === undefined || globalThis.document?.fonts === undefined) {
    throw new RendererBillboardResourceError('fontLoadFailed', 'browser FontFace host is unavailable');
  }
  const font = await new globalThis.FontFace(family, bytes).load();
  globalThis.document.fonts.add(font);
}

class RendererBillboardResourceError extends Error {
  constructor(
    readonly code: 'contentHashMismatch' | 'fontLoadFailed' | 'iconLoadFailed' | 'hostFailure',
    message: string,
  ) {
    super(message);
  }
}

function classifyBillboardHostError(
  error: unknown,
): BillboardProjectionDiagnostic['code'] {
  if (error instanceof RendererBillboardResourceError) {
    return error.code;
  }
  return 'hostFailure';
}

async function validateResourceHash(bytes: ArrayBuffer, expected: string): Promise<void> {
  if (globalThis.crypto?.subtle === undefined) {
    throw new RendererBillboardResourceError('hostFailure', 'Web Crypto SHA-256 is unavailable');
  }
  const digest = await globalThis.crypto.subtle.digest('SHA-256', bytes);
  const actual = Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, '0'))
    .join('');
  if (actual !== expected) {
    throw new RendererBillboardResourceError(
      'contentHashMismatch',
      `billboard resource hash mismatch: expected ${expected}, got ${actual}`,
    );
  }
}
