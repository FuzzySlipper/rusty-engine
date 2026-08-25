import {
  append,
  engineCapability,
  gameplayDefinition,
  inputAction,
  kernelCapability,
  observePairs,
  productIntent,
  schedule,
  Standard,
  system,
  timeline,
  timelineStep,
} from '@rusty-engine/runtime-composition-authoring';

// This is deliberately the smallest complete Product Model composition.  The
// physical W key becomes a typed digital intent; the Product Kernel, rather
// than the DOM, owns the resulting counter mutation.
export default {
  product: 'rusty.product.conformance',
  capabilities: [
    kernelCapability('counter.increment', 'counter-increment'),
    engineCapability('counter.observe', 'runtime.observe-pairs'),
    kernelCapability('counter.observe-result', 'counter-observe'),
    kernelCapability('counter.recurring', 'counter-recurring'),
    kernelCapability('counter.recurring-result', 'counter-recurring-result'),
    kernelCapability('counter.timeline', 'counter-timeline'),
  ],
  intentDescriptors: [
    productIntent({
      id: 'increment',
      valueKind: 'digital',
      capability: 'counter.increment',
      payload: { amount: 1 },
    }),
  ],
  inputMap: [
    inputAction({
      id: 'increment-w',
      intent: 'increment',
      trigger: {
        kind: 'key',
        code: 'key-w',
        edge: 'pressed',
        context: 'gameplay.default',
      },
    }),
  ],
  schedule: schedule({
    simulation: append(
      Standard.simulation,
      observePairs({
        id: 'counter.observe-pairs',
        engineBinding: engineCapability('counter.observe', 'runtime.observe-pairs'),
        operationBinding: kernelCapability('counter.observe-result', 'counter-observe'),
        observerRole: 'counter.observer',
        targetRole: 'counter.target',
        quotas: { observers: 1, targets: 1, pairs: 1, aggregates: 1 },
        cadence: 1,
      }),
      system('counter.recurring', {
        capability: 'counter.recurring',
        after: ['counter.observe-pairs'],
        definition: 'counter.rules',
        reads: ['counter.value'],
        writes: ['counter.recurring-readout'],
        payload: { kind: 'counter.recurring.v1' },
      }),
    ),
  }),
  gameplayDefinitions: [gameplayDefinition('counter.rules', {
    kind: 'counter.rules.v1',
    recurringReadout: 'simulation-step',
    timelineAmount: 2,
  })],
  timelines: [timeline('counter.pulse', [timelineStep({
    id: 'counter.timeline.increment',
    capability: 'counter.timeline',
    payload: { amount: 2 },
  })])],
};
