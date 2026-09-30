import type {
  ProductDevDebugCatalog,
  ProductDevRendererInspection,
  ProductDevTimeAnswer,
  ProductDevCameraPose,
} from './generated/contracts.js';

type Camera = ProductDevCameraPose;
const TIME_MODES = ['realtime', 'manual', 'action-driven'] as const satisfies readonly ProductDevTimeAnswer['mode'][];
const DRAWING_MODES = ['continuous', 'on-demand'] as const satisfies readonly ProductDevRendererInspection['drawing'][];
export interface PlaytestInspectionRequest {
  op: 'discover' | 'observe' | 'action' | 'look' | 'time' | 'advance' | 'drawing' | 'frame' | 'camera' | 'targets' | 'route' | 'flush' | 'focus' | 'interaction' | 'grid' | 'probe' | 'jump-plan' | 'clearance';
  mode?: string; id?: string; ms?: number; yaw?: number; pitch?: number; camera?: Camera | null;
  x?: number; y?: number; z?: number; radius?: number; verticalRadius?: number; cellSize?: number; distance?: number;
  move?: readonly [number, number, number]; lookAt?: readonly [number, number, number]; orbit?: { target: readonly [number, number, number]; yaw: number };
}
type InspectionState = ProductDevRendererInspection;
type InspectionRequest = { drawing?: ProductDevRendererInspection['drawing']; simulationMs?: number | null; camera?: Camera | null };
/** Where drawing, the observer camera and explicit frames are answered. */
interface Presenter {
  inspect(request: InspectionRequest): Promise<InspectionState>;
  /** Draw one frame of the current state, even on demand, and show it. */
  draw(): Promise<void>;
  /** Draw and describe the drawn frame. */
  frame(): Promise<unknown>;
}
/** The streamed frame the Engine canvas shows. */
const shownFrameSequence = (): number =>
  Number(document.querySelector<HTMLCanvasElement>('canvas[data-rusty-application-renderer="engine-owned"]')?.dataset['rustyFrameSequence'] ?? 0);
const FRAME_SHOWN_WAIT_MS = 2000;
/**
 * Engine-owned browser inspection adapter; gameplay remains in the product.
 * The world is rendered in the runtime: inspection goes to its
 * `engine.renderer.*` commands, and each answer waits until the canvas shows
 * the frame the command drew.
 */
