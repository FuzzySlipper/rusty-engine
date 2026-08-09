import assert from 'node:assert/strict';
import test from 'node:test';

import {
  StudioAdapterClient,
  StudioAdapterDecodeError,
  StudioAdapterOperationRejected,
  MAX_VOXEL_OBJECT_INSTANCE_BATCH,
  MAX_STUDIO_ENTITY_COMPONENT_REFERENCES,
  MAX_STUDIO_ENTITY_COMPONENTS_PER_OWNER,
  MAX_STUDIO_ENTITY_INSPECTOR_CONTRACTS,
  MAX_STUDIO_ENTITY_INSPECTOR_IDENTITY_BYTES,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  STUDIO_ADAPTER_OPERATIONS,
  VOXEL_OBJECT_COMPONENT_TYPE_ID,
  VOXEL_OBJECT_INSPECTOR_CONTRACT_ID,
  VOXEL_OBJECT_INSPECTOR_CONTRACT_VERSION,
  decodeStudioAdapterResponse,
  type StudioAdapterRequest,
  type StudioAdapterTransport,
} from './index.js';

test('decodes the closed project response and delegates projection validation', () => {
  const response = projectOpened('request-1');
  response.project.identity.projectId = 'intentionally not semantically revalidated';
  response.project.animatedMeshResources = [{
    asset: 'mesh-animation/character',
    contentHash: `sha256:${'1'.repeat(64)}`,
    clipIds: ['idle', 'run'],
    sourcePath: 'content/assets/character.glb',
  }];
  response.project.meshResources = [{
    resource: `mesh-resource/${'2'.repeat(64)}`,
    contentHash: `sha256:${'2'.repeat(64)}`,
    byteLength: 1024,
    sourcePath: `content/render/${'2'.repeat(64)}.rmesh`,
  }];

  const decoded = decodeStudioAdapterResponse(response);

  assert.equal(decoded.type, 'projectOpened');
  assert.equal(
    decoded.project.identity.projectId,
    'intentionally not semantically revalidated',
  );
  assert.equal(decoded.project.projection.ops.length, 0);
  assert.deepEqual(decoded.project.animatedMeshResources[0]?.clipIds, ['idle', 'run']);
  assert.equal(decoded.project.meshResources?.[0]?.byteLength, 1024);
});

test('rejects unknown response families, extra fields, and malformed renderer frames', () => {
  assert.throws(
    () => decodeStudioAdapterResponse({ type: 'sendMessage' }),
    StudioAdapterDecodeError,
  );
  assert.throws(
    () =>
      decodeStudioAdapterResponse({
        type: 'projectClosed',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: 'close-1',
        ambientState: true,
      }),
    /ambientState.*unknown/,
  );

  const response = projectOpened('request-2');
  response.project.projection = { schemaVersion: 99, ops: [] };
  assert.throws(
    () => decodeStudioAdapterResponse(response),
    /projection.*schemaVersion.*must equal 1/,
  );

  const malformedHierarchy = projectOpened('request-3');
  malformedHierarchy.project.sceneHierarchy.rootNodeIds = [1.5];
  assert.throws(
    () => decodeStudioAdapterResponse(malformedHierarchy),
    /sceneHierarchy\.rootNodeIds\[0\].*integer/,
  );

  const incrementalFrame = projectOpened('request-4');
  incrementalFrame.project.projectionReadout.frameKind = 'incremental';
  assert.throws(
    () => decodeStudioAdapterResponse(incrementalFrame),
    /frameKind.*complete/,
  );

  const malformedResource = projectOpened('request-5');
  malformedResource.project.animatedMeshResources = [{
    asset: 'mesh-animation/character',
    contentHash: `sha256:${'1'.repeat(64)}`,
    clipIds: ['idle'],
    sourcePath: 'content/assets/character.glb',
    ambientUrl: 'file:///untrusted.glb',
  }];
  assert.throws(
    () => decodeStudioAdapterResponse(malformedResource),
    /animatedMeshResources\[0\]\.ambientUrl.*unknown/,
  );

  const malformedMeshResource = projectOpened('request-6');
  malformedMeshResource.project.meshResources = [{
    resource: `mesh-resource/${'2'.repeat(64)}`,
    contentHash: `sha256:${'3'.repeat(64)}`,
    byteLength: 1024,
    sourcePath: 'content/render/mismatch.rmesh',
  }];
  assert.throws(
    () => decodeStudioAdapterResponse(malformedMeshResource),
    /content-addressed mesh resource/u,
  );
});

test('decodes bounded identity-only component references and rejects malformed ownership', () => {
  const response = projectOpened('components-1');
  setCanonicalOwners(response, [17]);
  response.project.entityComponents = [
    {
      ownerEntityId: 17,
      componentTypeId: 'rusty-engine-demo.loading-bay.weapon',
      inspectorContract: {
        contractId: 'rusty-engine-demo.loading-bay.weapon-authoring',
        contractVersion: 1,
      },
    },
    {
      ownerEntityId: 17,
      componentTypeId: 'vendor.runtime-only-observation',
      inspectorContract: null,
    },
  ];

  const decoded = decodeStudioAdapterResponse(response);
  assert.equal(decoded.type, 'projectOpened');
  assert.equal(
    decoded.type === 'projectOpened'
      ? decoded.project.entityComponents[1]?.inspectorContract
      : undefined,
    null,
    'unknown components remain visible as identity-only rows',
  );

  const duplicate = structuredClone(response);
  duplicate.project.entityComponents.push(entityComponentAt(duplicate, 0));
  assert.throws(
    () => decodeStudioAdapterResponse(duplicate),
    /duplicates an entity\/component reference/u,
  );

  const absentFromInspection = structuredClone(response);
  entityStateInspection(absentFromInspection)['entityIds'] = [];
  assert.throws(
    () => decodeStudioAdapterResponse(absentFromInspection),
    /absent from canonical entity inspection/u,
  );

  const absentFromHierarchy = structuredClone(response);
  absentFromHierarchy.project.sceneHierarchy.nodes = [];
  assert.throws(
    () => decodeStudioAdapterResponse(absentFromHierarchy),
    /absent from the canonical scene hierarchy/u,
  );

  const extraField = structuredClone(response);
  entityComponentAt(extraField, 0)['fieldSchema'] = {};
  assert.throws(
    () => decodeStudioAdapterResponse(extraField),
    /fieldSchema.*unknown/u,
  );
});

test('validates inspector contract advertisement and version consistency across responses', async () => {
  const contract = {
    contractId: 'rusty-engine-demo.loading-bay.weapon-authoring',
    contractVersion: 1,
  };
  const transport = new RecordingTransport((request) => {
    if (request.type === 'describe') {
      return described(request.requestId, [contract]);
    }
    if (request.type !== 'openProject') throw new Error('unexpected operation');
    const opened = projectOpened(request.requestId);
    setCanonicalOwners(opened, [17]);
    opened.project.entityComponents = [{
      ownerEntityId: 17,
      componentTypeId: 'rusty-engine-demo.loading-bay.weapon',
      inspectorContract: contract,
    }];
    return opened;
  });
  const client = new StudioAdapterClient(transport);

  await client.describe();
  const opened = await client.openProject('/trusted/project', 'project.json');
  assert.equal(opened.project.entityComponents[0]?.inspectorContract?.contractVersion, 1);

  const mismatchedTransport = new RecordingTransport((request) => {
    if (request.type === 'describe') {
      return described(request.requestId, [contract]);
    }
    if (request.type !== 'openProject') throw new Error('unexpected operation');
    const openedResponse = projectOpened(request.requestId);
    setCanonicalOwners(openedResponse, [17]);
    openedResponse.project.entityComponents = [{
      ownerEntityId: 17,
      componentTypeId: 'rusty-engine-demo.loading-bay.weapon',
      inspectorContract: { ...contract, contractVersion: 2 },
    }];
    return openedResponse;
  });
  const mismatchedClient = new StudioAdapterClient(mismatchedTransport);
  await mismatchedClient.describe();
  await assert.rejects(
    mismatchedClient.openProject('/trusted/project', 'project.json'),
    /not advertised by the current adapter with the same version/u,
  );

  const undescribed = projectOpened('undescribed-contract');
  setCanonicalOwners(undescribed, [17]);
  undescribed.project.entityComponents = [{
    ownerEntityId: 17,
    componentTypeId: 'vendor.component',
    inspectorContract: contract,
  }];
  const undescribedClient = new StudioAdapterClient(
    new RecordingTransport((request) => ({
      ...undescribed,
      requestId: request.requestId,
    })),
  );
  await assert.rejects(
    undescribedClient.openProject('/trusted/project', 'project.json'),
    /require a prior described adapter/u,
  );
});

