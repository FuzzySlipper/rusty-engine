import type {
  RustyApplicationUiContext,
  RustyApplicationUiOwner,
} from '@rusty-engine/application-host';

/**
 * The product playtest is deliberately a DOM view only. Engine owns browser
 * capture, cadence, camera integration, and the physical/DOM ordered lane;
 * this fixture demonstrates a product UI claim and a read-only projection.
 */
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
  instructions.textContent = 'Click the Engine viewport for physical input; use the UI action for a typed claim.';
  const status = document.createElement('p');
  status.className = 'playtest-status';
  status.setAttribute('role', 'status');
  status.textContent = 'Ready to explore';
  const claim = document.createElement('button');
  claim.type = 'button';
  claim.dataset['rustyUiInteractive'] = 'true';
  claim.textContent = 'Claim UI action';
  claim.addEventListener('click', () => {
    context.intents?.claim('ui.confirm', { kind: 'digital', active: true });
    status.textContent = 'UI claim submitted to the Runtime Lifecycle lane';
  });

  const hud = document.createElement('section');
  hud.className = 'playtest-hud';
  hud.setAttribute('aria-label', 'Public product controls');
  hud.append(heading, instructions, status, claim);
  surface.append(hud);
  root.append(surface);

  const unsubscribe = context.projection?.subscribe((projection) => {
    if (projection === null) {
      status.textContent = 'Waiting for the bound Rust product projection';
      return;
    }
    status.textContent = `Rust projection ${projection.sequence} received`;
  }) ?? (() => undefined);

  return {
    dispose: () => {
      unsubscribe();
      surface.remove();
    },
  };
}
