import { signal, type Signal } from '@angular/core';
import type {
  AdapterDescription,
  CanonicalOwnerContent,
  LoadingBayDomainReadout,
  OwnerInspections,
  ProjectionReadout,
  SceneHierarchyNodeReadout,
  SceneHierarchyReadout,
  StudioProjectIdentity,
  StudioProjectReadout,
} from '@rusty-engine/studio-adapter-client';
import {
  StudioAdapterOperationRejected,
} from '@rusty-engine/studio-adapter-client';
import type { StudioAdapterClient } from '@rusty-engine/studio-adapter-client';
import type {
  RenderDiff,
  RenderFrameDiff,
  RenderHandle,
  RenderMetadata,
  Transform,
} from '@rusty-engine/render-contracts';

export type StudioConnectionState =
  | { readonly kind: 'disconnected'; readonly message: string }
  | { readonly kind: 'connecting'; readonly message: string }
  | { readonly kind: 'connected'; readonly adapter: AdapterDescription; readonly message: string }
  | { readonly kind: 'unavailable'; readonly message: string };

export interface AuthoringDocumentView {
  readonly identity: StudioProjectIdentity;
  readonly canonical: CanonicalOwnerContent;
  readonly inspections: OwnerInspections;
  readonly sceneHierarchy: SceneHierarchyReadout;
  readonly domain: LoadingBayDomainReadout;
  readonly voxel: Readonly<Record<string, unknown>> | null;
}

export interface ProjectedEntityView {
  readonly entityId: number;
  readonly label: string;
  readonly asset: string | null;
  readonly transform: Transform | null;
  readonly renderHandle: RenderHandle | null;
  readonly projected: boolean;
}

export interface LiveProjectionView {
  readonly frame: RenderFrameDiff;
  readonly readout: ProjectionReadout;
  readonly entities: readonly ProjectedEntityView[];
  readonly generation: number;
}

export interface TransformPreviewState {
  readonly entityId: number;
  readonly original: readonly [number, number, number];
  readonly translation: readonly [number, number, number];
}

export interface EditorSelectionState {
  readonly sceneNodeId: number | null;
  readonly entityId: number | null;
  readonly source: 'hierarchy' | 'renderer' | 'inspector' | null;
}

export interface StudioViewSettings {
  readonly gridVisible: boolean;
  readonly snappingEnabled: boolean;
  readonly translationSnap: number;
  readonly theme: 'graphite' | 'highContrast';
}

export interface StudioWorkspaceSnapshot {
  readonly connection: StudioConnectionState;
  readonly authoringDocument: AuthoringDocumentView | null;
  readonly liveProjection: LiveProjectionView | null;
  readonly preview: TransformPreviewState | null;
  readonly selection: EditorSelectionState;
  readonly operation: 'idle' | 'opening' | 'refreshing' | 'committing' | 'closing';
  readonly hierarchyFilter: string;
  readonly activeMenu: 'file' | 'edit' | 'view' | 'tools' | null;
  readonly bottomPanel: 'diagnostics' | 'owners' | 'output';
  readonly settingsOpen: boolean;
  readonly settings: StudioViewSettings;
  readonly lastError: string | null;
}

const INITIAL_SETTINGS: StudioViewSettings = {
  gridVisible: true,
  snappingEnabled: true,
  translationSnap: 0.5,
  theme: 'graphite',
};

function initialSnapshot(): StudioWorkspaceSnapshot {
  return {
    connection: { kind: 'disconnected', message: 'Studio host is not connected.' },
    authoringDocument: null,
    liveProjection: null,
    preview: null,
    selection: { sceneNodeId: null, entityId: null, source: null },
    operation: 'idle',
    hierarchyFilter: '',
    activeMenu: null,
    bottomPanel: 'diagnostics',
    settingsOpen: false,
    settings: INITIAL_SETTINGS,
    lastError: null,
  };
}

export class StudioWorkspaceStore {
  readonly #client: StudioAdapterClient;
  readonly #snapshot = signal<StudioWorkspaceSnapshot>(initialSnapshot());
  readonly snapshot: Signal<StudioWorkspaceSnapshot> = this.#snapshot.asReadonly();

