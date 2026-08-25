import {
  inputAction,
  kernelCapability,
  productIntent,
  schedule,
} from '@rusty-engine/runtime-composition-authoring';

// This is deliberately the smallest complete Product Model composition.  The
// physical W key becomes a typed digital intent; the Product Kernel, rather
// than the DOM, owns the resulting counter mutation.
export default {
  product: 'rusty.product.conformance',
  capabilities: [kernelCapability('counter.increment', 'counter-increment')],
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
  schedule: schedule({}),
  gameplayDefinitions: [],
  timelines: [],
};
