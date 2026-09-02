# Live debug browser artifact

`pnpm --dir studio run build:live-debug-panel-artifact` writes the optional,
import-closed browser ESM artifact to `studio/artifacts/live-debug-panel`.
Copy that directory into a product only when its developer build explicitly
opts into live debugging; it is not part of an Engine host default.

```ts
import { mountLiveDebugPanel } from './vendor/live-debug-panel/index.js';

const debugPanel = await mountLiveDebugPanel(debugElement, {
  enabled: import.meta.env.DEV,
  presentation: 'dock',
  // Omit transport for the same-origin Engine dev-host endpoint, or inject
  // the product's existing LiveDebugTransport.
});

// When the product-owned UI is removed:
debugPanel.dispose();
```

The artifact contains Angular and the Engine live-debug client. It owns only
this optional DOM UI and forwards raw command lines to the generated product
debug surface. It does not read gameplay state, render game elements, define
commands, or replace a product transport.

For a compact Engine-owned renderer readout, mount the widget beside the
product's canvas or developer UI. `initiallyVisible` is explicit: omitting it
preserves the Engine default of hidden, while `true` establishes a visible
shared state for every mounted widget. The ordinary console commands
`engine.renderer.show`, `.hide`, `.toggle`, and `.status` change that shared
state. The widget only polls the latest admitted renderer observation; it does
not schedule an animation frame or submit rendering work.

```ts
import { mountRendererMetricsWidget } from './vendor/live-debug-panel/index.js';

const metrics = mountRendererMetricsWidget(metricsElement, {
  initiallyVisible: true,
});

// When the product-owned UI is removed:
metrics.dispose();
```
