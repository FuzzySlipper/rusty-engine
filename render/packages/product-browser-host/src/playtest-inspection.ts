import type { RustyApplicationRendererPort } from '@rusty-engine/application-host';
type Camera = { position: readonly [number, number, number]; yawDegrees: number; pitchDegrees: number };
export interface PlaytestInspectionRequest {
  op: 'discover' | 'observe' | 'action' | 'look' | 'time' | 'advance' | 'drawing' | 'frame' | 'camera' | 'targets' | 'route' | 'flush' | 'focus';
  mode?: string; id?: string; ms?: number; yaw?: number; pitch?: number; camera?: Camera | null;
  move?: readonly [number, number, number]; lookAt?: readonly [number, number, number]; orbit?: { target: readonly [number, number, number]; yaw: number };
}
/** Engine-owned browser inspection adapter; gameplay remains in the product. */
export function installPlaytestInspection(renderer: RustyApplicationRendererPort, flushInput: () => Promise<void>, settle: (through?: string) => Promise<void>): () => void {
  const target = globalThis as typeof globalThis & { __rustyPlaytest?: (request: PlaytestInspectionRequest) => Promise<unknown> };
  async function debug(command: string): Promise<unknown> {
    const response = await fetch('/__rusty/product/runtime/debug/execute', { method: 'POST', headers: { 'Content-Type': 'text/plain; charset=utf-8' }, body: command });
    const text = await response.text(); if (!response.ok) throw new Error(text);
    const through = response.headers.get('x-rusty-output-through');
    await settle(through ?? undefined);
    return JSON.parse(text);
  }
  const inspect = renderer.inspection;
  const invoke = async (request: PlaytestInspectionRequest): Promise<unknown> => {
    if (!inspect) throw new Error('Engine inspection is unavailable');
    const id = request.id ?? '';
    if (id && !/^[A-Za-z0-9_-]{1,64}$/.test(id)) throw new Error('invalid target/action id');
    switch (request.op) {
      case 'discover': {
        const catalog = await fetch('/__rusty/product/runtime/debug/catalog').then(r => r.json()) as { commands: { name: string }[] };
        const names = catalog.commands.map(c => c.name);
        return { commands: names, time: names.includes('engine.time'), timeModes: ['realtime', 'manual', 'action-driven'], drawingModes: ['continuous', 'on-demand'], observer: ['pose', 'move', 'lookAt', 'orbit', 'restore'], lookAdvancesTime: false, inspection: true, product: names.includes('playtest.help') ? await debug('playtest.help') : null };
      }
      case 'focus': return { focused: document.activeElement?.tagName === 'CANVAS', pointerLocked: document.pointerLockElement !== null };
      case 'flush': await flushInput(); return { flushed: true };
      case 'observe': return debug('playtest.observe');
      case 'action': return debug(`playtest.action ${id}`);
      case 'targets': return debug('navigation.targets');
      case 'route': return debug(`navigation.route ${id}`);
      case 'look': {
        const yaw = request.yaw ?? 0, pitch = request.pitch ?? 0;
        if (![yaw, pitch].every(Number.isFinite) || Math.abs(yaw) > 360 || Math.abs(pitch) > 180) throw new Error('look degrees exceed bounds');
        const result = await debug(`playtest.look ${yaw} ${pitch}`); await settle(); renderer.renderOnce(); return result;
      }
      case 'time': {
        if (request.mode && !['realtime', 'manual', 'action-driven'].includes(request.mode)) throw new Error('unknown time mode');
        if (request.mode) await flushInput();
        const result = await debug(request.mode ? `engine.time.mode ${request.mode}` : 'engine.time') as { mode: string; simulationStep: string; fixedStepHz: number };
        inspect({ simulationMs: result.mode === 'realtime' ? null : Number(result.simulationStep) * 1000 / result.fixedStepHz }); return result;
      }
      case 'advance': {
        if (!Number.isFinite(request.ms) || request.ms! <= 0 || request.ms! > 2000) throw new Error('advance ms must be in (0, 2000]');
        await flushInput();
        const result = await debug(`engine.time.advance ${request.ms}`) as { simulationStep: string; fixedStepHz: number };
        await settle(); inspect({ simulationMs: Number(result.simulationStep) * 1000 / result.fixedStepHz });
        if (inspect({}).drawing !== 'on-demand') renderer.renderOnce(); return result;
      }
      case 'drawing': {
        if (request.mode !== 'continuous' && request.mode !== 'on-demand') throw new Error('drawing mode must be continuous or on-demand');
        return inspect({ drawing: request.mode });
      }
      case 'frame': await settle(); renderer.renderOnce(); return renderer.diagnosticsReadout();
      case 'camera': {
        if (request.camera && ![...request.camera.position, request.camera.yawDegrees, request.camera.pitchDegrees].every(Number.isFinite)) throw new Error('camera must be finite');
        let camera = request.camera;
        if (request.move || request.lookAt || request.orbit || request.yaw !== undefined || request.pitch !== undefined) {
          const current = camera ?? inspect({}).camera;
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
        const result = inspect(camera === undefined ? {} : { camera });
        await settle(); renderer.renderOnce(); return result;
      }
      default: throw new Error('unknown inspection operation');
    }
  };
  target.__rustyPlaytest = invoke;
  return () => { if (target.__rustyPlaytest === invoke) delete target.__rustyPlaytest; };
}
