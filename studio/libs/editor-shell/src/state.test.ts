import assert from 'node:assert/strict';
import test from 'node:test';

import {
  MAX_VOXEL_OBJECT_INSTANCE_BATCH,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  STUDIO_ADAPTER_OPERATIONS,
  StudioAdapterClient,
  type StudioAdapterRequest,
  type StudioAdapterTransport,
} from '@rusty-engine/studio-adapter-client';
import { decodeRenderFrameDiff, type RenderFrameDiff, type Transform } from '@rusty-engine/render-contracts';
import {
  HttpStudioUserSettingsClient,
  buildDefaultStudioHostUserSettings,
  serializeStudioHostUserSettings,
} from '@rusty-engine/studio-user-settings';

import { StudioEntityInspectorMutationError } from './entity-inspector.js';
import { StudioWorkspaceStore, type StudioPlaybackTimer } from './state.js';
import { HttpStudioAdapterTransport } from './transport.js';

const FIXTURE_COMPONENT_TYPE_ID = 'fixture.weapon';
const FIXTURE_CONTRACT_ID = 'fixture.weapon-authoring';
const FIXTURE_CONTRACT_VERSION = 1;

test('workspace opens only through the adapter and keeps authority, projection, preview, and selection distinct', async () => {
  const transport = new FixtureTransport();
  const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));

  assert.equal(store.snapshot().connection.kind, 'disconnected');
  assert.equal(store.snapshot().authoringDocument, null);

  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');

  assert.deepEqual(transport.requests.map((request) => request.type), ['describe', 'openProject']);
  assert.equal(store.snapshot().connection.kind, 'connected');
  assert.equal(store.snapshot().authoringDocument?.identity.projectId, 'sample-project');
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

test('visual-local mutation stays distinct from entity world authority', async () => {
  const transport = new FixtureTransport();
  const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');

  await store.setSceneObjectRenderableTransform(1, transform([0, -1.25, 0]) as Transform);

  const request = transport.requests.at(-1);
  assert.equal(request?.type, 'setSceneObjectRenderableTransform');
  if (request?.type === 'setSceneObjectRenderableTransform') {
    assert.equal(request.expectedProjectHash, 'hash-before');
    assert.equal(request.expectedSceneRevision, 11);
    assert.deepEqual(request.transform.translation, [0, -1.25, 0]);
  }
  const node = store.selectedHierarchyNode()
    ?? store.snapshot().authoringDocument?.sceneHierarchy.nodes[0];
  assert.deepEqual(node?.worldTransform.translation, [1, 2, 3]);
  assert.deepEqual(node?.renderableTransform.translation, [0, -1.25, 0]);
  assert.deepEqual(store.selectedEntity()?.transform?.translation ??
    store.snapshot().liveProjection?.entities[0]?.transform?.translation, [1, 0.75, 3]);
});

test('renderer world candidates update the local preview and explicit revert restores owner state', async () => {
  const store = new StudioWorkspaceStore(new StudioAdapterClient(new FixtureTransport()));
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
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
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
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
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
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
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  store.selectHierarchyNode(10);
  store.beginTranslationPreview(1);
  store.setPreviewTranslationAxis(2, 99);
  transport.rejectMutation = true;

  await store.selectHierarchyNode(20);

  assert.equal(store.snapshot().selection.entityId, 1);
  assert.deepEqual(store.snapshot().preview?.translation, [1, 2, 99]);
  assert.match(store.snapshot().lastError ?? '', /project\.staleHash/);
});

test('entity inspector mutation lease serializes edits and accepts only a matching canonical reread', async () => {
  const transport = new FixtureTransport();
  transport.readProjectChanged = true;
  const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  const context = await selectFixtureInspector(store);
  const lease = store.entityInspectorMutationPort.acquire(context);

  assert.equal(store.snapshot().operation, 'committing');
  assert.throws(
    () => store.entityInspectorMutationPort.acquire(context),
    (error: unknown) =>
      error instanceof StudioEntityInspectorMutationError
      && error.code === 'inspectorMutation.busy',
  );
  await store.setEntityCollision(1, { enabled: true, staticCollider: true });
  assert.equal(
    transport.requests.some((request) => request.type === 'setEntityCollision'),
    false,
    'ordinary core mutation remains serialized behind the downstream lease',
  );

  const settlement = await lease.settle({
    beforeProjectHash: 'hash-before',
    afterProjectHash: 'hash-after',
  });
  assert.deepEqual(settlement, { kind: 'accepted', projectHash: 'hash-after' });
  assert.equal(store.snapshot().authoringDocument?.identity.projectHash, 'hash-after');
  assert.equal(store.snapshot().operation, 'idle');
  assert.equal(transport.requests.at(-1)?.type, 'readProject');
  await assert.rejects(
    lease.settle({
      beforeProjectHash: 'hash-before',
      afterProjectHash: 'hash-after',
    }),
    (error: unknown) =>
      error instanceof StudioEntityInspectorMutationError
      && error.code === 'inspectorMutation.closed',
  );
});

test('entity inspector rejection and hash mismatch preserve the accepted project', async (t) => {
  await t.test('typed owner rejection', async () => {
    const transport = new FixtureTransport();
    const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
    await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
    const lease = store.entityInspectorMutationPort.acquire(
      await selectFixtureInspector(store),
    );

    assert.deepEqual(lease.reject(new Error('weapon policy rejected candidate')), {
      kind: 'rejected',
      message: 'weapon policy rejected candidate',
    });
    assert.equal(store.snapshot().operation, 'idle');
    assert.equal(store.snapshot().authoringDocument?.identity.projectHash, 'hash-before');
    assert.match(store.snapshot().lastError ?? '', /weapon policy rejected candidate/u);
    assert.equal(transport.requests.at(-1)?.type, 'openProject');
  });

  await t.test('canonical reread hash mismatch', async () => {
    const transport = new FixtureTransport();
    const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
    await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
    const lease = store.entityInspectorMutationPort.acquire(
      await selectFixtureInspector(store),
    );

    await assert.rejects(
      lease.settle({
        beforeProjectHash: 'hash-before',
        afterProjectHash: 'hash-after',
      }),
      (error: unknown) =>
        error instanceof StudioEntityInspectorMutationError
        && error.code === 'inspectorMutation.hashMismatch',
    );
    assert.equal(store.snapshot().operation, 'idle');
    assert.equal(store.snapshot().authoringDocument?.identity.projectHash, 'hash-before');
    assert.match(store.snapshot().lastError ?? '', /Canonical project reread/u);
  });
});

test('entity inspector settlement blocks selection remounts and discards replaced projects', async (t) => {
  await t.test('selection remains pinned until a delayed mutation is canonically reread', async () => {
    const transport = new FixtureTransport();
    const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
    await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
    const lease = store.entityInspectorMutationPort.acquire(
      await selectFixtureInspector(store),
    );

    await store.selectEntity(2, 'hierarchy');
    assert.equal(
      store.snapshot().selection.entityId,
      1,
      'selection cannot destroy the panel after its downstream mutation has begun',
    );

    transport.blockNextProjectRead();
    const pending = lease.settle({
      beforeProjectHash: 'hash-before',
      afterProjectHash: 'hash-after',
    });

    await store.selectEntity(2, 'hierarchy');
    assert.equal(
      store.snapshot().selection.entityId,
      1,
      'selection remains pinned while the canonical reread is outstanding',
    );
    transport.resolveBlockedProjectRead(true);

    assert.deepEqual(await pending, { kind: 'accepted', projectHash: 'hash-after' });
    assert.equal(store.snapshot().selection.entityId, 1);
    assert.equal(store.snapshot().authoringDocument?.identity.projectHash, 'hash-after');
    assert.equal(store.snapshot().operation, 'idle');

    await store.selectEntity(2, 'hierarchy');
    assert.equal(store.snapshot().selection.entityId, 2);
  });

  await t.test('project and contract generations', async () => {
    const transport = new FixtureTransport();
    const store = new StudioWorkspaceStore(new StudioAdapterClient(transport));
    await store.openProject('/external/sample-project-a', 'content/projects/sample-project.project.json');
    const lease = store.entityInspectorMutationPort.acquire(
      await selectFixtureInspector(store),
    );
    transport.blockNextProjectRead();
    const pending = lease.settle({
      beforeProjectHash: 'hash-before',
      afterProjectHash: 'hash-after',
    });

    transport.openedProjectId = 'sample-project-b';
    await store.openProject('/external/sample-project-b', 'content/projects/sample-project.project.json');
    transport.resolveBlockedProjectRead(true);

    assert.deepEqual(await pending, { kind: 'stale' });
    assert.equal(store.snapshot().authoringDocument?.identity.projectId, 'sample-project-b');
    assert.equal(store.snapshot().authoringDocument?.identity.projectHash, 'hash-before');
    assert.equal(store.snapshot().selection.entityId, null);
    assert.equal(store.snapshot().operation, 'idle');
  });
});

