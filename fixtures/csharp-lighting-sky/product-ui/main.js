export function mountProductUi(root) {
  const label = document.createElement('p');
  label.textContent = 'Engine lighting and sky fixture · lighting.torch true/false · lighting.sky 0…1 · lighting.room · lighting.inspect';
  label.style.cssText = 'position:absolute;top:12px;left:16px;color:white;font:16px system-ui;background:#102030cc;padding:12px';
  root.append(label);

  // The Engine's video options panel, opened from a button: the Engine's
  // renderer settings, plus one option of the fixture's own (the help label),
  // which the fixture applies itself.
  const toggle = document.createElement('button');
  toggle.textContent = 'Video options';
  toggle.className = 'lighting-video-options-toggle';
  toggle.style.cssText = 'position:absolute;top:12px;right:16px;font:15px system-ui;padding:8px 12px';
  const holder = document.createElement('div');
  holder.style.cssText = 'position:absolute;top:56px;right:16px;max-height:calc(100% - 72px);overflow:auto';
  root.append(toggle, holder);

  let panel = null;
  let helpShown = true;
  const productOptions = () => [{
    id: 'helpLabel',
    label: 'Help label',
    group: 'Fixture',
    description: 'Shows the fixture commands in the corner.',
    kind: 'toggle',
    value: helpShown,
    onChange: (value) => {
      helpShown = value === true;
      label.hidden = !helpShown;
      panel?.setProductOptions(productOptions());
    },
  }];
  toggle.addEventListener('click', async () => {
    if (panel) {
      panel.dispose();
      panel = null;
      return;
    }
    const { mountVideoOptions } = await import('@rusty-engine/video-options');
    panel = await mountVideoOptions(holder, { productOptions: productOptions() });
  });

  return {
    dispose() {
      panel?.dispose();
      label.remove();
      toggle.remove();
      holder.remove();
    },
  };
}
