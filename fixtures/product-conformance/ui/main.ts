import type {
  RustyApplicationUiContext,
  RustyApplicationUiOwner,
} from '@rusty-engine/application-host';

// Presentation is observational. The Product Kernel publishes the exact
// counter projection declared in rusty.toml; this UI owns only a local DOM
// label and disposes its subscription with the application host.
export function mountProductUi(
  root: HTMLElement,
  context: RustyApplicationUiContext,
): RustyApplicationUiOwner {
  const increment = document.createElement('button');
  increment.id = 'product-conformance-increment';
  increment.dataset.rustyTestIncrement = 'true';
  increment.type = 'button';
  increment.textContent = 'Increment';

  const label = document.createElement('output');
  label.id = 'product-conformance-counter';
  label.dataset.rustyTestCounter = 'true';
  label.textContent = '0';
  root.append(increment, label);

  // This is a semantic claim on the same host-owned ingress lane as the
  // physical W mapping. It never mutates the product projection directly.
  const onIncrement = (): void => {
    context.intents?.claim('increment', { kind: 'digital', active: true });
  };
  increment.addEventListener('click', onIncrement);

  const unsubscribe = context.projection?.subscribe((envelope) => {
    label.textContent = envelope === null ? '0' : String(envelope.value);
  }) ?? (() => {});

  return {
    dispose: () => {
      increment.removeEventListener('click', onIncrement);
      unsubscribe();
    },
  };
}
