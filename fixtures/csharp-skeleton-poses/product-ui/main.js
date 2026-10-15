import { mountSceneTreePanel } from '@rusty-engine/live-debug';

// The scene tree docked on the right, and two panes inspection views anchor
// to: engine.inspect.view create side anchor inspect-a, and inspect-b.
export function mountProductUi(root, context) {
  const dock = document.createElement('div');
  dock.dataset.rustyUiInteractive = '';
  dock.style.cssText = 'position:absolute;top:8px;right:8px;bottom:8px;width:24rem;overflow:auto';
  const panes = ['inspect-a', 'inspect-b'].map((name, index) => {
    const pane = document.createElement('div');
    pane.dataset.fixturePane = name;
    pane.style.cssText = `position:absolute;bottom:8px;left:${8 + index * 296}px;width:288px;height:180px;outline:1px solid #9aa8b8`;
    return pane;
  });
  root.append(dock, ...panes);
  const unanchor = panes.map((pane) => context.viewport.anchor(pane.dataset.fixturePane, pane));
  const tree = mountSceneTreePanel(dock);
  return {
    dispose: () => {
      tree.dispose();
      for (const release of unanchor) release();
      dock.remove();
      for (const pane of panes) pane.remove();
    },
  };
}
