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
