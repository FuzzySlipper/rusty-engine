const label: string = "ui v1";
document.body.dataset.exercise = label;

export function mountProductUi(root: HTMLElement): () => void {
  root.textContent = label;
  return () => { root.textContent = ""; };
}
