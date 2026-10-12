export function mountProductUi(root) {
  const label = document.createElement('p');
  label.textContent = 'Engine Dynamics water · a raft, a ball and a capsule in a lake; a tethered raft in a river · float.inspect, float.load true|false, float.expect drafts|tethered|tipped|righted, float.cost <bodies>';
  label.style.cssText = 'position:absolute;top:12px;left:16px;max-width:700px;color:white;font:16px system-ui;background:#102030cc;padding:12px';
  root.append(label);
  return { dispose() { label.remove(); } };
}
