import { strict as assert } from 'node:assert';
import { test } from 'node:test';

import {
  RuntimeCompositionAuthoringError,
  admitProductKernelComposition,
  bindProductKernelCatalog,
  createProductKernelCatalog,
  engineCapability,
} from './index.js';
import {
  PRODUCT_KERNEL_CATALOG as renderedCatalog,
  productKernel as renderedProductKernel,
  productKernelCapability as renderedProductKernelCapability,
} from './product-kernel-rendered.fixture.js';
import type { ProductKernelCatalogWire } from './index.js';

const rawCatalog = {
  artifact: 'product-kernel',
  schemas: [
    { identity: 'stealth.schema.v1', contractType: 'stealth.schema.v1' },
    { identity: 'stealth.schema.v2', contractType: 'stealth.schema.v2' },
  ],
  migrations: [
    {
      identity: 'stealth.migration.v1-to-v2',
      from: 'stealth.schema.v1',
      to: 'stealth.schema.v2',
      contractType: 'stealth.migration.v1-to-v2',
    },
  ],
  capabilities: [
    {
      identity: 'stealth.advance-alert',
      target: 'kernel.stealth.advance-alert',
      kind: 'operation',
      uses: ['schedule'],
      availability: 'linkable',
      access: { reads: ['stealth.observations'], writes: ['stealth.alerts'] },
      budget: { maximumCompactJsonPayloadBytes: 4096 },
      provenance: {
        owner: 'stealth.product.alerts',
        source: 'src/alerts.ts',
        logicalPath: 'advanceAlert',
      },
      contractType: 'stealth.operation.v1',
    },
    {
      identity: 'stealth.detect',
      target: 'kernel.stealth.detect',
      kind: 'system',
      uses: ['schedule', 'timeline'],
      availability: 'linkable',
      access: { reads: ['stealth.snapshot'], writes: ['stealth.observations'] },
      budget: { maximumCompactJsonPayloadBytes: 4096 },
      provenance: {
        owner: 'stealth.product.detection',
        source: 'src/detection.ts',
        logicalPath: 'detect',
      },
      contractType: 'stealth.system.v1',
    },
  ],
} as const satisfies ProductKernelCatalogWire;

function expectCode(action: () => unknown, code: RuntimeCompositionAuthoringError['code']): void {
  assert.throws(action, (error: unknown) => error instanceof RuntimeCompositionAuthoringError && error.code === code);
}

test('Product Kernel catalog is exact, versionless, detached, and deeply frozen', () => {
  const catalog = createProductKernelCatalog(rawCatalog);
  assert.deepEqual(catalog, rawCatalog);
  assert.ok(Object.isFrozen(catalog));
  assert.ok(Object.isFrozen(catalog.capabilities));
  assert.ok(Object.isFrozen(catalog.capabilities[0]));
  assert.ok(Object.isFrozen(catalog.capabilities[0]?.access));
  assert.ok(Object.isFrozen(catalog.capabilities[0]?.access.reads));
  assert.ok(Object.isFrozen(catalog.capabilities[0]?.budget));
  assert.ok(Object.isFrozen(catalog.capabilities[0]?.provenance));
  assert.throws(() => ((catalog.capabilities[0] as { identity: string }).identity = 'mutated'), TypeError);
});

test('bound Product Kernel helpers expose the closed target union and coexist with Engine bindings', () => {
  const bound = bindProductKernelCatalog(rawCatalog);
  const kernelBinding = bound.capability('alert-operation', 'kernel.stealth.advance-alert');
  const composition = bound.admit({
    product: 'stealth.pressure',
    capabilities: [kernelBinding, engineCapability('projection', 'render.entity-project')],
  });
  assert.deepEqual(composition.composition.capabilityBindings, [
    kernelBinding,
    { id: 'projection', target: 'engine.render.entity-project' },
  ]);
  assert.deepEqual(admitProductKernelComposition(bound.catalog, {
    product: 'stealth.pressure',
    capabilities: [kernelBinding],
  }).composition.capabilityBindings, [kernelBinding]);
});

test('catalog admission rejects unknown Product Kernel targets before startup linkage', () => {
  const bound = bindProductKernelCatalog(rawCatalog);
  expectCode(() => bound.capability('missing', 'kernel.stealth.missing' as never), 'unknown-product-kernel-capability');
  expectCode(() => bound.admit({
    product: 'stealth.pressure',
    capabilities: [{ id: 'missing', target: 'kernel.stealth.missing' }],
  }), 'unknown-product-kernel-capability');
  expectCode(() => bound.admitCompiled({
    product: 'stealth.pressure',
    intentDescriptors: [], inputMap: [], schedule: [
      { phase: 'input', mode: 'append', systems: [] },
      { phase: 'simulation', mode: 'append', systems: [] },
      { phase: 'consequences', mode: 'append', systems: [] },
      { phase: 'commit', mode: 'append', systems: [] },
      { phase: 'projection', mode: 'append', systems: [] },
    ],
    gameplayDefinitions: [], timelines: [],
    capabilityBindings: [{ id: 'missing', target: 'kernel.stealth.missing' }],
  }), 'unknown-product-kernel-capability');
});

