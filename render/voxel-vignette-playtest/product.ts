import type {
  RustyApplicationCameraPose,
  RustyApplicationUiContext,
  RustyApplicationUiOwner,
} from '@rusty-engine/application-host';

import {
  INITIAL_VIGNETTE_LIGHTING,
  loadVignetteContent,
  vignetteLightingFrame,
  VIGNETTE_VARIANTS,
  type VignetteLighting,
  type VignetteVariantId,
} from './scene.js';

const MOVEMENT_KEYS = new Set(['KeyW', 'KeyA', 'KeyS', 'KeyD']);
const MOVE_SPEED = 4.5;
const LOOK_SENSITIVITY = 0.14;
const INITIAL_VARIANT: VignetteVariantId = 'occupancy-axis-control';

interface MutablePose { position: [number, number, number]; pitchDegrees: number; yawDegrees: number; }

export function mountVignetteProduct(root: HTMLElement, context: RustyApplicationUiContext): RustyApplicationUiOwner {
  const surface = document.createElement('main');
  surface.className = 'vignette-surface';
  surface.setAttribute('aria-label', 'Voxel shading comparison viewport');
  const hud = document.createElement('section');
  hud.className = 'vignette-hud';
  hud.setAttribute('aria-label', 'Voxel shading comparison status');
  const title = document.createElement('h1');
  title.textContent = 'Voxel shading comparison · owner visual gate';
  const route = document.createElement('p');
  route.textContent = 'Same #6925 shrine, tree, door, camera route, transforms, scale, viewport, and adjustable lights. Terrain is intentionally disabled. Variant facts name palette/material encoding changes.';
  const controls = document.createElement('p');
  controls.textContent = 'Choose a variant · click canvas · WASD free-flight · pointer-look · Esc release';
  const variantButtons = document.createElement('div');
  variantButtons.className = 'vignette-variants';
  variantButtons.setAttribute('aria-label', 'Shading variants');
  const lightingControls = document.createElement('fieldset');
  lightingControls.className = 'vignette-lighting';
  const lightingLegend = document.createElement('legend');
  lightingLegend.textContent = 'Live retained lighting';
  const status = document.createElement('p');
  status.className = 'vignette-status';
  status.setAttribute('role', 'status');
  const facts = document.createElement('p');
  facts.className = 'vignette-facts';
  const caveat = document.createElement('p');
  caveat.className = 'vignette-caveat';
  caveat.textContent = 'Lighting controls publish ordinary Engine updateLight frames. Materials are not rewritten. Collision is not wired; free-flight is intentional.';
  lightingControls.append(lightingLegend);
  hud.append(title, route, controls, variantButtons, lightingControls, status, facts, caveat);
  surface.append(hud);
  root.append(surface);

  const pose: MutablePose = { position: [0, 1.6, 13], pitchDegrees: -8, yawDegrees: 0 };
  let lighting: VignetteLighting = { ...INITIAL_VIGNETTE_LIGHTING };
  const pressed = new Set<string>();
  const buttons = new Map<VignetteVariantId, HTMLButtonElement>();
  let activeVariant = INITIAL_VARIANT;
  let switching = false;
  let animationFrame = 0;
  let previousTime = performance.now();
  let disposed = false;
  const publishLighting = (): void => {
    const receipt = context.renderer.applyFrame(vignetteLightingFrame(lighting));
    if (!receipt.applied) {
      status.textContent = `Lighting update failed: ${receipt.diagnostics.map((diagnostic) => diagnostic.message).join('; ')}`;
      return;
    }
    context.renderer.renderOnce();
  };
  const publishPose = (): void => {
    context.renderer.setCameraPose({ position: pose.position, pitchDegrees: pose.pitchDegrees, yawDegrees: pose.yawDegrees });
    if (lighting.pointEnabled) {
      lighting = { ...lighting, pointPosition: [pose.position[0], pose.position[1] + 0.25, pose.position[2]] };
      publishLighting();
    }
  };
  const updateVariantReadout = (): void => {
    const variant = VIGNETTE_VARIANTS.find((candidate) => candidate.id === activeVariant);
    if (variant === undefined) return;
    status.textContent = `Ready — ${variant.label}`;
    facts.textContent = `Normals: ${variant.normalTreatment}. Material: ${variant.materialModel}. Lighting: ${variant.lighting}.`;
    for (const [id, button] of buttons) button.setAttribute('aria-pressed', String(id === activeVariant));
  };
  const selectVariant = async (id: VignetteVariantId): Promise<void> => {
    if (disposed || switching || id === activeVariant) return;
    switching = true;
    for (const button of buttons.values()) button.disabled = true;
    status.textContent = `Switching to ${VIGNETTE_VARIANTS.find((variant) => variant.id === id)?.label ?? id}…`;
    try {
      const receipt = await context.renderer.replaceContent(await loadVignetteContent(id));
      if (!receipt.applied) throw new Error(receipt.diagnostics.map((diagnostic) => diagnostic.message).join('; ') || 'application host rejected replacement');
      activeVariant = id;
      publishPose();
      publishLighting();
      updateVariantReadout();
    } catch (error) {
      status.textContent = `Variant switch failed: ${error instanceof Error ? error.message : String(error)}`;
    } finally {
      switching = false;
      for (const button of buttons.values()) button.disabled = false;
    }
  };
  for (const variant of VIGNETTE_VARIANTS) {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = variant.label;
    button.setAttribute('aria-pressed', String(variant.id === activeVariant));
    button.addEventListener('click', () => { void selectVariant(variant.id); });
    buttons.set(variant.id, button);
    variantButtons.append(button);
  }
  const addRange = (
    label: string,
    minimum: number,
    maximum: number,
    step: number,
    initial: number,
    update: (value: number) => void,
  ): void => {
    const row = document.createElement('label');
    const name = document.createElement('span');
    const value = document.createElement('output');
    const input = document.createElement('input');
    name.textContent = label;
    input.type = 'range';
    input.min = String(minimum);
    input.max = String(maximum);
    input.step = String(step);
    input.value = String(initial);
    value.value = initial.toFixed(step < 1 ? 2 : 0);
    input.addEventListener('input', () => {
      const next = Number(input.value);
      value.value = next.toFixed(step < 1 ? 2 : 0);
      update(next);
      publishLighting();
    });
    row.append(name, input, value);
    lightingControls.append(row);
  };
  addRange('Ambient', 0, 3, 0.05, lighting.ambientIntensity, (value) => { lighting = { ...lighting, ambientIntensity: value }; });
  addRange('Directional', 0, 5, 0.1, lighting.directionalIntensity, (value) => { lighting = { ...lighting, directionalIntensity: value }; });
  const pointToggle = document.createElement('label');
  pointToggle.className = 'vignette-light-toggle';
  const pointInput = document.createElement('input');
  pointInput.type = 'checkbox';
  pointInput.checked = lighting.pointEnabled;
  const pointLabel = document.createElement('span');
  pointLabel.textContent = 'Camera point light';
  pointInput.addEventListener('change', () => {
    lighting = {
      ...lighting,
      pointEnabled: pointInput.checked,
      pointPosition: [pose.position[0], pose.position[1] + 0.25, pose.position[2]],
    };
    publishLighting();
  });
  pointToggle.append(pointInput, pointLabel);
  lightingControls.append(pointToggle);
  addRange('Point intensity', 0, 200, 5, lighting.pointIntensity, (value) => { lighting = { ...lighting, pointIntensity: value }; });
  addRange('Point range', 1, 12, 0.5, lighting.pointRange, (value) => { lighting = { ...lighting, pointRange: value }; });
  const onKeyDown = (event: KeyboardEvent): void => { if (!MOVEMENT_KEYS.has(event.code) || !context.ui.allowsGameplayInput(event)) return; pressed.add(event.code); event.preventDefault(); };
  const onKeyUp = (event: KeyboardEvent): void => { pressed.delete(event.code); };
  const onMouseMove = (event: MouseEvent): void => { if (document.pointerLockElement === null || !context.ui.allowsGameplayInput(event)) return; pose.yawDegrees = normalizeDegrees(pose.yawDegrees + event.movementX * LOOK_SENSITIVITY); pose.pitchDegrees = clamp(pose.pitchDegrees - event.movementY * LOOK_SENSITIVITY, -80, 80); publishPose(); };
  const onPointerLockChange = (): void => { if (document.pointerLockElement === null) pressed.clear(); };
  const onWindowBlur = (): void => { pressed.clear(); };
  const tick = (time: number): void => { if (disposed) return; const elapsed = Math.min((time - previousTime) / 1_000, 0.1); previousTime = time; if (moveCamera(pose, pressed, elapsed)) publishPose(); animationFrame = requestAnimationFrame(tick); };
  window.addEventListener('keydown', onKeyDown); window.addEventListener('keyup', onKeyUp); window.addEventListener('mousemove', onMouseMove); window.addEventListener('blur', onWindowBlur); document.addEventListener('pointerlockchange', onPointerLockChange);
  publishPose(); context.renderer.renderOnce(); updateVariantReadout(); animationFrame = requestAnimationFrame(tick);
  return { dispose: () => { disposed = true; cancelAnimationFrame(animationFrame); window.removeEventListener('keydown', onKeyDown); window.removeEventListener('keyup', onKeyUp); window.removeEventListener('mousemove', onMouseMove); window.removeEventListener('blur', onWindowBlur); document.removeEventListener('pointerlockchange', onPointerLockChange); surface.remove(); } };
}

function moveCamera(pose: MutablePose, pressed: ReadonlySet<string>, elapsed: number): boolean {
  let forward = 0; let right = 0;
  if (pressed.has('KeyW')) forward += 1; if (pressed.has('KeyS')) forward -= 1; if (pressed.has('KeyD')) right += 1; if (pressed.has('KeyA')) right -= 1;
  if (forward === 0 && right === 0) return false;
  const length = Math.hypot(forward, right); const yaw = pose.yawDegrees * Math.PI / 180; const distance = MOVE_SPEED * elapsed;
  pose.position = [pose.position[0] + distance * (forward * Math.sin(yaw) + right * Math.cos(yaw)) / length, pose.position[1], pose.position[2] + distance * (forward * -Math.cos(yaw) + right * Math.sin(yaw)) / length];
  return true;
}

function normalizeDegrees(value: number): number { return ((value + 180) % 360 + 360) % 360 - 180; }
function clamp(value: number, minimum: number, maximum: number): number { return Math.min(Math.max(value, minimum), maximum); }
