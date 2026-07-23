import { fileURLToPath } from "node:url";

import { defineConfig } from "vite";

/**
 * Asha's renderer exposes one optional encoded-frame convenience through its
 * large runtime bridge. Rusty Engine supplies already typed render diffs, so
 * that entry point is deliberately fail-closed and the old runtime facade is
 * excluded from the product bundle.
 */
export default defineConfig({
  resolve: {
    alias: {
      "@asha/runtime-bridge": fileURLToPath(
        new URL("./src/renderer-runtime-shim.ts", import.meta.url),
      ),
    },
  },
  build: {
    chunkSizeWarningLimit: 700,
  },
});
