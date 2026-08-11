import type {
  RustyApplicationCameraPose,
  RustyApplicationUiContext,
  RustyApplicationUiOwner,
} from '@rusty-engine/application-host';

const MOVEMENT_KEYS = new Set(['KeyW', 'KeyA', 'KeyS', 'KeyD']);
const MOVE_SPEED = 3.5;
const LOOK_SENSITIVITY = 0.16;

interface MutablePose {
  position: [number, number, number];
  pitchDegrees: number;
  yawDegrees: number;
}

export function mountPlaytestProduct(
  root: HTMLElement,
  context: RustyApplicationUiContext,
): RustyApplicationUiOwner {
  const surface = document.createElement('main');
  surface.className = 'playtest-surface';
  surface.setAttribute('aria-label', 'Engine product playtest viewport');

  const heading = document.createElement('h1');
  heading.textContent = 'Engine Host Walkthrough';
  const instructions = document.createElement('p');
  instructions.textContent = 'Click to capture · WASD to move · Mouse to look · Esc to release';
  const status = document.createElement('p');
  status.className = 'playtest-status';
  status.setAttribute('role', 'status');
  status.textContent = 'Ready to explore';

  const hud = document.createElement('section');
  hud.className = 'playtest-hud';
  hud.setAttribute('aria-label', 'Public product controls');
  hud.append(heading, instructions, status);
  surface.append(hud);
  root.append(surface);

  const pose: MutablePose = {
    position: [0, 1.35, 6],
    pitchDegrees: -4,
    yawDegrees: 0,
  };
  const pressed = new Set<string>();
  let animationFrame = 0;
  let previousTime = performance.now();
  let disposed = false;

  const publishPose = (): void => {
    const cameraPose: RustyApplicationCameraPose = {
      position: pose.position,
      pitchDegrees: pose.pitchDegrees,
      yawDegrees: pose.yawDegrees,
    };
    context.renderer.setCameraPose(cameraPose);
  };

  const onKeyDown = (event: KeyboardEvent): void => {
    if (!MOVEMENT_KEYS.has(event.code) || !context.ui.allowsGameplayInput(event)) return;
    pressed.add(event.code);
    event.preventDefault();
  };
  const onKeyUp = (event: KeyboardEvent): void => {
    pressed.delete(event.code);
  };
  const onMouseMove = (event: MouseEvent): void => {
    if (document.pointerLockElement === null || !context.ui.allowsGameplayInput(event)) return;
    pose.yawDegrees = normalizeDegrees(pose.yawDegrees - event.movementX * LOOK_SENSITIVITY);
    pose.pitchDegrees = clamp(
      pose.pitchDegrees - event.movementY * LOOK_SENSITIVITY,
      -75,
      75,
    );
    publishPose();
    status.textContent = 'Looking around';
  };
  const onPointerLockChange = (): void => {
    status.textContent = document.pointerLockElement === null
      ? 'Ready to explore'
      : 'Input captured';
  };
  const tick = (time: number): void => {
    if (disposed) return;
    const elapsed = Math.min((time - previousTime) / 1_000, 0.1);
    previousTime = time;
    if (moveCamera(pose, pressed, elapsed)) {
      publishPose();
      status.textContent = 'Moving through the scene';
    }
    animationFrame = requestAnimationFrame(tick);
  };

  window.addEventListener('keydown', onKeyDown);
  window.addEventListener('keyup', onKeyUp);
  window.addEventListener('mousemove', onMouseMove);
  document.addEventListener('pointerlockchange', onPointerLockChange);
  publishPose();
  context.renderer.renderOnce();
  animationFrame = requestAnimationFrame(tick);

  return {
    dispose: () => {
      disposed = true;
      cancelAnimationFrame(animationFrame);
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('keyup', onKeyUp);
      window.removeEventListener('mousemove', onMouseMove);
      document.removeEventListener('pointerlockchange', onPointerLockChange);
      surface.remove();
    },
  };
}

function moveCamera(pose: MutablePose, pressed: ReadonlySet<string>, elapsed: number): boolean {
  let forward = 0;
  let right = 0;
  if (pressed.has('KeyW')) forward += 1;
  if (pressed.has('KeyS')) forward -= 1;
  if (pressed.has('KeyD')) right += 1;
  if (pressed.has('KeyA')) right -= 1;
  if (forward === 0 && right === 0) return false;

  const length = Math.hypot(forward, right);
  const yaw = pose.yawDegrees * Math.PI / 180;
  const distance = MOVE_SPEED * elapsed;
  const forwardX = Math.sin(yaw);
  const forwardZ = -Math.cos(yaw);
  const rightX = Math.cos(yaw);
  const rightZ = Math.sin(yaw);
  pose.position = [
    pose.position[0] + distance * (forward * forwardX + right * rightX) / length,
    pose.position[1],
    pose.position[2] + distance * (forward * forwardZ + right * rightZ) / length,
  ];
  return true;
}

function normalizeDegrees(value: number): number {
  return ((value + 180) % 360 + 360) % 360 - 180;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.min(Math.max(value, minimum), maximum);
}
