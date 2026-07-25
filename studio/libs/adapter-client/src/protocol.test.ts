import assert from 'node:assert/strict';
import test from 'node:test';

import {
  StudioAdapterClient,
  StudioAdapterDecodeError,
  StudioAdapterOperationRejected,
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  decodeStudioAdapterResponse,
  type StudioAdapterRequest,
  type StudioAdapterTransport,
} from './index.js';

test('decodes the closed project response and delegates projection validation', () => {
  const response = projectOpened('request-1');
  response.project.identity.projectId = 'intentionally not semantically revalidated';

  const decoded = decodeStudioAdapterResponse(response);

  assert.equal(decoded.type, 'projectOpened');
  assert.equal(
    decoded.project.identity.projectId,
    'intentionally not semantically revalidated',
  );
  assert.equal(decoded.project.projection.ops.length, 0);
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

test('protocol 4 closes history, file, and texture-policy response families', () => {
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
        sourceSchemaVersion: 9,
        currentSchemaVersion: 9,
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
      voxelAuthoring: {
        assets: [],
        instances: [],
        materials: [],
      },
      loadingBay: {
        sceneName: 'Loading Bay',
        entityCount: 8,
        doorCount: 1,
        switchCount: 1,
        enemyCount: 2,
        encounterCount: 1,
        extractionBeaconCount: 1,
        navigatorCount: 1,
        playerControllerCount: 1,
        weaponCount: 1,
        voxelEnvironment: 'generatedRoom',
      },
      projection: {
        schemaVersion: 1,
        ops: [],
      },
      projectionReadout: {
        frameKind: 'complete',
        sourceRevision: 0,
        retainedEntities: 0,
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
    voxelAuthoring: {
      assets: unknown[];
      instances: unknown[];
      materials: unknown[];
    };
    loadingBay: Record<string, string | number>;
    projection: { schemaVersion: number; ops: unknown[] };
    projectionReadout: {
      frameKind: string;
      sourceRevision: number;
      retainedEntities: number;
      retainedVoxelInstances: number;
      retainedVoxelChunks: number;
      diagnostics: unknown[];
    };
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