  constructor(client: StudioAdapterClient) {
    this.#client = client;
  }

  async connect(): Promise<boolean> {
    const current = this.#snapshot();
    if (current.connection.kind === 'connected') return true;
    this.#patch({
      connection: { kind: 'connecting', message: 'Connecting to the project adapter…' },
      lastError: null,
    });
    try {
      const response = await this.#client.describe();
      this.#patch({
        connection: {
          kind: 'connected',
          adapter: response.adapter,
          message: `${response.adapter.adapterId} is ready. No project is open.`,
        },
      });
      return true;
    } catch (error) {
      const message = errorMessage(error);
      this.#patch({
        connection: { kind: 'unavailable', message },
        lastError: message,
      });
      return false;
    }
  }

  async openProject(root: string, projectFile: string): Promise<void> {
    if (!(await this.connect())) return;
    this.#patch({ operation: 'opening', lastError: null, activeMenu: null });
    try {
      const response = await this.#client.openProject(root, projectFile);
      this.#acceptProject(response.project, true);
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async refreshProject(): Promise<void> {
    if (this.#snapshot().authoringDocument === null) return;
    this.#patch({ operation: 'refreshing', lastError: null });
    try {
      const response = await this.#client.readProject();
      this.#acceptProject(response.project, false);
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  async closeProject(): Promise<void> {
    if (this.#snapshot().connection.kind !== 'connected') return;
    this.#patch({ operation: 'closing', lastError: null });
    try {
      await this.#client.closeProject();
      const connection = this.#snapshot().connection;
      this.#patch({
        connection: connection.kind === 'connected'
          ? {
              ...connection,
              message: `${connection.adapter.adapterId} is ready. No project is open.`,
            }
          : connection,
        authoringDocument: null,
        liveProjection: null,
        preview: null,
        selection: { sceneNodeId: null, entityId: null, source: null },
        operation: 'idle',
      });
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  selectEntity(entityId: number | null, source: EditorSelectionState['source']): void {
    const sceneNodeId = entityId === null
      ? null
      : (this.#snapshot().authoringDocument?.sceneHierarchy.nodes.find(
          (node) => node.entityId === entityId,
        )?.nodeId ?? null);
    this.#patch({
      selection: {
        sceneNodeId,
        entityId,
        source: entityId === null ? null : source,
      },
      preview: this.#snapshot().preview?.entityId === entityId ? this.#snapshot().preview : null,
    });
  }

  selectHierarchyNode(sceneNodeId: number): void {
    const node = this.#snapshot().authoringDocument?.sceneHierarchy.nodes.find(
      (candidate) => candidate.nodeId === sceneNodeId,
    );
    if (node === undefined) return;
    this.#patch({
      selection: {
        sceneNodeId: node.nodeId,
        entityId: node.entityId,
        source: 'hierarchy',
      },
      preview: node.entityId !== null && this.#snapshot().preview?.entityId === node.entityId
        ? this.#snapshot().preview
        : null,
    });
  }

  beginTranslationPreview(entityId: number): void {
    const node = this.#snapshot().authoringDocument?.sceneHierarchy.nodes.find(
      (candidate) => candidate.entityId === entityId,
    );
    if (node === undefined) return;
    const translation = node.localTransform.translation;
    this.#patch({
      selection: { sceneNodeId: node.nodeId, entityId, source: 'inspector' },
      preview: { entityId, original: translation, translation },
      lastError: null,
    });
  }

  setPreviewTranslationAxis(axis: 0 | 1 | 2, value: number): void {
    const preview = this.#snapshot().preview;
    if (preview === null) return;
    const translation: [number, number, number] = [...preview.translation];
    translation[axis] = value;
    this.#patch({ preview: { ...preview, translation } });
  }

  cancelPreview(): void {
    this.#patch({ preview: null, lastError: null });
  }

  async commitPreview(): Promise<void> {
    const current = this.#snapshot();
    const preview = current.preview;
    const document = current.authoringDocument;
    if (preview === null || document === null) return;
    this.#patch({ operation: 'committing', lastError: null });
    try {
      const response = await this.#client.setEntityTranslation({
        expectedProjectHash: document.identity.projectHash,
        expectedSceneRevision: document.identity.sceneRevision,
        entityId: preview.entityId,
        translation: preview.translation,
      });
      this.#acceptProject(response.project, false);
      this.#patch({ preview: null });
    } catch (error) {
      this.#operationFailed(error);
    }
  }

  setHierarchyFilter(value: string): void {
    this.#patch({ hierarchyFilter: value });
  }

  visibleHierarchyNodes(): readonly SceneHierarchyNodeReadout[] {
    const state = this.#snapshot();
    const query = state.hierarchyFilter.trim().toLocaleLowerCase();
    const nodes = [...(state.authoringDocument?.sceneHierarchy.nodes ?? [])]
      .sort((left, right) => left.displayOrder - right.displayOrder);
    if (query.length === 0) return nodes;
    return nodes.filter((node) =>
      `${node.label} ${String(node.nodeId)} ${String(node.entityId ?? '')} ${node.nodeKind} ${node.asset ?? ''} ${node.tags.join(' ')}`
        .toLocaleLowerCase()
        .includes(query),
    );
  }

  selectedHierarchyNode(): SceneHierarchyNodeReadout | null {
    const state = this.#snapshot();
    return state.authoringDocument?.sceneHierarchy.nodes.find(
      (node) => node.nodeId === state.selection.sceneNodeId,
    ) ?? null;
  }

  selectedEntity(): ProjectedEntityView | null {
    const state = this.#snapshot();
    return state.liveProjection?.entities.find(
      (entity) => entity.entityId === state.selection.entityId,
    ) ?? null;
  }

  toggleMenu(menu: StudioWorkspaceSnapshot['activeMenu']): void {
    this.#patch({ activeMenu: this.#snapshot().activeMenu === menu ? null : menu });
  }

  setBottomPanel(panel: StudioWorkspaceSnapshot['bottomPanel']): void {
    this.#patch({ bottomPanel: panel });
  }

  setSettingsOpen(open: boolean): void {
    this.#patch({ settingsOpen: open, activeMenu: null });
  }

  updateSettings(update: Partial<StudioViewSettings>): void {
    this.#patch({ settings: { ...this.#snapshot().settings, ...update } });
  }

  clearError(): void {
    this.#patch({ lastError: null });
  }

  reportUiError(message: string): void {
    this.#patch({ lastError: message });
  }

  #acceptProject(project: StudioProjectReadout, resetSelection: boolean): void {
    const current = this.#snapshot();
    const entities = summarizeProjectionForUi(
      project.projection,
      project.inspections.entityState.entityIds,
      [],
    );
    const selection = acceptedSelection(project, current.selection, resetSelection);
    const connection = current.connection.kind === 'connected'
      ? {
          ...current.connection,
          message: `${current.connection.adapter.adapterId} · ${project.identity.name} is open.`,
        }
      : current.connection;
    this.#patch({
      connection,
      authoringDocument: {
        identity: project.identity,
        canonical: project.canonical,
        inspections: project.inspections,
        sceneHierarchy: project.sceneHierarchy,
        domain: project.loadingBay,
        voxel: project.voxel ?? null,
      },
      liveProjection: {
        frame: project.projection,
        readout: project.projectionReadout,
        entities,
        generation: (current.liveProjection?.generation ?? 0) + 1,
      },
      selection,
      preview: null,
      operation: 'idle',
      lastError: null,
    });
  }

  #operationFailed(error: unknown): void {
    const message = error instanceof StudioAdapterOperationRejected
      ? `${error.rejection.code}: ${error.rejection.message}`
      : errorMessage(error);
    this.#patch({ operation: 'idle', lastError: message });
  }

  #patch(update: Partial<StudioWorkspaceSnapshot>): void {
    this.#snapshot.update((current) => ({ ...current, ...update }));
  }
}

