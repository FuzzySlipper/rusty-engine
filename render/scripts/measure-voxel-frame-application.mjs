import { readdir, readFile } from 'node:fs/promises';
import { performance } from 'node:perf_hooks';

import { decodeRenderFrameDiff } from '../packages/render-contracts/dist/index.js';
import { RenderProjection } from '../packages/render-projection/dist/index.js';
import { ThreeRenderer } from '../packages/renderer-three/dist/index.js';

const directory = process.argv[2];
if (directory === undefined) {
  throw new Error('usage: measure-voxel-frame-application.mjs FRAME_DIRECTORY');
}

const repetitions = 7;
console.log('frame,decode_retained_median_us,three_apply_median_us');
for (const name of (await readdir(directory)).filter((value) => value.endsWith('.json')).sort()) {
  const bytes = await readFile(`${directory}/${name}`, 'utf8');
  const pair = JSON.parse(bytes);
  const updateBytes = JSON.stringify(pair.update);
  const retainedSamples = [];
  const threeSamples = [];
  for (let iteration = 0; iteration < repetitions; iteration += 1) {
    const retainedBase = decodeRenderFrameDiff(pair.base);
    const projection = new RenderProjection();
    projection.applyFrame(retainedBase);
    let started = performance.now();
    const retainedUpdate = decodeRenderFrameDiff(JSON.parse(updateBytes));
    projection.applyFrame(retainedUpdate);
    retainedSamples.push((performance.now() - started) * 1_000);

    const threeBase = decodeRenderFrameDiff(pair.base);
    const renderer = new ThreeRenderer();
    renderer.applyFrame(threeBase);
    started = performance.now();
    const threeUpdate = decodeRenderFrameDiff(JSON.parse(updateBytes));
    renderer.applyFrame(threeUpdate);
    threeSamples.push((performance.now() - started) * 1_000);
    renderer.dispose();
  }
  console.log(`${name},${median(retainedSamples).toFixed(0)},${median(threeSamples).toFixed(0)}`);
}

function median(samples) {
  const values = [...samples].sort((left, right) => left - right);
  return values[Math.floor(values.length / 2)];
}