test('enforces exact identity, contract, total reference, and per-owner limits', () => {
  const identityAtLimit = `a${'b'.repeat(MAX_STUDIO_ENTITY_INSPECTOR_IDENTITY_BYTES - 1)}`;
  const identityOneOver = `${identityAtLimit}c`;
  const exactContractDescription = described(
    'contracts-exact',
    Array.from({ length: MAX_STUDIO_ENTITY_INSPECTOR_CONTRACTS }, (_, index) => ({
      contractId: `contract.${String(index)}`,
      contractVersion: 1,
    })),
  );
  assert.equal(decodeStudioAdapterResponse(exactContractDescription).type, 'described');
  const tooManyContracts = structuredClone(exactContractDescription);
  tooManyContracts.adapter.entityInspectorContracts.push({
    contractId: 'contract.one-over',
    contractVersion: 1,
  });
  assert.throws(
    () => decodeStudioAdapterResponse(tooManyContracts),
    /at most 64 contracts/u,
  );

  const invalidVersion = described('contracts-version', [{
    contractId: 'contract.zero',
    contractVersion: 0,
  }]);
  assert.throws(
    () => decodeStudioAdapterResponse(invalidVersion),
    /contractVersion.*positive/u,
  );

  const duplicateContract = described('contracts-duplicate', [
    { contractId: 'contract.same', contractVersion: 1 },
    { contractId: 'contract.same', contractVersion: 1 },
  ]);
  assert.throws(
    () => decodeStudioAdapterResponse(duplicateContract),
    /duplicates an advertised inspector contract/u,
  );

  const invalidContractField = described('contracts-closed', [{
    contractId: 'contract.closed',
    contractVersion: 1,
    moduleUrl: 'https://invalid.example/plugin.js',
  }]);
  assert.throws(
    () => decodeStudioAdapterResponse(invalidContractField),
    /moduleUrl.*unknown/u,
  );

  const ownerCount = Math.ceil(
    MAX_STUDIO_ENTITY_COMPONENT_REFERENCES / MAX_STUDIO_ENTITY_COMPONENTS_PER_OWNER,
  );
  const owners = Array.from({ length: ownerCount + 1 }, (_, index) => index + 1);
  const exactReferences = projectOpened('references-exact');
  setCanonicalOwners(exactReferences, owners);
  exactReferences.project.entityComponents = Array.from(
    { length: MAX_STUDIO_ENTITY_COMPONENT_REFERENCES },
    (_, index) => ({
      ownerEntityId: Math.floor(index / MAX_STUDIO_ENTITY_COMPONENTS_PER_OWNER) + 1,
      componentTypeId: `component.${String(index)}`,
      inspectorContract: null,
    }),
  );
  entityComponentAt(exactReferences, 0)['componentTypeId'] = identityAtLimit;
  assert.equal(decodeStudioAdapterResponse(exactReferences).type, 'projectOpened');

  const identityTooLong = structuredClone(exactReferences);
  entityComponentAt(identityTooLong, 0)['componentTypeId'] = identityOneOver;
  assert.throws(
    () => decodeStudioAdapterResponse(identityTooLong),
    /1\.\.=128 ASCII bytes/u,
  );

  const badIdentitySyntax = structuredClone(exactReferences);
  entityComponentAt(badIdentitySyntax, 0)['componentTypeId'] = 'Rust.Component';
  assert.throws(
    () => decodeStudioAdapterResponse(badIdentitySyntax),
    /lowercase ASCII letter/u,
  );

  const tooManyReferences = structuredClone(exactReferences);
  tooManyReferences.project.entityComponents.push({
    ownerEntityId: owners.at(-1) as number,
    componentTypeId: 'component.one-over',
    inspectorContract: null,
  });
  assert.throws(
    () => decodeStudioAdapterResponse(tooManyReferences),
    /at most 4096 references/u,
  );

  const perOwnerExact = projectOpened('per-owner-exact');
  setCanonicalOwners(perOwnerExact, [1]);
  perOwnerExact.project.entityComponents = Array.from(
    { length: MAX_STUDIO_ENTITY_COMPONENTS_PER_OWNER },
    (_, index) => ({
      ownerEntityId: 1,
      componentTypeId: `component.${String(index)}`,
      inspectorContract: null,
    }),
  );
  assert.equal(decodeStudioAdapterResponse(perOwnerExact).type, 'projectOpened');
  perOwnerExact.project.entityComponents.push({
    ownerEntityId: 1,
    componentTypeId: 'component.one-over',
    inspectorContract: null,
  });
  assert.throws(
    () => decodeStudioAdapterResponse(perOwnerExact),
    /exceeds 32 component references/u,
  );
});

test('named client methods emit only closed operations and correlate responses', async () => {
  const transport = new RecordingTransport((request) => {
    assert.equal(request.type, 'openProject');
    if (request.type !== 'openProject') throw new Error('unexpected operation');
    assert.equal(request.root, '/trusted/project');
    assert.equal(request.projectFile, 'content/projects/loading-bay.project.json');
    return projectOpened(request.requestId);
  });
  const client = new StudioAdapterClient(transport);

  const opened = await client.openProject(
    '/trusted/project',
    'content/projects/loading-bay.project.json',
  );

  assert.equal(opened.project.identity.projectId, 'loading-bay');
  assert.deepEqual(
    transport.requests.map((request) => request.type),
    ['openProject'],
  );
});

test('typed rejection becomes an operation error without interpreting Rust semantics', async () => {
  const transport = new RecordingTransport((request) => ({
    type: 'rejected',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: request.requestId,
    error: {
      code: 'project.staleHash',
      path: 'content/projects/loading-bay.project.json',
      message: 'source changed',
    },
  }));
  const client = new StudioAdapterClient(transport);

  await assert.rejects(
    client.readProject(),
    (error: unknown) =>
      error instanceof StudioAdapterOperationRejected &&
      error.rejection.code === 'project.staleHash',
  );
});

test('a correlated rejection must belong to the request being completed', async () => {
  const transport = new RecordingTransport(() => ({
    type: 'rejected',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'another-request',
    error: {
      code: 'project.staleHash',
      message: 'source changed',
    },
  }));
  const client = new StudioAdapterClient(transport);

  await assert.rejects(
    client.readProject(),
    (error: unknown) =>
      error instanceof Error &&
      !(error instanceof StudioAdapterOperationRejected) &&
      /requestId .* did not match/.test(error.message),
  );
});

