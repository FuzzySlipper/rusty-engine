export function mountProductUi(root) {
  const text = document.createElement('p');
  text.textContent = 'Engine recorded audio proof: WAV, Vorbis, Opus, MP3 and FLAC. Click Enable audio, then use audio.proof.play through live debug. audio.proof.inspect reports admitted clips and realization; audio.proof.stop releases loops.';
  const button = document.createElement('button'); button.textContent = 'Enable audio';
  root.append(text, button);
  return { dispose() { text.remove(); button.remove(); } };
}