test('opening a second project clears project-scoped selection, preview, and private object work', async () => {
  const client = new VoxelObjectFixtureClient();
  client.openedProjectId = 'sample-project-a';
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/sample-project-a', 'content/projects/sample-project.project.json');
  store.selectHierarchyNode(10);
  store.beginTranslationPreview(1);
  store.setPreviewTranslationAxis(0, 99);
  await store.runVoxelAction({
    kind: 'inspectObjectSource',
    sourceKind: 'animated',
    sourceAssetId: 'mesh-animation/character',
    source: { scope: 'host', path: '/trusted/character.glb' },
  });
  await store.runVoxelAction(objectPrepareAction());
  const staleConversion = store.snapshot().voxelWorkspace.objectConversion;
  assert.ok(staleConversion !== null);
  assert.equal(store.snapshot().selection.entityId, 1);
  assert.deepEqual(store.snapshot().preview?.translation, [99, 2, 3]);
  assert.equal(firstProjectionLabel(store), 'voxel-object-candidate-0');

  client.openedProjectId = 'sample-project-b';
  await store.openProject('/external/sample-project-b', 'content/projects/sample-project.project.json');

  assert.equal(store.snapshot().authoringDocument?.identity.projectId, 'sample-project-b');
  assert.equal(store.snapshot().selection.entityId, null);
  assert.equal(store.snapshot().selection.sceneNodeId, null);
  assert.equal(store.snapshot().preview, null);
  assert.equal(store.snapshot().voxelWorkspace.objectSourceInspection, null);
  assert.equal(store.snapshot().voxelWorkspace.objectConversion, null);
  assert.equal(firstProjectionLabel(store), 'player');

  await store.runVoxelAction({
    kind: 'previewObjectFrame',
    planId: staleConversion.plan.planId,
    expectedPlanHash: staleConversion.plan.planHash,
    frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
    maxPreviewSamples: 64,
  });
  await store.runVoxelAction({
    kind: 'applyObjectConversion',
    planId: staleConversion.plan.planId,
    expectedPlanHash: staleConversion.plan.planHash,
    expectedOutputHash: staleConversion.preview.outputHash,
  });
  assert.equal(client.previewRequestCount, 0);
  assert.equal(client.applyRequestCount, 0);
  assert.equal(firstProjectionLabel(store), 'player');
});

test('project replacement ignores late object-preview success and failure without changing replacement state', async (t) => {
  const replacements = ['open', 'create', 'saveAs'] as const;
  const settlements = ['success', 'failure'] as const;

  for (const replacement of replacements) {
    for (const settlement of settlements) {
      await t.test(`${replacement} followed by late ${settlement}`, async () => {
        const client = new VoxelObjectFixtureClient();
        client.openedProjectId = 'sample-project-a';
        const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
        await store.openProject('/external/sample-project-a', 'content/projects/sample-project.project.json');
        await store.runVoxelAction(objectPrepareAction());
        const conversion = store.snapshot().voxelWorkspace.objectConversion;
        assert.ok(conversion !== null);

        client.blockNextPreview();
        const pendingPreview = store.runVoxelAction({
          kind: 'previewObjectFrame',
          planId: conversion.plan.planId,
          expectedPlanHash: conversion.plan.planHash,
          frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
          maxPreviewSamples: 64,
        });
        assert.equal(client.previewRequestCount, 1);
        assert.equal(store.snapshot().operation, 'voxel');

        client.openedProjectId = 'sample-project-b';
        switch (replacement) {
          case 'open':
            await store.openProject('/external/sample-project-b', 'content/projects/sample-project.project.json');
            break;
          case 'create':
            await store.createProject({
              root: '/external/sample-project-b',
              projectFile: 'content/projects/sample-project.project.json',
              projectId: 'sample-project-b',
              name: 'Sample Project B',
              entryScene: 'scene/sample-scene',
              entrySceneName: 'Sample Project',
            });
            break;
          case 'saveAs':
            await store.saveProjectAs({
              root: '/external/sample-project-b',
              projectFile: 'content/projects/sample-project.project.json',
              projectId: 'sample-project-b',
              name: 'Sample Project B',
            });
            break;
        }

        assert.equal(store.snapshot().authoringDocument?.identity.projectId, 'sample-project-b');
        assert.equal(store.snapshot().operation, 'idle');
        assert.equal(store.snapshot().voxelWorkspace.objectConversion, null);
        assert.equal(firstProjectionLabel(store), 'player');
        const acceptedReplacement = store.snapshot();

        if (settlement === 'success') client.resolveBlockedPreview();
        else client.rejectBlockedPreview();
        await pendingPreview;

        assert.strictEqual(store.snapshot(), acceptedReplacement);
        assert.equal(store.snapshot().lastError, null);
        assert.equal(firstProjectionLabel(store), 'player');
      });
    }
  }
});

test('refresh and a newer object request retain ownership over late object-preview settlement', async (t) => {
  for (const settlement of ['success', 'failure'] as const) {
    await t.test(`late preview ${settlement}`, async () => {
      const client = new VoxelObjectFixtureClient();
      const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
      await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
      await store.runVoxelAction(objectPrepareAction());
      const conversion = store.snapshot().voxelWorkspace.objectConversion;
      assert.ok(conversion !== null);

      client.blockNextPreview();
      const stalePreview = store.runVoxelAction({
        kind: 'previewObjectFrame',
        planId: conversion.plan.planId,
        expectedPlanHash: conversion.plan.planHash,
        frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
        maxPreviewSamples: 64,
      });
      assert.equal(store.snapshot().operation, 'voxel');

      await store.refreshProject();
      assert.equal(store.snapshot().operation, 'idle');
      assert.strictEqual(store.snapshot().voxelWorkspace.objectConversion, conversion);
      assert.equal(firstProjectionLabel(store), 'player');

      client.blockNextInspection();
      const newerInspection = store.runVoxelAction({
        kind: 'inspectObjectSource',
        sourceKind: 'animated',
        sourceAssetId: 'mesh-animation/character',
        source: { scope: 'host', path: '/trusted/character.glb' },
      });
      assert.equal(store.snapshot().operation, 'voxel');
      const newerOperation = store.snapshot();

      if (settlement === 'success') client.resolveBlockedPreview();
      else client.rejectBlockedPreview();
      await stalePreview;

      assert.strictEqual(store.snapshot(), newerOperation);
      assert.equal(store.snapshot().operation, 'voxel');
      assert.strictEqual(store.snapshot().voxelWorkspace.objectConversion, conversion);
      assert.equal(store.snapshot().voxelWorkspace.objectSourceInspection, null);
      assert.equal(store.snapshot().lastError, null);
      assert.equal(firstProjectionLabel(store), 'player');

      client.resolveBlockedInspection();
      await newerInspection;

      assert.equal(store.snapshot().operation, 'idle');
      assert.equal(store.snapshot().voxelWorkspace.objectSourceInspection?.clips[0]?.name, 'Walk');
      assert.strictEqual(store.snapshot().voxelWorkspace.objectConversion, conversion);
      assert.equal(store.snapshot().lastError, null);
      assert.equal(firstProjectionLabel(store), 'player');
    });
  }
});

