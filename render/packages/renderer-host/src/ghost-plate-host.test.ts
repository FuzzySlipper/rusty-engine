import assert from 'node:assert/strict';
import test from 'node:test';

import type {
  GhostPlateDescriptor,
  GhostPlateProjectionOp,
  PresentationFrameDiff,
} from '@rusty-engine/render-contracts';

import { RendererGhostPlateHost, type RendererGhostPlatePresentation } from './ghost-plate-host.js';

const descriptor: GhostPlateDescriptor = {
  source: 7 as GhostPlateDescriptor['source'],
  placement: { transform: { translation: [1, 2, 3], rotation: [0, 0, 0, 1], scale: [1, 1, 1] }, width: 2, height: 3 },
  capture: {
    resolution: 64, azimuthDegrees: 0, elevationDegrees: 10, near: 0.1, far: 20, fieldOfViewDegrees: 35,
    lighting: { mode: 'isolated', ambientColor: [1, 1, 1], ambientIntensity: 1, keyDirection: [1, 1, 1], keyColor: [1, 1, 1], keyIntensity: 2, fillDirection: [-1, 1, 1], fillColor: [1, 1, 1], fillIntensity: 1 },
  },
  config: { depthRetention: 0.15, anchorPolicy: 'bounds-center', anchorValue: 0.5, plateMapping: 'plate-locked', shellMode: 'whole-mesh', shellDepthEpsilon: 0.12, sectorCount: 8, sectorHysteresisDegrees: 3 },
};

void test('typed ghost plate host preserves the live presentation when a recapture fails and exposes focused readout', () => {
  const created: FakePresentation[] = [];
  const host = new RendererGhostPlateHost({
    createPresentation: () => {
      const presentation = new FakePresentation();
      created.push(presentation);
      return presentation;
    },
  });
  const handle = 3 as GhostPlateProjectionOp['handle'];
  assert.equal(host.applyPresentation(frame([{ op: 'create', handle, descriptor }])).applied, 1);
  assert.equal(host.applyPresentation(frame([{ op: 'update', handle, patch: { config: { ...descriptor.config, sectorCount: 16 } } }])).applied, 1);
  const failed = host.applyPresentation(frame([{ op: 'recapture', handle, capture: { ...descriptor.capture, resolution: 13 } }]));
  assert.equal(failed.applied, 0);
  assert.equal(host.readout().activePlates, 1);
  assert.equal(host.readout().plates[0]?.currentSector, 5);
  assert.equal(host.readout().plates[0]?.config.sectorCount, 16);
  assert.equal(host.applyPresentation(frame([{ op: 'destroy', handle }])).applied, 1);
  assert.equal(created[0]?.disposed, true);
});

function frame(ops: readonly GhostPlateProjectionOp[]): PresentationFrameDiff {
  return {
    schemaVersion: 1,
    ops: ops.map((op, sequence) => ({ domain: 'ghostPlate' as const, meta: { sequence }, op })),
  };
}

class FakePresentation implements RendererGhostPlatePresentation {
  descriptor: GhostPlateDescriptor | null = null;
  disposed = false;

  create(value: GhostPlateDescriptor) { this.descriptor = value; return applied(); }
  update(patch: { readonly config?: GhostPlateDescriptor['config'] }) {
    if (this.descriptor === null) return rejected();
    this.descriptor = { ...this.descriptor, ...(patch.config === undefined ? {} : { config: patch.config }) };
    return applied();
  }
  recapture(capture: GhostPlateDescriptor['capture'] | null) {
    if (capture?.resolution === 13) return rejected();
    if (this.descriptor !== null && capture !== null) this.descriptor = { ...this.descriptor, capture };
    return applied();
  }
  destroy() { this.descriptor = null; return applied(); }
  dispose() { this.disposed = true; }
  readout() {
    return {
      source: this.descriptor?.source ?? 0,
      sourceMatch: this.descriptor !== null,
      currentSector: 5,
      localAzimuthDegrees: 203,
      capture: this.descriptor?.capture ?? descriptor.capture,
      config: this.descriptor?.config ?? descriptor.config,
      fallbackActive: false,
      fallbackReason: null,
      preparationCpuMilliseconds: 4,
      captureCpuSubmissionMilliseconds: null,
      retainedResourceCounts: { sectors: 16, meshes: 32, materials: 32, borrowedTextures: 48 },
      disposed: this.disposed,
    };
  }
}

function applied() { return { applied: true, diagnostics: [], readout: {} } as const; }
function rejected() { return { applied: false, diagnostics: [{ code: 'capture_failed', message: 'capture failed' }], readout: {} } as const; }
