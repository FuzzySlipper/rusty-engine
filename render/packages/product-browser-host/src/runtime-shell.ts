import { createProductBrowserLocalHttpAdapter } from './local-transport.js';
import { mountProductBrowserHost } from './product-browser-host.js';
import { BOOTSTRAP_PATH, type ProductDevBrowserBootstrap } from './generated/contracts.js';

/**
 * The runtime pack's page: reads the bootstrap the product host writes from
 * the product's manifest, imports the product UI and mounts it over the
 * Engine canvas.
 */
export async function startProductBrowserShell(root: HTMLElement): Promise<void> {
  const response = await fetch(new URL(BOOTSTRAP_PATH, document.baseURI));
  if (!response.ok) throw new Error(`Product bootstrap failed: HTTP ${String(response.status)}`);
  const bootstrap = await response.json() as ProductDevBrowserBootstrap;
  document.title = bootstrap.product.title;
  // `window`: the runtime presents the world to the desktop shell's window
  // under this page, which must let it show through.
  if (bootstrap.renderer.output === 'window') {
    document.documentElement.style.background = 'transparent';
    document.body.style.background = 'transparent';
  }
  // Relative to the page, not to this module under `engine/`.
  const productUi = await import(new URL(bootstrap.ui.entry, document.baseURI).href) as {
    readonly mountProductUi?: Parameters<typeof mountProductBrowserHost>[0]['mountUi'];
  };
  const mountUi = productUi.mountProductUi;
  if (typeof mountUi !== 'function') {
    throw new Error('Product UI entry must export mountProductUi(root, context)');
  }
  await mountProductBrowserHost({
    root,
    transport: createProductBrowserLocalHttpAdapter(),
    lifecycleMode: bootstrap.lifecycle.mode,
    realtimeAdvanceOwner: 'rust-host',
    initialInteractionMode: 'gameplay',
    gameplayCursorMode: bootstrap.input.cursorMode,
    runtimeInput: {
      maximumWheelDelta: 64,
      selectedController: { index: 0 },
    },
    output: bootstrap.renderer.output,
    ...(bootstrap.uiProjection === undefined ? {} : { uiProjection: bootstrap.uiProjection }),
    mountUi: (uiRoot, context) => mountUi(uiRoot, context),
  });
}
