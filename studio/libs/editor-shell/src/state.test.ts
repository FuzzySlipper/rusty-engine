import assert from 'node:assert/strict';
import test from 'node:test';

import {
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  STUDIO_ADAPTER_OPERATIONS,
  StudioAdapterClient,
  type StudioAdapterRequest,
  type StudioAdapterTransport,
} from '@rusty-engine/studio-adapter-client';
import {
  HttpStudioUserSettingsClient,
  buildDefaultStudioHostUserSettings,
  serializeStudioHostUserSettings,
} from '@rusty-engine/studio-user-settings';

import { StudioWorkspaceStore } from './state.js';
import { HttpStudioAdapterTransport } from './transport.js';

test('workspace opens only through the adapter and keeps authority, projection, preview, and selection distinct', async () => {
  const transport = new FixtureTransport();
  const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));

  assert.equal(store.snapshot().connection.kind, 'disconnected');
  assert.equal(store.snapshot().authoringDocument, null);

  await store.openProject('/external/loading-bay', 'content/projects/loading-bay.project.json');

  assert.deepEqual(transport.requests.map((request) => request.type), ['describe', 'openProject']);
  assert.equal(store.snapshot().connection.kind, 'connected');
  assert.equal(store.snapshot().authoringDocument?.identity.projectId, 'loading-bay');
  assert.equal(store.snapshot().liveProjection?.frame.ops.length, 1);
  assert.equal(store.snapshot().preview, null);
  assert.equal(store.snapshot().selection.entityId, null);
  assert.equal(store.snapshot().selection.sceneNodeId, null);
  assert.deepEqual(store.snapshot().liveProjection?.entities.map((entity) => entity.entityId), [1, 2]);
  assert.equal(store.snapshot().liveProjection?.entities[0]?.label, 'player');
  assert.equal(store.snapshot().liveProjection?.entities[1]?.projected, false);

  store.setHierarchyFilter('player');
  assert.deepEqual(store.visibleHierarchyNodes().map((node) => node.nodeId), [10]);
  store.selectHierarchyNode(10);
  store.beginTranslationPreview(1);
  store.setPreviewTranslationAxis(0, 4.5);

  assert.equal(store.snapshot().selection.source, 'hierarchy');
  assert.equal(store.snapshot().selection.sceneNodeId, 10);
  assert.deepEqual(store.snapshot().preview?.translation, [4.5, 2, 3]);
  assert.equal(store.snapshot().authoringDocument?.identity.projectHash, 'hash-before');
  assert.deepEqual(transport.requests.map((request) => request.type), ['describe', 'openProject']);

  await store.commitPreview();

  const mutation = transport.requests[2];
  assert.equal(mutation?.type, 'setSceneObjectTransform');
  if (mutation?.type === 'setSceneObjectTransform') {
    assert.equal(mutation.expectedProjectHash, 'hash-before');
    assert.equal(mutation.expectedSceneRevision, 11);
    assert.deepEqual(mutation.transform, {
      translation: [4.5, 2, 3],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
    });
  }
  assert.equal(store.snapshot().authoringDocument?.identity.projectHash, 'hash-after');
  assert.equal(store.snapshot().preview, null);
  assert.deepEqual(store.selectedEntity()?.transform?.translation, [4.5, 2, 3]);
});

test('renderer world candidates update the local preview and explicit revert restores owner state', async () => {
  const store = new StudioWorkspaceStore(new StudioAdapterClient(new FixtureTransport()));
  await store.openProject('/external/loading-bay', 'content/projects/loading-bay.project.json');
  store.beginTransformPreview(1, 'translate', 'world');

  store.applyPreviewWorldTransform({
    translation: [2.5, 3, 4],
    rotation: [0, 0, 0, 1],
    scale: [2, 3, 4],
  });
  assert.deepEqual(store.snapshot().preview?.translation, [2.5, 3, 4]);
  assert.deepEqual(store.snapshot().preview?.scale, [2, 3, 4]);

  store.revertPreview();
  assert.equal(store.snapshot().preview, null);
  assert.deepEqual(store.selectedEntity()?.transform?.translation, [1, 2, 3]);
});

test('changing selection commits a pending transform before selecting the next owner', async () => {
  const transport = new FixtureTransport();
  const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
  await store.openProject('/external/loading-bay', 'content/projects/loading-bay.project.json');
  store.selectHierarchyNode(10);
  store.beginTranslationPreview(1);
  store.setPreviewTranslationAxis(0, 4.5);

  await store.selectHierarchyNode(20);

  assert.equal(transport.requests.at(-1)?.type, 'setSceneObjectTransform');
  assert.equal(store.snapshot().selection.entityId, 2);
  assert.equal(store.snapshot().selection.sceneNodeId, 20);
  assert.equal(store.snapshot().preview, null);
});