test('catalog admission refuses stale, malformed, duplicate, unsorted, and versioned contracts', () => {
  expectCode(() => createProductKernelCatalog({ ...rawCatalog, version: 1 }), 'product-kernel-catalog-invalid');
  expectCode(() => createProductKernelCatalog({
    ...rawCatalog,
    capabilities: [{ ...rawCatalog.capabilities[0], target: 'kernel.stealth.stale' }, rawCatalog.capabilities[1]],
  }), 'product-kernel-catalog-stale');
  expectCode(() => createProductKernelCatalog({
    ...rawCatalog,
    capabilities: [{ ...rawCatalog.capabilities[0], kind: 'migration' as never }, rawCatalog.capabilities[1]],
  }), 'product-kernel-catalog-invalid');
  expectCode(() => createProductKernelCatalog({
    ...rawCatalog,
    capabilities: [rawCatalog.capabilities[0], rawCatalog.capabilities[0]],
  }), 'product-kernel-catalog-invalid');
  expectCode(() => createProductKernelCatalog({
    ...rawCatalog,
    capabilities: [rawCatalog.capabilities[1], rawCatalog.capabilities[0]],
  }), 'product-kernel-catalog-unsorted');
  expectCode(() => createProductKernelCatalog({
    ...rawCatalog,
    schemas: [rawCatalog.schemas[1], rawCatalog.schemas[0]],
  }), 'product-kernel-catalog-unsorted');
  expectCode(() => createProductKernelCatalog({
    ...rawCatalog,
    schemas: [{ ...rawCatalog.schemas[0], contractType: 'stale.schema.contract' }, rawCatalog.schemas[1]],
  }), 'product-kernel-catalog-stale');
  expectCode(() => createProductKernelCatalog({
    ...rawCatalog,
    migrations: [{ ...rawCatalog.migrations[0], from: 'stealth.schema.missing' }],
  }), 'product-kernel-catalog-stale');
  expectCode(() => createProductKernelCatalog({
    ...rawCatalog,
    migrations: [{ ...rawCatalog.migrations[0], contractType: 'stale.migration.contract' }],
  }), 'product-kernel-catalog-stale');
  expectCode(() => createProductKernelCatalog({
    ...rawCatalog,
    migrations: [{ ...rawCatalog.migrations[0], uses: ['schedule'] }],
  }), 'product-kernel-catalog-invalid');
  expectCode(() => createProductKernelCatalog({
    artifact: 'product-kernel',
    capabilities: [{ ...rawCatalog.capabilities[0], access: { reads: ['stealth.snapshot'], writes: [] } }],
  }), 'product-kernel-catalog-invalid');
  const accessor = { ...rawCatalog } as Record<string, unknown>;
  Object.defineProperty(accessor, 'capabilities', { enumerable: true, get: () => rawCatalog.capabilities });
  expectCode(() => createProductKernelCatalog(accessor), 'product-kernel-catalog-invalid');
});

test('unavailable catalog metadata remains inspectable but is excluded from target selection and admission', () => {
  const unavailable = {
    identity: 'stealth.future',
    target: 'kernel.stealth.future',
    kind: 'operation',
    uses: ['schedule'],
    availability: 'unavailable',
    access: { reads: [], writes: ['stealth.future'] },
    budget: { maximumCompactJsonPayloadBytes: 4096 },
    provenance: {
      owner: 'stealth.product.future',
      source: 'src/future.ts',
      logicalPath: 'future',
    },
    contractType: 'stealth.future.v1',
  } as const;
  const catalog = createProductKernelCatalog({
    ...rawCatalog,
    capabilities: [...rawCatalog.capabilities, unavailable],
  } as const);
  assert.equal(catalog.capabilities[2]?.availability, 'unavailable');
  const bound = bindProductKernelCatalog(catalog);
  if (false) {
    // @ts-expect-error unavailable Product Kernel targets are not in the authoring union.
    bound.capability('future', 'kernel.stealth.future');
  }
  expectCode(() => bound.capability('future', 'kernel.stealth.future' as never), 'unknown-product-kernel-capability');
  expectCode(() => bound.admit({
    product: 'stealth.pressure',
    capabilities: [{ id: 'future', target: 'kernel.stealth.future' }],
  }), 'unknown-product-kernel-capability');
});

test('Rust-rendered product module shape compiles and executes through the package root', () => {
  assert.equal(renderedCatalog.artifact, 'product-kernel');
  assert.ok(Object.isFrozen(renderedProductKernel.catalog));
  const binding = renderedProductKernelCapability('alert-operation', 'kernel.stealth.advance-alert');
  const artifact = renderedProductKernel.admit({
    product: 'stealth.pressure',
    capabilities: [binding],
  });
  assert.deepEqual(artifact.composition.capabilityBindings, [binding]);
});
