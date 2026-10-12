import { mountTunablesPanel } from '@rusty-engine/live-debug';

export function mountProductUi(root) {
  const label = document.createElement('output');
  label.textContent = 'Live tuning fixture: tune the marker in the panel, Save writes content/tuning/marker.json; edit that file and the marker follows without a restart.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;right:24rem;color:white;pointer-events:none;font:14px system-ui,sans-serif';
  const dock = document.createElement('div');
  dock.dataset.rustyUiInteractive = '';
  dock.style.cssText = 'position:absolute;top:8px;right:8px;bottom:8px;width:22rem;overflow:auto';
  root.append(label, dock);
  const panel = mountTunablesPanel(dock);
  return {
    dispose: () => {
      panel.dispose();
      label.remove();
      dock.remove();
    },
  };
}