test('rejected mutation preserves the accepted document and disposable preview', async () => {
  const transport = new FixtureTransport();
  const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
  await store.openProject('/external/loading-bay', 'content/projects/loading-bay.project.json');
  store.selectHierarchyNode(10);
  store.beginTranslationPreview(1);
  store.setPreviewTranslationAxis(2, 99);
  transport.rejectMutation = true;

  await store.commitPreview();

  assert.equal(store.snapshot().authoringDocument?.identity.projectHash, 'hash-before');
  assert.deepEqual(store.snapshot().preview?.translation, [1, 2, 99]);
  assert.match(store.snapshot().lastError ?? '', /project\.staleHash/);
});

test('selection does not change when automatic transform settlement is rejected', async () => {
  const transport = new FixtureTransport();
  const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
  await store.openProject('/external/loading-bay', 'content/projects/loading-bay.project.json');
  store.selectHierarchyNode(10);
  store.beginTranslationPreview(1);
  store.setPreviewTranslationAxis(2, 99);
  transport.rejectMutation = true;

  await store.selectHierarchyNode(20);

  assert.equal(store.snapshot().selection.entityId, 1);
  assert.deepEqual(store.snapshot().preview?.translation, [1, 2, 99]);
  assert.match(store.snapshot().lastError ?? '', /project\.staleHash/);
});

test('opening a second project clears project-scoped selection and preview even when entity ids overlap', async () => {
  const transport = new FixtureTransport();
  const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
  await store.openProject('/external/loading-bay-a', 'content/projects/loading-bay.project.json');
  store.selectHierarchyNode(10);
  store.beginTranslationPreview(1);
  store.setPreviewTranslationAxis(0, 99);
  assert.equal(store.snapshot().selection.entityId, 1);
  assert.deepEqual(store.snapshot().preview?.translation, [99, 2, 3]);

  transport.openedProjectId = 'loading-bay-b';
  await store.openProject('/external/loading-bay-b', 'content/projects/loading-bay.project.json');

  assert.equal(store.snapshot().authoringDocument?.identity.projectId, 'loading-bay-b');
  assert.equal(store.snapshot().selection.entityId, null);
  assert.equal(store.snapshot().selection.sceneNodeId, null);
  assert.equal(store.snapshot().preview, null);
  assert.deepEqual(
    transport.requests.map((request) => request.type),
    ['describe', 'openProject', 'openProject'],
  );
});

test('host-user camera and keyboard settings persist outside project authority and reload by project root', async () => {
  const settingsHost = new FixtureSettingsHost();
  const first = new StudioWorkspaceStore(
    new StudioAdapterClient(new FixtureTransport()),
    new HttpStudioUserSettingsClient('/api/studio-user-settings', settingsHost.fetch),
  );
  await first.openProject('/external/loading-bay', 'content/projects/loading-bay.project.json');
  assert.equal(first.snapshot().userSettings.status, 'defaulted');
  assert.equal(first.snapshot().settings.cameraMoveSpeed, 6);
  assert.equal(first.snapshot().settings.lightingMode, 'work_light');

  first.updateSettings({
    lightingMode: 'authored_lights',
    cameraMoveSpeed: 14,
    cameraBoostMultiplier: 5,
    invertLookY: true,
    keyboard: { ...first.snapshot().settings.keyboard, moveForward: 'ArrowUp' },
  });
  await first.closeProject();
  assert.match(settingsHost.text ?? '', /"cameraMoveSpeed": 14/);
  assert.match(settingsHost.text ?? '', /"moveForward": "ArrowUp"/);
  assert.match(settingsHost.text ?? '', /"lightingMode": "authored_lights"/);

  const second = new StudioWorkspaceStore(
    new StudioAdapterClient(new FixtureTransport()),
    new HttpStudioUserSettingsClient('/api/studio-user-settings', settingsHost.fetch),
  );
  await second.openProject('/external/loading-bay', 'content/projects/loading-bay.project.json');
  assert.equal(second.snapshot().userSettings.status, 'loaded');
  assert.equal(second.snapshot().settings.cameraMoveSpeed, 14);
  assert.equal(second.snapshot().settings.lightingMode, 'authored_lights');
  assert.equal(second.snapshot().settings.cameraBoostMultiplier, 5);
  assert.equal(second.snapshot().settings.invertLookY, true);
  assert.equal(second.snapshot().settings.keyboard.moveForward, 'ArrowUp');
});

