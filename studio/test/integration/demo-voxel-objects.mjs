import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import {
  cp,
  mkdtemp,
  readFile,
  rm,
  symlink,
  truncate,
  writeFile,
} from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { isAbsolute, join } from 'node:path';
import { createInterface } from 'node:readline';

import {
  STUDIO_ADAPTER_PROTOCOL_VERSION,
  StudioAdapterClient,
  StudioAdapterOperationRejected,
} from '../../libs/adapter-client/dist/index.js';

const staticProjectFile = 'content/projects/converted-wall.project.json';
const animatedProjectFile = 'content/projects/loading-bay.project.json';
const staticSourceFile = 'fixtures/voxel-conversion/kenney-wall-a.glb';
const animatedSourceFile = 'content/assets/kenney-retro-character-medium.glb';
const licenseFile = 'fixtures/voxel-conversion/KENNEY-RETRO-URBAN-KIT-LICENSE.txt';
const animatedLicenseFile = 'content/assets/KENNEY-ANIMATED-CHARACTERS-RETRO-LICENSE.txt';

class JsonLineProcessTransport {
  #child;
  #pending = [];
  #stderr = '';

  constructor(binaryPath) {
    this.#child = spawn(binaryPath, [], { stdio: ['pipe', 'pipe', 'pipe'] });
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
      if (this.#pending.length === 0) return;
      this.#failAll(new Error(
        `adapter exited code=${String(code)} signal=${String(signal)} stderr=${this.#stderr}`,
      ));
    });
  }

  exchange(request) {
    return new Promise((resolve, reject) => {
      this.#pending.push({ resolve, reject });
      this.#child.stdin.write(`${JSON.stringify(request)}\n`, (error) => {
        if (error === null || error === undefined) return;
        this.#pending.pop()?.reject(error);
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

async function main() {
  const demoRoot = argumentValue('--demo-root');
  if (!isAbsolute(demoRoot)) throw new Error('--demo-root must be an absolute path');
  const binary = argumentValue(
    '--adapter-binary',
    join(demoRoot, 'target/debug/studio-adapter'),
  );
  const root = await mkdtemp(join(tmpdir(), 'rusty-engine-studio-voxel-objects.'));
  try {
    await Promise.all([
      cp(join(demoRoot, 'content'), join(root, 'content'), { recursive: true }),
      cp(join(demoRoot, 'fixtures'), join(root, 'fixtures'), { recursive: true }),
    ]);
    const staticEvidence = await verifyStaticObjectWorkflow(binary, root);
    const animatedEvidence = await verifyAnimatedObjectWorkflow(binary, root);
    process.stdout.write(`${JSON.stringify({
      kind: 'studioVoxelObjectIntegrationEvidence',
      protocolVersion: STUDIO_ADAPTER_PROTOCOL_VERSION,
      static: staticEvidence,
      animated: animatedEvidence,
    })}\n`);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
}

async function verifyStaticObjectWorkflow(binary, root) {
  const projectPath = join(root, staticProjectFile);
  const sourcePath = join(root, staticSourceFile);
  const originalSource = await readFile(sourcePath);
  let persisted;

  await withClient(binary, async (client) => {
    const opened = (await client.openProject(root, staticProjectFile)).project;
    const originalProject = await readFile(projectPath);
    assert.deepEqual(opened.voxelObjectAuthoring, { assets: [], instances: [] });

    const projectInspection = (await client.inspectVoxelObjectSource({
      expectedProjectHash: opened.identity.projectHash,
      sourceKind: 'static',
      sourceAssetId: 'mesh/kenney-wall-a',
      source: { scope: 'project', path: staticSourceFile },
      meshPrimitive: 'group/0',
    })).inspection;
    const hostInspection = (await client.inspectVoxelObjectSource({
      expectedProjectHash: opened.identity.projectHash,
      sourceKind: 'static',
      sourceAssetId: 'mesh/kenney-wall-a',
      source: { scope: 'host', path: sourcePath },
      meshPrimitive: 'group/0',
    })).inspection;
    assert.equal(projectInspection.sourceKind, 'static');
    assert.equal(projectInspection.diagnostics[0]?.code, 'voxelObject.staticSource');
    assert.equal(projectInspection.metadata.groups.length, 1);
    assert.deepEqual(hostInspection.metadata, projectInspection.metadata);
    assert.equal(hostInspection.source.sourceSha256, projectInspection.source.sourceSha256);

    const linkedSource = join(root, 'fixtures/voxel-conversion/linked-wall.glb');
    await symlink('kenney-wall-a.glb', linkedSource);
    try {
      await expectRejection(client.inspectVoxelObjectSource({
        expectedProjectHash: opened.identity.projectHash,
        sourceKind: 'static',
        sourceAssetId: 'mesh/kenney-wall-a',
        source: { scope: 'host', path: linkedSource },
      }), 'conversion.hostFileRejected');
    } finally {
      await rm(linkedSource, { force: true });
    }

    const oversizedSource = join(root, 'fixtures/voxel-conversion/oversized.glb');
    await writeFile(oversizedSource, '');
    await truncate(oversizedSource, 64 * 1024 * 1024 + 1);
    try {
      await expectRejection(client.inspectVoxelObjectSource({
        expectedProjectHash: opened.identity.projectHash,
        sourceKind: 'static',
        sourceAssetId: 'mesh/kenney-wall-a',
        source: { scope: 'project', path: 'fixtures/voxel-conversion/oversized.glb' },
      }), 'conversion.projectFileRejected');
    } finally {
      await rm(oversizedSource, { force: true });
    }
    assert.deepEqual(await readFile(projectPath), originalProject);

    const discardedCandidate = await prepareStaticObject(client, opened.identity.projectHash);
    assertCompleteObjectProjection(discardedCandidate, 'voxel-object/integration-wall');
    assert.equal(discardedCandidate.preview.selectedFrame.samplesTruncated, true);
    assert.equal(discardedCandidate.preview.selectedFrame.sampleVoxels.length, 1);
    const discarded = await client.discardVoxelObjectConversion({
      planId: discardedCandidate.plan.planId,
    });
    assert.equal(discarded.projectionReadout.frameKind, 'complete');
    assert.equal(objectOperation(discarded.projection, 'defineVoxelObject'), undefined);
    assert.deepEqual(await readFile(projectPath), originalProject);

    const prepared = await prepareStaticObject(client, opened.identity.projectHash);
    assertCompleteObjectProjection(prepared, 'voxel-object/integration-wall');
    await expectRejection(client.previewVoxelObjectConversion({
      planId: prepared.plan.planId,
      expectedPlanHash: prepared.plan.planHash,
      frame: { kind: 'default' },
      maxPreviewSamples: 4_097,
    }), 'conversion.queryQuotaExceeded');
    const previewed = await client.previewVoxelObjectConversion({
      planId: prepared.plan.planId,
      expectedPlanHash: prepared.plan.planHash,
      frame: { kind: 'default' },
      maxPreviewSamples: 1,
    });
    assertCompleteObjectProjection(previewed, 'voxel-object/integration-wall');

    await writeFile(sourcePath, 'source drift');
    await expectRejection(client.applyVoxelObjectConversion({
      expectedProjectHash: opened.identity.projectHash,
      planId: prepared.plan.planId,
      expectedPlanHash: prepared.plan.planHash,
      expectedOutputHash: prepared.preview.outputHash,
    }), 'conversion.staleSource');
    assert.deepEqual(await readFile(projectPath), originalProject);
    await writeFile(sourcePath, originalSource);
    await expectRejection(client.applyVoxelObjectConversion({
      expectedProjectHash: opened.identity.projectHash,
      planId: prepared.plan.planId,
      expectedPlanHash: prepared.plan.planHash,
      expectedOutputHash: `sha256:${'a'.repeat(64)}`,
    }), 'conversion.staleOutput');
    assert.deepEqual(await readFile(projectPath), originalProject);

    const applied = await client.applyVoxelObjectConversion({
      expectedProjectHash: opened.identity.projectHash,
      planId: prepared.plan.planId,
      expectedPlanHash: prepared.plan.planHash,
      expectedOutputHash: prepared.preview.outputHash,
    });
    assert.equal(applied.receipt.kind, 'voxelObjectConversionApplied');
    const asset = objectAsset(applied.project, 'voxel-object/integration-wall');
    assert.equal(asset.provenance.kind, 'convertedStaticMesh');
    assert.equal(asset.provenance.licensePath, licenseFile);
    assert.equal(asset.provenance.sourceClips.length, 0);

    const placement = await client.prepareVoxelObjectPlacement({
      expectedProjectHash: applied.project.identity.projectHash,
      assetId: asset.assetId,
      expectedObjectContentHash: asset.contentHash,
    });
    assertPlacementResources(placement, asset);

    const attached = await client.attachVoxelObjectInstance({
      expectedProjectHash: applied.project.identity.projectHash,
      sceneId: 'scene/converted-wall',
      instance: {
        instanceId: 'integration-wall-object',
        voxelObjectAssetId: asset.assetId,
        surfaceMode: 'greedyCubes',
        frame: { kind: 'default' },
        translation: [3, 2, 1],
        rotation: [0, 0, 0, 1],
        scale: [1, 2, 1],
        materialOverrides: [{ materialSlot: 7, materialAssetId: 'material/concrete' }],
      },
    });
    assert.equal(attached.receipt.kind, 'voxelObjectInstanceAttached');
    const instance = objectInstance(attached.project, 'integration-wall-object');
    assert.deepEqual(instance.instance.translation, [3, 2, 1]);
    assert.deepEqual(instance.instance.scale, [1, 2, 1]);
    assert.ok(attached.project.sceneHierarchy.nodes.some(
      (node) => node.entityId === instance.ownerEntityId,
    ));
    const create = objectOperationForAsset(
      attached.project.projection,
      'createVoxelObjectInstance',
      asset.assetId,
    );
    assert.ok(create !== undefined);
    assert.deepEqual(create.instance.transform.translation, [3, 2, 1]);
    assert.deepEqual(create.instance.transform.scale, [1, 2, 1]);
    assert.equal(create.instance.metadata.sourceEntity, instance.ownerEntityId);

    const duplicateCandidate = {
      instanceId: 'integration-wall-object-copy',
      voxelObjectAssetId: asset.assetId,
      surfaceMode: 'greedyCubes',
      frame: { kind: 'default' },
      translation: [5, 2, 1],
      rotation: [0, 0, 0, 1],
      scale: [1, 2, 1],
      materialOverrides: [{ materialSlot: 7, materialAssetId: 'material/concrete' }],
    };
    const duplicated = await client.attachVoxelObjectInstance({
      expectedProjectHash: attached.project.identity.projectHash,
      sceneId: 'scene/converted-wall',
      instance: duplicateCandidate,
    });
    const duplicate = objectInstance(duplicated.project, duplicateCandidate.instanceId);
    assert.notEqual(duplicate.ownerEntityId, instance.ownerEntityId);
    const definitions = duplicated.project.projection.ops.filter(
      (operation) => operation.op === 'defineVoxelObject'
        && operation.asset.asset === asset.assetId,
    );
    const instances = duplicated.project.projection.ops.filter(
      (operation) => operation.op === 'createVoxelObjectInstance'
        && operation.instance.asset === asset.assetId,
    );
    assert.equal(definitions.length, 1, 'repeated placement reuses one object definition');
    assert.equal(instances.length, 2, 'repeated placement retains distinct instances');
    assert.deepEqual(
      new Set(instances.map((operation) => operation.instance.metadata.sourceEntity)),
      new Set([instance.ownerEntityId, duplicate.ownerEntityId]),
    );

    const undone = await client.deleteSceneObject({
      expectedProjectHash: duplicated.project.identity.projectHash,
      expectedSceneRevision: duplicated.project.identity.sceneRevision,
      entityId: duplicate.ownerEntityId,
    });
    assert.equal(
      undone.project.voxelObjectAuthoring.instances.some(
        (entry) => entry.instance.instanceId === duplicateCandidate.instanceId,
      ),
      false,
    );
    const reapplied = await client.attachVoxelObjectInstance({
      expectedProjectHash: undone.project.identity.projectHash,
      sceneId: 'scene/converted-wall',
      instance: duplicateCandidate,
    });
    const reappliedDuplicate = objectInstance(reapplied.project, duplicateCandidate.instanceId);
    assert.notEqual(reappliedDuplicate.ownerEntityId, instance.ownerEntityId);
    assert.equal(reapplied.project.projection.ops.filter(
      (operation) => operation.op === 'defineVoxelObject'
        && operation.asset.asset === asset.assetId,
    ).length, 1);
    assert.equal(reapplied.project.projection.ops.filter(
      (operation) => operation.op === 'createVoxelObjectInstance'
        && operation.instance.asset === asset.assetId,
    ).length, 2);
    persisted = {
      authoring: structuredClone(reapplied.project.voxelObjectAuthoring),
      projectHash: reapplied.project.identity.projectHash,
      bytes: await readFile(projectPath),
      ownerEntityId: instance.ownerEntityId,
      duplicateOwnerEntityId: reappliedDuplicate.ownerEntityId,
      projectionOperations: objectOperationNames(reapplied.project.projection),
    };
    await client.closeProject();
  });

  assert.ok(persisted !== undefined);
  await withClient(binary, async (client) => {
    const reopened = (await client.openProject(root, staticProjectFile)).project;
    assert.equal(reopened.identity.projectHash, persisted.projectHash);
    assert.deepEqual(reopened.voxelObjectAuthoring, persisted.authoring);
    assert.deepEqual(await readFile(projectPath), persisted.bytes);
    assert.ok(objectOperationForAsset(
      reopened.projection,
      'defineVoxelObject',
      'voxel-object/integration-wall',
    ) !== undefined);
    assert.ok(objectOperationForAsset(
      reopened.projection,
      'createVoxelObjectInstance',
      'voxel-object/integration-wall',
    ) !== undefined);
    await client.closeProject();
  });

  return {
    storedVoxels: staticVoxelCount(persisted.authoring),
    ownerEntityId: persisted.ownerEntityId,
    duplicateOwnerEntityId: persisted.duplicateOwnerEntityId,
    retainedDefinitionCount: 1,
    retainedInstanceCount: 2,
    placementUndoReapplied: true,
    projectionOperations: persisted.projectionOperations,
    dirtyApplyPreservedBytes: true,
    freshProcessReadoutMatched: true,
  };
}

async function verifyAnimatedObjectWorkflow(binary, root) {
  const projectPath = join(root, animatedProjectFile);
  let persisted;
  let evidence;

  await withClient(binary, async (client) => {
    const opened = (await client.openProject(root, animatedProjectFile)).project;
    const material = await client.upsertMaterial({
      expectedProjectHash: opened.identity.projectHash,
      assetId: 'material/integration-character-voxel',
      definition: materialDefinition(),
    });
    const inspection = (await client.inspectVoxelObjectSource({
      expectedProjectHash: material.project.identity.projectHash,
      sourceKind: 'animated',
      sourceAssetId: 'mesh-animation/kenney-retro-character-medium',
      source: { scope: 'project', path: animatedSourceFile },
    })).inspection;
    assert.equal(inspection.sourceKind, 'animated');
    assert.equal(inspection.diagnostics[0]?.code, 'voxelObject.animatedSource');
    assert.ok(inspection.metadata.nodes.length > 0);
    assert.ok(inspection.metadata.groups.length > 0);
    assert.ok(inspection.metadata.materialSlots.length > 0);
    assert.ok(inspection.metadata.textureCoordinates.length > 0);
    assert.ok(inspection.clips.length >= 3);
    const sourceClip = inspection.clips.find(
      (clip) => clip.name.toLocaleLowerCase('en-US').includes('run'),
    ) ?? inspection.clips[0];
    assert.ok(sourceClip !== undefined);
    assert.ok(sourceClip.channelCount > 0);
    assert.ok(sourceClip.targetNodeIndices.length > 0);
    assert.ok(sourceClip.properties.length > 0);
    const outputClipId = 'clip/integration-run';
    const endMicroseconds = Math.min(sourceClip.durationMicroseconds, 500_000);
    const materialMap = inspection.metadata.materialSlots.map((slot) => ({
      sourceMaterialSlot: slot.sourceMaterialSlot,
      ...(slot.sourceMaterialName === undefined
        ? {}
        : { sourceMaterialName: slot.sourceMaterialName }),
      voxelMaterialSlot: 1,
    }));
    const prepared = await client.prepareVoxelObjectConversion({
      expectedProjectHash: material.project.identity.projectHash,
      sourceKind: 'animated',
      sourceAssetId: 'mesh-animation/kenney-retro-character-medium',
      source: { scope: 'project', path: animatedSourceFile },
      targetAssetId: 'voxel-object/integration-character',
      license: { scope: 'project', path: animatedLicenseFile },
      settings: objectConversionSettings(
        'material/integration-character-voxel',
        1,
        materialMap,
        [8, 12, 8],
      ),
      clips: [{
        sourceClipName: sourceClip.name,
        outputClipId,
        outputName: 'Integration Run',
        sampleRateHz: 4,
        startMicroseconds: 0,
        endMicroseconds,
        endPolicy: 'includeClipEnd',
      }],
      defaultClip: outputClipId,
      frame: { kind: 'clip', clipId: outputClipId, frameIndex: 0 },
      maxPreviewSamples: 1,
    });
    assertCompleteObjectProjection(prepared, 'voxel-object/integration-character');
    assert.ok(prepared.preview.storedFrameCount >= 2);
    assert.equal(prepared.preview.selectedFrame.samplesTruncated, true);
    const previewed = await client.previewVoxelObjectConversion({
      planId: prepared.plan.planId,
      expectedPlanHash: prepared.plan.planHash,
      frame: { kind: 'clip', clipId: outputClipId, frameIndex: 1 },
      maxPreviewSamples: 1,
    });
    assert.equal(previewed.preview.selectedFrame.selection.kind, 'clip');
    assert.equal(previewed.preview.selectedFrame.selection.frameIndex, 1);
    assertCompleteObjectProjection(previewed, 'voxel-object/integration-character');

    const applied = await client.applyVoxelObjectConversion({
      expectedProjectHash: material.project.identity.projectHash,
      planId: prepared.plan.planId,
      expectedPlanHash: prepared.plan.planHash,
      expectedOutputHash: prepared.preview.outputHash,
    });
    const asset = objectAsset(applied.project, 'voxel-object/integration-character');
    assert.equal(asset.defaultClip, outputClipId);
    assert.equal(asset.clips.length, 1);
    assert.equal(
      asset.clips[0]?.frames.length,
      prepared.preview.clips[0]?.storedFrameCount,
    );
    assert.deepEqual(asset.materialMap, materialMap);
    assert.equal(asset.provenance.kind, 'convertedAnimatedMesh');
    assert.equal(asset.provenance.licensePath, animatedLicenseFile);
    assert.equal(asset.provenance.sourceClips[0]?.sourceClipName, sourceClip.name);
    assert.equal(asset.provenance.sourceClips[0]?.endMicroseconds, endMicroseconds);

    const placement = await client.prepareVoxelObjectPlacement({
      expectedProjectHash: applied.project.identity.projectHash,
      assetId: asset.assetId,
      expectedObjectContentHash: asset.contentHash,
    });
    assertPlacementResources(placement, asset);

    const attached = await client.attachVoxelObjectInstance({
      expectedProjectHash: applied.project.identity.projectHash,
      sceneId: 'scene/loading-bay',
      instance: {
        instanceId: 'integration-character-object',
        voxelObjectAssetId: asset.assetId,
        surfaceMode: 'greedyCubes',
        frame: { kind: 'clip', clipId: outputClipId, frameIndex: 1 },
        translation: [4, 1, 8],
        rotation: [0, 0, 0, 1],
        scale: [0.5, 0.5, 0.5],
        materialOverrides: [],
      },
    });
    const instance = objectInstance(attached.project, 'integration-character-object');
    assert.ok(attached.project.sceneHierarchy.nodes.some(
      (node) => node.entityId === instance.ownerEntityId,
    ));
    const durableCreate = objectOperationForAsset(
      attached.project.projection,
      'createVoxelObjectInstance',
      asset.assetId,
    );
    assert.ok(durableCreate !== undefined);
    const durableRuntimeFrame = durableCreate.instance.frame;
    const durableBytes = await readFile(projectPath);
    const durableProjectHash = attached.project.identity.projectHash;

    const scrubbed = await client.previewVoxelObjectInstance({
      expectedProjectHash: durableProjectHash,
      sceneId: 'scene/loading-bay',
      instanceId: instance.instance.instanceId,
      nowMicroseconds: 1_000,
      command: {
        kind: 'scrub',
        clipId: outputClipId,
        clipFrame: 0,
        loopMode: 'repeat',
      },
    });
    assert.equal(scrubbed.playback.status, 'paused');
    assert.equal(scrubbed.playback.clipFrame, 0);
    assert.ok(objectOperation(scrubbed.projection, 'setVoxelObjectFrame') !== undefined);
    assert.equal(objectOperation(scrubbed.projection, 'createVoxelObjectInstance'), undefined);
    const playing = await client.previewVoxelObjectInstance({
      expectedProjectHash: durableProjectHash,
      sceneId: 'scene/loading-bay',
      instanceId: instance.instance.instanceId,
      nowMicroseconds: 2_000,
      command: { kind: 'play' },
    });
    assert.equal(playing.playback.status, 'playing');
    assert.deepEqual(playing.projection.ops, []);
    const paused = await client.previewVoxelObjectInstance({
      expectedProjectHash: durableProjectHash,
      sceneId: 'scene/loading-bay',
      instanceId: instance.instance.instanceId,
      nowMicroseconds: 2_001,
      command: { kind: 'pause' },
    });
    assert.equal(paused.playback.status, 'paused');
    assert.deepEqual(paused.projection.ops, []);
    await client.previewVoxelObjectInstance({
      expectedProjectHash: durableProjectHash,
      sceneId: 'scene/loading-bay',
      instanceId: instance.instance.instanceId,
      nowMicroseconds: 3_000,
      command: { kind: 'play' },
    });
    const duration = asset.clips[0].frames[0].durationMicroseconds;
    const sampled = await client.previewVoxelObjectInstance({
      expectedProjectHash: durableProjectHash,
      sceneId: 'scene/loading-bay',
      instanceId: instance.instance.instanceId,
      nowMicroseconds: 3_000 + duration,
      command: { kind: 'sample' },
    });
    assert.equal(sampled.playback.status, 'playing');
    assert.equal(sampled.playback.clipFrame, 1);
    assert.ok(objectOperation(sampled.projection, 'setVoxelObjectFrame') !== undefined);
    const restored = await client.previewVoxelObjectInstance({
      expectedProjectHash: durableProjectHash,
      sceneId: 'scene/loading-bay',
      instanceId: instance.instance.instanceId,
      nowMicroseconds: 3_000 + duration,
      command: { kind: 'stop' },
    });
    assert.equal(restored.playback.status, 'stopped');
    assert.equal(restored.playback.runtimeFrame, durableRuntimeFrame);
    assert.deepEqual(await readFile(projectPath), durableBytes);
    assert.equal(restored.playback.projectHash, durableProjectHash);

    persisted = {
      authoring: structuredClone(attached.project.voxelObjectAuthoring),
      bytes: durableBytes,
      projectHash: durableProjectHash,
    };
    evidence = {
      sourceClip: sourceClip.name,
      storedFrames: asset.clips[0].frames.length,
      aggregateVoxels: prepared.preview.aggregateVoxelCount,
      ownerEntityId: instance.ownerEntityId,
      projectionOperations: [
        ...new Set([
          ...objectOperationNames(attached.project.projection),
          ...objectOperationNames(scrubbed.projection),
          ...objectOperationNames(sampled.projection),
        ]),
      ],
      playbackFrames: [scrubbed.playback.clipFrame, sampled.playback.clipFrame],
    };
    await client.closeProject();
  });

  assert.ok(persisted !== undefined);
  assert.ok(evidence !== undefined);
  await withClient(binary, async (client) => {
    const reopened = (await client.openProject(root, animatedProjectFile)).project;
    assert.equal(reopened.identity.projectHash, persisted.projectHash);
    assert.deepEqual(reopened.voxelObjectAuthoring, persisted.authoring);
    assert.deepEqual(await readFile(projectPath), persisted.bytes);
    await expectRejection(client.previewVoxelObjectInstance({
      expectedProjectHash: reopened.identity.projectHash,
      sceneId: 'scene/loading-bay',
      instanceId: 'integration-character-object',
      nowMicroseconds: 10_000,
      command: { kind: 'sample' },
    }), 'voxelObject.playbackNotSelected');
    await client.closeProject();
  });
  return { ...evidence, freshProcessReadoutMatched: true, playbackChangedBytes: false };
}

async function prepareStaticObject(client, expectedProjectHash) {
  return client.prepareVoxelObjectConversion({
    expectedProjectHash,
    sourceKind: 'static',
    sourceAssetId: 'mesh/kenney-wall-a',
    source: { scope: 'project', path: staticSourceFile },
    targetAssetId: 'voxel-object/integration-wall',
    license: { scope: 'project', path: licenseFile },
    meshPrimitive: 'group/0',
    settings: objectConversionSettings(
      'material/wall-lines',
      7,
      [{
        sourceMaterialSlot: 0,
        sourceMaterialName: 'wall_lines',
        voxelMaterialSlot: 7,
      }],
      [32, 24, 16],
      0.125,
    ),
    clips: [],
    frame: { kind: 'default' },
    maxPreviewSamples: 1,
  });
}

function objectConversionSettings(materialAssetId, materialSlot, materialMap, resolution, cellSize = 1) {
  return {
    mesh: {
      conversion: {
        resolution,
        cellSize,
        chunkSize: 16,
        origin: [0, 0, 0],
        fitPolicy: 'contain',
        originPolicy: 'sourceOrigin',
        mode: 'surface',
        materialPalette: [{
          materialSlot,
          materialAssetId,
          displayName: 'Integration material',
        }],
        materialMap,
        maxOutputVoxels: resolution.reduce((total, value) => total * value, 1),
      },
      transform: [
        1, 0, 0, 0,
        0, 1, 0, 0,
        0, 0, 1, 0,
        0, 0, 0, 1,
      ],
      materialPolicy: { textureAssets: [], textureBindings: [] },
    },
    pivot: [0, 0, 0],
    anchorPolicy: { kind: 'preserveSourceSpace' },
  };
}

function materialDefinition() {
  return {
    authority: {
      solid: true,
      collidable: true,
      occludes: true,
      structuralClass: 'solid',
    },
    style: {
      color: [0.25, 0.8, 0.45, 1],
      texture: null,
      textureTint: [1, 1, 1, 1],
      emissionColor: [0, 0, 0, 1],
      roughness: 0.7,
      emissive: 0,
      uvStrategy: 'flat',
    },
  };
}

function assertCompleteObjectProjection(response, assetId) {
  assert.equal(response.projectionReadout.frameKind, 'complete');
  assert.ok(response.projection.ops.length > 0);
  assert.ok(objectOperationForAsset(response.projection, 'defineVoxelObject', assetId) !== undefined);
  assert.ok(objectOperationForAsset(
    response.projection,
    'createVoxelObjectInstance',
    assetId,
  ) !== undefined);
  assert.ok(Buffer.byteLength(JSON.stringify(response.projection), 'utf8') < 32 * 1024 * 1024);
}

function assertPlacementResources(response, asset) {
  assert.equal(response.assetId, asset.assetId);
  assert.equal(response.objectContentHash, asset.contentHash);
  const allowed = new Set(['defineMaterial', 'defineTexture', 'defineVoxelObject']);
  assert.equal(response.resourceFrame.ops.every((operation) => allowed.has(operation.op)), true);
  const definitions = response.resourceFrame.ops.filter(
    (operation) => operation.op === 'defineVoxelObject',
  );
  assert.equal(definitions.length, 1);
  assert.equal(definitions[0].asset.asset, asset.assetId);
  assert.equal(definitions[0].asset.contentHash, asset.contentHash);
  assert.equal(response.resourceFrame.ops.some(
    (operation) => operation.op === 'createVoxelObjectInstance',
  ), false);
}

function objectOperation(frame, kind) {
  return frame.ops.find((operation) => operation.op === kind);
}

function objectOperationForAsset(frame, kind, assetId) {
  return frame.ops.find((operation) => {
    if (operation.op !== kind) return false;
    if (kind === 'defineVoxelObject') return operation.asset.asset === assetId;
    return operation.instance.asset === assetId;
  });
}

function objectOperationNames(frame) {
  return [...new Set(frame.ops
    .map((operation) => operation.op)
    .filter((operation) => [
      'defineVoxelObject',
      'createVoxelObjectInstance',
      'setVoxelObjectFrame',
    ].includes(operation)))];
}

function objectAsset(project, assetId) {
  const asset = project.voxelObjectAuthoring.assets.find((entry) => entry.assetId === assetId);
  assert.ok(asset !== undefined, `missing voxel-object asset ${assetId}`);
  return asset;
}

function objectInstance(project, instanceId) {
  const instance = project.voxelObjectAuthoring.instances.find(
    (entry) => entry.instance.instanceId === instanceId,
  );
  assert.ok(instance !== undefined, `missing voxel-object instance ${instanceId}`);
  return instance;
}

function staticVoxelCount(authoring) {
  const asset = authoring.assets.find((entry) => entry.assetId === 'voxel-object/integration-wall');
  return asset?.defaultFrame.voxelCount ?? 0;
}

async function expectRejection(promise, code) {
  await assert.rejects(promise, (error) => {
    assert.ok(error instanceof StudioAdapterOperationRejected);
    assert.equal(error.rejection.code, code);
    return true;
  });
}

async function withClient(binary, action) {
  const transport = new JsonLineProcessTransport(binary);
  try {
    const client = new StudioAdapterClient(transport);
    await client.describe();
    return await action(client);
  } finally {
    await transport.close();
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

await main();
