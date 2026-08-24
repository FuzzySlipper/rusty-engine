import { mountRustyApplication } from '@rusty-engine/application-host';

import { mountVignetteProduct } from './product.js';
import { loadVignetteContent } from './scene.js';

const root = document.querySelector<HTMLElement>('#application');
if (root === null) throw new Error('voxel vignette application root is missing');

try {
  const content = await loadVignetteContent();
  root.replaceChildren();
  const host = await mountRustyApplication({
    root,
    initialInteractionMode: 'gameplay',
    presentationAspectBounds: { minimum: 4 / 3, maximum: 16 / 9 },
    loadingLabel: 'Admitting checked visual inputs…',
    failureLabel: 'Voxel vignette could not start',
    renderer: {
      clearColor: 0x89a8b8,
      fog: { color: 0x89a8b8, near: 30, far: 70 },
      initialContent: content,
      pixelRatio: 1,
    },
    runtimeInput: {
      binding: {
        runtime: { instanceId: '7', generation: '3', controlRevision: '11' },
        context: 'gameplay.default',
      },
    },
    uiProjection: {
      expectedStream: 'product.vignette',
      expectedContract: 'product.vignette.v1',
    },
    mountUi: mountVignetteProduct,
  });
  window.addEventListener('pagehide', () => {
    void host.dispose();
  }, { once: true });
} catch (error) {
  const message = error instanceof Error ? error.message : String(error);
  root.replaceChildren();
  const failure = document.createElement('p');
  failure.className = 'vignette-loading vignette-failure';
  failure.setAttribute('role', 'alert');
  failure.textContent = `Visual gate failed before ready: ${message}`;
  root.append(failure);
  throw error;
}