test('voxel response families are closed and named authoring calls preserve guards', async () => {
  const pick = {
    type: 'voxelPickValidated',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'pick-1',
    anchor: {
      sceneId: 'scene/converted-wall',
      instanceId: 'wall-primary',
      assetId: 'voxel-volume/kenney-wall-a',
      hitVoxel: [0, 0, 0],
      hitFace: 'positiveZ',
      placeVoxel: [0, 0, 1],
      authorityHitVoxel: [4, 0, 6],
      authorityPlaceVoxel: [4, 0, 7],
      instanceLocalPoint: [4.5, 0.5, 7],
      worldPoint: [4.5, 0.5, 7],
      worldDistance: 12,
      hitPreviewTransform: {
        translation: [4.5, 0.5, 6.5],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
      placePreviewTransform: {
        translation: [4.5, 0.5, 7.5],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
      },
    },
  };
  assert.equal(decodeStudioAdapterResponse(pick).type, 'voxelPickValidated');
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...pick,
      anchor: { ...pick.anchor, ambientRendererState: true },
    }),
    (error: unknown) =>
      error instanceof StudioAdapterDecodeError
      && /ambientRendererState.*unknown/.test(error.message),
  );

  const transport = new RecordingTransport((request) => {
    assert.equal(request.type, 'applyVoxelBrush');
    if (request.type !== 'applyVoxelBrush') throw new Error('unexpected operation');
    assert.equal(request.expectedProjectHash, '11'.repeat(32));
    assert.equal(request.expectedAssetContentHash, 'sha256:asset-before');
    return {
      type: 'projectMutationApplied',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      requestId: request.requestId,
      receipt: {
        kind: 'voxelBrushApplied',
        assetId: request.assetId,
        contentHashBefore: request.expectedAssetContentHash,
        contentHashAfter: 'sha256:asset-after',
        changedVoxels: 1,
        sourceRevision: 2,
        historyCursor: 1,
        undoDepth: 1,
        redoDepth: 0,
      },
      project: projectOpened(request.requestId).project,
    };
  });
  const client = new StudioAdapterClient(transport);
  const applied = await client.applyVoxelBrush({
    expectedProjectHash: '11'.repeat(32),
    assetId: 'voxel-volume/kenney-wall-a',
    expectedAssetContentHash: 'sha256:asset-before',
    center: [0, 0, 0],
    radius: 0,
    mode: 'erase',
    materialSlot: null,
  });
  assert.equal(applied.receipt.kind, 'voxelBrushApplied');
});

test('protocol 7 closes history, file, and texture-policy response families', () => {
  const history = decodeStudioAdapterResponse({
    type: 'voxelRead',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'history-1',
    readout: {
      kind: 'history',
      assetId: 'voxel-volume/wall',
      cursor: 1,
      undoDepth: 1,
      redoDepth: 0,
      entryCount: 1,
      entriesTruncated: false,
      entries: [{
        transactionId: 1,
        parentTransactionId: null,
        beforeHash: 'before',
        afterHash: 'after',
        changedVoxels: 1,
        deltasTruncated: false,
        deltas: [{ address: [0, 0, 0], beforeMaterial: 7, afterMaterial: null }],
      }],
    },
  });
  assert.equal(history.type, 'voxelRead');

  const prepared = decodeStudioAdapterResponse({
    type: 'voxelHistoryRevertPrepared',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'history-2',
    preview: {
      previewId: 'history-preview-1',
      assetId: 'voxel-volume/wall',
      expectedProjectHash: 'project',
      expectedAssetContentHash: 'asset',
      cursorBefore: 1,
      cursorAfter: 0,
      undoDepthAfter: 0,
      redoDepthAfter: 1,
      revisionBefore: 1,
      revisionAfter: 2,
      changedVoxels: 1,
      bounds: { min: [0, 0, 0], max: [0, 0, 0] },
      materialDeltas: [{ beforeMaterial: 7, afterMaterial: null, changedVoxels: 1 }],
      samples: [{ address: [0, 0, 0], beforeMaterial: 7, afterMaterial: null }],
      samplesTruncated: false,
      includedTransactionIds: [1],
    },
  });
  assert.equal(prepared.type, 'voxelHistoryRevertPrepared');

  assert.equal(decodeStudioAdapterResponse({
    type: 'voxelAssetFileExported',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'file-1',
    assetId: 'voxel-volume/wall',
    targetPath: '/tmp/wall.voxel.json',
    byteCount: 42,
    sha256: 'sha256:file',
    replacedExisting: false,
  }).type, 'voxelAssetFileExported');

  const conversion = conversionPrepared('conversion-1');
  const texture = conversion.plan.settings.materialPolicy.textureAssets[0];
  assert.ok(texture !== undefined);
  const malformedConversion = {
    ...conversion,
    plan: {
      ...conversion.plan,
      settings: {
        ...conversion.plan.settings,
        materialPolicy: {
          ...conversion.plan.settings.materialPolicy,
          textureAssets: [{ ...texture, ambientBrowserTexture: true }],
        },
      },
    },
  };
  assert.throws(
    () => decodeStudioAdapterResponse(malformedConversion),
    /ambientBrowserTexture.*unknown/,
  );
});