test('voxel-object source, shared candidate frames, stale apply, explicit discard, apply, and reopen stay distinct', async () => {
  const client = new VoxelObjectFixtureClient();
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');

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
  assert.match(store.snapshot().liveProjection?.meshResources[0]?.sourcePath ?? '', /candidate-0/u);

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
  assert.match(store.snapshot().liveProjection?.meshResources[0]?.sourcePath ?? '', /candidate-1/u);

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
  assert.match(store.snapshot().liveProjection?.meshResources[0]?.sourcePath ?? '', /canonical/u);

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
    sceneId: 'scene/sample-scene',
    instance: {
      instanceId: 'character-one',
      voxelObjectAssetId: 'voxel-object/character',
      surfaceMode: 'greedyCubes',
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
  await reopened.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  assert.deepEqual(
    reopened.snapshot().authoringDocument?.voxelObjectAuthoring,
    store.snapshot().authoringDocument?.voxelObjectAuthoring,
  );
  await reopened.selectEntity(1, 'hierarchy');
  assert.equal(reopened.selectedVoxelObjectInstance()?.ownerEntityId, 1);
  assert.equal(reopened.selectedVoxelObjectInstance()?.instance.instanceId, 'character-one');
  assert.equal(reopened.selectedVoxelObjectAsset()?.assetId, 'voxel-object/character');

  await reopened.runVoxelAction({
    kind: 'setObjectSurfaceMode',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    surfaceMode: 'marchingCubes',
  });
  assert.equal(
    reopened.selectedVoxelObjectInstance()?.instance.surfaceMode,
    'marchingCubes',
  );
  assert.match(
    reopened.snapshot().voxelWorkspace.message ?? '',
    /surface changed from greedyCubes to marchingCubes/u,
  );

  client.rejectObjectSurfaceMode = true;
  await reopened.runVoxelAction({
    kind: 'setObjectSurfaceMode',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    surfaceMode: 'dualContouring',
  });
  assert.match(reopened.snapshot().lastError ?? '', /surfaceTextureUnsupported/u);
  assert.equal(
    reopened.selectedVoxelObjectInstance()?.instance.surfaceMode,
    'marchingCubes',
  );
  client.rejectObjectSurfaceMode = false;

  await reopened.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_000_000,
    command: {
      kind: 'scrub',
      clipId: 'clip/walk-1',
      clipFrame: 0,
      loopMode: 'repeat',
    },
  });
  assert.equal(reopened.snapshot().voxelWorkspace.objectPlayback?.status, 'paused');
  assert.deepEqual(
    reopened.snapshot().voxelWorkspace.objectPlayback?.durableFrame,
    { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
  );
  assert.equal(reopened.snapshot().voxelWorkspace.objectPlayback?.clipFrame, 0);
  assert.equal(reopened.snapshot().authoringDocument?.identity.projectHash, 'hash-before');

  await reopened.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_100_000,
    command: { kind: 'play' },
  });
  assert.equal(reopened.snapshot().voxelWorkspace.objectPlayback?.status, 'playing');
  assert.equal(reopened.snapshot().authoringDocument?.identity.projectHash, 'hash-before');
});

test('voxel-object placement selects canonical owners and keeps one bounded undo/reapply candidate', async () => {
  const client = new VoxelObjectFixtureClient();
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  await store.runVoxelAction(objectPrepareAction());
  const conversion = store.snapshot().voxelWorkspace.objectConversion;
  assert.ok(conversion !== null);
  await store.runVoxelAction({
    kind: 'applyObjectConversion',
    planId: conversion.plan.planId,
    expectedPlanHash: conversion.plan.planHash,
    expectedOutputHash: conversion.preview.outputHash,
  });

  const first = storedObjectInstance('placed-wall-a', [2, 0, 3]);
  await store.runVoxelAction({
    kind: 'attachObjectInstance',
    sceneId: 'scene/sample-scene',
    instance: first,
  });
  assert.equal(store.snapshot().selection.entityId, 1);
  assert.equal(store.snapshot().voxelWorkspace.objectPlacementHistory?.state, 'placed');

  const second = storedObjectInstance('placed-wall-b', [4, 0, 3]);
  await store.runVoxelAction({
    kind: 'attachObjectInstance',
    sceneId: 'scene/sample-scene',
    instance: second,
  });
  assert.equal(store.snapshot().authoringDocument?.voxelObjectAuthoring.instances.length, 2);
  assert.equal(store.snapshot().selection.entityId, 2);
  assert.equal(
    store.snapshot().voxelWorkspace.objectPlacementHistory?.instance.instanceId,
    'placed-wall-b',
  );

  await store.runVoxelAction({ kind: 'undoObjectPlacement', instanceId: 'placed-wall-b' });
  assert.equal(store.snapshot().authoringDocument?.voxelObjectAuthoring.instances.length, 1);
  assert.deepEqual(store.snapshot().voxelWorkspace.objectPlacementHistory, {
    state: 'undone',
    sceneId: 'scene/sample-scene',
    ownerEntityId: null,
    instance: second,
  });

  await store.runVoxelAction({ kind: 'reapplyObjectPlacement', instanceId: 'placed-wall-b' });
  assert.equal(store.snapshot().authoringDocument?.voxelObjectAuthoring.instances.length, 2);
  assert.equal(store.snapshot().voxelWorkspace.objectPlacementHistory?.state, 'placed');
  assert.equal(
    store.snapshot().voxelWorkspace.objectPlacementHistory?.instance.voxelObjectAssetId,
    first.voxelObjectAssetId,
  );

  await store.refreshProject();
  assert.equal(store.snapshot().voxelWorkspace.objectPlacementHistory?.state, 'placed');
  await store.closeProject();
  assert.equal(store.snapshot().voxelWorkspace.objectPlacementHistory, null);
});

test('placement resources are bounded, cancellable, and retained across attach rereads', async () => {
  const client = new VoxelObjectFixtureClient();
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  await store.runVoxelAction(objectPrepareAction());
  const conversion = store.snapshot().voxelWorkspace.objectConversion;
  assert.ok(conversion !== null);
  await store.runVoxelAction({
    kind: 'applyObjectConversion',
    planId: conversion.plan.planId,
    expectedPlanHash: conversion.plan.planHash,
    expectedOutputHash: conversion.preview.outputHash,
  });

  await store.runVoxelAction({
    kind: 'prepareObjectPlacementResource',
    assetId: 'voxel-object/character',
    expectedObjectContentHash: 'sha256:object',
  });
  const prepared = store.snapshot().voxelWorkspace.objectPlacementResource;
  assert.ok(prepared !== null);
  assert.equal(prepared.resourceFrame.ops.filter(
    (operation) => operation.op === 'defineVoxelObject',
  ).length, 1);
  const definition = prepared.resourceFrame.ops.find(
    (operation) => operation.op === 'defineVoxelObject',
  );
  assert.ok(definition?.op === 'defineVoxelObject');
  assert.equal(definition.asset.meshes.every(
    (mesh) => mesh.payload.source.kind === 'resource',
  ), true);
  assert.equal(
    definition.asset.meshes[0]?.payload.source.kind === 'resource'
      ? definition.asset.meshes[0].payload.source.resource
      : null,
    prepared.meshResources[0]?.resource,
  );
  assert.equal(prepared.resourceFrame.ops.some(
    (operation) => operation.op === 'createVoxelObjectInstance',
  ), false);
  assert.match(prepared.meshResources[0]?.sourcePath ?? '', /placement/u);

  await store.runVoxelAction({
    kind: 'attachObjectInstance',
    sceneId: 'scene/sample-scene',
    instance: storedObjectInstance('placed-with-resource', [2, 0, 3]),
  });
  assert.strictEqual(store.snapshot().voxelWorkspace.objectPlacementResource, prepared);
  await store.refreshProject();
  assert.strictEqual(store.snapshot().voxelWorkspace.objectPlacementResource, prepared);

  await store.runVoxelAction({ kind: 'discardObjectPlacementResource' });
  assert.equal(store.snapshot().voxelWorkspace.objectPlacementResource, null);

  client.blockNextPlacementResource();
  const pending = store.runVoxelAction({
    kind: 'prepareObjectPlacementResource',
    assetId: 'voxel-object/character',
    expectedObjectContentHash: 'sha256:object',
  });
  assert.equal(store.snapshot().operation, 'voxel');
  await store.runVoxelAction({ kind: 'discardObjectPlacementResource' });
  assert.equal(store.snapshot().operation, 'idle');
  client.resolveBlockedPlacementResource();
  await pending;
  assert.equal(store.snapshot().voxelWorkspace.objectPlacementResource, null);
  assert.equal(store.snapshot().lastError, null);
});

test('mismatched placement resources fail without publishing a preview candidate', async () => {
  const client = new VoxelObjectFixtureClient();
  client.stalePlacementResource = true;
  client.applied = true;
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');

  await store.runVoxelAction({
    kind: 'prepareObjectPlacementResource',
    assetId: 'voxel-object/character',
    expectedObjectContentHash: 'sha256:object',
  });
  assert.equal(store.snapshot().voxelWorkspace.objectPlacementResource, null);
  assert.match(store.snapshot().lastError ?? '', /did not match the requested asset identity/u);
});

test('rejected voxel-object placement publishes no candidate and leaves prior history current', async () => {
  const client = new VoxelObjectFixtureClient();
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  await store.runVoxelAction(objectPrepareAction());
  const conversion = store.snapshot().voxelWorkspace.objectConversion;
  assert.ok(conversion !== null);
  await store.runVoxelAction({
    kind: 'applyObjectConversion',
    planId: conversion.plan.planId,
    expectedPlanHash: conversion.plan.planHash,
    expectedOutputHash: conversion.preview.outputHash,
  });
  await store.runVoxelAction({
    kind: 'attachObjectInstance',
    sceneId: 'scene/sample-scene',
    instance: storedObjectInstance('accepted-wall', [1, 0, 1]),
  });
  const accepted = store.snapshot().voxelWorkspace.objectPlacementHistory;
  client.rejectObjectAttach = true;
  await store.runVoxelAction({
    kind: 'attachObjectInstance',
    sceneId: 'scene/sample-scene',
    instance: storedObjectInstance('rejected-wall', [9, 0, 9]),
  });
  assert.match(store.snapshot().lastError ?? '', /placement quota exhausted/u);
  assert.equal(store.snapshot().authoringDocument?.voxelObjectAuthoring.instances.length, 1);
  assert.strictEqual(store.snapshot().voxelWorkspace.objectPlacementHistory, accepted);
});

