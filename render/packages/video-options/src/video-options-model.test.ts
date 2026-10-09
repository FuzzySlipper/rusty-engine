import assert from 'node:assert/strict';
import { test } from 'node:test';

import {
  createVideoOptionsHttpTransport,
  formatRange,
  groupOptions,
  parseCatalogue,
  shownValue,
  VIDEO_OPTIONS_PATH,
  type ProductVideoOption,
  type VideoOptionsCatalogue,
} from './video-options-model.js';

const catalogue: VideoOptionsCatalogue = {
  version: 1,
  stored: true,
  presets: [{ id: 'low', label: 'Low' }],
  options: [
    {
      id: 'antialiasing',
      label: 'Anti-aliasing',
      group: 'Display',
      description: '',
      kind: 'choice',
      choices: [
        { value: 'off', label: 'Off' },
        { value: '4x', label: '4× MSAA' },
      ],
      value: '4x',
      requested: 'off',
      gameDefault: '4x',
      chosen: true,
      refused: null,
    },
    {
      id: 'gpuCulling',
      label: 'GPU culling',
      group: 'Advanced',
      description: '',
      kind: 'toggle',
      value: false,
      requested: false,
      gameDefault: false,
      chosen: false,
      refused: 'no indirect draws',
    },
  ],
};

const fieldOfView: ProductVideoOption = {
  id: 'fieldOfView',
  label: 'Field of view',
  group: 'Game',
  description: '',
  kind: 'range',
  min: 60,
  max: 110,
  step: 1,
  value: 90,
  onChange: () => {},
};

test('options group in catalogue order with the game options after', () => {
  const groups = groupOptions(catalogue, [fieldOfView]);
  assert.deepEqual(
    groups.map((group) => [group.name, group.rows.map((row) => row.option.id)]),
    [
      ['Display', ['antialiasing']],
      ['Advanced', ['gpuCulling']],
      ['Game', ['fieldOfView']],
    ],
  );
});

test('a game hides and relabels options, and unknown ids are ignored', () => {
  const groups = groupOptions(catalogue, [fieldOfView], {
    hide: ['gpuCulling', 'aSettingFromANewerPair'],
    labels: { antialiasing: 'Edge smoothing', Display: 'Screen' },
  });
  assert.deepEqual(
    groups.map((group) => group.label),
    ['Screen', 'Game'],
  );
  assert.equal(groups[0]?.rows[0]?.label, 'Edge smoothing');
});

test('an Engine row shows what the player asked for, a game row its value', () => {
  const [display, , game] = groupOptions(catalogue, [fieldOfView]);
  assert.equal(shownValue(display!.rows[0]!), 'off');
  assert.equal(shownValue(game!.rows[0]!), 90);
});

test('ranges format with their step and unit', () => {
  assert.equal(formatRange(0.75, 0.05, '×'), '0.75×');
  assert.equal(formatRange(1.25, 0.05, 'm'), '1.25 m');
  assert.equal(formatRange(90, 1), '90');
});

test('the HTTP transport reads and posts changes to the route', async () => {
  const calls: Array<{ url: string; init: RequestInit | undefined }> = [];
  const fake = (async (url: string, init?: RequestInit) => {
    calls.push({ url, init });
    return new Response(JSON.stringify(catalogue), { status: 200 });
  }) as typeof fetch;
  const transport = createVideoOptionsHttpTransport({ origin: 'http://127.0.0.1:1', fetch: fake });
  await transport.read();
  await transport.change({ preset: 'low' });
  assert.equal(calls[0]?.url, `http://127.0.0.1:1${VIDEO_OPTIONS_PATH}`);
  assert.equal(calls[1]?.init?.method, 'POST');
  assert.equal(calls[1]?.init?.body, '{"preset":"low"}');
});

test('a refused change surfaces the host detail', async () => {
  const fake = (async () =>
    new Response(JSON.stringify({
        accepted: false,
        error: { code: 'PRODUCT_HOST_VIDEO_OPTIONS', diagnostic: 'renderScale takes a number' },
      }), {
      status: 400,
    })) as typeof fetch;
  const transport = createVideoOptionsHttpTransport({ fetch: fake });
  await assert.rejects(transport.change({ choose: { id: 'renderScale', value: 9 } }), /renderScale takes a number/);
});

test('a body without options is refused', () => {
  assert.throws(() => parseCatalogue({ presets: [] }), /no options/);
  assert.equal(parseCatalogue(catalogue).options.length, 2);
});