test('protocol 15 keeps entity-owned voxel objects, applied playback, and durable readouts closed', async () => {
  const inspected = voxelObjectSourceInspected('object-source-1');
  assert.equal(decodeStudioAdapterResponse(inspected).type, 'voxelObjectSourceInspected');
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...inspected,
      inspection: { ...inspected.inspection, browserUrl: 'blob:ambient' },
    }),
    /browserUrl.*unknown/,
  );

  const prepared = voxelObjectConversionPrepared('object-plan-1');
  const decoded = decodeStudioAdapterResponse(prepared);
  assert.equal(decoded.type, 'voxelObjectConversionPrepared');
  assert.equal(decoded.preview.selectedFrame.voxelCount, 1);
  assert.equal(decoded.projection.ops[0]?.op, 'defineVoxelObject');
  const digest = '4'.repeat(64);
  const decodedWithResources = decodeStudioAdapterResponse({
    ...prepared,
    meshResources: [{
      resource: `mesh-resource/${digest}`,
      contentHash: `sha256:${digest}`,
      byteLength: 1024,
      sourcePath: `target/studio-render-resources/${digest}.rmesh`,
    }],
  });
  assert.equal(
    decodedWithResources.type === 'voxelObjectConversionPrepared'
      ? decodedWithResources.meshResources?.[0]?.byteLength
      : null,
    1024,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...prepared,
      preview: {
        ...prepared.preview,
        selectedFrame: { ...prepared.preview.selectedFrame, computedByAngular: true },
      },
    }),
    /computedByAngular.*unknown/,
  );

  const project = projectOpened('object-project-1');
  setCanonicalOwners(project, [17]);
  project.project.voxelObjectAuthoring.assets = [voxelObjectAssetReadout()];
  project.project.voxelObjectAuthoring.instances = [{
    sceneId: 'scene/loading-bay',
    ownerEntityId: 17,
    instance: {
      instanceId: 'character-one',
      voxelObjectAssetId: 'voxel-object/character',
      surfaceMode: 'greedyCubes',
      frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 0 },
      translation: [0, 0, 0],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
      materialOverrides: [],
    },
  }];
  project.project.entityComponents = [{
    ownerEntityId: 17,
    componentTypeId: VOXEL_OBJECT_COMPONENT_TYPE_ID,
    inspectorContract: {
      contractId: VOXEL_OBJECT_INSPECTOR_CONTRACT_ID,
      contractVersion: VOXEL_OBJECT_INSPECTOR_CONTRACT_VERSION,
    },
  }];
  assert.equal(decodeStudioAdapterResponse(project).type, 'projectOpened');
  const decodedProject = decodeStudioAdapterResponse(project);
  assert.equal(
    decodedProject.type === 'projectOpened'
      ? decodedProject.project.voxelObjectAuthoring.instances[0]?.ownerEntityId
      : null,
    17,
  );
  project.project.voxelObjectAuthoring.assets[0] = {
    ...voxelObjectAssetReadout(),
    updateCallback: 'tick',
  };
  assert.throws(() => decodeStudioAdapterResponse(project), /updateCallback.*unknown/);

  const playback = voxelObjectInstancePreviewed('object-playback-1');
  const decodedPlayback = decodeStudioAdapterResponse(playback);
  assert.equal(decodedPlayback.type, 'voxelObjectInstancePreviewed');
  assert.equal(decodedPlayback.playback.durableFrame.kind, 'clip');
  assert.equal(decodedPlayback.playback.runtimeFrame, 2);
  const incrementalPlayback = {
    ...playback,
    projection: {
      schemaVersion: 1,
      ops: [{ op: 'setVoxelObjectFrame', handle: 101, frame: 1 }],
    },
    projectionReadout: {
      ...playback.projectionReadout,
      frameKind: 'incremental',
    },
  };
  const decodedIncremental = decodeStudioAdapterResponse(incrementalPlayback);
  assert.equal(decodedIncremental.type, 'voxelObjectInstancePreviewed');
  assert.equal(decodedIncremental.projectionReadout.frameKind, 'incremental');
  const preparedWithIncrementalReadout = voxelObjectConversionPrepared('object-incremental-1');
  preparedWithIncrementalReadout.projectionReadout.frameKind = 'incremental';
  assert.throws(
    () => decodeStudioAdapterResponse(preparedWithIncrementalReadout),
    /frameKind.*complete/,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...playback,
      playback: { ...playback.playback, browserTimer: 17 },
    }),
    /browserTimer.*unknown/,
  );

  const transport = new RecordingTransport((request) => {
    if (request.type === 'inspectVoxelObjectSource') {
      assert.equal(request.sourceKind, 'animated');
      assert.equal(request.source.scope, 'host');
      return voxelObjectSourceInspected(request.requestId);
    }
    if (request.type === 'previewVoxelObjectConversion') {
      assert.deepEqual(request.frame, { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 });
      const response = voxelObjectConversionPrepared(request.requestId);
      return {
        type: 'voxelObjectConversionPreviewed',
        protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
        requestId: request.requestId,
        preview: response.preview,
        projection: response.projection,
        projectionReadout: response.projectionReadout,
      };
    }
    if (request.type === 'previewVoxelObjectInstance') {
      assert.deepEqual(request.command, {
        kind: 'scrub',
        clipId: 'clip/walk-1',
        clipFrame: 0,
        loopMode: 'repeat',
      });
      return voxelObjectInstancePreviewed(request.requestId);
    }
    throw new Error(`unexpected ${request.type}`);
  });
  const client = new StudioAdapterClient(transport);
  await client.inspectVoxelObjectSource({
    expectedProjectHash: 'project-hash',
    sourceKind: 'animated',
    sourceAssetId: 'mesh-animation/character',
    source: { scope: 'host', path: '/trusted/character.glb' },
  });
  await client.previewVoxelObjectConversion({
    planId: 'plan/object',
    expectedPlanHash: 'sha256:plan',
    frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
    maxPreviewSamples: 128,
  });
  await client.previewVoxelObjectInstance({
    expectedProjectHash: 'project-hash',
    sceneId: 'scene/loading-bay',
    instanceId: 'character-one',
    nowMicroseconds: 1_000_000,
    command: {
      kind: 'scrub',
      clipId: 'clip/walk-1',
      clipFrame: 0,
      loopMode: 'repeat',
    },
  });
  assert.deepEqual(transport.requests.map((request) => request.type), [
    'inspectVoxelObjectSource', 'previewVoxelObjectConversion', 'previewVoxelObjectInstance',
  ]);
});

test('protocol 15 carries one closed authoritative voxel-object surface-mode mutation', async () => {
  const project = projectOpened('surface-mode-project');
  setCanonicalOwners(project, [17]);
  project.project.voxelObjectAuthoring.assets = [voxelObjectAssetReadout()];
  project.project.voxelObjectAuthoring.instances = [{
    sceneId: 'scene/loading-bay',
    ownerEntityId: 17,
    instance: {
      instanceId: 'character-one',
      voxelObjectAssetId: 'voxel-object/character',
      surfaceMode: 'marchingCubes',
      frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 0 },
      translation: [0, 0, 0],
      rotation: [0, 0, 0, 1],
      scale: [1, 1, 1],
      materialOverrides: [],
    },
  }];
  project.project.entityComponents = [{
    ownerEntityId: 17,
    componentTypeId: VOXEL_OBJECT_COMPONENT_TYPE_ID,
    inspectorContract: {
      contractId: VOXEL_OBJECT_INSPECTOR_CONTRACT_ID,
      contractVersion: VOXEL_OBJECT_INSPECTOR_CONTRACT_VERSION,
    },
  }];
  const applied = {
    type: 'projectMutationApplied' as const,
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'surface-mode-1',
    receipt: {
      kind: 'voxelObjectSurfaceModeSet' as const,
      sceneId: 'scene/loading-bay',
      instanceId: 'character-one',
      before: 'greedyCubes' as const,
      after: 'marchingCubes' as const,
    },
    project: project.project,
  };
  const decoded = decodeStudioAdapterResponse(applied);
  assert.equal(decoded.type, 'projectMutationApplied');
  assert.equal(
    decoded.type === 'projectMutationApplied'
      && decoded.receipt.kind === 'voxelObjectSurfaceModeSet'
      ? decoded.receipt.after
      : null,
    'marchingCubes',
  );
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...applied,
      receipt: { ...applied.receipt, after: 'smoothEnough' },
    }),
    /receipt\.after is not a closed value/u,
  );

  const transport = new RecordingTransport((request) => {
    if (request.type === 'describe') {
      return described(request.requestId, [{
        contractId: VOXEL_OBJECT_INSPECTOR_CONTRACT_ID,
        contractVersion: VOXEL_OBJECT_INSPECTOR_CONTRACT_VERSION,
      }]);
    }
    assert.equal(request.type, 'setVoxelObjectInstanceSurfaceMode');
    if (request.type !== 'setVoxelObjectInstanceSurfaceMode') throw new Error('unexpected operation');
    assert.equal(request.expectedProjectHash, 'project-hash');
    assert.equal(request.sceneId, 'scene/loading-bay');
    assert.equal(request.instanceId, 'character-one');
    assert.equal(request.surfaceMode, 'marchingCubes');
    return { ...applied, requestId: request.requestId };
  });
  const client = new StudioAdapterClient(transport);
  await client.describe();
  await client.setVoxelObjectInstanceSurfaceMode({
    expectedProjectHash: 'project-hash',
    sceneId: 'scene/loading-bay',
    instanceId: 'character-one',
    surfaceMode: 'marchingCubes',
  });
  assert.deepEqual(transport.requests.map((request) => request.type), [
    'describe', 'setVoxelObjectInstanceSurfaceMode',
  ]);
});