test('voxel-object batch placement is one bounded mutation with deterministic owner selection', async () => {
  const client = new VoxelObjectFixtureClient();
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  await store.runVoxelAction(objectPrepareAction());
  const conversion = store.snapshot().voxelWorkspace.objectConversion;
  assert.ok(conversion !== null);
  await store.runVoxelAction({
    kind: 'applyObjectConversion',
    planId: conversion.plan.planId,
    expectedPlanHash: conversion.plan.planHash,
    expectedOutputHash: conversion.preview.outputHash,
  });

  await store.runVoxelAction({
    kind: 'attachObjectInstance',
    sceneId: 'scene/sample-scene',
    instance: storedObjectInstance('prior-single', [0, 0, 0]),
  });
  assert.ok(store.snapshot().voxelWorkspace.objectPlacementHistory !== null);

  const placements = Array.from(
    { length: MAX_VOXEL_OBJECT_INSTANCE_BATCH },
    (_, index) => ({
      sceneId: 'scene/sample-scene',
      instance: storedObjectInstance(
        `batch-${String(index + 1)}`,
        [index + 1, 0, 2],
      ),
    }),
  );
  await store.runVoxelAction({ kind: 'attachObjectInstances', placements });

  assert.equal(client.batchAttachRequestCount, 1);
  assert.equal(
    store.snapshot().authoringDocument?.voxelObjectAuthoring.instances.length,
    MAX_VOXEL_OBJECT_INSTANCE_BATCH + 1,
  );
  assert.equal(
    store.snapshot().selection.entityId,
    MAX_VOXEL_OBJECT_INSTANCE_BATCH + 1,
  );
  assert.equal(store.snapshot().voxelWorkspace.objectPlacementHistory, null);
  const receipt = store.snapshot().voxelWorkspace.lastReceipt;
  assert.equal(receipt?.kind, 'voxelObjectInstancesAttached');
  assert.equal(
    receipt?.kind === 'voxelObjectInstancesAttached'
      ? receipt.placements.length
      : 0,
    MAX_VOXEL_OBJECT_INSTANCE_BATCH,
  );
});

test('voxel-object batch preflight and late rejection preserve project, selection, and history', async () => {
  const client = new VoxelObjectFixtureClient();
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  await store.runVoxelAction(objectPrepareAction());
  const conversion = store.snapshot().voxelWorkspace.objectConversion;
  assert.ok(conversion !== null);
  await store.runVoxelAction({
    kind: 'applyObjectConversion',
    planId: conversion.plan.planId,
    expectedPlanHash: conversion.plan.planHash,
    expectedOutputHash: conversion.preview.outputHash,
  });
  await store.runVoxelAction({
    kind: 'attachObjectInstance',
    sceneId: 'scene/sample-scene',
    instance: storedObjectInstance('accepted-before-batch', [1, 0, 1]),
  });
  const beforeProject = store.snapshot().authoringDocument;
  const beforeSelection = store.snapshot().selection;
  const beforeHistory = store.snapshot().voxelWorkspace.objectPlacementHistory;

  const oneOver = Array.from(
    { length: MAX_VOXEL_OBJECT_INSTANCE_BATCH + 1 },
    (_, index) => ({
      sceneId: 'scene/sample-scene',
      instance: storedObjectInstance(`one-over-${String(index)}`, [index, 0, 0]),
    }),
  );
  await store.runVoxelAction({ kind: 'attachObjectInstances', placements: oneOver });
  assert.match(store.snapshot().lastError ?? '', /requires 1\.\.=32 instances/u);
  assert.equal(client.batchAttachRequestCount, 0);
  assert.strictEqual(store.snapshot().authoringDocument, beforeProject);
  assert.strictEqual(store.snapshot().selection, beforeSelection);
  assert.strictEqual(store.snapshot().voxelWorkspace.objectPlacementHistory, beforeHistory);

  const duplicate = {
    sceneId: 'scene/sample-scene',
    instance: storedObjectInstance('duplicate-in-batch', [2, 0, 2]),
  };
  await store.runVoxelAction({
    kind: 'attachObjectInstances',
    placements: [duplicate, duplicate],
  });
  assert.match(store.snapshot().lastError ?? '', /duplicates instance duplicate-in-batch/u);
  assert.equal(client.batchAttachRequestCount, 0);

  await store.runVoxelAction({
    kind: 'attachObjectInstances',
    placements: [{
      sceneId: 'scene/sample-scene',
      instance: storedObjectInstance('accepted-before-batch', [3, 0, 3]),
    }],
  });
  assert.match(store.snapshot().lastError ?? '', /collides with existing instance/u);
  assert.equal(client.batchAttachRequestCount, 0);

  client.rejectObjectBatchAtIndex = MAX_VOXEL_OBJECT_INSTANCE_BATCH - 1;
  const lateInvalid = Array.from(
    { length: MAX_VOXEL_OBJECT_INSTANCE_BATCH },
    (_, index) => ({
      sceneId: 'scene/sample-scene',
      instance: storedObjectInstance(`late-invalid-${String(index)}`, [index, 1, 0]),
    }),
  );
  await store.runVoxelAction({ kind: 'attachObjectInstances', placements: lateInvalid });
  assert.match(store.snapshot().lastError ?? '', /placement index 31 rejected/u);
  assert.equal(client.batchAttachRequestCount, 1);
  assert.strictEqual(store.snapshot().authoringDocument, beforeProject);
  assert.strictEqual(store.snapshot().selection, beforeSelection);
  assert.strictEqual(store.snapshot().voxelWorkspace.objectPlacementHistory, beforeHistory);
});

test('pause and restore queue behind an in-flight applied-object sample', async () => {
  const client = new VoxelObjectFixtureClient();
  client.applied = true;
  client.attached = true;
  const timer = new ManualPlaybackTimer();
  const store = new StudioWorkspaceStore(
    client as unknown as StudioAdapterClient,
    null,
    timer,
  );
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_000_000,
    command: {
      kind: 'scrub',
      clipId: 'clip/walk-1',
      clipFrame: 0,
      loopMode: 'repeat',
    },
  });
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_100_000,
    command: { kind: 'play' },
  });

  client.blockNextPlayback();
  const sample = store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_200_000,
    command: { kind: 'sample' },
  });
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_210_000,
    command: { kind: 'pause' },
  });
  assert.equal(store.snapshot().operation, 'voxel');
  assert.deepEqual(client.playbackCommands.slice(-1), ['sample']);
  client.resolveBlockedPlayback();
  await sample;
  await Promise.resolve();
  assert.equal(store.snapshot().voxelWorkspace.objectPlayback?.status, 'paused');
  assert.deepEqual(client.playbackCommands.slice(-2), ['sample', 'pause']);

  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_300_000,
    command: { kind: 'play' },
  });
  client.blockNextPlayback();
  const secondSample = store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_400_000,
    command: { kind: 'sample' },
  });
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_410_000,
    command: { kind: 'pause' },
  });
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_420_000,
    command: { kind: 'stop' },
  });
  client.resolveBlockedPlayback();
  await secondSample;
  await Promise.resolve();
  assert.equal(store.snapshot().voxelWorkspace.objectPlayback?.status, 'stopped');
  assert.deepEqual(client.playbackCommands.slice(-2), ['sample', 'stop']);
});