function acceptedSelection(
  project: StudioProjectReadout,
  previous: EditorSelectionState,
  reset: boolean,
): EditorSelectionState {
  if (reset) return { sceneNodeId: null, entityId: null, source: null };
  const node = project.sceneHierarchy.nodes.find((candidate) =>
    previous.entityId === null
      ? candidate.nodeId === previous.sceneNodeId
      : candidate.entityId === previous.entityId,
  );
  if (node === undefined) return { sceneNodeId: null, entityId: null, source: null };
  return { sceneNodeId: node.nodeId, entityId: node.entityId, source: previous.source };
}

/**
 * Produces a disposable UI list from owner IDs and shared projection metadata.
 * It is not an authored hierarchy or a retained renderer implementation.
 */
export function summarizeProjectionForUi(
  frame: RenderFrameDiff,
  ownerEntityIds: readonly number[],
  previous: readonly ProjectedEntityView[],
): readonly ProjectedEntityView[] {
  const byEntity = new Map(previous.map((entity) => [entity.entityId, entity]));
  const handleToEntity = new Map(
    previous.flatMap((entity) =>
      entity.renderHandle === null ? [] : [[entity.renderHandle, entity.entityId] as const],
    ),
  );
  for (const operation of frame.ops) {
    applyProjectionOperation(operation, byEntity, handleToEntity);
  }
  for (const entityId of ownerEntityIds) {
    if (!byEntity.has(entityId)) {
      byEntity.set(entityId, {
        entityId,
        label: `Entity ${String(entityId)}`,
        asset: null,
        transform: null,
        renderHandle: null,
        projected: false,
      });
    }
  }
  return [...byEntity.values()]
    .filter((entity) => ownerEntityIds.includes(entity.entityId))
    .sort((left, right) => left.entityId - right.entityId);
}

