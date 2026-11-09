// The fixture's label, and a dialogue frame in the lower left: a bordered
// box whose window the product's bust is drawn in (Product.cs PlaceBust), and
// the dialogue panel beside it. The window is 127 CSS pixels times the
// largest whole scale, at most 3, within 60% of the window's height and 45%
// of its width, 32 pixels in from the lower left: the product's rule.
const BUST_TEXELS = 127;
const BUST_MAX_SCALE = 3;
const BUST_HEIGHT_SHARE = 0.6;
const BUST_WIDTH_SHARE = 0.45;
const BUST_INSET = 32;
const FRAME_BORDER = 10;
const PANEL_WIDTH = 420;

export function mountProductUi(root) {
  const label = document.createElement('output');
  label.textContent = 'Parts rigs fixture: the Engine draws the view of the townsperson each camera sees; live debug rigs.orbit, rigs.spin, rigs.play, rigs.turn. Lower left: a bust placed in the frame\'s window.';
  label.style.cssText = 'position:absolute;top:8px;left:8px;color:white;pointer-events:none;font:14px system-ui,sans-serif';

  const frame = document.createElement('div');
  frame.style.cssText = `position:absolute;box-sizing:content-box;border:${FRAME_BORDER}px solid #6b4a2b;`
    + 'outline:2px solid #2a1a0c;pointer-events:none';
  const panel = document.createElement('div');
  panel.style.cssText = 'position:absolute;box-sizing:border-box;padding:16px;background:#2a1d12;color:#f2e6d0;'
    + `border:${FRAME_BORDER}px solid #6b4a2b;border-left:none;font:16px system-ui,sans-serif;pointer-events:none`;
  panel.textContent = 'Vela: The Engine draws me in this window, each texel a whole number of pixels, while I breathe and blink.';

  const layout = () => {
    const fits = Math.min(window.innerHeight * BUST_HEIGHT_SHARE, window.innerWidth * BUST_WIDTH_SHARE);
    const scale = Math.min(Math.max(Math.floor(fits / BUST_TEXELS), 1), BUST_MAX_SCALE);
    const side = BUST_TEXELS * scale;
    Object.assign(frame.style, {
      left: `${BUST_INSET - FRAME_BORDER}px`,
      bottom: `${BUST_INSET - FRAME_BORDER}px`,
      width: `${side}px`,
      height: `${side}px`,
    });
    Object.assign(panel.style, {
      left: `${BUST_INSET + side + FRAME_BORDER}px`,
      bottom: `${BUST_INSET - FRAME_BORDER}px`,
      width: `${Math.max(0, Math.min(PANEL_WIDTH, window.innerWidth - BUST_INSET * 2 - side - FRAME_BORDER))}px`,
      height: `${side + FRAME_BORDER * 2}px`,
    });
  };
  layout();
  window.addEventListener('resize', layout);
  root.append(label, frame, panel);
  return {
    dispose: () => {
      window.removeEventListener('resize', layout);
      label.remove();
      frame.remove();
      panel.remove();
    },
  };
}
