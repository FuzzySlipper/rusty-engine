import type {
  BillboardAnchor,
  BillboardContent,
  BillboardDescriptor,
  BillboardFontRef,
  BillboardHandle,
  BillboardIndicator,
  BillboardLayoutPolicy,
  BillboardPatch,
  BillboardProjectionOp,
  PresentationFrameDiff,
  PresentationOp,
} from '@rusty-engine/render-contracts';
import type {
  BillboardProjectionDiagnostic,
  BillboardProjectionReadout,
} from './host-types.js';
import { rendererResourceContentHash } from './resource-content-hash.js';

type Vec3 = readonly [number, number, number];
type BillboardPresentationOp = Extract<PresentationOp, { readonly domain: 'billboard' }>;

export interface RendererBillboardResource {
  readonly bytes: ArrayBuffer;
  readonly url?: string;
}

export type RendererBillboardResourceResolver = (
  asset: string,
  contentHash?: string,
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
  placement: { readonly x: number; readonly y: number; readonly scale: number } | null;
}

interface LayoutCandidate {
  readonly active: ActiveBillboard;
  readonly handle: BillboardHandle;
  readonly policy: BillboardLayoutPolicy;
  readonly projection: RendererBillboardScreenProjection;
  readonly width: number;
  readonly height: number;
}

interface ScreenRect {
  readonly left: number;
  readonly right: number;
  readonly top: number;
  readonly bottom: number;
}

