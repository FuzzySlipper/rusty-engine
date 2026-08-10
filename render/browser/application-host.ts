import {
  mountRustyApplication,
  type RustyApplicationHost,
} from '@rusty-engine/application-host';

declare global {
  interface Window {
    __rustyApplicationHost?: RustyApplicationHost;
    __rustyApplicationMount?: () => Promise<RustyApplicationHost>;
    __rustyApplicationFailureProbe?: () => Promise<string>;
    __rustyApplicationUiDisposed?: boolean;
  }
}

const root = document.querySelector<HTMLElement>('#application');
if (root === null) throw new Error('application root is missing');

window.__rustyApplicationMount = () =>
  mountRustyApplication({
    root,
    initialInteractionMode: 'gameplay',
    mountUi: (uiRoot, context) => {
      const gameplay = document.createElement('div');
      gameplay.id = 'gameplay-zone';
      gameplay.textContent = 'Gameplay surface';
      const toolbar = document.createElement('div');
      toolbar.id = 'toolbar';
      const button = document.createElement('button');
      button.id = 'interface-button';
      button.textContent = 'Interface action';
      const input = document.createElement('input');
      input.id = 'text-entry';
      input.setAttribute('aria-label', 'Text entry');
      const modal = document.createElement('section');
      modal.id = 'modal';
      modal.setAttribute('role', 'dialog');
      modal.hidden = true;
      modal.textContent = 'Modal content';
      toolbar.append(button, input, modal);
      uiRoot.append(gameplay, toolbar);
      button.addEventListener('click', () => {
        context.ui.setInteractionMode('interface');
      });
      return {
        dispose: () => {
          window.__rustyApplicationUiDisposed = true;
        },
      };
    },
  });
window.__rustyApplicationHost = await window.__rustyApplicationMount();

window.__rustyApplicationFailureProbe = async () => {
  await window.__rustyApplicationHost?.dispose();
  try {
    await mountRustyApplication({
      root,
      mountUi: () => {
        throw new Error('trusted UI mount rejected');
      },
    });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return 'unexpected success';
};
