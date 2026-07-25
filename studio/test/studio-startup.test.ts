import assert from 'node:assert/strict';
import test from 'node:test';

import { readStudioStartupProject } from '../apps/studio-app/src/app/studio-startup.js';

test('startup leaves project selection empty when no pair is supplied', () => {
  assert.deepEqual(readStudioStartupProject('http://127.0.0.1:4300/'), { status: 'none' });
});

test('startup accepts exactly one explicit external root and relative project file', () => {
  assert.deepEqual(
    readStudioStartupProject(
      'http://127.0.0.1:4300/?root=%2Fwork%2Floading-bay&project=content%2Fprojects%2Floading-bay.project.json',
    ),
    {
      status: 'open',
      root: '/work/loading-bay',
      projectFile: 'content/projects/loading-bay.project.json',
    },
  );
});

test('startup rejects partial, duplicate, empty, and bounded selections', () => {
  assert.equal(readStudioStartupProject('?root=/work').status, 'invalid');
  assert.equal(readStudioStartupProject('?root=/a&root=/b&project=p.json').status, 'invalid');
  assert.equal(readStudioStartupProject('?root=%00&project=p.json').status, 'invalid');
  assert.equal(
    readStudioStartupProject(`?root=/${'x'.repeat(4096)}&project=p.json`).status,
    'invalid',
  );
});
