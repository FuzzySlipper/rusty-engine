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
  const label = document.createElement('output');
  label.id = 'product-conformance-counter';
  label.textContent = '0';
  root.append(label);

  const unsubscribe = context.projection?.subscribe((envelope) => {
    label.textContent = envelope === null ? '0' : String(envelope.value);
  }) ?? (() => {});

  return { dispose: unsubscribe };
}
