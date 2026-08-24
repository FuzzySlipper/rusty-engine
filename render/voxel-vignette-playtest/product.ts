import type {
  RustyApplicationUiContext,
  RustyApplicationUiOwner,
} from '@rusty-engine/application-host';

/** A presentation-only visual-gate UI; Engine owns viewport and physical input. */
export function mountVignetteProduct(
  root: HTMLElement,
  context: RustyApplicationUiContext,
): RustyApplicationUiOwner {
  const surface = document.createElement('main');
  surface.className = 'vignette-surface';
  surface.setAttribute('aria-label', 'Voxel vignette visual gate viewport');

  const hud = document.createElement('section');
  hud.className = 'vignette-hud';
  hud.setAttribute('aria-label', 'Voxel vignette visual gate status');
  const title = document.createElement('h1');
  title.textContent = 'Voxel vignette · visual gate';
  const run = document.createElement('p');
  run.textContent = 'run 003 · palette-unlit producer derivative · four checked local GLBs · 34 MB';
  const controls = document.createElement('p');
  controls.textContent = 'Engine owns viewport capture and the Runtime Lifecycle physical lane.';
  const status = document.createElement('p');
  status.className = 'vignette-status';
  status.setAttribute('role', 'status');
  status.textContent = 'Ready — palette-unlit producer derivative admitted';
  const caveat = document.createElement('p');
  caveat.className = 'vignette-caveat';
  caveat.textContent = 'Temporary static-GLB-through-animated-mesh route using a palette-unlit producer derivative. Runtime voxel not wired; conventional comparator absent; collision not wired.';
  const claim = document.createElement('button');
  claim.type = 'button';
  claim.dataset['rustyUiInteractive'] = 'true';
  claim.textContent = 'Acknowledge visual gate';
  claim.addEventListener('click', () => {
    context.intents?.claim('visual.acknowledge', { kind: 'digital', active: true });
    status.textContent = 'Visual-gate acknowledgement submitted';
  });
  hud.append(title, run, controls, status, caveat, claim);
  surface.append(hud);
  root.append(surface);

  const unsubscribe = context.projection?.subscribe((projection) => {
    if (projection !== null) status.textContent = `Rust projection ${projection.sequence} received`;
  }) ?? (() => undefined);

  return {
    dispose: () => {
      unsubscribe();
      surface.remove();
    },
  };
}