test('applied-object playback advances one virtual frame only after renderer acknowledgement and completion', async () => {
  const client = new VoxelObjectFixtureClient();
  client.applied = true;
  client.attached = true;
  const timer = new ManualPlaybackTimer();
  const store = new StudioWorkspaceStore(
    client as unknown as StudioAdapterClient,
    null,
    timer,
  );
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_000_000,
    command: {
      kind: 'scrub',
      clipId: 'clip/walk-1',
      clipFrame: 0,
      loopMode: 'repeat',
    },
  });
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_100_000,
    command: { kind: 'play' },
  });

  const playGeneration = store.snapshot().liveProjection?.generation;
  assert.ok(playGeneration !== undefined);
  assert.equal(timer.pendingCount, 0, 'adapter completion alone must not advance playback');
  store.acknowledgeProjectionGeneration(playGeneration);
  assert.equal(timer.nextDelayMilliseconds, 500);

  client.blockNextPlayback();
  timer.fireNext();
  assert.deepEqual(client.playbackCommands.slice(-2), ['play', 'sample']);
  assert.equal(client.playbackTimes.at(-1), 5_600_000);
  assert.equal(timer.pendingCount, 0, 'an in-flight sample must not schedule its successor');

  client.resolveBlockedPlayback();
  await eventLoopTurn();
  const sampleGeneration = store.snapshot().liveProjection?.generation;
  assert.ok(sampleGeneration !== undefined && sampleGeneration > playGeneration);
  assert.equal(timer.pendingCount, 0, 'a completed sample still waits for renderer acknowledgement');
  store.acknowledgeProjectionGeneration(sampleGeneration);
  assert.equal(timer.nextDelayMilliseconds, 500);
  timer.fireNext();
  await eventLoopTurn();
  assert.equal(client.playbackTimes.at(-1), 6_100_000);
});

test('applied playback restores its canonical entity base before retaining patches over a conversion candidate', async () => {
  const client = new VoxelObjectFixtureClient();
  client.applied = true;
  client.attached = true;
  const timer = new ManualPlaybackTimer();
  const store = new StudioWorkspaceStore(
    client as unknown as StudioAdapterClient,
    null,
    timer,
  );
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');

  await store.runVoxelAction(objectPrepareAction());
  const candidate = store.snapshot().liveProjection?.frame.ops.find(
    (operation) => operation.op === 'create' && operation.handle === 901,
  );
  assert.ok(candidate?.op === 'create');
  assert.equal(candidate.node.metadata.sourceEntity, null);
  assert.equal(candidate.node.metadata.label, 'voxel-object-candidate-0');

  store.selectHierarchyNode(10);
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_000_000,
    command: {
      kind: 'scrub',
      clipId: 'clip/walk-1',
      clipFrame: 0,
      loopMode: 'repeat',
    },
  });

  const restored = store.snapshot().liveProjection;
  assert.ok(restored !== null);
  const restoredInstance = restored.frame.ops.find(
    (operation) => operation.op === 'createVoxelObjectInstance' && operation.handle === 901,
  );
  assert.ok(restoredInstance?.op === 'createVoxelObjectInstance');
  assert.doesNotThrow(() => decodeRenderFrameDiff(restored.frame));
  assert.equal(restored.framePatch, null, 'candidate display must force a complete canonical replace');
  assert.equal(restoredInstance.instance.asset, 'voxel-object/character');
  assert.equal(restoredInstance.instance.frame, 0);
  assert.deepEqual(restoredInstance.instance.transform, transform([4, 0, 2], [2, 2, 2]));
  assert.deepEqual(restoredInstance.instance.metadata, {
    sourceEntity: 1,
    sourceSceneNode: 10,
    tags: ['voxel-object'],
    label: 'character-one',
  });
  assert.equal(
    restored.frame.ops.some(
      (operation) => operation.op === 'create'
        && operation.node.metadata.label?.startsWith('voxel-object-candidate') === true,
    ),
    false,
  );

  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_100_000,
    command: { kind: 'play' },
  });
  const playGeneration = store.snapshot().liveProjection?.generation;
  assert.ok(playGeneration !== undefined);
  assert.deepEqual(store.snapshot().liveProjection?.framePatch?.ops, []);
  store.acknowledgeProjectionGeneration(playGeneration);
  assert.equal(timer.nextDelayMilliseconds, 500);
  timer.fireNext();
  await eventLoopTurn();

  const retained = store.snapshot().liveProjection;
  assert.deepEqual(retained?.framePatch?.ops, [
    { op: 'setVoxelObjectFrame', handle: 901, frame: 1 },
  ]);
  const retainedInstance = retained?.frame.ops.find(
    (operation) => operation.op === 'createVoxelObjectInstance' && operation.handle === 901,
  );
  assert.ok(retainedInstance?.op === 'createVoxelObjectInstance');
  assert.equal(retainedInstance.instance.metadata.sourceEntity, 1);
  assert.equal(retainedInstance.instance.metadata.sourceSceneNode, 10);
});

test('applied playback rejects a retained frame patch outside the canonical project base', async () => {
  const client = new VoxelObjectFixtureClient();
  client.applied = true;
  client.attached = true;
  client.playbackHandle = 902;
  const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
  await store.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
  const canonicalFrame = store.snapshot().liveProjection?.frame;

  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_000_000,
    command: {
      kind: 'scrub',
      clipId: 'clip/walk-1',
      clipFrame: 0,
      loopMode: 'repeat',
    },
  });

  assert.match(store.snapshot().lastError ?? '', /does not match its canonical project base/);
  assert.deepEqual(store.snapshot().liveProjection?.frame, canonicalFrame);
  assert.equal(store.snapshot().voxelWorkspace.objectPlayback, null);
});

test('project lifecycle discards queued applied-object controls from the replaced scope', async (t) => {
  const replacements = ['open', 'create', 'saveAs', 'read', 'close'] as const;
  const controls = ['pause', 'stop'] as const;

  for (const replacement of replacements) {
    for (const control of controls) {
      await t.test(`${replacement} drops queued ${control}`, async () => {
        const client = new VoxelObjectFixtureClient();
        client.applied = true;
        client.attached = true;
        client.openedProjectId = 'sample-project-a';
        const store = new StudioWorkspaceStore(client as unknown as StudioAdapterClient);
        await store.openProject(
          '/external/sample-project-a',
          'content/projects/sample-project.project.json',
        );
        await beginAppliedObjectPlayback(store);

        client.blockNextPlayback();
        const staleSample = store.runVoxelAction({
          kind: 'previewObjectInstance',
          sceneId: 'scene/sample-scene',
          instanceId: 'character-one',
          nowMicroseconds: 5_200_000,
          command: { kind: 'sample' },
        });
        await store.runVoxelAction({
          kind: 'previewObjectInstance',
          sceneId: 'scene/sample-scene',
          instanceId: 'character-one',
          nowMicroseconds: 5_210_000,
          command: { kind: control },
        });
        assert.equal(store.snapshot().operation, 'voxel');
        assert.equal(client.playbackCommands.at(-1), 'sample');

        client.openedProjectId = 'sample-project-b';
        switch (replacement) {
          case 'open':
            await store.openProject(
              '/external/sample-project-b',
              'content/projects/sample-project.project.json',
            );
            break;
          case 'create':
            await store.createProject({
              root: '/external/sample-project-b',
              projectFile: 'content/projects/sample-project.project.json',
              projectId: 'sample-project-b',
              name: 'Sample Project B',
              entryScene: 'scene/sample-scene',
              entrySceneName: 'Sample Project',
            });
            break;
          case 'saveAs':
            await store.saveProjectAs({
              root: '/external/sample-project-b',
              projectFile: 'content/projects/sample-project.project.json',
              projectId: 'sample-project-b',
              name: 'Sample Project B',
            });
            break;
          case 'read':
            await store.refreshProject();
            break;
          case 'close':
            await store.closeProject();
            break;
        }

        if (replacement === 'close') {
          assert.equal(store.snapshot().authoringDocument, null);
        } else {
          assert.equal(store.snapshot().authoringDocument?.identity.projectId, 'sample-project-b');
          assert.equal(
            store.snapshot().authoringDocument?.voxelObjectAuthoring.instances[0]?.instance.instanceId,
            'character-one',
            'replacement deliberately overlaps the old scene and instance identities',
          );
          assert.equal(store.snapshot().voxelWorkspace.objectPlayback, null);
        }
        assert.equal(store.snapshot().operation, 'idle');
        assert.equal(store.snapshot().lastError, null);
        const acceptedReplacement = store.snapshot();
        const commandCount = client.playbackCommands.length;

        client.resolveBlockedPlayback();
        await staleSample;
        await Promise.resolve();

        assert.strictEqual(store.snapshot(), acceptedReplacement);
        assert.equal(client.playbackCommands.length, commandCount);
        assert.equal(client.playbackCommands.at(-1), 'sample');
      });
    }
  }
});

async function beginAppliedObjectPlayback(store: StudioWorkspaceStore): Promise<void> {
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_000_000,
    command: {
      kind: 'scrub',
      clipId: 'clip/walk-1',
      clipFrame: 0,
      loopMode: 'repeat',
    },
  });
  await store.runVoxelAction({
    kind: 'previewObjectInstance',
    sceneId: 'scene/sample-scene',
    instanceId: 'character-one',
    nowMicroseconds: 5_100_000,
    command: { kind: 'play' },
  });
}