export function installPlaytestInspection(flushInput: () => Promise<void>, settle: (through?: string) => Promise<void>): () => void {
  const target = globalThis as typeof globalThis & { __rustyPlaytest?: (request: PlaytestInspectionRequest) => Promise<unknown> };
  async function debug(command: string): Promise<unknown> {
    const response = await fetch('/__rusty/product/runtime/debug/execute', { method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: command });
    const text = await response.text(); if (!response.ok) throw new Error(text);
    const through = response.headers.get('x-rusty-output-through');
    await settle(through ?? undefined);
    return JSON.parse(text);
  }
  const shown = async (answer: InspectionState): Promise<InspectionState> => {
    const sequence = answer.frame?.sequence ?? 0;
    const deadline = performance.now() + FRAME_SHOWN_WAIT_MS;
    while (shownFrameSequence() < sequence && performance.now() < deadline) {
      await new Promise((resolve) => requestAnimationFrame(resolve));
    }
    return answer;
  };
  const presenter: Presenter = {
    inspect: async (request) => {
      if (request.drawing !== undefined) await debug(`engine.renderer.drawing ${request.drawing}`);
      if (request.camera === null) return shown(await debug('engine.renderer.camera none') as InspectionState);
      if (request.camera !== undefined) {
        const { position: [x, y, z], yawDegrees, pitchDegrees } = request.camera;
        return shown(await debug(`engine.renderer.camera ${x} ${y} ${z} ${yawDegrees} ${pitchDegrees}`) as InspectionState);
      }
      // Held simulation time is the runtime's own; there is nothing to set.
      return debug('engine.renderer.camera') as Promise<InspectionState>;
    },
    draw: async () => { await shown(await debug('engine.renderer.frame') as InspectionState); },
    frame: async () => { await presenter.draw(); return debug('engine.renderer.presentation'); },
  };
  const invoke = async (request: PlaytestInspectionRequest): Promise<unknown> => {
    const id = request.id ?? '';
    if (id && !/^[A-Za-z0-9_-]{1,64}$/.test(id)) throw new Error('invalid target/action id');
    switch (request.op) {
      case 'discover': {
        const catalog = await fetch('/__rusty/product/runtime/debug/catalog').then(r => r.json()) as ProductDevDebugCatalog;
        const names = catalog.commands.map(c => c.name);
        return { commands: names, nativeCommands: names, operations: ['discover','observe','action','look','time','advance','drawing','frame','camera','targets','route','focus', ...(names.includes('interaction.inspect') ? ['interaction'] : []), ...(names.includes('spatial.grid') ? ['grid'] : []), ...(names.includes('spatial.probe') ? ['probe'] : []), ...(names.includes('spatial.clearance') ? ['clearance'] : []), ...(names.includes('playtest.jump-plan') ? ['jump-plan'] : [])], commandNote: 'nativeCommands are debug catalog names, not assist operations; act/jump/survey/record are harness compositions', time: names.includes('engine.time'), timeModes: TIME_MODES, drawingModes: DRAWING_MODES, observer: ['pose', 'move', 'lookAt', 'orbit', 'restore'], lookAdvancesTime: false, inspection: true, product: names.includes('playtest.help') ? await debug('playtest.help') : null };
      }
      case 'focus': return { focused: document.activeElement?.tagName === 'CANVAS', pointerLocked: document.pointerLockElement !== null };
      case 'flush': await flushInput(); return { flushed: true };
      case 'observe': return debug('playtest.observe');
      case 'interaction': return debug('interaction.inspect');
      case 'grid': {
        const radius = request.radius ?? 4, vertical = request.verticalRadius ?? 4, size = request.cellSize ?? 0.25;
        if (!Number.isInteger(radius) || !Number.isInteger(vertical) || radius < 0 || vertical < 0 || radius > 15 || vertical > 15 || !Number.isFinite(size) || size < .125 || size > 2) throw new Error('invalid grid dimensions; radii0..15, cellSize.125..2');
        return debug(`spatial.grid ${radius} ${vertical} ${size}`);
      }
      case 'probe': {
        const distance = request.distance ?? 2;
        if (!Number.isFinite(distance) || distance <= 0 || distance > 8) throw new Error('probe distance must be in (0,8]');
        return debug(`spatial.probe ${distance}`);
      }
      case 'clearance':
      case 'jump-plan': {
        if (![request.x, request.y, request.z].every(v => typeof v === 'number' && Number.isFinite(v))) throw new Error(`${request.op} requires finite x/y/z target feet coordinates`);
        return debug(`${request.op === 'clearance' ? 'spatial.clearance' : 'playtest.jump-plan'} ${request.x} ${request.y} ${request.z}`);
      }

      case 'action': return debug(`playtest.action ${id}`);
      case 'targets': return debug('navigation.targets');
      case 'route': {
        if (!id) throw new Error('route requires id from targets; example: {op:"route",id:"door-north-wing"}');
        return debug(`navigation.route ${id}`);
      }
      case 'look': {
        const yaw = request.yaw ?? 0, pitch = request.pitch ?? 0;
        if (![yaw, pitch].every(Number.isFinite) || Math.abs(yaw) > 360 || Math.abs(pitch) > 180) throw new Error('look degrees exceed bounds');
        const result = await debug(`playtest.look ${yaw} ${pitch}`); await settle(); await presenter.draw(); return result;
      }
      case 'time': {
        if (request.mode && !(TIME_MODES as readonly string[]).includes(request.mode)) throw new Error('unknown time mode');
        if (request.mode) await flushInput();
        const result = await debug(request.mode ? `engine.time.mode ${request.mode}` : 'engine.time') as ProductDevTimeAnswer;
        await presenter.inspect({ simulationMs: result.mode === 'realtime' ? null : Number(result.simulationStep) * 1000 / result.fixedStepHz }); return result;
      }
      case 'advance': {
        if (!Number.isFinite(request.ms) || request.ms! <= 0 || request.ms! > 2000) throw new Error('advance ms must be in (0, 2000]');
        await flushInput();
        const result = await debug(`engine.time.advance ${request.ms}`) as ProductDevTimeAnswer;
        await settle();
        const state = await presenter.inspect({ simulationMs: Number(result.simulationStep) * 1000 / result.fixedStepHz });
        if (state.drawing !== 'on-demand') await presenter.draw(); return result;
      }
      case 'drawing': {
        if (!(DRAWING_MODES as readonly string[]).includes(request.mode ?? '')) throw new Error('drawing mode must be continuous or on-demand');
        return presenter.inspect({ drawing: request.mode as ProductDevRendererInspection['drawing'] });
      }
      case 'frame': await settle(); return presenter.frame();
      case 'camera': {
        if (request.camera && ![...request.camera.position, request.camera.yawDegrees, request.camera.pitchDegrees].every(Number.isFinite)) throw new Error('camera must be finite');
        let camera = request.camera;
        if (request.move || request.lookAt || request.orbit || request.yaw !== undefined || request.pitch !== undefined) {
          const current = camera ?? (await presenter.inspect({})).camera;
          if (current === null) throw new Error('no camera to move from; set an absolute camera');
          let position: [number, number, number] = [...current.position];
          if (request.move) position = position.map((v, i) => v + request.move![i]!) as typeof position;
          if (request.orbit) {
            const [x, , z] = position.map((v, i) => v - request.orbit!.target[i]!);
            const angle = request.orbit.yaw * Math.PI / 180;
            position = [request.orbit.target[0] + x! * Math.cos(angle) - z! * Math.sin(angle), position[1], request.orbit.target[2] + x! * Math.sin(angle) + z! * Math.cos(angle)];
          }
          let yawDegrees = current.yawDegrees + (request.yaw ?? 0), pitchDegrees = current.pitchDegrees + (request.pitch ?? 0);
          const target = request.lookAt ?? request.orbit?.target;
          if (target) {
            const [x, y, z] = target.map((v, i) => v - position[i]!);
            yawDegrees = Math.atan2(x!, -z!) * 180 / Math.PI;
            pitchDegrees = Math.atan2(y!, Math.hypot(x!, z!)) * 180 / Math.PI;
          }
          camera = { position, yawDegrees, pitchDegrees };
        }
        if (camera && (camera.position.length !== 3 || ![...camera.position, camera.yawDegrees, camera.pitchDegrees].every(Number.isFinite))) throw new Error('camera must be finite');
        const result = await presenter.inspect(camera === undefined ? {} : { camera });
        await settle(); await presenter.draw(); return result;
      }
      default: throw new Error('unknown inspection operation');
    }
  };
  target.__rustyPlaytest = invoke;
  return () => { if (target.__rustyPlaytest === invoke) delete target.__rustyPlaytest; };
}