function applyProjectionOperation(
  operation: RenderDiff,
  entities: Map<number, ProjectedEntityView>,
  handles: Map<RenderHandle, number>,
): void {
  if (operation.op === 'destroy') {
    const entityId = handles.get(operation.handle);
    if (entityId !== undefined) {
      const previous = entities.get(entityId);
      if (previous !== undefined) {
        entities.set(entityId, {
          ...previous,
          asset: null,
          transform: null,
          renderHandle: null,
          projected: false,
        });
      }
      handles.delete(operation.handle);
    }
    return;
  }
  if (operation.op === 'update') {
    const entityId = handles.get(operation.handle);
    if (entityId === undefined) return;
    const previous = entities.get(entityId);
    if (previous === undefined) return;
    entities.set(entityId, {
      ...previous,
      label: operation.metadata?.label ?? previous.label,
      transform: operation.transform ?? previous.transform,
    });
    return;
  }

  const descriptor = projectionDescriptor(operation);
  if (descriptor === null || descriptor.metadata.sourceEntity === null) return;
  const entityId = descriptor.metadata.sourceEntity;
  entities.set(entityId, {
    entityId,
    label: descriptor.metadata.label ?? `Entity ${String(entityId)}`,
    asset: descriptor.asset,
    transform: descriptor.transform,
    renderHandle: descriptor.handle,
    projected: true,
  });
  handles.set(descriptor.handle, entityId);
}

function projectionDescriptor(operation: RenderDiff): {
  readonly handle: RenderHandle;
  readonly asset: string | null;
  readonly transform: Transform;
  readonly metadata: RenderMetadata;
} | null {
  switch (operation.op) {
    case 'create':
      return {
        handle: operation.handle,
        asset: operation.node.geometry.kind,
        transform: operation.node.transform,
        metadata: operation.node.metadata,
      };
    case 'createStaticMeshInstance':
      return {
        handle: operation.handle,
        asset: operation.instance.asset,
        transform: operation.instance.transform,
        metadata: operation.instance.metadata,
      };
    case 'createAnimatedMeshInstance':
      return {
        handle: operation.handle,
        asset: operation.instance.asset,
        transform: operation.instance.transform,
        metadata: operation.instance.metadata,
      };
    case 'createSprite':
      return {
        handle: operation.handle,
        asset: operation.sprite.asset,
        transform: operation.sprite.transform,
        metadata: operation.sprite.metadata,
      };
    default:
      return null;
  }
}

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : 'Unknown Studio adapter failure';
}
