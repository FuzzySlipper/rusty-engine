// Host the wasm renderer: fetch the capture, hand its bytes over, and call
// `frame` from requestAnimationFrame. No drawing happens here.
import init, { Viewer } from './pkg/render_wgpu_web.js';

const params = new URLSearchParams(location.search);
const FRAMES = Number(params.get('frames') ?? 600);
const WIDTH = Number(params.get('width') ?? 1280);
const HEIGHT = Number(params.get('height') ?? 720);
const status = document.getElementById('status');
const spike = (window.__spike = { state: 'loading', webgpu: 'gpu' in navigator });

const bytes = async (url) => new Uint8Array(await (await fetch(url)).arrayBuffer());
const median = (values, q = 0.5) => {
  const sorted = [...values].sort((a, b) => a - b);
  return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * q))];
};

try {
  const t0 = performance.now();
  await init();
  const t1 = performance.now();
  const canvas = document.getElementById('engine');
  canvas.width = WIDTH;
  canvas.height = HEIGHT;
  canvas.style.width = `${WIDTH}px`;
  canvas.style.height = `${HEIGHT}px`;
  const viewer = await Viewer.create(canvas);
  const t2 = performance.now();
  const manifest = await (await fetch('./capture/manifest.json')).json();
  let resourceBytes = 0;
  for (const name of manifest.resources) {
    const data = await bytes(`./capture/resources/${name}`);
    resourceBytes += data.length;
    viewer.add_resource(name, data);
  }
  const world = await bytes('./capture/world-frame.json');
  const view = await bytes('./capture/view.json');
  const t3 = performance.now();
  const report = JSON.parse(viewer.load(world, view));
  const t4 = performance.now();
  Object.assign(spike, {
    state: 'drawing',
    load: {
      ...report,
      wasmInitMs: t1 - t0,
      deviceMs: t2 - t1,
      fetchMs: t3 - t2,
      loadCallMs: t4 - t3,
      worldFrameBytes: world.length,
      resourceBytes,
    },
  });

  const cpu = [];
  const intervals = [];
  let last = null;
  let frame = 0;
  const tick = (now) => {
    if (last !== null) intervals.push(now - last);
    last = now;
    // Frame 0 is drawn at presentation time 0, as the native reference is.
    cpu.push(viewer.frame(frame / 60));
    frame += 1;
    status.textContent = `frame ${frame}  cpu ${cpu.at(-1).toFixed(2)} ms`;
    if (frame === 1) spike.firstFrameMs = performance.now() - t0;
    if (frame < FRAMES) {
      requestAnimationFrame(tick);
    } else {
      // Leave the time-0 frame on the canvas for the capture pair.
      viewer.frame(0);
      status.textContent = '';
      Object.assign(spike, {
        state: 'done',
        frames: frame,
        cpuMs: { median: median(cpu), p90: median(cpu, 0.9), max: Math.max(...cpu) },
        rafMs: { median: median(intervals), p90: median(intervals, 0.9), max: Math.max(...intervals) },
      });
    }
  };
  requestAnimationFrame(tick);
} catch (error) {
  spike.state = 'failed';
  spike.error = String(error?.stack ?? error);
  status.textContent = spike.error;
}
