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

test('voxel-object source, shared candidate frames, stale apply, explicit discard, apply, and reopen stay distinct', async () => {
  const client = new VoxelObjectFixtureClient();
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/loading-bay', 'content/projects/loading-bay.project.json');

  await store.runVoxelAction({
    kind: 'inspectObjectSource',
    sourceKind: 'animated',
    sourceAssetId: 'mesh-animation/character',
    source: { scope: 'host', path: '/trusted/character.glb' },
  });
  assert.equal(store.snapshot().voxelWorkspace.objectSourceInspection?.clips[0]?.name, 'Walk');

  await store.runVoxelAction(objectPrepareAction());
  assert.equal(store.snapshot().voxelWorkspace.objectConversion?.preview.storedFrameCount, 3);
  assert.equal(store.snapshot().liveProjection?.frame.ops[0]?.op, 'create');
  assert.equal(firstProjectionLabel(store), 'voxel-object-candidate-0');

  const conversion = store.snapshot().voxelWorkspace.objectConversion;
  assert.ok(conversion !== null);
  await store.runVoxelAction({
    kind: 'previewObjectFrame',
    planId: conversion.plan.planId,
    expectedPlanHash: conversion.plan.planHash,
    frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
    maxPreviewSamples: 64,
  });
  assert.equal(store.snapshot().voxelWorkspace.objectConversion?.preview.selectedFrame.selection.kind, 'clip');
  assert.equal(firstProjectionLabel(store), 'voxel-object-candidate-1');

  client.rejectObjectApply = true;
  await store.runVoxelAction({
    kind: 'applyObjectConversion',
    planId: conversion.plan.planId,
    expectedPlanHash: conversion.plan.planHash,
    expectedOutputHash: conversion.preview.outputHash,
  });
  assert.match(store.snapshot().lastError ?? '', /project\.staleHash/);
  assert.equal(store.snapshot().authoringDocument?.voxelObjectAuthoring.assets.length, 0);
  assert.ok(store.snapshot().voxelWorkspace.objectConversion !== null);

  client.rejectObjectApply = false;
  await store.runVoxelAction({
    kind: 'discardObjectConversion',
    planId: conversion.plan.planId,
  });
  assert.equal(store.snapshot().voxelWorkspace.objectConversion, null);
  assert.equal(firstProjectionLabel(store), 'player');

  await store.runVoxelAction(objectPrepareAction());
  const preparedAgain = store.snapshot().voxelWorkspace.objectConversion;
  assert.ok(preparedAgain !== null);
  await store.runVoxelAction({
    kind: 'applyObjectConversion',
    planId: preparedAgain.plan.planId,
    expectedPlanHash: preparedAgain.plan.planHash,
    expectedOutputHash: preparedAgain.preview.outputHash,
  });
  assert.equal(store.snapshot().voxelWorkspace.objectConversion, null);
  assert.equal(store.snapshot().authoringDocument?.voxelObjectAuthoring.assets[0]?.clips[0]?.frames.length, 2);

  await store.runVoxelAction({
    kind: 'attachObjectInstance',
    sceneId: 'scene/loading-bay',
    instance: {
      instanceId: 'character-one',
      voxelObjectAssetId: 'voxel-object/character',
      frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
      translation: [4, 0, 2],
      rotation: [0, 0, 0, 1],
      scale: [2, 2, 2],
      materialOverrides: [],
    },
  });
  assert.deepEqual(
    store.snapshot().authoringDocument?.voxelObjectAuthoring.instances[0]?.instance.translation,
    [4, 0, 2],
  );

  const reopened = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await reopened.openProject('/external/loading-bay', 'content/projects/loading-bay.project.json');
  assert.deepEqual(
    reopened.snapshot().authoringDocument?.voxelObjectAuthoring,
    store.snapshot().authoringDocument?.voxelObjectAuthoring,
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

class VoxelObjectFixtureClient {
  rejectObjectApply = false;
  applied = false;
  attached = false;

  describe() {
    return Promise.resolve(described('describe-object') as never);
  }

  openProject() {
    return Promise.resolve({
      type: 'projectOpened',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: 'open-object',
      project: this.#project(),
    } as never);
  }

  readProject() {
    return Promise.resolve({
      type: 'projectRead',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: 'read-object',
      project: this.#project(),
    } as never);
  }

  inspectVoxelObjectSource() {
    return Promise.resolve({ inspection: objectSourceInspection() } as never);
  }

  prepareVoxelObjectConversion() {
    return Promise.resolve({
      plan: objectPlan(),
      preview: objectPreview(0),
      projection: objectCandidateProjection(0),
      projectionReadout: projectionReadout(20),
    } as never);
  }

  previewVoxelObjectConversion(input: { readonly frame: { readonly kind: string; readonly frameIndex?: number } }) {
    const frame = input.frame.kind === 'clip' ? input.frame.frameIndex ?? 0 : 0;
    return Promise.resolve({
      preview: objectPreview(frame),
      projection: objectCandidateProjection(frame),
      projectionReadout: projectionReadout(20 + frame),
    } as never);
  }

  applyVoxelObjectConversion() {
    if (this.rejectObjectApply) return Promise.reject(new Error('project.staleHash: source changed'));
    this.applied = true;
    return Promise.resolve({
      receipt: {
        kind: 'voxelObjectConversionApplied',
        planId: 'plan/object',
        planHash: 'sha256:plan',
        assetId: 'voxel-object/character',
        outputHash: 'sha256:object',
        storedFrames: 3,
        aggregateVoxels: 3,
      },
      project: this.#project(),
    } as never);
  }

  discardVoxelObjectConversion() {
    const project = this.#project();
    return Promise.resolve({
      planId: 'plan/object',
      projection: project.projection,
      projectionReadout: project.projectionReadout,
    } as never);
  }

  attachVoxelObjectInstance() {
    this.attached = true;
    return Promise.resolve({
      receipt: {
        kind: 'voxelObjectInstanceAttached',
        sceneId: 'scene/loading-bay',
        instanceId: 'character-one',
        assetId: 'voxel-object/character',
        frameKind: 'clip',
      },
      project: this.#project(),
    } as never);
  }

  #project() {
    const project = projectReadout(false);
    return {
      ...project,
      voxelObjectAuthoring: {
        assets: this.applied ? [objectAssetReadout()] : [],
        instances: this.attached ? [{
          sceneId: 'scene/loading-bay',
          instance: {
            instanceId: 'character-one',
            voxelObjectAssetId: 'voxel-object/character',
            frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
            translation: [4, 0, 2],
            rotation: [0, 0, 0, 1],
            scale: [2, 2, 2],
            materialOverrides: [],
          },
        }] : [],
      },
    };
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

function projectReadout(changed: boolean, projectId = 'loading-bay') {
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
    voxelObjectAuthoring: {
      assets: [],
      instances: [],
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

function objectPrepareAction() {
  return {
    kind: 'prepareObjectConversion' as const,
    sourceKind: 'animated' as const,
    sourceAssetId: 'mesh-animation/character',
    source: { scope: 'host' as const, path: '/trusted/character.glb' },
    targetAssetId: 'voxel-object/character',
    settings: {
      mesh: objectMeshSettings(),
      pivot: [0, 0, 0] as const,
      anchorPolicy: { kind: 'preserveSourceSpace' as const },
    },
    clips: [{
      sourceClipName: 'Walk',
      outputClipId: 'clip/walk-1',
      outputName: 'Walk',
      sampleRateHz: 12,
      startMicroseconds: 0,
      endPolicy: 'excludeLoopSeam' as const,
    }],
    defaultClip: 'clip/walk-1',
    frame: { kind: 'clip' as const, clipId: 'clip/walk-1', frameIndex: 0 },
    maxPreviewSamples: 64,
  };
}

function objectSourceInspection() {
  return {
    sourceKind: 'animated' as const,
    source: {
      assetId: 'mesh-animation/character',
      assetVersion: 1,
      sourceSha256: 'sha256:source',
    },
    sourcePath: '/trusted/character.glb',
    sourceByteCount: 1024,
    metadata: {
      sourceSceneIndex: 0,
      sourceSceneName: 'Character',
      sourceBounds: { min: [-1, 0, -1] as const, max: [1, 2, 1] as const },
      vertexCount: 4,
      triangleCount: 2,
      groups: [],
      materialSlots: [{ sourceMaterialSlot: 0, sourceMaterialName: 'Body' }],
      nodes: [],
      textureCoordinates: [],
    },
    clips: [{
      sourceAnimationIndex: 0,
      name: 'Walk',
      durationMicroseconds: 1_000_000,
      channelCount: 2,
      targetNodeIndices: [0],
      properties: ['translation' as const],
    }],
    diagnostics: [],
  };
}

function objectPlan() {
  return {
    planId: 'plan/object',
    source: objectSourceInspection().source,
    sourcePath: '/trusted/character.glb',
    targetAssetId: 'voxel-object/character',
    settings: {
      mesh: objectMeshSettings(),
      pivot: [0, 0, 0] as const,
      anchorPolicy: { kind: 'preserveSourceSpace' as const },
    },
    clips: objectPrepareAction().clips,
    defaultClip: 'clip/walk-1',
    planner: 'rusty-engine.voxel-object-conversion.v1',
    expectedSourceSha256: 'sha256:source',
    settingsSha256: 'sha256:settings',
    expectedOutputContentHash: 'sha256:object',
    planHash: 'sha256:plan',
    estimatedSampledFrames: 2,
    estimatedStoredFrames: 3,
    estimatedAggregateVoxels: 3,
    estimatedArtifactBytes: 2048,
    estimatedBounds: objectBounds(),
    clipSummaries: [{
      outputClipId: 'clip/walk-1',
      sourceClipName: 'Walk',
      sourceAnimationIndex: 0,
      startMicroseconds: 0,
      endMicroseconds: 1_000_000,
      sampleRateHz: 12,
      sampledFrameCount: 2,
      storedFrameCount: 2,
      durationMicroseconds: 1_000_000,
    }],
  };
}

function objectPreview(frameIndex: number) {
  const frame = {
    storedFrameIndex: frameIndex,
    sourceTimestampsMicroseconds: [frameIndex * 500_000],
    durationMicroseconds: 500_000,
    bounds: objectBounds(),
    voxelCount: 1,
    sparseRunCount: 1,
    voxelDataHash: `sha256:frame-${String(frameIndex)}`,
  };
  return {
    planId: 'plan/object',
    planHash: 'sha256:plan',
    outputHash: 'sha256:object',
    sampledFrameCount: 2,
    storedFrameCount: 3,
    aggregateVoxelCount: 3,
    artifactBytes: 2048,
    unionBounds: objectBounds(),
    clips: [{
      outputClipId: 'clip/walk-1',
      sourceClipName: 'Walk',
      sourceAnimationIndex: 0,
      startMicroseconds: 0,
      endMicroseconds: 1_000_000,
      sampleRateHz: 12,
      endPolicy: 'excludeLoopSeam' as const,
      sampledFrameCount: 2,
      storedFrameCount: 2,
      durationMicroseconds: 1_000_000,
      frames: [{ ...frame, storedFrameIndex: 0 }, { ...frame, storedFrameIndex: 1 }],
    }],
    selectedFrame: {
      selection: { kind: 'clip' as const, clipId: 'clip/walk-1', frameIndex },
      bounds: objectBounds(),
      voxelCount: 1,
      sparseRunCount: 1,
      voxelDataHash: frame.voxelDataHash,
      durationMicroseconds: 500_000,
      sourceTimestampsMicroseconds: frame.sourceTimestampsMicroseconds,
      sampleVoxels: [{ coordinate: [0, 0, 0] as const, materialSlot: 7 }],
      samplesTruncated: false,
    },
  };
}

function objectCandidateProjection(frameIndex: number) {
  return {
    schemaVersion: 1 as const,
    ops: [{
      op: 'create' as const,
      handle: 901,
      parent: null,
      node: {
        geometry: { kind: 'cube' as const },
        material: { color: [0.8, 0.5, 0.2, 1] as const, wireframe: false },
        transform: transform([0, 0, 0]),
        visible: true,
        layer: 'scene' as const,
        metadata: {
          sourceEntity: null,
          sourceSceneNode: null,
          tags: ['voxel-object-candidate'],
          label: `voxel-object-candidate-${String(frameIndex)}`,
        },
      },
    }],
  };
}

function projectionReadout(sourceRevision: number) {
  return {
    frameKind: 'complete' as const,
    sourceRevision,
    retainedEntities: 0,
    retainedLights: 0,
    retainedVoxelInstances: 0,
    retainedVoxelChunks: 0,
    diagnostics: [],
  };
}

function objectAssetReadout() {
  const defaultFrame = objectFrameReadout(null, 'sha256:default');
  return {
    assetId: 'voxel-object/character',
    contentHash: 'sha256:object',
    grid: {
      coordinateSystem: 'rightHandedYUp' as const,
      cellSize: 1,
      chunkSize: 16,
      pivot: [0, 0, 0] as const,
    },
    bounds: objectBounds(),
    defaultFrame,
    clips: [{
      clipId: 'clip/walk-1',
      name: 'Walk',
      framesPerSecond: 12,
      frames: [
        objectFrameReadout(500_000, 'sha256:frame-0'),
        objectFrameReadout(500_000, 'sha256:frame-1'),
      ],
    }],
    defaultClip: 'clip/walk-1',
    materialPalette: [{ materialSlot: 7, materialAssetId: 'material/wall-lines' }],
    materialMap: [{ sourceMaterialSlot: 0, sourceMaterialName: 'Body', voxelMaterialSlot: 7 }],
    provenance: {
      kind: 'convertedAnimatedMesh' as const,
      sourcePath: '/trusted/character.glb',
      sourceSha256: 'sha256:source',
      sourceByteCount: 1024,
      converter: 'rusty-engine.mesh-to-voxel-object.v1',
      settingsSha256: 'sha256:settings',
      licensePath: null,
      sourceClips: [{
        outputClipId: 'clip/walk-1',
        sourceClipName: 'Walk',
        sourceAnimationIndex: 0,
        startMicroseconds: 0,
        endMicroseconds: 1_000_000,
        sampleRateHz: 12,
        includedClipEnd: false,
      }],
    },
  };
}

function objectFrameReadout(durationMicroseconds: number | null, voxelDataHash: string) {
  return {
    bounds: objectBounds(),
    voxelDataHash,
    voxelCount: 1,
    sparseRunCount: 1,
    durationMicroseconds,
  };
}

function objectBounds() {
  return { min: [0, 0, 0] as const, max: [0, 0, 0] as const };
}

function objectMeshSettings() {
  return {
    conversion: {
      resolution: [1, 1, 1] as const,
      cellSize: 1,
      chunkSize: 16,
      origin: [0, 0, 0] as const,
      fitPolicy: 'contain' as const,
      originPolicy: 'targetMin' as const,
      mode: 'surface' as const,
      materialPalette: [{ materialSlot: 7, materialAssetId: 'material/wall-lines' }],
      materialMap: [{ sourceMaterialSlot: 0, voxelMaterialSlot: 7 }],
      maxOutputVoxels: 1,
    },
    transform: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
    materialPolicy: { textureAssets: [], textureBindings: [], defaultVoxelMaterial: 7 },
  };
}

function firstProjectionLabel(store: StudioWorkspaceStore): string | null {
  const operation = store.snapshot().liveProjection?.frame.ops[0];
  return operation?.op === 'create' ? operation.node.metadata.label : null;
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
