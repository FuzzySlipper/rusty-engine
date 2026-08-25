import { mountRustyApplication } from '@rusty-engine/application-host';

import { mountPlaytestProduct } from './product.js';
import { productPlaytestFrame } from './scene.js';

const root = document.querySelector<HTMLElement>('#application');
if (root === null) throw new Error('product playtest application root is missing');

await mountRustyApplication({
  root,
  initialInteractionMode: 'gameplay',
  loadingLabel: 'Preparing Engine-hosted product…',
  failureLabel: 'Engine-hosted product failed to start',
  renderer: {
    clearColor: 0x071012,
    initialFrame: productPlaytestFrame(),
    pixelRatio: 1,
  },
  runtimeInput: {
    binding: {
      runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
      context: 'gameplay.default',
    },
  },
  uiProjection: {
    expectedStream: 'product.hud',
    expectedContract: 'product.hud.v1',
  },
  mountUi: mountPlaytestProduct,
});
