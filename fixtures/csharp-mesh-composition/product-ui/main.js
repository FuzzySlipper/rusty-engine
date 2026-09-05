export function mountProductUi(root, context) {
  const panel = document.createElement('section');
  panel.className = 'mesh-composition-panel';
  Object.assign(panel.style, {
    position: 'absolute', left: '16px', bottom: '16px', padding: '16px',
    background: '#182332ee', borderRadius: '8px', maxWidth: '620px',
    font: '15px system-ui, sans-serif', color: '#edf6ff',
  });
  panel.innerHTML = `
    <strong>C# Mesh Composition</strong>
    <p>Two material slots, one immutable mesh resource, two appearances.</p>
    <output aria-live="polite">Waiting for C# projection…</output>
    <div>
      <button type="button">Pulse shape</button>
      <button type="button">Clear and recreate</button>
    </div>`;
  root.append(panel);

  const [pulse, recreate] = panel.querySelectorAll('button');
  const output = panel.querySelector('output');
  for (const button of [pulse, recreate]) {
    Object.assign(button.style, { margin: '12px 8px 0 0', padding: '8px 12px' });
  }
  const submitDigital = (intent) => {
    context.intents?.claim(intent, { kind: 'digital', active: true });
    context.intents?.claim(intent, { kind: 'digital', active: false });
  };
  const onPulse = () => submitDigital('mesh.pulse');
  const onRecreate = () => submitDigital('mesh.recreate');
  pulse.addEventListener('click', onPulse);
  recreate.addEventListener('click', onRecreate);

  const unsubscribe = context.projection?.subscribe((projection) => {
    if (projection?.contract !== 'mesh-composition.ui.snapshot.v1' || !isMeshReadout(projection.value)) return;
    const value = projection.value;
    output.textContent = `${value.shape}; pulses ${value.pulses}; rebuilds ${value.rebuilds}; `
      + `presentation objects ${value.objects}, appearances ${value.appearances}, `
      + `materials ${value.materials}, resources ${value.resources}.`;
  }) ?? (() => {});

  return {
    dispose: () => {
      unsubscribe();
      pulse.removeEventListener('click', onPulse);
      recreate.removeEventListener('click', onRecreate);
      panel.remove();
    },
  };
}

function isMeshReadout(value) {
  return typeof value === 'object' && value !== null
    && typeof value.pulses === 'number'
    && typeof value.shape === 'string'
    && typeof value.rebuilds === 'number'
    && typeof value.objects === 'number'
    && typeof value.appearances === 'number'
    && typeof value.materials === 'number'
    && typeof value.resources === 'number';
}
