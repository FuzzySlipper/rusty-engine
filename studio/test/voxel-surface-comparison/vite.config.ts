import { readFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { dirname, isAbsolute, resolve } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const report = process.env['RUSTY_SURFACE_REPORT'];
if (report === undefined || !isAbsolute(report)) {
  throw new Error('RUSTY_SURFACE_REPORT must be an absolute path');
}

export default {
  root: here,
  resolve: {
    alias: {
      '@rusty-engine/render-contracts': resolve(here, '../../../render/packages/render-contracts'),
      '@rusty-engine/renderer-host': resolve(here, '../../../render/packages/renderer-host'),
      '@rusty-engine/renderer-three/backend': resolve(
        here,
        '../../../render/packages/renderer-three/dist/backend.js',
      ),
      '@rusty-engine/renderer-three': resolve(here, '../../../render/packages/renderer-three'),
      '@rusty-engine/render-projection': resolve(here, '../../../render/packages/render-projection'),
      '@rusty-engine/studio-viewport/submission': resolve(
        here,
        '../../libs/viewport/dist/viewport-submission.js',
      ),
      '@rusty-engine/studio-viewport': resolve(here, '../../libs/viewport'),
    },
  },
  plugins: [{
    name: 'voxel-surface-report',
    configureServer(server) {
      server.middlewares.use('/comparison.json', async (_request, response) => {
        response.setHeader('Content-Type', 'application/json');
        response.end(await readFile(report));
      });
    },
  }],
};