test('protocol 14 placement preparation carries one bounded resource-only voxel object', async () => {
  const conversion = voxelObjectConversionPrepared('placement-source');
  const objectContentHash = `sha256:${'5'.repeat(64)}`;
  const resourceFrame = {
    schemaVersion: 1 as const,
    ops: conversion.projection.ops.map((operation) => operation.op === 'defineVoxelObject'
      ? { ...operation, asset: { ...operation.asset, contentHash: objectContentHash } }
      : operation),
  };
  const prepared = {
    type: 'voxelObjectPlacementPrepared',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'placement-1',
    assetId: 'voxel-object/character',
    objectContentHash,
    resourceFrame,
  };
  const decoded = decodeStudioAdapterResponse(prepared);
  assert.equal(decoded.type, 'voxelObjectPlacementPrepared');
  assert.equal(decoded.resourceFrame.ops[0]?.op, 'defineVoxelObject');

  assert.throws(
    () => decodeStudioAdapterResponse({
      ...prepared,
      resourceFrame: {
        schemaVersion: 1,
        ops: [...prepared.resourceFrame.ops, { op: 'setVoxelObjectFrame', handle: 1, frame: 0 }],
      },
    }),
    /must be defineMaterial, defineTexture, or defineVoxelObject/u,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...prepared,
      resourceFrame: {
        schemaVersion: 1,
        ops: [...prepared.resourceFrame.ops, ...prepared.resourceFrame.ops],
      },
    }),
    /exactly one matching defineVoxelObject/u,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...prepared,
      assetId: 'voxel-object/other',
    }),
    /asset\.asset.*must match/u,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...prepared,
      objectContentHash: `sha256:${'6'.repeat(64)}`,
    }),
    /asset\.contentHash.*must match/u,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...prepared,
      ambientRegistry: true,
    }),
    /ambientRegistry.*unknown/u,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({ ...prepared, assetId: 'x'.repeat(129) }),
    /1\.\.=128 printable ASCII bytes/u,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({ ...prepared, objectContentHash: 'sha256:not-a-digest' }),
    /lowercase SHA-256 content identity/u,
  );

  const transport = new RecordingTransport((request) => {
    assert.equal(request.type, 'prepareVoxelObjectPlacement');
    if (request.type !== 'prepareVoxelObjectPlacement') throw new Error('unexpected operation');
    assert.equal(request.expectedProjectHash, 'project-hash');
    assert.equal(request.assetId, prepared.assetId);
    assert.equal(request.expectedObjectContentHash, prepared.objectContentHash);
    return { ...prepared, requestId: request.requestId };
  });
  const client = new StudioAdapterClient(transport);
  await client.prepareVoxelObjectPlacement({
    expectedProjectHash: 'project-hash',
    assetId: prepared.assetId,
    expectedObjectContentHash: prepared.objectContentHash,
  });
  assert.deepEqual(transport.requests.map((request) => request.type), [
    'prepareVoxelObjectPlacement',
  ]);
});

test('protocol 14 carries one closed bounded voxel-object placement batch and one readout', async () => {
  const project = projectOpened('batch-project');
  const owners = Array.from(
    { length: MAX_VOXEL_OBJECT_INSTANCE_BATCH },
    (_, index) => index + 1,
  );
  setCanonicalOwners(project, owners);
  const placements = owners.map((ownerEntityId, index) => ({
    sceneId: 'scene/loading-bay',
    instanceId: `batch-${String(index + 1)}`,
    assetId: 'voxel-object/character',
    frameKind: 'clip' as const,
    ownerEntityId,
  }));
  const applied = {
    type: 'projectMutationApplied',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'batch-1',
    receipt: {
      kind: 'voxelObjectInstancesAttached',
      placements,
    },
    project: project.project,
  };
  const decoded = decodeStudioAdapterResponse(applied);
  assert.equal(decoded.type, 'projectMutationApplied');
  assert.equal(
    decoded.type === 'projectMutationApplied'
      && decoded.receipt.kind === 'voxelObjectInstancesAttached'
      ? decoded.receipt.placements.length
      : 0,
    MAX_VOXEL_OBJECT_INSTANCE_BATCH,
  );

  assert.throws(
    () => decodeStudioAdapterResponse({
      ...applied,
      receipt: {
        kind: 'voxelObjectInstancesAttached',
        placements: [...placements, {
          ...placements[0],
          instanceId: 'batch-one-over',
          ownerEntityId: MAX_VOXEL_OBJECT_INSTANCE_BATCH + 1,
        }],
      },
    }),
    /must contain 1\.\.=32 placements/u,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...applied,
      receipt: {
        kind: 'voxelObjectInstancesAttached',
        placements: [placements[0], { ...placements[1], instanceId: placements[0]?.instanceId }],
      },
    }),
    /instanceId duplicates/u,
  );
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...applied,
      receipt: {
        kind: 'voxelObjectInstancesAttached',
        placements: [placements[0], { ...placements[1], ownerEntityId: placements[0]?.ownerEntityId }],
      },
    }),
    /ownerEntityId duplicates/u,
  );

  const transport = new RecordingTransport((request) => {
    assert.equal(request.type, 'attachVoxelObjectInstances');
    if (request.type !== 'attachVoxelObjectInstances') throw new Error('unexpected operation');
    assert.equal(request.expectedProjectHash, 'project-hash');
    assert.equal(request.placements.length, MAX_VOXEL_OBJECT_INSTANCE_BATCH);
    assert.deepEqual(
      request.placements.map((placement) => placement.instance.instanceId),
      placements.map((placement) => placement.instanceId),
    );
    return { ...applied, requestId: request.requestId };
  });
  const client = new StudioAdapterClient(transport);
  await client.attachVoxelObjectInstances({
    expectedProjectHash: 'project-hash',
    placements: placements.map((placement, index) => ({
      sceneId: placement.sceneId,
      instance: {
        instanceId: placement.instanceId,
        voxelObjectAssetId: placement.assetId,
        surfaceMode: 'greedyCubes',
        frame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 0 },
        translation: [index, 0, 0],
        rotation: [0, 0, 0, 1],
        scale: [1, 1, 1],
        materialOverrides: [],
      },
    })),
  });
  assert.deepEqual(transport.requests.map((request) => request.type), [
    'attachVoxelObjectInstances',
  ]);
});

class RecordingTransport implements StudioAdapterTransport {
  readonly requests: StudioAdapterRequest[] = [];
  readonly #respond: (request: StudioAdapterRequest) => unknown;

  constructor(respond: (request: StudioAdapterRequest) => unknown) {
    this.#respond = respond;
  }