test('HTTP transport bounds both directions and leaves semantic decoding to the adapter client', async () => {
  const requests: string[] = [];
  const transport = new HttpStudioAdapterTransport('/api/studio-adapter', async (input, init) => {
    requests.push(`${input}:${String(init.method)}`);
    return {
      ok: true,
      status: 200,
      headers: new Headers({ 'content-length': '91' }),
      text: async () => JSON.stringify(described('studio-describe-1')),
    };
  });
  const client = new StudioAdapterClient(transport);

  const response = await client.describe();

  assert.equal(response.adapter.adapterId, 'rusty-engine-demo.loading-bay');
  assert.deepEqual(requests, ['/api/studio-adapter:POST']);

  const oversized = new HttpStudioAdapterTransport('/api/studio-adapter', async () => ({
    ok: true,
    status: 200,
    headers: new Headers({ 'content-length': String(32 * 1024 * 1024 + 1) }),
    text: async () => '',
  }));
  await assert.rejects(
    oversized.exchange({
      type: 'describe',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: 'x',
    }),
    /response exceeds/,
  );
});

class FixtureTransport implements StudioAdapterTransport {
  readonly requests: StudioAdapterRequest[] = [];
  rejectMutation = false;
  openedProjectId = 'loading-bay';

  exchange(request: StudioAdapterRequest): Promise<unknown> {
    this.requests.push(request);
    if (request.type === 'describe') return Promise.resolve(described(request.requestId));
    if (request.type === 'openProject') {
      return Promise.resolve(
        projectResponse('projectOpened', request.requestId, false, this.openedProjectId),
      );
    }
    if (request.type === 'setSceneObjectTransform') {
      if (this.rejectMutation) {
        return Promise.resolve({
          type: 'rejected',
          protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
          requestId: request.requestId,
          error: { code: 'project.staleHash', message: 'project changed outside Studio' },
        });
      }
      return Promise.resolve({
        type: 'projectMutationApplied',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: request.requestId,
        receipt: { kind: 'sceneObjectTransformSet', entityId: request.entityId },
        project: projectReadout(true),
      });
    }
    if (request.type === 'readProject') {
      return Promise.resolve(projectResponse('projectRead', request.requestId, false));
    }
    return Promise.resolve({
      type: 'projectClosed',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: request.requestId,
    });
  }
}

class FixtureSettingsHost {
  readonly projectKey = 'rusty-studio-project:fixture';
  text: string | null = null;
  sha256: string | null = null;

  readonly fetch = async (_input: string, init?: RequestInit): Promise<{
    readonly ok: boolean;
    readonly status: number;
    readonly json: () => Promise<unknown>;
  }> => {
    if (init?.method === 'PUT') {
      const body = JSON.parse(String(init.body)) as {
        readonly text: string;
        readonly expectedHash: string | null;
      };
      if (body.expectedHash !== this.sha256) {
        return response(false, 409, { ok: false, message: 'stale settings' });
      }
      this.text = body.text;
      this.sha256 = `settings-${String(body.text.length)}`;
      return response(true, 200, {
        ok: true,
        path: '/config/rusty-studio/fixture.json',
        sha256: this.sha256,
      });
    }
    if (this.text === null) {
      const defaults = buildDefaultStudioHostUserSettings(this.projectKey);
      assert.equal(serializeStudioHostUserSettings(defaults).includes(this.projectKey), true);
    }
    return response(true, 200, {
      ok: true,
      canonicalProjectRoot: '/external/loading-bay',
      projectKey: this.projectKey,
      path: '/config/rusty-studio/fixture.json',
      text: this.text,
      sha256: this.sha256,
    });
  };
}

function response(ok: boolean, status: number, body: unknown): {
  readonly ok: boolean;
  readonly status: number;
  readonly json: () => Promise<unknown>;
} {
  return { ok, status, json: () => Promise.resolve(body) };
}

function described(requestId: string): unknown {
  return {
    type: 'described',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    adapter: {
      adapterId: 'rusty-engine-demo.loading-bay',
      adapterVersion: 6,
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      projectKind: 'loadingBayProject',
      projectSchemaVersion: 11,
      operations: STUDIO_ADAPTER_OPERATIONS,
    },
  };
}

function projectResponse(
  type: 'projectOpened' | 'projectRead',
  requestId: string,
  changed: boolean,
  projectId = 'loading-bay',
): unknown {
  return {
    type,
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    project: projectReadout(changed, projectId),
  };
}

