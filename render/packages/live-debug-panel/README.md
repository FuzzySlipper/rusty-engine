# Live-debug panel

Optional developer UI for the generated product debug catalog, written as plain
DOM over `@rusty-engine/live-debug-client`. It forwards raw command lines and
shows engine diagnostics and product/runtime telemetry. It does not read
gameplay state, render game elements or define commands.

`pnpm --dir render run build` writes one
import-closed ES module to `render/artifacts/live-debug-panel` (ignored build
output). The runtime pack installs it at `share/browser/engine/live-debug-panel`,
and its browser shell maps it to the `@rusty-engine/live-debug` import:

```ts
import { mountLiveDebugPanel, mountRendererMetricsWidget } from '@rusty-engine/live-debug';

const debugPanel = await mountLiveDebugPanel(debugElement, {
  enabled: true,
  presentation: 'dock', // 'inline' | 'dock' | 'overlay'
  // Omit transport for the same-origin dev-host endpoints.
});
const metrics = mountRendererMetricsWidget(metricsElement, { initiallyVisible: true });

// When the product-owned UI is removed:
debugPanel.dispose();
metrics.dispose();
```

`initiallyVisible` sets the shared renderer-widget state once; omitting it
keeps the Engine default (hidden). The console commands `engine.renderer.show`,
`.hide`, `.toggle` and `.status` change that shared state.
