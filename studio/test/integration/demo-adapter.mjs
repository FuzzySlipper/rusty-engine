import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import { cp, mkdtemp, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { createInterface } from 'node:readline';
import { isAbsolute, join } from 'node:path';

import {
  StudioAdapterClient,
  StudioAdapterOperationRejected,
} from '../../libs/adapter-client/dist/index.js';
import {
  StudioWorkspaceStore,
} from '../../libs/editor-shell/dist/state.js';

class JsonLineProcessTransport {
  #child;
  #pending = [];
  #stderr = '';

  constructor(binaryPath) {
    this.#child = spawn(binaryPath, [], {
      stdio: ['pipe', 'pipe', 'pipe'],
    });
    this.#child.stderr.setEncoding('utf8');
    this.#child.stderr.on('data', (chunk) => {
      this.#stderr += chunk;
    });
    const lines = createInterface({ input: this.#child.stdout });
    lines.on('line', (line) => {
      const pending = this.#pending.shift();
      if (pending === undefined) {
        this.#failAll(new Error(`adapter emitted an unsolicited response: ${line}`));
        return;
      }
      try {
        pending.resolve(JSON.parse(line));
      } catch (error) {
        pending.reject(error);
      }
    });
    this.#child.on('error', (error) => this.#failAll(error));
    this.#child.on('exit', (code, signal) => {
      if (this.#pending.length !== 0) {
        this.#failAll(
          new Error(
            `adapter exited code=${String(code)} signal=${String(signal)} stderr=${this.#stderr}`,
          ),
        );
      }
    });
  }

  exchange(request) {
    return new Promise((resolve, reject) => {
      this.#pending.push({ resolve, reject });
      this.#child.stdin.write(`${JSON.stringify(request)}\n`, (error) => {
        if (error !== null && error !== undefined) {
          const pending = this.#pending.pop();
          pending?.reject(error);
        }
      });
    });
  }

  async close() {
    if (!this.#child.stdin.destroyed) this.#child.stdin.end();
    if (this.#child.exitCode !== null) return;
    await new Promise((resolve) => this.#child.once('exit', resolve));
  }

  #failAll(error) {
    for (const pending of this.#pending.splice(0)) pending.reject(error);
  }
}

function argumentValue(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index === -1) {
    if (fallback !== undefined) return fallback;
    throw new Error(`${name} is required`);
  }
  const value = process.argv[index + 1];
  if (value === undefined || value.startsWith('--')) {
    throw new Error(`${name} requires a value`);
  }
  return value;
}

async function main() {
  const demoRoot = argumentValue('--demo-root');
  if (!isAbsolute(demoRoot)) {
    throw new Error('--demo-root must be an explicit absolute path');
  }
  const binary = argumentValue(
    '--adapter-binary',
    join(demoRoot, 'target/debug/studio-adapter'),
  );
  await withClient(binary, async (client) => {
    const store = new StudioWorkspaceStore(client);
    await store.openProject(
      demoRoot,
      'content/projects/loading-bay.project.json',
    );
    const opened = store.snapshot();
    assert.equal(opened.connection.kind, 'connected');
    assert.equal(opened.authoringDocument.identity.projectId, 'loading-bay');
    assert.equal(opened.authoringDocument.inspections.catalog.entryCount, 6);
    assert.equal(opened.authoringDocument.inspections.scene.nodeCount, 8);
    assert.equal(opened.authoringDocument.inspections.entityState.entityCount, 8);
    assert.equal(opened.authoringDocument.sceneHierarchy.nodes.length, 8);
    assert.deepEqual(
      opened.authoringDocument.sceneHierarchy.nodes.map((node) => node.entityId),
      [1, 2, 3, 4, 5, 6, 7, 10],
    );
    assert.equal(opened.authoringDocument.domain.voxelEnvironment, 'generatedRoom');
    assert.equal(opened.authoringDocument.domain.enemyCount, 2);
    assert.equal(opened.authoringDocument.voxel.solidVoxelCount, 366);
    assert.equal(opened.liveProjection.frame.ops.length, 19);
    assert.equal(opened.liveProjection.readout.frameKind, 'complete');
    assert.equal(opened.liveProjection.readout.diagnostics.length, 0);
    assert.equal(opened.liveProjection.entities.length, 8);

    await store.refreshProject();
    const reread = store.snapshot();
    assert.equal(
      reread.authoringDocument.identity.projectHash,
      opened.authoringDocument.identity.projectHash,
    );
    assert.equal(
      reread.authoringDocument.identity.sceneRevision,
      opened.authoringDocument.identity.sceneRevision,
    );
    assert.equal(reread.liveProjection.frame.ops.length, 19);
    assert.equal(reread.liveProjection.generation, opened.liveProjection.generation + 1);

    await store.closeProject();
    assert.equal(store.snapshot().authoringDocument, null);
  });

  await verifyVoxelPersistenceAcrossProcesses(binary, demoRoot);
  process.stdout.write('Studio editor store + complete voxel adapter integration passed\n');
}

async function verifyVoxelPersistenceAcrossProcesses(binary, demoRoot) {
  const root = await mkdtemp(join(tmpdir(), 'rusty-engine-studio-adapter.'));
  try {
    await Promise.all([
      cp(join(demoRoot, 'content'), join(root, 'content'), { recursive: true }),
      cp(join(demoRoot, 'fixtures'), join(root, 'fixtures'), { recursive: true }),
    ]);

    let persisted;
    await withClient(binary, async (client) => {
      const opened = (await client.openProject(
        root,
        'content/projects/converted-wall.project.json',
      )).project;
      const initial = voxelAsset(opened, 'voxel-volume/kenney-wall-a');
      assert.equal(opened.voxelAuthoring.instances.length, 2);
      assert.equal(opened.projectionReadout.retainedVoxelInstances, 2);
      assert.ok(opened.projectionReadout.retainedVoxelChunks > 0);

      const brushed = await client.applyVoxelBrush({
        expectedProjectHash: opened.identity.projectHash,
        assetId: initial.inspection.assetId,
        expectedAssetContentHash: initial.inspection.contentHash,
        center: [0, 0, 0],
        radius: 0,
        mode: 'erase',
        materialSlot: null,
      });
      assert.equal(brushed.receipt.kind, 'voxelBrushApplied');
      const edited = voxelAsset(brushed.project, initial.inspection.assetId);
      assert.equal(edited.history.cursor, 1);
      assert.equal(edited.history.undoDepth, 1);

      const queried = await client.queryVoxelModel({
        expectedProjectHash: brushed.project.identity.projectHash,
        assetId: edited.inspection.assetId,
        expectedAssetContentHash: edited.inspection.contentHash,
        window: {
          expectedContentHash: edited.inspection.contentHash,
          bounds: { min: edited.inspection.boundsMin, max: edited.inspection.boundsMax },
          includeEmpty: false,
          materialFilter: [],
          maxSamples: 32,
        },
      });
      assert.equal(queried.readout.kind, 'model');

      const annotated = await client.createVoxelAnnotationLayer({
        expectedProjectHash: brushed.project.identity.projectHash,
        assetId: edited.inspection.assetId,
        draft: {
          layerId: 'voxel-annotation/integration-semantics',
          targetVoxelAssetId: edited.inspection.assetId,
          targetVoxelDataHash: edited.inspection.voxelDataHash,
          targetBounds: { min: edited.inspection.boundsMin, max: edited.inspection.boundsMax },
          regions: [{
            regionId: 'region/integration-cover',
            label: 'Integration cover',
            kind: 'cover',
            tags: ['integration', 'cover'],
            bounds: { min: [1, 0, 0], max: [1, 0, 0] },
            selection: { sparseRuns: [{ start: [1, 0, 0], length: 1 }] },
          }],
          provenance: [],
        },
      });
      assert.equal(annotated.receipt.kind, 'voxelAnnotationCreated');
      const annotation = voxelAsset(
        annotated.project,
        edited.inspection.assetId,
      ).annotations[0];
      assert.ok(annotation !== undefined);
      const annotationQuery = await client.queryVoxelAnnotation({
        expectedProjectHash: annotated.project.identity.projectHash,
        assetId: edited.inspection.assetId,
        layerId: annotation.layerId,
        query: {
          expectedLayerHash: annotation.canonicalLayerHash,
          mode: { kind: 'layerSummary' },
          maxResults: 32,
        },
      });
      assert.equal(annotationQuery.readout.kind, 'annotationQuery');
      const annotationExport = await client.exportVoxelAnnotation({
        expectedProjectHash: annotated.project.identity.projectHash,
        assetId: edited.inspection.assetId,
        layerId: annotation.layerId,
        expectedLayerHash: annotation.canonicalLayerHash,
      });
      assert.equal(annotationExport.readout.kind, 'annotationExport');

      const prepared = await client.prepareVoxelConversion({
        expectedProjectHash: annotated.project.identity.projectHash,
        sourceAssetId: 'mesh/kenney-wall-a',
        sourcePath: 'fixtures/voxel-conversion/kenney-wall-a.glb',
        targetAssetId: 'voxel-volume/integration-converted',
        licensePath: 'fixtures/voxel-conversion/KENNEY-RETRO-URBAN-KIT-LICENSE.txt',
        settings: conversionSettings(),
        maxPreviewSamples: 32,
      });
      const bytesBeforeForgedApply = await readFile(
        join(root, 'content/projects/converted-wall.project.json'),
      );
      await assert.rejects(
        client.applyVoxelConversion({
          expectedProjectHash: annotated.project.identity.projectHash,
          planId: prepared.plan.planId,
          expectedPlanHash: prepared.plan.planHash,
          expectedOutputHash: 'sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',
        }),
        (error) => rejectionCode(error) === 'conversion.staleOutput',
      );
      assert.deepEqual(
        await readFile(join(root, 'content/projects/converted-wall.project.json')),
        bytesBeforeForgedApply,
      );

      const applied = await client.applyVoxelConversion({
        expectedProjectHash: annotated.project.identity.projectHash,
        planId: prepared.plan.planId,
        expectedPlanHash: prepared.plan.planHash,
        expectedOutputHash: prepared.preview.outputHash,
      });
      assert.equal(applied.receipt.kind, 'voxelConversionApplied');
      persisted = {
        projectHash: applied.project.identity.projectHash,
        editedContentHash: voxelAsset(
          applied.project,
          initial.inspection.assetId,
        ).inspection.contentHash,
        convertedContentHash: voxelAsset(
          applied.project,
          'voxel-volume/integration-converted',
        ).inspection.contentHash,
      };
      await client.closeProject();
    });

    assert.ok(persisted !== undefined);
    await withClient(binary, async (client) => {
      const reopened = (await client.openProject(
        root,
        'content/projects/converted-wall.project.json',
      )).project;
      assert.equal(reopened.identity.projectHash, persisted.projectHash);
      const edited = voxelAsset(reopened, 'voxel-volume/kenney-wall-a');
      assert.equal(edited.inspection.contentHash, persisted.editedContentHash);
      assert.equal(edited.history.persisted, true);
      assert.equal(edited.history.cursor, 1);
      assert.equal(edited.annotations[0]?.layerId, 'voxel-annotation/integration-semantics');
      assert.equal(
        voxelAsset(reopened, 'voxel-volume/integration-converted').inspection.contentHash,
        persisted.convertedContentHash,
      );

      const bytesBeforeStale = await readFile(
        join(root, 'content/projects/converted-wall.project.json'),
      );
      await assert.rejects(
        client.upsertMaterial({
          expectedProjectHash: 'bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb',
          assetId: 'material/should-not-persist',
          definition: materialDefinition(),
        }),
        (error) => rejectionCode(error) === 'project.staleHash',
      );
      assert.deepEqual(
        await readFile(join(root, 'content/projects/converted-wall.project.json')),
        bytesBeforeStale,
      );
      await client.closeProject();
    });
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

async function withClient(binary, action) {
  const transport = new JsonLineProcessTransport(binary);
  try {
    return await action(new StudioAdapterClient(transport));
  } finally {
    await transport.close();
  }
}

function voxelAsset(project, assetId) {
  const asset = project.voxelAuthoring.assets.find(
    (candidate) => candidate.inspection.assetId === assetId,
  );
  assert.ok(asset !== undefined, `missing voxel authoring asset ${assetId}`);
  return asset;
}

function rejectionCode(error) {
  return error instanceof StudioAdapterOperationRejected ? error.rejection.code : null;
}

function conversionSettings() {
  return {
    conversion: {
      resolution: [4, 3, 2],
      cellSize: 1,
      chunkSize: 16,
      origin: [4, 0, 6],
      fitPolicy: 'contain',
      originPolicy: 'targetMin',
      mode: 'surface',
      materialPalette: [
        { materialSlot: 7, materialAssetId: 'material/wall-lines', displayName: 'Wall lines' },
        { materialSlot: 8, materialAssetId: 'material/concrete', displayName: 'Concrete' },
      ],
      materialMap: [
        { sourceMaterialSlot: 0, sourceMaterialName: 'wall_lines', voxelMaterialSlot: 7 },
        { sourceMaterialSlot: 1, sourceMaterialName: 'concrete', voxelMaterialSlot: 8 },
      ],
      maxOutputVoxels: 64,
    },
    transform: [
      1, 0, 0, 0,
      0, 1, 0, 0,
      0, 0, 1, 0,
      0, 0, 0, 1,
    ],
    materialPolicy: { textureAssets: [], textureBindings: [] },
  };
}

function materialDefinition() {
  return {
    authority: {
      solid: true,
      collidable: true,
      occludes: true,
      structuralClass: 'structural',
    },
    style: {
      color: [0.9, 0.2, 0.2, 1],
      texture: null,
      textureTint: [1, 1, 1, 1],
      emissionColor: [0.9, 0.2, 0.2, 1],
      roughness: 0.8,
      emissive: 0,
      uvStrategy: 'flat',
    },
  };
}

await main();