test('host-user camera and keyboard settings persist outside project authority and reload by project root', async () => {
  const settingsHost = new FixtureSettingsHost();
  const first = new StudioWorkspaceStore(
    new StudioAdapterClient(new FixtureTransport()),
    new HttpStudioUserSettingsClient('/api/studio-user-settings', settingsHost.fetch),
  );
  await first.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
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
  await second.openProject('/external/sample-project', 'content/projects/sample-project.project.json');
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

  assert.equal(response.adapter.adapterId, 'studio.fixture');
  assert.deepEqual(requests, ['/api/studio-adapter:POST']);

  let oversizedRequestFetched = false;
  const oversizedRequest = new HttpStudioAdapterTransport('/api/studio-adapter', async () => {
    oversizedRequestFetched = true;
    throw new Error('fetch must not run for an oversized request');
  });
  await assert.rejects(
    oversizedRequest.exchange({
      type: 'describe',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: 'oversized-request',
      padding: 'x'.repeat(256 * 1024),
    } as unknown as StudioAdapterRequest),
    /request exceeds the protocol byte bound/u,
  );
  assert.equal(oversizedRequestFetched, false);

  const oversized = new HttpStudioAdapterTransport('/api/studio-adapter', async () => ({
    ok: true,
    status: 200,
    headers: new Headers({ 'content-length': String(64 * 1024 * 1024 + 1) }),
    text: async () => '',
  }));
  await assert.rejects(
    oversized.exchange({
      type: 'describe',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: 'x',
    }),
    /response is 67108865 bytes; the protocol limit is 67108864 bytes/u,
  );

  const typedHostFailure = new HttpStudioAdapterTransport('/api/studio-adapter', async () => ({
    ok: false,
    status: 502,
    headers: new Headers({ 'content-length': '151' }),
    text: async () => JSON.stringify({
      ok: false,
      code: 'studio_adapter_response_too_large',
      message: 'Studio adapter response exceeded the 67108864-byte protocol limit '
        + 'after receiving 67108865 bytes',
      limitBytes: 67_108_864,
      actualBytes: 67_108_865,
    }),
  }));
  await assert.rejects(
    typedHostFailure.exchange({
      type: 'describe',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: 'typed-host-failure',
    }),
    /response exceeded the 67108864-byte protocol limit after receiving 67108865 bytes/u,
  );
});

async function selectFixtureInspector(store: StudioWorkspaceStore) {
  await store.selectEntity(1, 'inspector');
  const reference = store.snapshot().authoringDocument?.entityComponents.find(
    (candidate) => candidate.componentTypeId === FIXTURE_COMPONENT_TYPE_ID,
  );
  assert.ok(reference !== undefined);
  const context = store.entityInspectorContext(reference);
  assert.ok(context !== null);
  return context;
}

class FixtureTransport implements StudioAdapterTransport {
  readonly requests: StudioAdapterRequest[] = [];
  rejectMutation = false;
  openedProjectId = 'sample-project';
  readProjectChanged = false;
  #blockNextProjectRead = false;
  #blockedProjectRead: {
    readonly requestId: string;
    readonly resolve: (response: unknown) => void;
  } | null = null;

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
    if (request.type === 'setSceneObjectRenderableTransform') {
      const project = projectReadout(false) as ReturnType<typeof projectReadout>;
      project.identity.projectHash = 'hash-visual-after';
      project.identity.sceneRevision = 12;
      project.sceneHierarchy.revision = 12;
      (project.sceneHierarchy.nodes[0] as { renderableTransform: unknown }).renderableTransform =
        request.transform;
      const operation = project.projection.ops[0];
      if (operation?.op === 'create') {
        operation.node.transform = transform([1, 0.75, 3]);
      }
      return Promise.resolve({
        type: 'projectMutationApplied',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: request.requestId,
        receipt: { kind: 'sceneObjectRenderableTransformSet', entityId: request.entityId },
        project,
      });
    }
    if (request.type === 'readProject') {
      if (this.#blockNextProjectRead) {
        this.#blockNextProjectRead = false;
        return new Promise((resolve) => {
          this.#blockedProjectRead = { requestId: request.requestId, resolve };
        });
      }
      return Promise.resolve(
        projectResponse('projectRead', request.requestId, this.readProjectChanged),
      );
    }
    return Promise.resolve({
      type: 'projectClosed',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: request.requestId,
    });
  }

  blockNextProjectRead(): void {
    assert.equal(this.#blockedProjectRead, null);
    this.#blockNextProjectRead = true;
  }

  resolveBlockedProjectRead(changed: boolean): void {
    const blocked = this.#blockedProjectRead;
    assert.ok(blocked !== null);
    this.#blockedProjectRead = null;
    blocked.resolve(projectResponse('projectRead', blocked.requestId, changed));
  }
}

class ManualPlaybackTimer implements StudioPlaybackTimer {
  #nextHandle = 1;
  readonly #pending = new Map<number, {
    readonly callback: () => void;
    readonly delayMilliseconds: number;
  }>();

  get pendingCount(): number {
    return this.#pending.size;
  }

  get nextDelayMilliseconds(): number | null {
    return this.#pending.values().next().value?.delayMilliseconds ?? null;
  }

  readonly cancel = (handle: unknown): void => {
    if (typeof handle === 'number') this.#pending.delete(handle);
  };

  readonly schedule = (callback: () => void, delayMilliseconds: number): unknown => {
    const handle = this.#nextHandle++;
    this.#pending.set(handle, { callback, delayMilliseconds });
    return handle;
  };

  fireNext(): void {
    const entry = this.#pending.entries().next().value as
      | readonly [number, { readonly callback: () => void }]
      | undefined;
    assert.ok(entry !== undefined, 'expected a pending playback timer');
    this.#pending.delete(entry[0]);
    entry[1].callback();
  }
}

