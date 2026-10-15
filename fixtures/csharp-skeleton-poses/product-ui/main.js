import { mountSceneTreePanel } from '@rusty-engine/live-debug';

export function mountProductUi(root) {
  const dock = document.createElement('div');
  dock.dataset.rustyUiInteractive = '';
  dock.style.cssText = 'position:absolute;top:8px;right:8px;bottom:8px;width:24rem;overflow:auto';
  root.append(dock);
  const tree = mountSceneTreePanel(dock);
  return {
    dispose: () => {
      tree.dispose();
      dock.remove();
    },
  };
}
