import assert from 'node:assert/strict';
import test from 'node:test';

import {
  StudioAdapterClient,
  StudioAdapterDecodeError,
  StudioAdapterOperationRejected,
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
        protocolVersion: 1,
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
    protocolVersion: 1,
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
    protocolVersion: 1,
    requestId,
    project: {
      identity: {
        projectId: 'loading-bay',
        name: 'Loading Bay',
        entryScene: 'scene/loading-bay',
        sourceSchemaVersion: 8,
        currentSchemaVersion: 8,
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
        catalog: {},
        scene: {},
        entityState: {},
        persistence: {},
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
        sourceRevision: 0,
        retainedEntities: 0,
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
    loadingBay: Record<string, string | number>;
    projection: { schemaVersion: number; ops: unknown[] };
    projectionReadout: {
      sourceRevision: number;
      retainedEntities: number;
      diagnostics: unknown[];
    };
  };
}