const MAX_SUBMITTED_BILLBOARDS = 500;
const MAX_VISIBLE_BILLBOARDS = 256;
const MAX_DIAGNOSTICS = 256;

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
  #generation = 0;

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
        this.#recordDiagnostics([diagnostic]);
      }
    }
    diagnostics.push(...this.refreshLayout());
    return { applied, diagnostics, readout: this.readout() };
  }

  advance(_deltaSeconds: number): RendererBillboardFrameReceipt {
    const diagnostics = this.refreshLayout();
    return { applied: this.#active.size, diagnostics, readout: this.readout() };
  }

  requiresAnimationFrame(): boolean {
    return this.#active.size > 0;
  }

  refreshLayout(): readonly BillboardProjectionDiagnostic[] {
    const diagnostics: BillboardProjectionDiagnostic[] = [];
    let culled = 0;
    const candidates: LayoutCandidate[] = [];
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
      const content = active.descriptor.content;
      const structured = content.kind === 'structured';
      const hidden = !active.descriptor.visible
        || (!structured && !projection.insideViewport)
        || projection.distance > active.descriptor.maxDistance
        || (active.descriptor.layer === 'occluded' && projection.occluded);
      active.element.style.display = 'none';
      if (hidden) {
        culled += 1;
        continue;
      }
      if (!structured) {
        active.element.style.display = 'block';
        active.element.style.left = `${projection.xPixels}px`;
        active.element.style.top = `${projection.yPixels}px`;
        active.element.style.zIndex = billboardZIndex(active.descriptor.layer, projection.depth);
        continue;
      }
      const policy = active.descriptor.layout;
      if (policy === undefined) {
        culled += 1;
        continue;
      }
      candidates.push({
        active,
        handle,
        policy,
        projection,
        width: content.kind === 'structured' ? content.indicator.widthPixels : 1,
        height: indicatorHeight(active.descriptor),
      });
    }
    const layout = this.#layoutStructuredCandidates(candidates);
    culled += layout.culled;
    this.#culledBillboards = culled;
    this.#recordDiagnostics(diagnostics);
    return diagnostics;
  }

  #layoutStructuredCandidates(candidates: readonly LayoutCandidate[]): { readonly culled: number } {
    const viewport = billboardViewport(this.#container);
    const occupied: ScreenRect[] = [];
    let visible = 0;
    let culled = 0;
    const ordered = [...candidates].sort((left, right) =>
      right.policy.priority - left.policy.priority
      || (left.handle as number) - (right.handle as number));
    for (const candidate of ordered) {
      if (visible >= MAX_VISIBLE_BILLBOARDS) {
        candidate.active.element.style.display = 'none';
        culled += 1;
        continue;
      }
      const scale = layoutScale(candidate.policy, candidate.projection.distance);
      const halfWidth = candidate.width * scale / 2;
      const height = candidate.height * scale;
      const safe = candidate.policy.safeArea;
      let x = candidate.projection.xPixels;
      let y = candidate.projection.yPixels;
      const outside = x + halfWidth < safe.leftPixels
        || x - halfWidth > viewport.width - safe.rightPixels
        || y < safe.topPixels
        || y - height > viewport.height - safe.bottomPixels;
      if (candidate.policy.edgeBehavior === 'cull'
        && (!candidate.projection.insideViewport || outside)) {
        candidate.active.element.style.display = 'none';
        culled += 1;
        continue;
      }
      if (candidate.policy.edgeBehavior === 'clamp') {
        x = clamp(x, safe.leftPixels + halfWidth, viewport.width - safe.rightPixels - halfWidth);
        y = clamp(y, safe.topPixels + height, viewport.height - safe.bottomPixels);
      }
      const previous = candidate.active.placement;
      if (previous !== null
        && Math.abs(previous.x - x) < 0.5
        && Math.abs(previous.y - y) < 0.5
        && Math.abs(previous.scale - scale) < 0.005) {
        ({ x, y } = previous);
      }
      let rect = screenRect(x, y, candidate.width * scale, height);
      if (candidate.policy.overlapBehavior === 'stack') {
        const step = Math.max(4, candidate.active.descriptor.content.kind === 'structured'
          ? candidate.active.descriptor.content.indicator.spacingPixels + height
          : height);
        while (occupied.some((item) => rectanglesOverlap(item, rect))
          && y - step - height >= safe.topPixels) {
          y -= step;
          rect = screenRect(x, y, candidate.width * scale, height);
        }
      } else if (occupied.some((item) => rectanglesOverlap(item, rect))) {
        candidate.active.element.style.display = 'none';
        culled += 1;
        continue;
      }
      occupied.push(rect);
      candidate.active.placement = { x, y, scale };
      candidate.active.element.style.display = 'flex';
      candidate.active.element.style.left = `${x}px`;
      candidate.active.element.style.top = `${y}px`;
      candidate.active.element.style.transform = `translate(-50%, -100%) scale(${scale})`;
      candidate.active.element.style.zIndex = billboardZIndex(
        candidate.active.descriptor.layer,
        candidate.projection.depth,
      );
      visible += 1;
    }
    return { culled };
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

  #recordDiagnostics(diagnostics: readonly BillboardProjectionDiagnostic[]): void {
    for (const diagnostic of diagnostics) {
      const previous = this.#diagnostics.at(-1);
      if (previous?.code === diagnostic.code
        && previous.handle === diagnostic.handle
        && previous.message === diagnostic.message) {
        continue;
      }
      this.#diagnostics.push(diagnostic);
      if (this.#diagnostics.length > MAX_DIAGNOSTICS) this.#diagnostics.shift();
    }
  }

  cleanup(): void {
    this.#generation += 1;
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
    if (this.#active.size >= MAX_SUBMITTED_BILLBOARDS) {
      return this.#diagnostic(
        'hostFailure',
        meta.sequence,
        op.handle,
        `billboard host accepts at most ${MAX_SUBMITTED_BILLBOARDS} active descriptors`,
      );
    }
    const generation = this.#generation;
    await this.#prepareResources(op.descriptor);
    if (generation !== this.#generation) {
      return this.#diagnostic(
        'hostFailure',
        meta.sequence,
        op.handle,
        'billboard host lifecycle changed while resources were loading',
      );
    }
    if (this.#active.has(op.handle)) {
      return this.#diagnostic(
        'duplicateHandle',
        meta.sequence,
        op.handle,
        'billboard handle became active while resources were loading',
      );
    }
    const element = this.#createElement();
    element.setAttribute('data-rusty-billboard-handle', String(op.handle as number));
    this.#applyElementDescriptor(element, op.descriptor);
    appendBillboardElement(this.#container, element);
    this.#active.set(op.handle, {
      descriptor: op.descriptor,
      element,
      placement: null,
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
    const generation = this.#generation;
    await this.#prepareResources(descriptor);
    if (generation !== this.#generation || this.#active.get(op.handle) !== active) {
      return this.#diagnostic(
        'hostFailure',
        meta.sequence,
        op.handle,
        'billboard host lifecycle changed while resources were loading',
      );
    }
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
      await this.#prepareTexture(descriptor.content.texture);
    } else if (descriptor.content.kind === 'structured') {
      const textures = [
        descriptor.content.indicator.icon,
        ...descriptor.content.indicator.statusCues.map((cue) => cue.icon),
      ].filter((texture) => texture !== null);
      for (const texture of textures) {
        await this.#prepareTexture(texture);
      }
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
    const resource = await this.#resolveResource(font.asset, font.contentHash);
    if (resource === null) {
      throw new RendererBillboardResourceError('fontLoadFailed', `font resource ${font.asset} is unavailable`);
    }
    await validateResourceHash(resource.bytes, font.contentHash);
    await this.#loadFont(font.family, resource.bytes);
    this.#loadedFonts.add(cacheKey);
  }

  async #prepareTexture(texture: { readonly asset: string; readonly contentHash: string }): Promise<void> {
    const cacheKey = `${texture.asset}:${texture.contentHash}`;
    if (this.#loadedIcons.has(cacheKey)) {
      return;
    }
    const resource = await this.#resolveResource(texture.asset, texture.contentHash);
    if (resource === null || resource.url === undefined) {
      throw new RendererBillboardResourceError(
        'iconLoadFailed',
        `icon resource ${texture.asset} is unavailable or has no host URL`,
      );
    }
    await validateResourceHash(resource.bytes, texture.contentHash);
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
    if (descriptor.content.kind === 'structured') {
      this.#applyStructuredIndicator(element, descriptor.content.indicator);
      return;
    }
    if (isBrowserBillboardElement(element)) {
      resetStructuredElement(element);
    }
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

  #applyStructuredIndicator(
    element: RendererBillboardElement,
    indicator: BillboardIndicator,
  ): void {
    if (!isBrowserBillboardElement(element)) {
      element.textContent = structuredFallbackText(indicator, this.#localize);
      element.setAttribute('role', 'group');
      element.setAttribute(
        'aria-label',
        indicatorAccessibleLabel(indicator, this.#localize),
      );
      return;
    }
    if (element.dataset['rustyStructuredIndicator'] !== 'true') {
      element.textContent = '';
      element.dataset['rustyStructuredIndicator'] = 'true';
    }
    element.setAttribute('role', 'group');
    element.setAttribute('aria-label', indicatorAccessibleLabel(indicator, this.#localize));
    element.style.width = `${indicator.widthPixels}px`;
    element.style.boxSizing = 'border-box';
    element.style.padding = `${indicator.spacingPixels}px`;
    element.style.display = 'flex';
    element.style.flexDirection = 'column';
    element.style.alignItems = indicator.alignment === 'start'
      ? 'flex-start'
      : indicator.alignment === 'end' ? 'flex-end' : 'center';
    element.style.gap = `${indicator.spacingPixels}px`;
    element.style.opacity = String(indicator.style.opacity);
    element.style.backgroundColor = rgba(indicator.style.backing);
    element.style.border = `1px solid ${rgba(indicator.style.border)}`;
    element.style.borderRadius = `${indicator.style.radiusPixels}px`;

    syncLabel(element, indicator, this.#localize);
    syncIndicatorIcon(element, indicator, this.#iconUrls);
    syncMeters(element, indicator, this.#localize);
    syncStatusCues(element, indicator, this.#localize, this.#iconUrls);
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
    if (content.kind === 'icon') {
      return this.#localize(content.altKey, content.fallbackAlt, {});
    }
    return structuredFallbackText(content.indicator, this.#localize);
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
  const layout = patch.layout ?? (
    patch.content !== null && patch.content.kind !== 'structured'
      ? undefined
      : descriptor.layout
  );
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
    ...(layout === undefined ? {} : { layout }),
  };
}

function indicatorHeight(descriptor: BillboardDescriptor): number {
  if (descriptor.content.kind !== 'structured') return descriptor.heightPixels;
  const indicator = descriptor.content.indicator;
  const rows = (indicator.label === null ? 0 : 1)
    + indicator.meters.length
    + (indicator.statusCues.length === 0 ? 0 : 1);
  return Math.max(
    descriptor.heightPixels,
    rows * descriptor.heightPixels + (rows + 1) * indicator.spacingPixels,
  );
}

function layoutScale(policy: BillboardLayoutPolicy, distance: number): number {
  if (policy.sizing.kind === 'constantPixels') return 1;
  return clamp(
    policy.sizing.referenceDistance / Math.max(distance, Number.EPSILON),
    policy.sizing.minScale,
    policy.sizing.maxScale,
  );
}

function billboardViewport(container: RendererBillboardContainer): {
  readonly width: number;
  readonly height: number;
} {
  if (isHtmlElement(container)) {
    const rect = container.getBoundingClientRect();
    return {
      width: Math.max(1, rect.width || globalThis.innerWidth || 1),
      height: Math.max(1, rect.height || globalThis.innerHeight || 1),
    };
  }
  return { width: 1_000_000, height: 1_000_000 };
}

function clamp(value: number, minimum: number, maximum: number): number {
  if (minimum > maximum) return (minimum + maximum) / 2;
  return Math.max(minimum, Math.min(maximum, value));
}

function screenRect(x: number, y: number, width: number, height: number): ScreenRect {
  return { left: x - width / 2, right: x + width / 2, top: y - height, bottom: y };
}

function rectanglesOverlap(left: ScreenRect, right: ScreenRect): boolean {
  return left.left < right.right
    && left.right > right.left
    && left.top < right.bottom
    && left.bottom > right.top;
}

function localizedText(
  text: { readonly localizationKey: string; readonly fallbackText: string },
  localize: RendererBillboardLocalizer,
): string {
  return localize(text.localizationKey, text.fallbackText, {});
}

function structuredFallbackText(
  indicator: BillboardIndicator,
  localize: RendererBillboardLocalizer,
): string {
  return [
    indicator.label === null ? '' : localizedText(indicator.label, localize),
    ...indicator.statusCues.map((cue) => localizedText(cue.label, localize)),
  ].filter((value) => value !== '').join(' ');
}

function indicatorAccessibleLabel(
  indicator: BillboardIndicator,
  localize: RendererBillboardLocalizer,
): string {
  return [
    localizedText(indicator.accessibleLabel, localize),
    ...indicator.statusCues.map((cue) => localizedText(cue.label, localize)),
  ].join('; ');
}

function resetStructuredElement(element: HTMLElement): void {
  delete element.dataset['rustyStructuredIndicator'];
  element.removeAttribute('aria-label');
  element.style.width = '';
  element.style.boxSizing = '';
  element.style.padding = '';
  element.style.flexDirection = '';
  element.style.alignItems = '';
  element.style.gap = '';
  element.style.opacity = '';
  element.style.border = '';
}

function isHtmlElement(value: unknown): value is HTMLElement {
  return globalThis.HTMLElement !== undefined && value instanceof globalThis.HTMLElement;
}

function isBrowserBillboardElement(value: RendererBillboardElement): value is HTMLElement & RendererBillboardElement {
  return isHtmlElement(value);
}

function childByKey(root: HTMLElement, kind: string, id: string): HTMLElement | null {
  return [...root.children].find((child) =>
    child instanceof HTMLElement
      && child.dataset['rustyIndicatorKind'] === kind
      && child.dataset['rustyIndicatorId'] === id,
  ) as HTMLElement | undefined ?? null;
}

function keyedChild(root: HTMLElement, kind: string, id: string, tag = 'div'): HTMLElement {
  const existing = childByKey(root, kind, id);
  if (existing !== null) return existing;
  const element = root.ownerDocument.createElement(tag);
  element.dataset['rustyIndicatorKind'] = kind;
  element.dataset['rustyIndicatorId'] = id;
  root.append(element);
  return element;
}

function removeMissingChildren(root: HTMLElement, kind: string, ids: ReadonlySet<string>): void {
  for (const child of [...root.children]) {
    if (child instanceof HTMLElement
      && child.dataset['rustyIndicatorKind'] === kind
      && !ids.has(child.dataset['rustyIndicatorId'] ?? '')) {
      child.remove();
    }
  }
}

function syncLabel(
  root: HTMLElement,
  indicator: BillboardIndicator,
  localize: RendererBillboardLocalizer,
): void {
  if (indicator.label === null) {
    childByKey(root, 'label', 'label')?.remove();
    return;
  }
  const label = keyedChild(root, 'label', 'label');
  label.textContent = localizedText(indicator.label, localize);
  label.setAttribute('aria-hidden', 'true');
}

function iconUrl(
  texture: { readonly asset: string; readonly contentHash: string },
  urls: ReadonlyMap<string, string>,
): string | undefined {
  return urls.get(`${texture.asset}:${texture.contentHash}`);
}

function syncIndicatorIcon(
  root: HTMLElement,
  indicator: BillboardIndicator,
  urls: ReadonlyMap<string, string>,
): void {
  if (indicator.icon === null) {
    childByKey(root, 'icon', 'icon')?.remove();
    return;
  }
  const icon = keyedChild(root, 'icon', 'icon', 'img') as HTMLImageElement;
  const url = iconUrl(indicator.icon, urls);
  if (url !== undefined) icon.src = url;
  icon.alt = '';
  icon.setAttribute('aria-hidden', 'true');
}

function syncMeters(
  root: HTMLElement,
  indicator: BillboardIndicator,
  localize: RendererBillboardLocalizer,
): void {
  const ids = new Set(indicator.meters.map((meter) => meter.id));
  removeMissingChildren(root, 'meter', ids);
  for (const meter of indicator.meters) {
    const element = keyedChild(root, 'meter', meter.id);
    element.setAttribute('role', 'progressbar');
    element.setAttribute('aria-label', localizedText(meter.accessibleLabel, localize));
    element.setAttribute('aria-valuemin', String(meter.min));
    element.setAttribute('aria-valuemax', String(meter.max));
    element.setAttribute('aria-valuenow', String(meter.current));
    element.style.position = 'relative';
    element.style.width = '100%';
    element.style.height = '0.5em';
    element.style.overflow = 'hidden';
    element.style.background = rgba(meter.back);
    element.style.border = `1px solid ${rgba(meter.border)}`;
    const preview = keyedChild(element, 'meterPreview', meter.id);
    const fill = keyedChild(element, 'meterFill', meter.id);
    const segments = keyedChild(element, 'meterSegments', meter.id);
    for (const part of [preview, fill]) {
      part.setAttribute('aria-hidden', 'true');
      part.style.position = 'absolute';
      part.style.inset = '0';
      part.style.transformOrigin = fillOrigin(meter.fillDirection);
    }
    preview.style.background = rgba(meter.previewFill);
    preview.style.transform = fillTransform(
      meter.preview ?? meter.current,
      meter.min,
      meter.max,
      meter.fillDirection,
    );
    fill.style.background = rgba(meter.fill);
    fill.style.transform = fillTransform(meter.current, meter.min, meter.max, meter.fillDirection);
    segments.setAttribute('aria-hidden', 'true');
    segments.style.position = 'absolute';
    segments.style.inset = '0';
    segments.style.zIndex = '1';
    segments.style.backgroundImage = meter.segments === 1
      ? 'none'
      : segmentGradient(meter.segments, meter.fillDirection);
  }
}

function fillOrigin(direction: string): string {
  if (direction === 'rightToLeft') return 'right center';
  if (direction === 'bottomToTop') return 'center bottom';
  if (direction === 'topToBottom') return 'center top';
  return 'left center';
}

function segmentGradient(segments: number, direction: string): string {
  const axis = direction === 'bottomToTop' || direction === 'topToBottom'
    ? 'to bottom'
    : 'to right';
  const step = 100 / segments;
  return `repeating-linear-gradient(${axis}, transparent 0, transparent calc(${step}% - 1px), rgba(0, 0, 0, 0.72) calc(${step}% - 1px), rgba(0, 0, 0, 0.72) ${step}%)`;
}

function fillTransform(
  value: number,
  minimum: number,
  maximum: number,
  direction: string,
): string {
  const fraction = (value - minimum) / (maximum - minimum);
  return direction === 'bottomToTop' || direction === 'topToBottom'
    ? `scaleY(${fraction})`
    : `scaleX(${fraction})`;
}

function syncStatusCues(
  root: HTMLElement,
  indicator: BillboardIndicator,
  localize: RendererBillboardLocalizer,
  urls: ReadonlyMap<string, string>,
): void {
  const ids = new Set(indicator.statusCues.map((cue) => cue.id));
  removeMissingChildren(root, 'status', ids);
  for (const cue of indicator.statusCues) {
    const element = keyedChild(root, 'status', cue.id);
    element.setAttribute('aria-hidden', 'true');
    element.textContent = localizedText(cue.label, localize);
    element.style.backgroundImage = '';
    element.style.backgroundRepeat = '';
    element.style.backgroundPosition = '';
    element.style.backgroundSize = '';
    if (cue.icon !== null) {
      const url = iconUrl(cue.icon, urls);
      if (url !== undefined) {
        element.style.backgroundImage = `url("${url}")`;
        element.style.backgroundRepeat = 'no-repeat';
        element.style.backgroundPosition = 'left center';
        element.style.backgroundSize = 'contain';
      }
    }
  }
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
  const actual = await rendererResourceContentHash(bytes, expected).catch((cause: unknown) => {
    throw new RendererBillboardResourceError(
      'contentHashMismatch',
      cause instanceof Error ? cause.message : String(cause),
    );
  });
  if (actual !== expected) {
    throw new RendererBillboardResourceError(
      'contentHashMismatch',
      `billboard resource hash mismatch: expected ${expected}, got ${actual}`,
    );
  }
}
