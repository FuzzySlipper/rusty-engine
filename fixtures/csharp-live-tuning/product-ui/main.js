import { mountDeveloperShell } from '@rusty-engine/live-debug';

// The developer shell docked on the right with the tunables panel and the
// scene tree, open from the start; F3 closes and opens it.
export function mountProductUi(root) {
  const label = document.createElement('output');
  label.textContent = 'Live tuning fixture: tune the marker in the shell, Save writes content/tuning/marker.json; edit that file and the marker follows without a restart. F3 toggles the shell.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;right:28rem;color:white;pointer-events:none;font:14px system-ui,sans-serif';
  root.append(label);
  const shell = mountDeveloperShell(root, { panels: [{ kind: 'tunables' }, { kind: 'scene' }] });
  void shell.open();
  const onKey = (event) => {
    if (event.code === 'F3' && !event.repeat) void shell.toggle();
  };
  window.addEventListener('keydown', onKey);
  return {
    dispose: () => {
      window.removeEventListener('keydown', onKey);
      shell.dispose();
      label.remove();
    },
  };
}