  exchange(request: StudioAdapterRequest): Promise<unknown> {
    this.requests.push(request);
    return Promise.resolve(this.#respond(request));
  }
}

test('asset import plans, browser provenance, and named client calls stay closed', async () => {
  const prepared = assetImportPrepared('asset-prepare-1');
  const decoded = decodeStudioAdapterResponse(prepared);
  assert.equal(decoded.type, 'assetImportPrepared');
  assert.equal(decoded.plan.meshAssetId, 'mesh/studio-triangle');
  const animated = decodeStudioAdapterResponse({
    ...prepared,
    plan: {
      ...prepared.plan,
      source: { scope: 'project', path: 'content/assets/actor-medium.glb' },
      meshAssetId: 'mesh-animation/actor-medium',
      generatedArtifacts: [
        { relativePath: 'actor-medium.animated-mesh.json', byteCount: 512 },
        { relativePath: 'actor-medium.glb', byteCount: 335_000 },
      ],
      generatedAssetIds: ['mesh-animation/actor-medium'],
    },
  });
  assert.equal(animated.type, 'assetImportPrepared');
  assert.equal(animated.plan.meshAssetId, 'mesh-animation/actor-medium');
  assert.throws(
    () => decodeStudioAdapterResponse({
      ...prepared,
      plan: { ...prepared.plan, callback: 'run-me' },
    }),
    /callback.*unknown/,
  );

  const transport = new RecordingTransport((request) => {
    assert.equal(request.type, 'prepareAssetImport');
    if (request.type !== 'prepareAssetImport') throw new Error('unexpected operation');
    assert.equal(request.source.scope, 'project');
    assert.equal(request.settings.materialNamespace, 'studio');
    return assetImportPrepared(request.requestId);
  });
  const client = new StudioAdapterClient(transport);
  const response = await client.prepareAssetImport({
    expectedProjectHash: '00'.repeat(32),
    source: { scope: 'project', path: 'content/assets/studio-triangle.mesh.json' },
    settings: { scale: 1, generateCollision: false, materialNamespace: 'studio' },
  });
  assert.equal(response.plan.hasErrors, false);
});

test('protocol 14 admits exact Rust-owned voxel surface resources and rejects drift', () => {
  const opened = projectOpened('surface-project');
  const digest = 'a'.repeat(64);
  opened.project.textureResources = [{
    resource: `texture-resource/${digest}`,
    contentHash: `sha256:${digest}`,
    byteLength: 82,
    sourcePath: `.rusty-engine/textures/${digest}.png`,
  }];
  opened.project.voxelSurfaceAuthoring = {
    textures: [{
      textureAssetId: 'texture/voxel/checker',
      version: 1,
      contentHash: `sha256:${digest}`,
      sourcePath: `.rusty-engine/textures/${digest}.png`,
      width: 2,
      height: 2,
      encodedByteLength: 82,
      filter: 'nearest',
      wrap: 'clamp',
    }],
    atlases: [{
      atlasAssetId: 'sprite-sheet/voxel/checker',
      version: 1,
      contentHash: `sha256:${'b'.repeat(64)}`,
      textureAssetId: 'texture/voxel/checker',
      textureVersion: 1,
      textureContentHash: `sha256:${digest}`,
      regions: [{
        id: 'left',
        contentMin: [0, 0],
        contentExtent: [1, 2],
        padding: { left: 0, right: 1, bottom: 0, top: 0 },
        inset: 'halfTexel',
      }],
    }],
    materials: [{
      materialAssetId: 'material/voxel/checker',
      version: 1,
      contentHash: `sha256:${'c'.repeat(64)}`,
      definition: surfaceMaterialDefinition(),
      textureAssetId: 'texture/voxel/checker',
      textureVersion: 1,
      textureContentHash: `sha256:${digest}`,
      alphaMode: { kind: 'opaque' },
      mapping: {
        kind: 'atlas',
        atlasAssetId: 'sprite-sheet/voxel/checker',
        atlasVersion: 1,
        atlasContentHash: `sha256:${'b'.repeat(64)}`,
        regionId: 'left',
        tileScaleCells: [0.5, 2],
        tileOriginCells: [0.25, -0.5],
      },
      assignments: [{ sceneId: 'scene/loading-bay', instanceId: 'fixture', materialSlot: 1 }],
    }],
  };
  const decoded = decodeStudioAdapterResponse(opened);
  assert.equal(decoded.type, 'projectOpened');
  if (decoded.type !== 'projectOpened') throw new Error('unexpected response');
  assert.equal(decoded.project.voxelSurfaceAuthoring.materials[0]?.mapping.kind, 'atlas');

  const applied = decodeStudioAdapterResponse({
    type: 'projectMutationApplied',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'surface-applied',
    receipt: {
      kind: 'voxelSurfaceMaterialUpserted',
      textureAssetId: 'texture/voxel/checker',
      textureContentHash: `sha256:${digest}`,
      materialAssetId: 'material/voxel/checker',
      materialContentHash: `sha256:${'c'.repeat(64)}`,
      atlas: {
        atlasAssetId: 'sprite-sheet/voxel/checker',
        atlasContentHash: `sha256:${'b'.repeat(64)}`,
      },
      sceneId: 'scene/loading-bay',
      instanceId: 'fixture',
      materialSlot: 1,
    },
    project: opened.project,
  });
  assert.equal(applied.type, 'projectMutationApplied');
  assert.equal(applied.type === 'projectMutationApplied' ? applied.receipt.kind : '',
    'voxelSurfaceMaterialUpserted');
  assert.equal(decodeStudioAdapterResponse({
    type: 'projectMutationApplied',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId: 'surface-removed',
    receipt: {
      kind: 'voxelSurfaceMaterialRemoved',
      materialAssetId: 'material/voxel/checker',
      textureAssetId: 'texture/voxel/checker',
    },
    project: opened.project,
  }).type, 'projectMutationApplied');

  const wrongResource = structuredClone(opened);
  (wrongResource.project.textureResources?.[0] as Record<string, unknown>)['resource'] =
    `mesh-resource/${digest}`;
  assert.throws(
    () => decodeStudioAdapterResponse(wrongResource),
    /content-addressed texture resource/u,
  );

  const widenedAtlas = structuredClone(opened);
  const mapping = (widenedAtlas.project.voxelSurfaceAuthoring.materials[0] as {
    mapping: Record<string, unknown>;
  }).mapping;
  mapping['regions'] = [];
  assert.throws(() => decodeStudioAdapterResponse(widenedAtlas), /regions.*not allowed/u);
});

function surfaceMaterialDefinition() {
  return {
    authority: {
      solid: false,
      collidable: false,
      occludes: false,
      structuralClass: 'decorative',
    },
    style: {
      color: [1, 1, 1, 1],
      texture: null,
      textureTint: [1, 1, 1, 1],
      emissionColor: [0, 0, 0, 1],
      roughness: 0.8,
      emissive: 0,
      uvStrategy: 'atlas',
    },
  };
}

function projectOpened(requestId: string): ProjectOpenedFixture {
  return {
    type: 'projectOpened',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    project: {
      identity: {
        projectId: 'loading-bay',
        name: 'Loading Bay',
        entryScene: 'scene/loading-bay',
        sourceSchemaVersion: 11,
        currentSchemaVersion: 11,
        projectHash: '00'.repeat(32),
        sceneRevision: 1,
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
          entryCount: 0,
          dependencyCount: 0,
          kinds: [],
          diagnostics: { diagnostics: [] },
        },
        scene: {
          sceneId: 1,
          revision: 1,
          schemaVersion: 4,
          name: 'Loading Bay',
          nodeCount: 0,
          rootCount: 0,
          dependencyCount: 0,
          nodeKinds: [],
          diagnostics: { diagnostics: [] },
        },
        entityState: {
          schemaVersion: 3,
          revision: 0,
          entityCount: 0,
          lifecycle: [],
          sources: [],
          capabilities: [],
          relationships: [],
          entityIds: [],
          diagnostics: { diagnostics: [] },
        },
        persistence: {
          schemaVersion: 1,
          artifactCount: 1,
          requiredArtifactCount: 1,
          declaredByteCount: 2,
          classes: [],
          roles: [],
          loadSteps: [],
          diagnostics: { diagnostics: [] },
        },
      },
      sceneHierarchy: {
        sceneId: 1,
        revision: 1,
        name: 'Loading Bay',
        rootNodeIds: [],
        nodes: [],
      },
      assetBrowser: {
        assets: [],
        lockEntries: [],
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
      entityComponents: [],
      projection: {
        schemaVersion: 1,
        ops: [],
      },
      projectionReadout: {
        frameKind: 'complete',
        sourceRevision: 0,
        retainedEntities: 0,
        retainedLights: 0,
        retainedVoxelInstances: 0,
        retainedVoxelChunks: 0,
        diagnostics: [],
      },
    },
  };
}

interface ProjectOpenedFixture {
  type: string;
  protocolVersion: number;
  requestId: string;
  project: {
    identity: {
      projectId: string;
      name: string;
      entryScene: string;
      sourceSchemaVersion: number;
      currentSchemaVersion: number;
      projectHash: string;
      sceneRevision: number;
      relativeProjectFile: string;
    };
    canonical: Record<string, string>;
    inspections: Record<string, Record<string, unknown>>;
    sceneHierarchy: {
      sceneId: number;
      revision: number;
      name: string | null;
      rootNodeIds: number[];
      nodes: unknown[];
    };
    assetBrowser: {
      assets: unknown[];
      lockEntries: unknown[];
    };
    voxelAuthoring: {
      assets: unknown[];
      instances: unknown[];
      materials: unknown[];
    };
    voxelSurfaceAuthoring: {
      textures: unknown[];
      atlases: unknown[];
      materials: unknown[];
    };
    voxelObjectAuthoring: {
      assets: unknown[];
      instances: unknown[];
    };
    textureResources?: unknown[];
    animatedMeshResources: unknown[];
    meshResources?: unknown[];
    entityComponents: Array<Record<string, unknown>>;
    projection: { schemaVersion: number; ops: unknown[] };
    projectionReadout: {
      frameKind: string;
      sourceRevision: number;
      retainedEntities: number;
      retainedLights: number;
      retainedVoxelInstances: number;
      retainedVoxelChunks: number;
      diagnostics: unknown[];
    };
  };
}

function described(
  requestId: string,
  entityInspectorContracts: Array<Record<string, unknown>> = [],
) {
  return {
    type: 'described',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    adapter: {
      adapterId: 'fixture.adapter',
      adapterVersion: 1,
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      projectKind: 'fixtureProject',
      projectSchemaVersion: 1,
      operations: STUDIO_ADAPTER_OPERATIONS,
      entityInspectorContracts,
    },
  };
}

function setCanonicalOwners(
  response: ProjectOpenedFixture,
  owners: readonly number[],
): void {
  const entityState = entityStateInspection(response);
  entityState['entityCount'] = owners.length;
  entityState['entityIds'] = [...owners];
  response.project.sceneHierarchy.rootNodeIds = [...owners];
  response.project.sceneHierarchy.nodes = owners.map((ownerEntityId, index) => ({
    nodeId: ownerEntityId,
    parentNodeId: null,
    childOrder: index,
    displayOrder: index,
    depth: 0,
    nodeKind: 'entityInstance',
    label: `Owner ${String(ownerEntityId)}`,
    tags: [],
    asset: null,
    entityId: ownerEntityId,
    localTransform: identityTransform(),
    worldTransform: identityTransform(),
    renderableTransform: identityTransform(),
  }));
}

function entityStateInspection(
  response: ProjectOpenedFixture,
): Record<string, unknown> {
  const entityState = response.project.inspections['entityState'];
  assert.ok(entityState !== undefined);
  return entityState;
}

function entityComponentAt(
  response: ProjectOpenedFixture,
  index: number,
): Record<string, unknown> {
  const reference = response.project.entityComponents[index];
  assert.ok(reference !== undefined);
  return reference;
}

function identityTransform() {
  return {
    translation: [0, 0, 0],
    rotation: [0, 0, 0, 1],
    scale: [1, 1, 1],
  };
}

function assetImportPrepared(requestId: string) {
  return {
    type: 'assetImportPrepared',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    plan: {
      planId: 'asset-import-plan',
      planHash: '11'.repeat(32),
      expectedProjectHash: '00'.repeat(32),
      source: { scope: 'project', path: 'content/assets/studio-triangle.mesh.json' },
      sourceHash: '22'.repeat(32),
      sourceByteCount: 512,
      meshAssetId: 'mesh/studio-triangle',
      reimportKind: 'structuralReload',
      hasErrors: false,
      diagnostics: [],
      generatedArtifacts: [{ relativePath: 'studio-triangle.static-mesh.json', byteCount: 256 }],
      generatedAssetIds: ['material/studio/paint', 'mesh/studio-triangle'],
      settings: { scale: 1, generateCollision: false, materialNamespace: 'studio' },
    },
  };
}

function conversionPrepared(requestId: string) {
  const texture = {
    textureAssetId: 'texture/palette',
    assetVersion: 1,
    contentHash: 'sha256:texture',
    width: 1,
    height: 1,
    colorSpace: 'linear',
    channelLayout: 'palette_index_u16',
  };
  return {
    type: 'voxelConversionPrepared',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    plan: {
      planId: 'plan-1',
      source: { assetId: 'mesh/wall', assetVersion: 1, sourceSha256: 'sha256:source' },
      targetAssetId: 'voxel-volume/wall',
      sourcePath: '/tmp/wall.glb',
      settings: {
        conversion: {
          resolution: [1, 1, 1],
          cellSize: 1,
          chunkSize: 16,
          origin: [0, 0, 0],
          fitPolicy: 'contain',
          originPolicy: 'targetMin',
          mode: 'surface',
          materialPalette: [],
          materialMap: [],
          maxOutputVoxels: 1,
        },
        transform: [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1],
        materialPolicy: {
          textureAssets: [{ texture, texelMaterials: [7] }],
          textureBindings: [{
            sourceMaterialSlot: 0,
            texture,
            uvAttribute: { attributeName: 'TEXCOORD_0', sourceHash: 'sha256:uv' },
            sampleUv: [0.5, 0.5],
            samplingPolicy: 'nearest_texel',
            wrapPolicy: 'clamp_to_edge',
            materialMode: 'sample_palette_index',
          }],
          defaultVoxelMaterial: 7,
        },
      },
      planner: 'rusty-engine.mesh-to-voxel.v1',
      expectedSourceSha256: 'sha256:source',
      settingsSha256: 'sha256:settings',
      expectedOutputContentHash: 'sha256:output',
      planHash: 'sha256:plan',
      estimatedOutputVoxels: 1,
      estimatedBounds: { min: [0, 0, 0], max: [0, 0, 0] },
    },
    preview: {
      planId: 'plan-1',
      planHash: 'sha256:plan',
      outputHash: 'sha256:output',
      outputVoxelCount: 1,
      outputBounds: { min: [0, 0, 0], max: [0, 0, 0] },
      sampleVoxels: [{ coordinate: [0, 0, 0], materialSlot: 7 }],
      samplesTruncated: false,
    },
  };
}

function voxelObjectSourceInspected(requestId: string) {
  return {
    type: 'voxelObjectSourceInspected',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    inspection: {
      sourceKind: 'animated',
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
        sourceBounds: { min: [-1, 0, -1], max: [1, 2, 1] },
        vertexCount: 4,
        triangleCount: 2,
        groups: [{
          groupId: 'group/0',
          label: 'Body',
          sourceMaterialSlot: 0,
          sourceNodeIndex: 0,
          sourceMeshIndex: 0,
          sourcePrimitiveIndex: 0,
          indexStart: 0,
          indexCount: 6,
          bounds: { min: [-1, 0, -1], max: [1, 2, 1] },
        }],
        materialSlots: [{ sourceMaterialSlot: 0, sourceMaterialName: 'Body' }],
        nodes: [{
          nodeId: 'node/0',
          label: 'Root',
          sourceNodeIndex: 0,
          childSourceNodeIndices: [],
          sourceMeshIndex: 0,
          localTransform: identityMatrix(),
          modelTransform: identityMatrix(),
        }],
        textureCoordinates: [],
      },
      clips: [{
        sourceAnimationIndex: 0,
        name: 'Walk',
        durationMicroseconds: 1_000_000,
        channelCount: 2,
        targetNodeIndices: [0],
        properties: ['translation', 'rotation'],
      }],
      diagnostics: [],
    },
  };
}

