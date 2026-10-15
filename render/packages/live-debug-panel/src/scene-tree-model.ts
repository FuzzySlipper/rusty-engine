/** One committed scene object as `engine.scene.objects` lists it. */
export interface SceneObject {
  readonly handle: number;
  readonly parent?: number;
  readonly parentJoint?: string;
  readonly kind: string;
  readonly layer: string;
  readonly visible?: boolean;
  readonly entity?: number;
  readonly label?: string;
  readonly resource?: string;
  readonly local?: SceneTransform;
  readonly world: SceneTransform;
  readonly bounds?: SceneBounds;
  readonly children: number;
  readonly changedRevision: number;
}

export interface SceneTransform {
  readonly translation: readonly [number, number, number];
  readonly rotation: readonly [number, number, number, number];
  readonly scale: readonly [number, number, number];
}

export interface SceneBounds {
  readonly localMin: readonly [number, number, number];
  readonly localMax: readonly [number, number, number];
  readonly worldMin: readonly [number, number, number];
  readonly worldMax: readonly [number, number, number];
}

/** One page of `engine.scene.objects`. */
export interface SceneObjectPage {
  readonly revision: number;
  readonly page: number;
  readonly pages: number;
  readonly total: number;
  readonly objects: readonly (SceneObject & { readonly depth: number })[];
}

/** `engine.scene.object`: the listing plus the committed description. */
export interface SceneObjectDetail extends SceneObject {
  readonly revision: number;
  readonly description: Readonly<Record<string, unknown>>;
  readonly childHandles: readonly number[];
  readonly pose?: unknown;
  readonly materialOverride?: unknown;
  readonly materialParameters?: unknown;
}

export function parseObjectPage(text: string): SceneObjectPage {
  const page = JSON.parse(text) as SceneObjectPage;
  if (!Array.isArray(page.objects)) throw new Error('engine.scene.objects answered without objects');
  return page;
}

export function parseObjectDetail(text: string): SceneObjectDetail {
  const detail = JSON.parse(text) as SceneObjectDetail;
  if (typeof detail.handle !== 'number') throw new Error('engine.scene.object answered without a handle');
  return detail;
}

/** The roots, one page: `engine.scene.objects * 0`. */
export function rootsCommand(page = 0): string {
  return `engine.scene.objects * 0${page > 0 ? ` --page ${page}` : ''}`;
}

/** A node and its children, one page: `engine.scene.objects <handle> 1`. */
export function childrenCommand(handle: number, page = 0): string {
  return `engine.scene.objects ${handle} 1${page > 0 ? ` --page ${page}` : ''}`;
}

export function objectCommand(handle: number): string {
  return `engine.scene.object ${handle}`;
}

/** The tree row's text: what the object is and what names it. */
export function objectLabel(object: SceneObject): string {
  const name = object.label ?? (object.entity !== undefined ? `entity ${object.entity}` : object.resource);
  const joint = object.parentJoint ? ` @${object.parentJoint}` : '';
  return `${object.handle} ${object.kind}${name ? ` · ${name}` : ''}${joint}`;
}

export function formatVector(values: readonly number[]): string {
  return values.map((value) => (Math.abs(value) < 5e-5 ? 0 : value).toFixed(3)).join(', ');
}

/** The selected object's facts, as label/value rows. */
export function objectFacts(object: SceneObject): [string, string][] {
  const rows: [string, string][] = [
    ['handle', String(object.handle)],
    ['kind', object.kind],
    ['layer', object.layer],
  ];
  if (object.parent !== undefined) rows.push(['parent', String(object.parent)]);
  if (object.parentJoint) rows.push(['joint', object.parentJoint]);
  if (object.entity !== undefined) rows.push(['entity', String(object.entity)]);
  if (object.label) rows.push(['label', object.label]);
  if (object.resource) rows.push(['resource', object.resource]);
  if (object.visible !== undefined) rows.push(['visible', String(object.visible)]);
  if (object.local) rows.push(['local', formatVector(object.local.translation)]);
  rows.push(['world', formatVector(object.world.translation)]);
  rows.push(['rotation', formatVector(object.world.rotation)]);
  rows.push(['scale', formatVector(object.world.scale)]);
  if (object.bounds) {
    rows.push(['bounds min', formatVector(object.bounds.worldMin)]);
    rows.push(['bounds max', formatVector(object.bounds.worldMax)]);
  }
  rows.push(['children', String(object.children)]);
  rows.push(['changed', `revision ${object.changedRevision}`]);
  return rows;
}

/** What `engine.inspect.view` lists: the views and the selected one. */
export interface InspectionViews {
  readonly views: readonly { readonly id: string }[];
  readonly selected?: string;
}

export function viewsCommand(): string {
  return 'engine.inspect.view list';
}

export function parseViews(text: string): InspectionViews {
  const views = JSON.parse(text) as InspectionViews;
  if (!Array.isArray(views.views)) throw new Error('engine.inspect.view answered without views');
  return views;
}

/** Frames `view` on the object from its side. */
export function frameCommand(view: string, handle: number): string {
  return `engine.inspect.view frame ${view} side ${handle}`;
}

export function overlaysCommand(): string {
  return 'engine.inspect.overlay list';
}

export function parseOverlays(text: string): { readonly bounds: readonly number[] } {
  const overlays = JSON.parse(text) as { bounds?: number[] };
  return { bounds: overlays.bounds ?? [] };
}

export function boundsCommand(handle: number, show: boolean): string {
  return `engine.inspect.overlay ${show ? 'show' : 'hide'} bounds ${handle}`;
}