class VoxelObjectFixtureClient {
  rejectObjectApply = false;
  applied = false;
  attached = false;
  rejectObjectAttach = false;
  rejectObjectSurfaceMode = false;
  rejectObjectBatchAtIndex: number | null = null;
  batchAttachRequestCount = 0;
  stalePlacementResource = false;
  readonly attachedInstances: {
    readonly sceneId: string;
    readonly ownerEntityId: number;
    readonly instance: ReturnType<typeof storedObjectInstance>;
  }[] = [];
  readonly surfaceModes = new Map<string, 'greedyCubes' | 'marchingCubes' | 'dualContouring'>();
  openedProjectId = 'sample-project';
  previewRequestCount = 0;
  applyRequestCount = 0;
  playbackStatus: 'stopped' | 'playing' | 'paused' = 'stopped';
  playbackFrame = 0;
  playbackHandle = 901;
  readonly playbackCommands: string[] = [];
  readonly playbackTimes: number[] = [];
  #blockedInspection: {
    readonly promise: Promise<unknown>;
    readonly resolve: (response: unknown) => void;
  } | null = null;
  #blockedPreview: {
    readonly promise: Promise<unknown>;
    readonly resolve: (response: unknown) => void;
    readonly reject: (error: unknown) => void;
    response: unknown | null;
  } | null = null;
  #blockedPlayback: {
    readonly promise: Promise<unknown>;
    readonly resolve: (response: unknown) => void;
    response: unknown | null;
  } | null = null;
  #blockedPlacementResource: {
    readonly promise: Promise<unknown>;
    readonly resolve: (response: unknown) => void;
    response: unknown | null;
  } | null = null;

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

  createProject() {
    return Promise.resolve({ project: this.#project() } as never);
  }

  saveProjectAs() {
    return Promise.resolve({ project: this.#project() } as never);
  }

  closeProject() {
    return Promise.resolve({} as never);
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
    if (this.#blockedInspection !== null) return this.#blockedInspection.promise;
    return Promise.resolve({ inspection: objectSourceInspection() } as never);
  }

  prepareVoxelObjectConversion() {
    return Promise.resolve({
      plan: objectPlan(),
      preview: objectPreview(0),
      projection: objectCandidateProjection(0),
      projectionReadout: projectionReadout(20),
      meshResources: [meshResourceReadout('2', 'candidate-0')],
    } as never);
  }

  blockNextInspection() {
    assert.equal(this.#blockedInspection, null);
    let resolve!: (response: unknown) => void;
    const promise = new Promise<unknown>((resolvePromise) => {
      resolve = resolvePromise;
    });
    this.#blockedInspection = { promise, resolve };
  }

  resolveBlockedInspection() {
    const blocked = this.#blockedInspection;
    assert.ok(blocked !== null);
    this.#blockedInspection = null;
    blocked.resolve({ inspection: objectSourceInspection() });
  }

  blockNextPreview() {
    assert.equal(this.#blockedPreview, null);
    let resolve!: (response: unknown) => void;
    let reject!: (error: unknown) => void;
    const promise = new Promise<unknown>((resolvePromise, rejectPromise) => {
      resolve = resolvePromise;
      reject = rejectPromise;
    });
    this.#blockedPreview = { promise, resolve, reject, response: null };
  }

  resolveBlockedPreview() {
    const blocked = this.#blockedPreview;
    assert.ok(blocked !== null && blocked.response !== null);
    this.#blockedPreview = null;
    blocked.resolve(blocked.response);
  }

  rejectBlockedPreview() {
    const blocked = this.#blockedPreview;
    assert.ok(blocked !== null);
    this.#blockedPreview = null;
    blocked.reject(new Error('late preview rejected'));
  }

  blockNextPlayback() {
    assert.equal(this.#blockedPlayback, null);
    let resolve!: (response: unknown) => void;
    const promise = new Promise<unknown>((resolvePromise) => {
      resolve = resolvePromise;
    });
    this.#blockedPlayback = { promise, resolve, response: null };
  }

  resolveBlockedPlayback() {
    const blocked = this.#blockedPlayback;
    assert.ok(blocked !== null && blocked.response !== null);
    this.#blockedPlayback = null;
    blocked.resolve(blocked.response);
  }

  previewVoxelObjectConversion(input: { readonly frame: { readonly kind: string; readonly frameIndex?: number } }) {
    this.previewRequestCount += 1;
    const frame = input.frame.kind === 'clip' ? input.frame.frameIndex ?? 0 : 0;
    const response = {
      preview: objectPreview(frame),
      projection: objectCandidateProjection(frame),
      projectionReadout: projectionReadout(20 + frame),
      meshResources: [meshResourceReadout(frame === 0 ? '2' : '3', `candidate-${String(frame)}`)],
    };
    if (this.#blockedPreview !== null) {
      this.#blockedPreview.response = response;
      return this.#blockedPreview.promise;
    }
    return Promise.resolve(response as never);
  }

  applyVoxelObjectConversion() {
    this.applyRequestCount += 1;
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
      meshResources: project.meshResources,
    } as never);
  }

  prepareVoxelObjectPlacement() {
    const project = projectReadout(false, this.openedProjectId);
    const projection = decodeRenderFrameDiff(appliedVoxelObjectProjection(project.projection));
    const resource = meshResourceReadout('4', 'placement');
    const response = {
      assetId: 'voxel-object/character',
      objectContentHash: this.stalePlacementResource ? 'sha256:stale' : 'sha256:object',
      resourceFrame: placementResourceFrame(projection, resource),
      meshResources: [resource],
    };
    if (this.#blockedPlacementResource !== null) {
      this.#blockedPlacementResource.response = response;
      return this.#blockedPlacementResource.promise;
    }
    return Promise.resolve(response as never);
  }

  blockNextPlacementResource() {
    assert.equal(this.#blockedPlacementResource, null);
    let resolve!: (response: unknown) => void;
    const promise = new Promise<unknown>((resolvePromise) => {
      resolve = resolvePromise;
    });
    this.#blockedPlacementResource = { promise, resolve, response: null };
  }

  resolveBlockedPlacementResource() {
    const blocked = this.#blockedPlacementResource;
    assert.ok(blocked !== null && blocked.response !== null);
    this.#blockedPlacementResource = null;
    blocked.resolve(blocked.response);
  }

  attachVoxelObjectInstance(input: {
    readonly sceneId: string;
    readonly instance: ReturnType<typeof storedObjectInstance>;
  }) {
    if (this.rejectObjectAttach) {
      return Promise.reject(new Error('voxelObject.placementQuota: placement quota exhausted'));
    }
    const ownerEntityId = this.attachedInstances.length + 1;
    this.attachedInstances.push({
      sceneId: input.sceneId,
      ownerEntityId,
      instance: structuredClone(input.instance),
    });
    this.surfaceModes.set(input.instance.instanceId, input.instance.surfaceMode);
    this.attached = true;
    return Promise.resolve({
      receipt: {
        kind: 'voxelObjectInstanceAttached',
        sceneId: input.sceneId,
        instanceId: input.instance.instanceId,
        assetId: input.instance.voxelObjectAssetId,
        frameKind: input.instance.frame.kind,
      },
      project: this.#project(),
    } as never);
  }

  attachVoxelObjectInstances(input: {
    readonly placements: readonly {
      readonly sceneId: string;
      readonly instance: ReturnType<typeof storedObjectInstance>;
    }[];
  }) {
    this.batchAttachRequestCount += 1;
    if (this.rejectObjectBatchAtIndex !== null) {
      return Promise.reject(
        new Error(`voxelObject.batchEntry: placement index ${String(this.rejectObjectBatchAtIndex)} rejected`),
      );
    }
    const nextOwnerEntityId = this.attachedInstances.reduce(
      (maximum, entry) => Math.max(maximum, entry.ownerEntityId),
      0,
    ) + 1;
    const staged = input.placements.map((placement, index) => ({
      sceneId: placement.sceneId,
      ownerEntityId: nextOwnerEntityId + index,
      instance: structuredClone(placement.instance),
    }));
    this.attachedInstances.push(...staged);
    for (const placement of staged) {
      this.surfaceModes.set(placement.instance.instanceId, placement.instance.surfaceMode);
    }
    this.attached = true;
    return Promise.resolve({
      receipt: {
        kind: 'voxelObjectInstancesAttached',
        placements: staged.map((placement) => ({
          sceneId: placement.sceneId,
          instanceId: placement.instance.instanceId,
          assetId: placement.instance.voxelObjectAssetId,
          frameKind: placement.instance.frame.kind,
          ownerEntityId: placement.ownerEntityId,
        })),
      },
      project: this.#project(),
    } as never);
  }

  deleteSceneObject(input: { readonly entityId: number }) {
    const index = this.attachedInstances.findIndex(
      (entry) => entry.ownerEntityId === input.entityId,
    );
    if (index < 0) return Promise.reject(new Error('sceneObject.missing: owner not found'));
    const [removed] = this.attachedInstances.splice(index, 1);
    if (removed !== undefined) this.surfaceModes.delete(removed.instance.instanceId);
    this.attached = this.attachedInstances.length > 0;
    return Promise.resolve({
      receipt: {
        kind: 'sceneObjectDeleted',
        sceneId: 'scene/sample-scene',
        entityId: input.entityId,
        removedObjects: 1,
      },
      project: this.#project(),
    } as never);
  }

  setVoxelObjectInstanceSurfaceMode(input: {
    readonly sceneId: string;
    readonly instanceId: string;
    readonly surfaceMode: 'greedyCubes' | 'marchingCubes' | 'dualContouring';
  }) {
    const entry = this.attachedInstances.find(
      (candidate) => candidate.sceneId === input.sceneId
        && candidate.instance.instanceId === input.instanceId,
    );
    if (entry === undefined) return Promise.reject(new Error('voxelObject.instanceMissing: instance not found'));
    const before = this.surfaceModes.get(input.instanceId) ?? entry.instance.surfaceMode;
    if (this.rejectObjectSurfaceMode) {
      return Promise.reject(new Error('voxelObject.surfaceTextureUnsupported: smooth textured surfaces are unsupported'));
    }
    this.surfaceModes.set(input.instanceId, input.surfaceMode);
    return Promise.resolve({
      receipt: {
        kind: 'voxelObjectSurfaceModeSet',
        sceneId: input.sceneId,
        instanceId: input.instanceId,
        before,
        after: input.surfaceMode,
      },
      project: this.#project(),
    } as never);
  }

  previewVoxelObjectInstance(input: {
    readonly nowMicroseconds: number;
    readonly command: {
      readonly kind: string;
      readonly clipFrame?: number;
    };
  }) {
    this.playbackCommands.push(input.command.kind);
    this.playbackTimes.push(input.nowMicroseconds);
    if (input.command.kind === 'scrub') {
      this.playbackStatus = 'paused';
      this.playbackFrame = input.command.clipFrame ?? 0;
    } else if (input.command.kind === 'play') {
      this.playbackStatus = 'playing';
    } else if (input.command.kind === 'pause') {
      this.playbackStatus = 'paused';
    } else if (input.command.kind === 'sample') {
      this.playbackFrame = (this.playbackFrame + 1) % 2;
    } else if (input.command.kind === 'stop') {
      this.playbackStatus = 'stopped';
    }
    const response = {
      playback: {
        sceneId: 'scene/sample-scene',
        instanceId: 'character-one',
        voxelObjectAssetId: 'voxel-object/character',
        projectHash: 'hash-before',
        objectContentHash: 'sha256:object',
        durableFrame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
        status: this.playbackStatus,
        clipId: this.playbackStatus === 'stopped' ? null : 'clip/walk-1',
        loopMode: this.playbackStatus === 'stopped' ? 'once' : 'repeat',
        rate: { numerator: 1, denominator: 1 },
        elapsedMicroseconds: this.playbackFrame * 100_000,
        runtimeFrame: this.playbackFrame + 1,
        clipFrame: this.playbackStatus === 'stopped' ? null : this.playbackFrame,
        ended: false,
      },
      projection: {
        schemaVersion: 1,
        ops: input.command.kind === 'play' || input.command.kind === 'pause'
          ? []
          : [{
              op: 'setVoxelObjectFrame',
              handle: this.playbackHandle,
              frame: this.playbackFrame,
            }],
      },
      projectionReadout: projectionReadout(20 + this.playbackFrame),
      meshResources: [meshResourceReadout('1', 'canonical')],
    };
    if (this.#blockedPlayback !== null) {
      this.#blockedPlayback.response = response;
      return this.#blockedPlayback.promise;
    }
    return Promise.resolve(response as never);
  }

  #project() {
    const project = projectReadout(false, this.openedProjectId);
    const instances = this.attachedInstances.length > 0
      ? this.attachedInstances.map((entry) => ({
          ...entry,
          instance: {
            ...entry.instance,
            surfaceMode: this.surfaceModes.get(entry.instance.instanceId) ?? entry.instance.surfaceMode,
          },
        }))
      : this.attached
        ? [{
            sceneId: 'scene/sample-scene',
            ownerEntityId: 1,
            instance: storedObjectInstance('character-one', [4, 0, 2]),
          }]
        : [];
    return {
      ...project,
      meshResources: [meshResourceReadout('1', 'canonical')],
      projection: this.attached
        ? appliedVoxelObjectProjection(project.projection)
        : project.projection,
      projectionReadout: this.attached
        ? { ...project.projectionReadout, retainedVoxelInstances: 1 }
        : project.projectionReadout,
      voxelObjectAuthoring: {
        assets: this.applied ? [objectAssetReadout()] : [],
        instances,
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
      canonicalProjectRoot: '/external/sample-project',
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
      adapterId: 'studio.fixture',
      adapterVersion: 6,
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      projectKind: 'sampleProject',
      projectSchemaVersion: 11,
      operations: STUDIO_ADAPTER_OPERATIONS,
      entityInspectorContracts: [{
        contractId: FIXTURE_CONTRACT_ID,
        contractVersion: FIXTURE_CONTRACT_VERSION,
      }],
    },
  };
}

function projectResponse(
  type: 'projectOpened' | 'projectRead',
  requestId: string,
  changed: boolean,
  projectId = 'sample-project',
): unknown {
  return {
    type,
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    project: projectReadout(changed, projectId),
  };
}

function meshResourceReadout(digestDigit: string, label: string) {
  const digest = digestDigit.repeat(64);
  return {
    resource: `mesh-resource/${digest}`,
    contentHash: `sha256:${digest}`,
    byteLength: 1024,
    sourcePath: `.rusty-engine-cache/render-resources/${label}-${digest}.rmesh`,
  };
}

function projectReadout(changed: boolean, projectId = 'sample-project') {
  const translation = changed ? [4.5, 2, 3] : [1, 2, 3];
  return {
    identity: {
      projectId,
      name: 'Sample Project',
      entryScene: 'scene/sample-scene',
      sourceSchemaVersion: 11,
      currentSchemaVersion: 11,
      projectHash: changed ? 'hash-after' : 'hash-before',
      sceneRevision: changed ? 12 : 11,
      relativeProjectFile: 'content/projects/sample-project.project.json',
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
        name: 'Sample Project',
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
        roles: [{ name: 'resource:sample-project-project', count: 1 }],
        loadSteps: [{ stage: 'resources', path: 'content/projects/sample-project.project.json' }],
        diagnostics: { diagnostics: [] },
      },
    },
    sceneHierarchy: {
      sceneId: 1,
      revision: changed ? 12 : 11,
      name: 'Sample Project',
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
    voxelSurfaceAuthoring: {
      textures: [],
      atlases: [],
      materials: [],
    },
    voxelObjectAuthoring: {
      assets: [],
      instances: [],
    },
    animatedMeshResources: [],
    entityComponents: [{
      ownerEntityId: 1,
      componentTypeId: FIXTURE_COMPONENT_TYPE_ID,
      inspectorContract: {
        contractId: FIXTURE_CONTRACT_ID,
        contractVersion: FIXTURE_CONTRACT_VERSION,
      },
    }],
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

function appliedVoxelObjectProjection(
  project: ReturnType<typeof projectReadout>['projection'],
) {
  const payload = {
    layout: {
      vertexCount: 4,
      indexCount: 6,
      indexWidth: 'u32' as const,
      attributes: [
        { name: 'position' as const, components: 3 as const, kind: 'f32' as const },
        { name: 'normal' as const, components: 3 as const, kind: 'f32' as const },
      ],
    },
    groups: [{ materialSlot: 7, start: 0, count: 6 }],
    bounds: { min: [0, 0, 0] as const, max: [1, 1, 0] as const },
    source: {
      kind: 'inline' as const,
      positions: [0, 0, 0, 1, 0, 0, 1, 1, 0, 0, 1, 0],
      normals: [0, 0, 1, 0, 0, 1, 0, 0, 1, 0, 0, 1],
      indices: [0, 1, 2, 0, 2, 3],
    },
    provenance: 'voxelObject' as const,
  };
  return {
    schemaVersion: 1 as const,
    ops: [
      ...project.ops,
      {
        op: 'defineMaterial' as const,
        material: {
          schemaVersion: 1,
          id: 'material/wall-lines',
          color: [0.8, 0.5, 0.2, 1] as const,
          texture: null,
          roughness: 1,
          textureTint: [1, 1, 1, 1] as const,
          emissionColor: [0, 0, 0] as const,
          emissionIntensity: 0,
          uvStrategy: 'flat' as const,
        },
      },
      {
        op: 'defineVoxelObject' as const,
        asset: {
          asset: 'voxel-object/character',
          contentHash: 'sha256:object',
          meshes: [{ payload }, { payload }],
          frames: [{ id: 'walk/0', mesh: 0 }, { id: 'walk/1', mesh: 1 }],
          materialSlots: [{ slot: 7, material: 'material/wall-lines' }],
        },
      },
      {
        op: 'createVoxelObjectInstance' as const,
        handle: 901,
        parent: null,
        instance: {
          asset: 'voxel-object/character',
          frame: 1,
          transform: transform([4, 0, 2], [2, 2, 2]),
          visible: true,
          materialOverrides: [],
          metadata: {
            sourceEntity: 1,
            sourceSceneNode: 10,
            tags: ['voxel-object'],
            label: 'character-one',
          },
        },
      },
    ],
  };
}

function placementResourceFrame(
  projection: RenderFrameDiff,
  resource: ReturnType<typeof meshResourceReadout>,
) {
  return {
    schemaVersion: 1 as const,
    ops: projection.ops
      .filter((operation) =>
        operation.op === 'defineMaterial' || operation.op === 'defineVoxelObject')
      .map((operation) => {
        if (operation.op !== 'defineVoxelObject') return operation;
        return {
          ...operation,
          asset: {
            ...operation.asset,
            meshes: operation.asset.meshes.map((mesh) => ({
              ...mesh,
              payload: {
                ...mesh.payload,
                source: {
                  kind: 'resource' as const,
                  resource: resource.resource,
                  contentHash: resource.contentHash,
                  byteLength: resource.byteLength,
                  encoding: 'packedStreamsLeV1' as const,
                  positionsByteOffset: 16,
                  normalsByteOffset: 64,
                  indicesByteOffset: 112,
                },
              },
            })),
          },
        };
      }),
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

function storedObjectInstance(
  instanceId: string,
  translation: readonly [number, number, number],
) {
  return {
    instanceId,
    voxelObjectAssetId: 'voxel-object/character',
    surfaceMode: 'greedyCubes' as const,
    frame: { kind: 'clip' as const, clipId: 'clip/walk-1', frameIndex: 1 },
    translation,
    rotation: [0, 0, 0, 1] as const,
    scale: [2, 2, 2] as const,
    materialOverrides: [],
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

function eventLoopTurn(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
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
    renderableTransform: transform([0, 0, 0]),
  };
}

function transform(
  translation: readonly number[],
  scale: readonly number[] = [1, 1, 1],
): unknown {
  return {
    translation,
    rotation: [0, 0, 0, 1],
    scale,
  };
}