function voxelObjectConversionPrepared(requestId: string) {
  const clipFrame = {
    storedFrameIndex: 0,
    sourceTimestampsMicroseconds: [0],
    durationMicroseconds: 83_333,
    bounds: { min: [0, 0, 0], max: [0, 0, 0] },
    voxelCount: 1,
    sparseRunCount: 1,
    voxelDataHash: 'sha256:frame',
  };
  const clip = {
    outputClipId: 'clip/walk-1',
    sourceClipName: 'Walk',
    sourceAnimationIndex: 0,
    startMicroseconds: 0,
    endMicroseconds: 1_000_000,
    sampleRateHz: 12,
    endPolicy: 'excludeLoopSeam',
    sampledFrameCount: 12,
    storedFrameCount: 1,
    durationMicroseconds: 1_000_000,
    frames: [clipFrame],
  };
  return {
    type: 'voxelObjectConversionPrepared',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    plan: {
      planId: 'plan/object',
      source: {
        assetId: 'mesh-animation/character',
        assetVersion: 1,
        sourceSha256: 'sha256:source',
      },
      sourcePath: '/trusted/character.glb',
      targetAssetId: 'voxel-object/character',
      settings: {
        mesh: objectMeshSettings(),
        pivot: [0, 0, 0],
        anchorPolicy: { kind: 'preserveSourceSpace' },
      },
      clips: [{
        sourceClipName: 'Walk',
        outputClipId: 'clip/walk-1',
        outputName: 'Walk',
        sampleRateHz: 12,
        startMicroseconds: 0,
        endPolicy: 'excludeLoopSeam',
      }],
      defaultClip: 'clip/walk-1',
      planner: 'rusty-engine.voxel-object-conversion.v1',
      expectedSourceSha256: 'sha256:source',
      settingsSha256: 'sha256:settings',
      expectedOutputContentHash: 'sha256:output',
      planHash: 'sha256:plan',
      estimatedSampledFrames: 12,
      estimatedStoredFrames: 2,
      estimatedAggregateVoxels: 2,
      estimatedArtifactBytes: 2048,
      estimatedBounds: { min: [0, 0, 0], max: [0, 0, 0] },
      clipSummaries: [{
        outputClipId: 'clip/walk-1',
        sourceClipName: 'Walk',
        sourceAnimationIndex: 0,
        startMicroseconds: 0,
        endMicroseconds: 1_000_000,
        sampleRateHz: 12,
        sampledFrameCount: 12,
        storedFrameCount: 1,
        durationMicroseconds: 1_000_000,
      }],
    },
    preview: {
      planId: 'plan/object',
      planHash: 'sha256:plan',
      outputHash: 'sha256:output',
      sampledFrameCount: 12,
      storedFrameCount: 2,
      aggregateVoxelCount: 2,
      artifactBytes: 2048,
      unionBounds: { min: [0, 0, 0], max: [0, 0, 0] },
      clips: [clip],
      selectedFrame: {
        selection: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 0 },
        bounds: { min: [0, 0, 0], max: [0, 0, 0] },
        voxelCount: 1,
        sparseRunCount: 1,
        voxelDataHash: 'sha256:frame',
        durationMicroseconds: 83_333,
        sourceTimestampsMicroseconds: [0],
        sampleVoxels: [{ coordinate: [0, 0, 0], materialSlot: 7 }],
        samplesTruncated: false,
      },
    },
    projection: {
      schemaVersion: 1,
      ops: [{
        op: 'defineVoxelObject',
        asset: {
          asset: 'voxel-object/character',
          contentHash: 'sha256:output',
          meshes: [{
            payload: {
              layout: {
                vertexCount: 3,
                indexCount: 3,
                indexWidth: 'u32',
                attributes: [
                  { name: 'position', components: 3, kind: 'f32' },
                  { name: 'normal', components: 3, kind: 'f32' },
                ],
              },
              groups: [{ materialSlot: 7, start: 0, count: 3 }],
              bounds: { min: [0, 0, 0], max: [1, 1, 0] },
              source: {
                kind: 'inline',
                positions: [0, 0, 0, 1, 0, 0, 0, 1, 0],
                normals: [0, 0, 1, 0, 0, 1, 0, 0, 1],
                indices: [0, 1, 2],
              },
              provenance: 'voxelObject',
            },
          }],
          frames: [{ id: 'default', mesh: 0 }],
          materialSlots: [{ slot: 7, material: 'material/wall-lines' }],
        },
      }],
    },
    projectionReadout: emptyProjectionReadout(),
  };
}

