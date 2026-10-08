import type { ProductHostCursorMode } from './generated/contracts.js';
import { requestPointerLockWhile } from './pointer-lock.js';

/** A point in client (CSS pixel) coordinates. */
export interface CursorPoint {
  readonly x: number;
  readonly y: number;
}

/** The cursor drawn while a browser confines it: the lock's deltas move it. */
export interface SoftwareCursor {
  readonly point: () => CursorPoint;
  readonly move: (dx: number, dy: number) => void;
}

/**
 * How gameplay holds the pointer in each cursor mode. `pointer-lock` locks
 * and hides it. `confined` keeps a visible cursor in the game view: the
 * desktop shell confines the real cursor, and a browser, which cannot,
 * locks the pointer and draws one that the lock's movement moves.
 */
export interface GameplayCursor {
  /** Take the pointer as the mode holds it; a confined cursor starts at `origin`. */
  readonly capture: (origin?: CursorPoint) => void;
  readonly release: () => void;
  /** Whether the pointer is held as the mode holds it. */
  readonly captured: () => boolean;
  /** The drawn cursor while it is shown, else null. */
  readonly software: () => SoftwareCursor | null;
  readonly dispose: () => void;
}

/**
 * The desktop shell's native confinement, which its page shim installs. A
 * browser has none. `confine` answers false, and does nothing, once the shell
 * has refused.
 */
interface DesktopCursor {
  readonly confine: () => boolean;
  readonly release: () => void;
  readonly confined: () => boolean;
}

/** The shim's event when confinement ends without the page asking. */
export const DESKTOP_CONFINEMENT_END = 'rusty-desktop-confinement-end';

const CURSOR_ARROW =
  '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="21" viewBox="0 0 14 21">'
  + '<path d="M1 1v16l4-4 3 7 2.5-1-3-6.8H13z" fill="#fff" stroke="#000" stroke-width="1.2" stroke-linejoin="round"/>'
  + '</svg>';

export function createGameplayCursor(options: {
  readonly canvas: HTMLCanvasElement;
  /** The positioned element the drawn cursor is laid out in. */
  readonly layer: HTMLElement;
  readonly mode: () => ProductHostCursorMode;
  /** Whether gameplay still wants the pointer when a lock is granted late. */
  readonly wanted: () => boolean;
  /** The shell ended a native confinement (Escape, focus loss). */
  readonly onConfinementLost: () => void;
}): GameplayCursor {
  const { canvas, layer, mode, wanted } = options;
  const document = canvas.ownerDocument;
  const desktop = (): DesktopCursor | null =>
    (document.defaultView as (Window & { __rustyDesktopCursor?: DesktopCursor }) | null)
      ?.__rustyDesktopCursor ?? null;
  const locked = (): boolean => document.pointerLockElement === canvas;
  // In confined mode a lock is held only for the drawn cursor.
  const drawing = (): boolean => mode() === 'confined' && locked();

  const element = document.createElement('div');
  element.dataset['rustyApplicationCursor'] = 'software';
  element.setAttribute('aria-hidden', 'true');
  element.style.cssText =
    'display:none;left:0;line-height:0;pointer-events:none;position:absolute;top:0;z-index:3;';
  element.innerHTML = CURSOR_ARROW;
  layer.append(element);

  let point: CursorPoint = { x: 0, y: 0 };
  let shown = false;
  const clamp = (target: CursorPoint): CursorPoint => {
    const bounds = canvas.getBoundingClientRect();
    return {
      x: Math.min(Math.max(target.x, bounds.left), Math.max(bounds.left, bounds.right - 1)),
      y: Math.min(Math.max(target.y, bounds.top), Math.max(bounds.top, bounds.bottom - 1)),
    };
  };
  const draw = (): void => {
    shown = drawing();
    element.style.display = shown ? 'block' : 'none';
    if (!shown) return;
    point = clamp(point);
    const origin = layer.getBoundingClientRect();
    element.style.transform =
      `translate(${String(point.x - origin.left)}px,${String(point.y - origin.top)}px)`;
  };
  const centre = (): CursorPoint => {
    const bounds = canvas.getBoundingClientRect();
    return { x: bounds.left + bounds.width / 2, y: bounds.top + bounds.height / 2 };
  };
  const lockWhileWanted = (): void => {
    const held = mode();
    requestPointerLockWhile(canvas, () => wanted() && mode() === held);
  };

  const capture = (origin?: CursorPoint): void => {
    switch (mode()) {
      case 'unlocked':
        return;
      case 'pointer-lock':
        desktop()?.release();
        if (locked()) draw();
        else lockWhileWanted();
        return;
      case 'confined':
        if (desktop()?.confine() === true) {
          // The shell's lock and confinement are one grab; this replaces the lock.
          if (locked()) document.exitPointerLock();
          return;
        }
        if (!shown) point = clamp(origin ?? centre());
        if (locked()) draw();
        else lockWhileWanted();
    }
  };
  const release = (): void => {
    desktop()?.release();
    if (locked()) document.exitPointerLock();
    draw();
  };

  const onPointerLockChange = (): void => draw();
  const onConfinementEnd = (event: Event): void => {
    const refused = (event as CustomEvent<{ refused?: boolean }>).detail?.refused === true;
    // A refusal falls back to the drawn cursor, which the shell's lock carries.
    if (refused && mode() === 'confined' && wanted()) capture();
    else options.onConfinementLost();
  };
  document.addEventListener('pointerlockchange', onPointerLockChange);
  document.addEventListener(DESKTOP_CONFINEMENT_END, onConfinementEnd);

  const software: SoftwareCursor = Object.freeze({
    point: () => point,
    move: (dx: number, dy: number) => {
      point = { x: point.x + dx, y: point.y + dy };
      draw();
    },
  });
  return Object.freeze({
    capture,
    release,
    captured: () => {
      switch (mode()) {
        case 'unlocked': return false;
        case 'pointer-lock': return locked();
        case 'confined': return desktop()?.confined() === true || locked();
      }
    },
    software: () => (drawing() ? software : null),
    dispose: () => {
      document.removeEventListener('pointerlockchange', onPointerLockChange);
      document.removeEventListener(DESKTOP_CONFINEMENT_END, onConfinementEnd);
      element.remove();
    },
  });
}
