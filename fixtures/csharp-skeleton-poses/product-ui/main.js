import { mountInspectionView, mountSceneTreePanel } from '@rusty-engine/live-debug';

// The scene tree docked on the right, and two inspection view panes along
// the bottom: a side observer of the player's camera and a close-up of the
// right hand from below.
export function mountProductUi(root, context) {
  const dock = document.createElement('div');
  dock.dataset.rustyUiInteractive = '';
  dock.style.cssText = 'position:absolute;top:8px;right:8px;bottom:8px;width:24rem;overflow:auto';
  const panes = [0, 1].map((index) => {
    const pane = document.createElement('div');
    pane.style.cssText = `position:absolute;bottom:8px;left:${8 + index * 336}px;width:328px;height:220px`;
    return pane;
  });
  root.append(dock, ...panes);
  const tree = mountSceneTreePanel(dock);
  const views = [
    mountInspectionView(panes[0], { viewport: context.viewport, id: 'side', subject: 'camera' }),
    mountInspectionView(panes[1], { viewport: context.viewport, id: 'hand', subject: 'target right-hand' }),
  ];
  return {
    dispose: () => {
      for (const view of views) view.dispose();
      tree.dispose();
      dock.remove();
      for (const pane of panes) pane.remove();
    },
  };
}