function voxelObjectInstancePreviewed(requestId: string) {
  const candidate = voxelObjectConversionPrepared(requestId);
  return {
    type: 'voxelObjectInstancePreviewed',
    protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
    requestId,
    playback: {
      sceneId: 'scene/loading-bay',
      instanceId: 'character-one',
      voxelObjectAssetId: 'voxel-object/character',
      projectHash: 'project-hash',
      objectContentHash: 'sha256:output',
      durableFrame: { kind: 'clip', clipId: 'clip/walk-1', frameIndex: 1 },
      status: 'paused',
      clipId: 'clip/walk-1',
      loopMode: 'repeat',
      rate: { numerator: 1, denominator: 1 },
      elapsedMicroseconds: 0,
      runtimeFrame: 2,
      clipFrame: 0,
      ended: false,
    },
    projection: candidate.projection,
    projectionReadout: candidate.projectionReadout,
  };
}

function voxelObjectAssetReadout() {
  const frame = {
    bounds: { min: [0, 0, 0], max: [0, 0, 0] },
    voxelDataHash: 'sha256:frame',
    voxelCount: 1,
    sparseRunCount: 1,
    durationMicroseconds: null,
  };
  return {
    assetId: 'voxel-object/character',
    contentHash: 'sha256:output',
    grid: {
      coordinateSystem: 'rightHandedYUp',
      cellSize: 1,
      chunkSize: 16,
      pivot: [0, 0, 0],
    },
    bounds: { min: [0, 0, 0], max: [0, 0, 0] },
    defaultFrame: frame,
    clips: [{ clipId: 'clip/walk-1', name: 'Walk', framesPerSecond: 12, frames: [frame] }],
    defaultClip: 'clip/walk-1',
    materialPalette: [{ materialSlot: 7, materialAssetId: 'material/wall-lines' }],
    materialMap: [{ sourceMaterialSlot: 0, sourceMaterialName: 'Body', voxelMaterialSlot: 7 }],
    provenance: {
      kind: 'convertedAnimatedMesh',
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

function objectMeshSettings() {
  return {
    conversion: {
      resolution: [1, 1, 1],
      cellSize: 1,
      chunkSize: 16,
      origin: [0, 0, 0],
      fitPolicy: 'contain',
      originPolicy: 'targetMin',
      mode: 'surface',
      materialPalette: [{ materialSlot: 7, materialAssetId: 'material/wall-lines' }],
      materialMap: [{ sourceMaterialSlot: 0, voxelMaterialSlot: 7 }],
      maxOutputVoxels: 1,
    },
    transform: identityMatrix(),
    materialPolicy: { textureAssets: [], textureBindings: [], defaultVoxelMaterial: 7 },
  };
}

function identityMatrix(): number[] {
  return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
}

function emptyProjectionReadout() {
  return {
    frameKind: 'complete',
    sourceRevision: 1,
    retainedEntities: 1,
    retainedLights: 0,
    retainedVoxelInstances: 0,
    retainedVoxelChunks: 0,
    diagnostics: [],
  };
}