function projectReadout(changed: boolean, projectId = 'loading-bay'): unknown {
  const translation = changed ? [4.5, 2, 3] : [1, 2, 3];
  return {
    identity: {
      projectId,
      name: 'Loading Bay',
      entryScene: 'scene/loading-bay',
      sourceSchemaVersion: 11,
      currentSchemaVersion: 11,
      projectHash: changed ? 'hash-after' : 'hash-before',
      sceneRevision: changed ? 12 : 11,
      relativeProjectFile: 'content/projects/loading-bay.project.json',
    },
    canonical: {
      projectJson: '{}',
      assetCatalogJson: '{}',
      authoredSceneJson: '{}',
      entityStateJson: '{}',
      contentManifestJson: '{}',
    },
    inspections: {
      catalog: {
        entryCount: 6,
        dependencyCount: 0,
        kinds: [{ name: 'mesh', count: 6 }],
        diagnostics: { diagnostics: [] },
      },
      scene: {
        sceneId: 1,
        revision: changed ? 12 : 11,
        schemaVersion: 4,
        name: 'Loading Bay',
        nodeCount: 2,
        rootCount: 2,
        dependencyCount: 1,
        nodeKinds: [{ name: 'staticMesh', count: 1 }],
        diagnostics: { diagnostics: [] },
      },
      entityState: {
        schemaVersion: 3,
        revision: 0,
        entityCount: 2,
        lifecycle: [{ name: 'active', count: 2 }],
        sources: [{ name: 'runtimeCreated', count: 2 }],
        capabilities: [{ name: 'transform', count: 1 }],
        relationships: [],
        entityIds: [1, 2],
        diagnostics: { diagnostics: [] },
      },
      persistence: {
        schemaVersion: 1,
        artifactCount: 1,
        requiredArtifactCount: 1,
        declaredByteCount: 10,
        classes: [{ name: 'durable', count: 1 }],
        roles: [{ name: 'resource:loading-bay-project', count: 1 }],
        loadSteps: [{ stage: 'resources', path: 'content/projects/loading-bay.project.json' }],
        diagnostics: { diagnostics: [] },
      },
    },
    sceneHierarchy: {
      sceneId: 1,
      revision: changed ? 12 : 11,
      name: 'Loading Bay',
      rootNodeIds: [10, 20],
      nodes: [
        hierarchyNode(10, 0, 'staticMesh', 'player', 1, 'mesh/player', translation),
        hierarchyNode(20, 1, 'emptyGroup', 'encounter', 2, null, [0, 0, 0]),
      ],
    },
    assetBrowser: {
      assets: [{
        assetId: 'mesh/player',
        kind: 'mesh',
        version: 1,
        hash: null,
        sourcePath: null,
        label: 'Player',
        dependencies: [],
        dependents: [],
        material: false,
        importedMesh: false,
        import: null,
      }],
      lockEntries: [{
        assetId: 'mesh/player',
        kind: 'mesh',
        version: 1,
        hash: null,
        dependencies: [],
      }],
    },
    voxelAuthoring: {
      assets: [],
      instances: [],
      materials: [],
    },
    animatedMeshResources: [],
    loadingBay: {
      sceneName: 'Loading Bay',
      entityCount: 2,
      doorCount: 0,
      switchCount: 0,
      enemyCount: 0,
      encounterCount: 0,
      extractionBeaconCount: 0,
      navigatorCount: 0,
      playerControllerCount: 1,
      weaponCount: 1,
      voxelEnvironment: 'generatedRoom',
    },
    projection: {
      schemaVersion: 1,
      ops: [{
        op: 'create',
        handle: 101,
        parent: null,
        node: {
          geometry: { kind: 'cube' },
          material: { color: [0.2, 0.4, 0.6, 1], wireframe: false },
          transform: transform(translation),
          visible: true,
          layer: 'scene',
          metadata: { sourceEntity: 1, sourceSceneNode: 10, tags: [], label: 'player' },
        },
      }],
    },
    projectionReadout: {
      frameKind: 'complete',
      sourceRevision: changed ? 12 : 11,
      retainedEntities: 1,
      retainedLights: 0,
      retainedVoxelInstances: 0,
      retainedVoxelChunks: 0,
      diagnostics: [],
    },
  };
}

function hierarchyNode(
  nodeId: number,
  displayOrder: number,
  nodeKind: 'staticMesh' | 'emptyGroup',
  label: string,
  entityId: number,
  asset: string | null,
  translation: readonly number[],
): unknown {
  return {
    nodeId,
    parentNodeId: null,
    childOrder: displayOrder,
    displayOrder,
    depth: 0,
    nodeKind,
    label,
    tags: [],
    asset,
    entityId,
    localTransform: transform(translation),
    worldTransform: transform(translation),
  };
}

function transform(translation: readonly number[]): unknown {
  return {
    translation,
    rotation: [0, 0, 0, 1],
    scale: [1, 1, 1],
  };
}
