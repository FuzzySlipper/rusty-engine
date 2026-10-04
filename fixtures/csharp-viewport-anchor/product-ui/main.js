// A side panel and the hero panel the camera view follows. The layout is
// plain CSS; resizing the page moves the hero and the view moves with it.
export function mountProductUi(root, context) {
  const layout = document.createElement('div');
  layout.style.cssText = 'display:flex;height:100dvh;width:100%;';
  const side = document.createElement('aside');
  side.style.cssText = 'background:#203040;color:#e0f0ff;flex:0 0 30%;font:1rem system-ui;padding:12px;box-sizing:border-box;';
  side.textContent = 'Side panel: the 3D view fills the panel to the right.';
  const hero = document.createElement('div');
  hero.dataset['fixtureHero'] = '';
  hero.style.cssText = 'flex:1;margin:24px;outline:2px solid #ffffff;';
  layout.append(side, hero);
  root.append(layout);
  const unanchor = context.viewport.anchor('hero', hero);
  // For the harness: the UI port, to set the UI scale.
  window.rustyFixtureUi = context.ui;
  return { dispose() { unanchor(); layout.remove(); delete window.rustyFixtureUi; } };
}
