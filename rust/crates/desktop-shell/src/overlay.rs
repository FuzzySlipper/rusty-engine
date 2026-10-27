//! The UI page over the scene: its web engine, window input into the page,
//! and the shell-provided pointer lock.

use std::collections::HashMap;
use std::sync::Arc;

use render_wgpu::web::{
    CursorImage, CursorShape, EventModifiers, KeyInput, MouseAction, MouseButton, MouseEvent,
    NavigationEvent, PageControl, WebOverlay, WebOverlayStats, WebRuntime, WebRuntimeConfig,
};
use render_wgpu::Gpu;
use winit::{
    dpi::PhysicalPosition,
    event::{ElementState, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoopProxy},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{Cursor, CursorGrabMode, CursorIcon, CustomCursor, Window},
};

use runtime_input::InputClearReason;

use crate::gameplay::GameplayRoute;
use crate::{keys, DesktopScene};

/// Console lines the pointer-lock and confinement shim writes; the shell
/// reads them from the page's console events.
const LOCK_REQUEST: &str = "rusty-desktop:pointer-lock";
const LOCK_EXIT: &str = "rusty-desktop:pointer-unlock";
const CONFINE_REQUEST: &str = "rusty-desktop:cursor-confine";
const CONFINE_EXIT: &str = "rusty-desktop:cursor-release";

/// Chromium's off-screen mode rejects the Pointer Lock API, so the page gets
/// a shell-backed one: `requestPointerLock` asks the shell to grab the
/// cursor, `document.pointerLockElement` reports the element, and the shell
/// feeds raw motion back through `__rustyDesktopMotion`.
///
/// A browser cannot keep a visible cursor in the page, but the shell can:
/// `__rustyDesktopCursor.confine()` asks it to confine the native cursor to
/// the window. The shell ends a confinement (Escape, focus loss) or refuses
/// one through `__rustyDesktopConfinementEnded`, which the page sees as a
/// `rusty-desktop-confinement-end` event; after a refusal `confine` answers
/// false and the page draws its own cursor under a lock.
///
/// The off-screen page also keeps Chromium's focus while the native window is
/// in the background, so the shell reports window focus through
/// `__rustyDesktopFocus`: the page gets `blur`/`focus` on `window` and
/// `document.hasFocus()` follows the native window, as in a browser tab.
///
/// Browser features the window cannot carry warn in the page's console when
/// used (docs/desktop-shell.md lists them). Two would otherwise lock the page:
/// a native HTML5 drag, which Chromium hands to the shell and the shell ends
/// at once (`dragend` with `dropEffect` `none`), and `print()`, which never
/// returns off screen and is replaced with a refusal. `alert`, `confirm` and
/// `prompt` return what Chromium's cancelled dialog returns, and file and
/// colour pickers do not open.
const PAGE_SHIM: &str = r#"(() => {
  if (window.__rustyDesktopMotion) return;
  let locked = null;
  let windowFocused = true;
  const hasFocus = Document.prototype.hasFocus;
  Document.prototype.hasFocus = function () {
    return windowFocused && hasFocus.call(this);
  };
  window.__rustyDesktopFocus = (focused) => {
    if (windowFocused === focused) return;
    windowFocused = focused;
    if (!focused) return window.dispatchEvent(new FocusEvent('blur'));
    // Chromium takes the focus the shell gives it a moment later: the page
    // hears focus once document.hasFocus() reads true, as in a browser, or
    // after 250 ms.
    const asked = performance.now();
    const tell = () => {
      if (!windowFocused) return;
      if (hasFocus.call(document) || performance.now() - asked > 250) window.dispatchEvent(new FocusEvent('focus'));
      else setTimeout(tell, 10);
    };
    tell();
  };
  const change = () => document.dispatchEvent(new Event('pointerlockchange'));
  Object.defineProperty(Document.prototype, 'pointerLockElement', {
    configurable: true,
    get() { return locked; },
  });
  Element.prototype.requestPointerLock = function () {
    if (locked !== this) {
      locked = this;
      console.info('rusty-desktop:pointer-lock');
      change();
    }
    return Promise.resolve();
  };
  Document.prototype.exitPointerLock = function () {
    if (locked === null) return;
    locked = null;
    console.info('rusty-desktop:pointer-unlock');
    change();
  };
  window.__rustyDesktopUnlocked = () => {
    if (locked === null) return;
    locked = null;
    change();
  };
  let confined = false;
  let confineRefused = false;
  window.__rustyDesktopCursor = Object.freeze({
    confine() {
      if (confineRefused) return false;
      if (!confined) {
        confined = true;
        console.info('rusty-desktop:cursor-confine');
      }
      return true;
    },
    release() {
      if (!confined) return;
      confined = false;
      console.info('rusty-desktop:cursor-release');
    },
    confined: () => confined,
  });
  window.__rustyDesktopConfinementEnded = (refused) => {
    if (refused) confineRefused = true;
    if (!confined) return;
    confined = false;
    document.dispatchEvent(new CustomEvent('rusty-desktop-confinement-end', { detail: { refused } }));
  };
  const unsupported = (what, instead) => console.warn(
    `rusty desktop shell: ${what} in the desktop window. ${instead} (docs/desktop-shell.md#browser-features-in-the-window)`);
  window.addEventListener('dragstart', (event) => {
    if (!event.isTrusted) return;
    unsupported('native drag and drop is refused; the drag ends at once with dropEffect none',
      'Drag with pointer events and set draggable="false" on images and links.');
  }, true);
  window.print = () => unsupported('print() does nothing', 'Nothing prints from the window.');
  const cancelled = { alert: undefined, confirm: false, prompt: null };
  for (const [name, answer] of Object.entries(cancelled)) {
    window[name] = () => {
      unsupported(`${name}() is cancelled at once`, 'Show a DOM modal such as <dialog> instead.');
      return answer;
    };
  }
  document.addEventListener('click', (event) => {
    const input = event.target;
    if (!event.isTrusted || !(input instanceof HTMLInputElement)) return;
    if (input.type === 'file' || input.type === 'color') {
      unsupported(`the ${input.type} picker does not open`, 'Draw the control in the page.');
    }
  }, true);
  window.__rustyDesktopMotion = (x, y) => {
    if (locked === null) return;
    const event = new PointerEvent('pointermove', {
      bubbles: true, composed: true, pointerType: 'mouse', isPrimary: true,
      movementX: x, movementY: y,
    });
    // Servo's events have no movement attributes yet.
    if (event.movementX !== x) {
      Object.defineProperties(event, { movementX: { value: x }, movementY: { value: y } });
    }
    locked.dispatchEvent(event);
  };
})();"#;

/// The console line a live-debug script's answer arrives on:
/// `rusty-desktop:eval <id> <JSON>`.
const PAGE_ANSWER: &str = "rusty-desktop:eval ";

/// Console lines the Servo page's `<select>` list writes: the chosen
/// options' ids, comma-separated, or none.
const SELECT_CHOSEN: &str = "rusty-desktop:select ";
const SELECT_CANCELLED: &str = "rusty-desktop:select-cancel";

/// Console lines the Servo page's shim writes for an image cursor, which
/// Servo reports only as its keyword: `rusty-desktop:cursor-image <id>
/// <width> <height> <hotspot x> <hotspot y> <scale> <RGBA hex>` once per
/// image, then `rusty-desktop:cursor <id>` each time it shows, or
/// `rusty-desktop:cursor none` to show the keyword again.
const CURSOR_IMAGE: &str = "rusty-desktop:cursor-image ";
const CURSOR_SHOWN: &str = "rusty-desktop:cursor ";

#[cfg(feature = "servo-overlay")]
/// What Servo's page needs beside [`PAGE_SHIM`], at the start of every
/// document. Chromium tells the shell about new windows, and draws a
/// `<select>`'s list itself; Servo does neither:
///
/// - `window.open` and `target="_blank"` links warn and open nothing, as
///   they do in Chromium's page; download links warn and download nothing;
///   `Notification` and `navigator.geolocation`, which Servo lacks, deny at
///   once;
/// - a change inside an inline `<svg>` (an attribute, child or text below
///   the `<svg>` element) redraws it. Servo draws an `<svg>` from its
///   serialized markup and redraws it only when the `<svg>` element itself
///   changes, so the shim touches that element. A click in an `<svg>`
///   leaves no caret there: Servo would draw the picture selected;
/// - a click on a `date`, `time` or `datetime-local` input, or Alt+Down on
///   one, opens a calendar and hour and minute lists, placed as a `<select>`'s
///   list is; a choice writes the input's value with `input` and `change`
///   events. Arrow keys and Page Up/Down move the day, Enter answers, Escape
///   or a click outside closes it;
/// - `__rustyDesktopSelect(rect, options, multiple)` draws the option list
///   the page asked for at the element's rect (physical pixels), in a closed
///   shadow root on top of the page, kept inside the view: below the element
///   or, where there is more room, above it, scrolling what does not fit.
///   Arrow keys, Enter, Escape, a click on an option or outside answer it
///   through the console. A `multiple` list toggles options with a click or
///   Space and answers with Enter or a click outside; Escape keeps the
///   element's choice.
const SERVO_SHIM: &str = r#"(() => {
  if (window.__rustyDesktopSelect) return;
  const newWindow = (url) => console.warn(
    `rusty desktop shell: a new window for ${url} is not opened in the desktop window. Keep game UI in this page. (docs/desktop-shell.md#browser-features-in-the-window)`);
  window.open = (url) => { newWindow(new URL(url ?? '', location.href).href); return null; };
  document.addEventListener('click', (event) => {
    const link = event.target instanceof Element && event.target.closest('a[target="_blank"]');
    if (!link || !event.isTrusted) return;
    event.preventDefault();
    newWindow(link.href);
  }, true);
  // Servo ignores a link's download attribute and would load the file in
  // place of the page; nothing downloads, as in Chromium's page.
  document.addEventListener('click', (event) => {
    const link = event.target instanceof Element && event.target.closest('a[download]');
    if (!link) return;
    event.preventDefault();
    console.warn(`rusty desktop shell: ${link.href} does not download in the desktop window. (docs/desktop-shell.md#browser-features-in-the-window)`);
  }, true);
  // Servo has no Notification or Geolocation API; they deny at once, as in
  // Chromium's page, rather than being missing.
  if (!('Notification' in window)) {
    window.Notification = class Notification extends EventTarget {
      static permission = 'denied';
      static requestPermission(callback) { callback?.('denied'); return Promise.resolve('denied'); }
      constructor() { super(); throw new TypeError('Notifications are denied in the desktop window'); }
    };
  }
  if (!navigator.geolocation) {
    const denied = (error) => error?.({ code: 1, message: 'Geolocation is denied in the desktop window', PERMISSION_DENIED: 1 });
    Object.defineProperty(navigator, 'geolocation', { value: {
      getCurrentPosition: (_, error) => denied(error),
      watchPosition: (_, error) => { denied(error); return 0; },
      clearWatch: () => {},
    } });
  }
  // Servo draws an inline <svg> as an image of its markup, made again only
  // when the outermost <svg> element's attributes or children change. A
  // change further in toggles an attribute on that element, which leaves
  // the markup as it was.
  const svgNamespace = 'http://www.w3.org/2000/svg';
  const outermostSvg = (node) => {
    let svg = null;
    for (let element = node; element; element = element.parentElement) {
      if (element.localName === 'svg' && element.namespaceURI === svgNamespace) svg = element;
    }
    return svg;
  };
  new MutationObserver((records) => {
    const changed = new Set();
    for (const record of records) {
      const target = record.type === 'characterData' ? record.target.parentElement : record.target;
      if (target?.namespaceURI !== svgNamespace) continue;
      const svg = outermostSvg(target);
      if (svg && svg !== target) changed.add(svg);
    }
    for (const svg of changed) {
      svg.toggleAttribute('data-rusty-redraw');
      svg.toggleAttribute('data-rusty-redraw');
    }
  }).observe(document, { subtree: true, childList: true, attributes: true, characterData: true });
  // A click in an <svg> leaves Servo's caret in it, and Servo then draws the
  // whole picture as selected when it next redraws it. Nothing in an <svg>
  // takes a caret, so it is dropped.
  const dropSvgCaret = () => {
    const selection = getSelection();
    if (!selection || selection.rangeCount === 0 || !selection.isCollapsed) return;
    const node = selection.anchorNode;
    const element = node?.nodeType === Node.ELEMENT_NODE ? node : node?.parentElement;
    if (element && outermostSvg(element)) selection.removeAllRanges();
  };
  document.addEventListener('selectionchange', dropSvgCaret);
  addEventListener('pointerup', () => setTimeout(dropSvgCaret, 0), true);
  // Servo tells the shell only a cursor's keyword. The shim reads the image
  // cursor of the element under the pointer, `url(...)` or an
  // `image-set(...)`'s first image, and hands its pixels to the shell once,
  // then names it each time it shows; `none` gives the keyword back.
  const cursorIds = new Map();
  let shownCursor = '', cursorTarget = null, cursorQueued = false, nextCursorId = 0;
  const imageCursor = (css) => {
    const found = /^(?:image-set\(\s*)?url\("((?:[^"\\]|\\.)*)"\)(?:\s+([\d.]+)(dppx|x))?\)?\s+(-?[\d.]+)\s+(-?[\d.]+)/.exec(css)
      ?? /^(?:image-set\(\s*)?url\("((?:[^"\\]|\\.)*)"\)(?:\s+([\d.]+)(dppx|x))?\)?/.exec(css);
    if (!found) return null;
    return { url: found[1].replace(/\\(.)/g, '$1'), scale: Number(found[2] ?? 1) || 1, x: Number(found[4] ?? 0), y: Number(found[5] ?? 0) };
  };
  const defineCursor = async (css, cursor) => {
    const image = new Image();
    image.src = cursor.url;
    await image.decode();
    const canvas = document.createElement('canvas');
    canvas.width = image.naturalWidth;
    canvas.height = image.naturalHeight;
    const context = canvas.getContext('2d');
    context.drawImage(image, 0, 0);
    const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
    let hex = '';
    for (const byte of pixels) hex += (byte < 16 ? '0' : '') + byte.toString(16);
    const id = nextCursorId++;
    const clamp = (value, size) => Math.max(0, Math.min(size - 1, Math.round(value * cursor.scale)));
    console.info(`rusty-desktop:cursor-image ${id} ${canvas.width} ${canvas.height} ${clamp(cursor.x, canvas.width)} ${clamp(cursor.y, canvas.height)} ${cursor.scale} ${hex}`);
    return id;
  };
  const showCursor = () => {
    cursorQueued = false;
    const css = document.pointerLockElement || !cursorTarget?.isConnected ? '' : getComputedStyle(cursorTarget).cursor;
    if (css === shownCursor) return;
    shownCursor = css;
    const cursor = imageCursor(css);
    if (!cursor) { console.info('rusty-desktop:cursor none'); return; }
    if (!cursorIds.has(css)) cursorIds.set(css, defineCursor(css, cursor).catch(() => null));
    cursorIds.get(css).then((id) => {
      if (shownCursor === css) console.info(id === null ? 'rusty-desktop:cursor none' : `rusty-desktop:cursor ${id}`);
    });
  };
  const queueCursor = (event) => {
    cursorTarget = event.target instanceof Element ? event.target : cursorTarget;
    if (!cursorQueued) { cursorQueued = true; requestAnimationFrame(showCursor); }
  };
  for (const type of ['pointermove', 'pointerdown', 'pointerup', 'pointerover']) addEventListener(type, queueCursor, true);
  let open = null;
  const close = () => {
    if (!open) return;
    open.host.remove();
    window.removeEventListener('keydown', open.keys, true);
    window.removeEventListener('mousedown', open.outside, true);
    open = null;
  };
  const answer = (ids) => {
    close();
    console.info(ids === null ? 'rusty-desktop:select-cancel' : `rusty-desktop:select ${ids.join(',')}`);
  };
  // Below the anchor (CSS pixels) where the panel fits, else on the side
  // with more room, scrolling what does not fit there; never past the view's
  // edges.
  const place = (host, panel, [x, y, width, height]) => {
    host.style.cssText = `position: fixed; z-index: 2147483647; left: ${x}px; top: ${y + height}px;
      min-width: ${Math.min(width, innerWidth)}px; max-width: ${innerWidth}px;`;
    document.documentElement.append(host);
    const natural = panel.getBoundingClientRect().height;
    const below = innerHeight - (y + height);
    const above = y;
    const downward = natural <= below || below >= above;
    const shown = Math.max(0, Math.min(natural, downward ? below : above));
    panel.style.maxHeight = `${shown}px`;
    host.style.top = `${Math.max(0, downward ? y + height : y - shown)}px`;
    const box = host.getBoundingClientRect();
    host.style.left = `${Math.max(0, Math.min(x, innerWidth - box.width))}px`;
  };
  const show = (host, keys, outside) => {
    window.addEventListener('keydown', keys, true);
    window.addEventListener('mousedown', outside, true);
    open = { host, keys, outside };
  };
  window.__rustyDesktopSelectClose = close;
  window.__rustyDesktopSelect = (rect, options, multiple) => {
    close();
    const ratio = window.devicePixelRatio || 1;
    const host = document.createElement('div');
    const root = host.attachShadow({ mode: 'closed' });
    const style = document.createElement('style');
    style.textContent = `
      .list { background: #fff; color: #000; border: 1px solid #767676; font: 13px sans-serif;
        box-sizing: border-box; max-height: 20em; overflow-y: auto; overflow-x: hidden;
        box-shadow: 0 2px 6px rgba(0,0,0,.35); }
      .group { padding: 2px 6px; font-weight: bold; }
      .row { padding: 2px 6px; white-space: pre; overflow: hidden; text-overflow: ellipsis; cursor: default; }
      .grouped { padding-left: 18px; }
      .multiple .row::before { content: '\\2713'; display: inline-block; width: 1.2em; visibility: hidden; }
      .multiple .row.chosen::before { visibility: visible; }
      .current { background: #1a73e8; color: #fff; }
      .disabled { color: #888; }`;
    const list = document.createElement('div');
    list.className = multiple ? 'list multiple' : 'list';
    const chosen = new Set(options.filter((option) => option.selected).map((option) => option.id));
    const rows = [];
    let group = null;
    for (const option of options) {
      if (option.group !== null && option.group !== group) {
        const label = document.createElement('div');
        label.className = 'group';
        label.textContent = option.group;
        list.append(label);
      }
      group = option.group;
      const row = document.createElement('div');
      row.className = 'row' + (option.group !== null ? ' grouped' : '') + (option.disabled ? ' disabled' : '');
      row.textContent = option.label;
      const index = rows.length;
      row.addEventListener('pointerenter', () => { if (!option.disabled) mark(index); });
      row.classList.toggle('chosen', chosen.has(option.id));
      row.addEventListener('pointerup', (event) => {
        event.stopPropagation();
        if (option.disabled) return;
        if (multiple) toggle(index);
        else answer([option.id]);
      });
      rows.push(row);
      list.append(row);
    }
    let current = Math.max(0, options.findIndex((option) => option.selected));
    const mark = (index) => {
      current = index;
      rows.forEach((row, i) => row.classList.toggle('current', i === current));
      rows[current]?.scrollIntoView({ block: 'nearest' });
    };
    const toggle = (index) => {
      const id = options[index].id;
      if (!chosen.delete(id)) chosen.add(id);
      rows[index].classList.toggle('chosen', chosen.has(id));
      mark(index);
    };
    const chosenIds = () => options.filter((option) => chosen.has(option.id)).map((option) => option.id);
    // A press moves focus off the <select>, and Servo closes its control,
    // ignoring later answers, when the element loses focus.
    list.addEventListener('mousedown', (event) => event.preventDefault());
    root.append(style, list);
    place(host, list, rect.map((value) => value / ratio));
    mark(current);
    const step = (by) => {
      for (let i = current + by; i >= 0 && i < rows.length; i += by) {
        if (!options[i].disabled) return mark(i);
      }
    };
    const keys = (event) => {
      if (event.key === 'ArrowDown') step(1);
      else if (event.key === 'ArrowUp') step(-1);
      else if (multiple && event.key === ' ') { if (!options[current]?.disabled) toggle(current); }
      else if (multiple && event.key === 'Enter') answer(chosenIds());
      else if (event.key === 'Enter' || event.key === ' ') answer(options[current] ? [options[current].id] : null);
      else if (event.key === 'Escape' || event.key === 'Tab') answer(null);
      else return;
      event.preventDefault();
      event.stopImmediatePropagation();
    };
    const outside = (event) => {
      if (event.composedPath().includes(host)) return;
      // A multiple list answers with what was toggled, so the press keeps
      // the element's focus.
      if (multiple) event.preventDefault();
      answer(multiple ? chosenIds() : null);
    };
    show(host, keys, outside);
  };
  // Servo draws date and time inputs as text fields: a click, or Alt+Down
  // on a focused one, opens the shell's picker, which writes the value back
  // with input and change events, as a browser's picker does.
  const TIMES = new Set(['date', 'time', 'datetime-local']);
  const pad = (number) => String(number).padStart(2, '0');
  const iso = (year, month, day) => `${String(year).padStart(4, '0')}-${pad(month + 1)}-${pad(day)}`;
  const parse = (value) => {
    const date = /^(\d{4,})-(\d{2})-(\d{2})/.exec(value);
    const time = /(?:^|T)(\d{2}):(\d{2})/.exec(value);
    return {
      date: date && { year: +date[1], month: +date[2] - 1, day: +date[3] },
      time: time && { hour: +time[1], minute: +time[2] },
    };
  };
  const monthName = (year, month) => {
    try { return new Date(year, month, 1).toLocaleString(undefined, { month: 'long', year: 'numeric' }); }
    catch { return `${year}-${pad(month + 1)}`; }
  };
  const pickTime = (input) => {
    close();
    const withDate = input.type !== 'time';
    const withTime = input.type !== 'date';
    const now = new Date();
    const value = parse(input.value);
    let date = value.date ?? { year: now.getFullYear(), month: now.getMonth(), day: now.getDate() };
    let time = value.time ?? { hour: now.getHours(), minute: 0 };
    let shown = { year: date.year, month: date.month };
    const [min, max] = [parse(input.min).date, parse(input.max).date]
      .map((limit) => limit && iso(limit.year, limit.month, limit.day));
    const allowed = (day) => (!min || day >= min) && (!max || day <= max);
    const host = document.createElement('div');
    const root = host.attachShadow({ mode: 'closed' });
    const style = document.createElement('style');
    style.textContent = `
      .panel { background: #fff; color: #000; border: 1px solid #767676; font: 13px sans-serif;
        box-sizing: border-box; overflow: auto; padding: 6px; box-shadow: 0 2px 6px rgba(0,0,0,.35);
        display: flex; flex-direction: column; gap: 6px; width: max-content; }
      .head { display: flex; align-items: center; justify-content: space-between; gap: 8px; font-weight: bold; }
      .grid { display: grid; grid-template-columns: repeat(7, 2.2em); }
      .grid div { text-align: center; padding: 3px 0; cursor: default; }
      .weekday { color: #666; font-size: 11px; }
      .other { color: #aaa; }
      .disabled { color: #ccc; }
      .chosen, .times div.chosen { background: #1a73e8; color: #fff; }
      .today { outline: 1px solid #1a73e8; }
      .times { display: flex; gap: 4px; }
      .times .column { height: 10em; overflow-y: auto; border: 1px solid #ddd; }
      .times .column div { padding: 2px 8px; cursor: default; }
      .foot { display: flex; justify-content: flex-end; gap: 6px; }
      button { font: inherit; }`;
    const panel = document.createElement('div');
    panel.className = 'panel';
    const commit = () => {
      const day = iso(date.year, date.month, date.day);
      // Days outside min and max are refused whichever way the choice comes.
      if (withDate && !allowed(day)) return;
      const clock = `${pad(time.hour)}:${pad(time.minute)}`;
      close();
      input.value = withDate && withTime ? `${day}T${clock}` : withDate ? day : clock;
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.dispatchEvent(new Event('change', { bubbles: true }));
    };
    const button = (label, action) => {
      const element = document.createElement('button');
      element.textContent = label;
      element.addEventListener('click', (event) => { event.stopPropagation(); action(); });
      return element;
    };
    const head = document.createElement('div');
    const title = document.createElement('span');
    const grid = document.createElement('div');
    const moveMonth = (by) => {
      const first = new Date(shown.year, shown.month + by, 1);
      shown = { year: first.getFullYear(), month: first.getMonth() };
      drawDays();
    };
    const drawDays = () => {
      title.textContent = monthName(shown.year, shown.month);
      grid.replaceChildren();
      for (const name of ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa']) {
        const cell = document.createElement('div');
        cell.className = 'weekday';
        cell.textContent = name;
        grid.append(cell);
      }
      const start = new Date(shown.year, shown.month, 1 - new Date(shown.year, shown.month, 1).getDay());
      const today = iso(now.getFullYear(), now.getMonth(), now.getDate());
      const chosen = iso(date.year, date.month, date.day);
      for (let i = 0; i < 42; i++) {
        const day = new Date(start.getFullYear(), start.getMonth(), start.getDate() + i);
        const key = iso(day.getFullYear(), day.getMonth(), day.getDate());
        const cell = document.createElement('div');
        cell.textContent = day.getDate();
        cell.classList.toggle('other', day.getMonth() !== shown.month);
        cell.classList.toggle('disabled', !allowed(key));
        cell.classList.toggle('chosen', key === chosen);
        cell.classList.toggle('today', key === today);
        cell.addEventListener('pointerup', (event) => {
          event.stopPropagation();
          if (!allowed(key)) return;
          date = { year: day.getFullYear(), month: day.getMonth(), day: day.getDate() };
          if (!withTime) return commit();
          shown = { year: date.year, month: date.month };
          drawDays();
        });
        grid.append(cell);
      }
    };
    if (withDate) {
      head.className = 'head';
      head.append(button('\u2039', () => moveMonth(-1)), title, button('\u203a', () => moveMonth(1)));
      grid.className = 'grid';
      panel.append(head, grid);
      drawDays();
    }
    const columns = [];
    if (withTime) {
      const times = document.createElement('div');
      times.className = 'times';
      for (const [part, count] of [['hour', 24], ['minute', 60]]) {
        const column = document.createElement('div');
        column.className = 'column';
        for (let n = 0; n < count; n++) {
          const cell = document.createElement('div');
          cell.textContent = pad(n);
          cell.classList.toggle('chosen', time[part] === n);
          cell.addEventListener('pointerup', (event) => {
            event.stopPropagation();
            time = { ...time, [part]: n };
            for (const other of column.children) other.classList.toggle('chosen', other === cell);
          });
          column.append(cell);
        }
        columns.push([column, part]);
        times.append(column);
      }
      panel.append(times);
    }
    const foot = document.createElement('div');
    foot.className = 'foot';
    foot.append(button('Clear', () => {
      close();
      input.value = '';
      input.dispatchEvent(new Event('input', { bubbles: true }));
      input.dispatchEvent(new Event('change', { bubbles: true }));
    }));
    if (withTime) foot.append(button('Done', commit));
    panel.append(foot);
    // A press inside keeps the input focused.
    panel.addEventListener('mousedown', (event) => event.preventDefault());
    root.append(style, panel);
    const box = input.getBoundingClientRect();
    place(host, panel, [box.left, box.top, box.width, box.height]);
    for (const [column, part] of columns) column.children[time[part]]?.scrollIntoView({ block: 'center' });
    const keys = (event) => {
      const by = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -7, ArrowDown: 7 }[event.key];
      if (withDate && by !== undefined) {
        const day = new Date(date.year, date.month, date.day + by);
        date = { year: day.getFullYear(), month: day.getMonth(), day: day.getDate() };
        shown = { year: date.year, month: date.month };
        drawDays();
      } else if (withDate && (event.key === 'PageUp' || event.key === 'PageDown')) {
        // The chosen day moves a month, kept inside the shorter month.
        const by = event.key === 'PageUp' ? -1 : 1;
        const first = new Date(date.year, date.month + by, 1);
        const last = new Date(first.getFullYear(), first.getMonth() + 1, 0).getDate();
        date = { year: first.getFullYear(), month: first.getMonth(), day: Math.min(date.day, last) };
        shown = { year: date.year, month: date.month };
        drawDays();
      } else if (event.key === 'Enter') {
        commit();
      } else if (event.key === 'Escape' || event.key === 'Tab') {
        close();
      } else return;
      event.preventDefault();
      event.stopImmediatePropagation();
    };
    const outside = (event) => { if (!event.composedPath().includes(host)) close(); };
    show(host, keys, outside);
  };
  document.addEventListener('click', (event) => {
    const input = event.target instanceof Element && event.target.closest('input');
    if (!input || !event.isTrusted || !TIMES.has(input.type) || input.disabled || input.readOnly) return;
    pickTime(input);
  });
  document.addEventListener('keydown', (event) => {
    const input = event.target instanceof Element && event.target.closest('input');
    if (!input || open || !TIMES.has(input.type) || input.disabled || input.readOnly
      || !(event.altKey && event.key === 'ArrowDown')) return;
    event.preventDefault();
    pickTime(input);
  });
})();"#;

/// Physical pixels per wheel line, as Chromium scrolls on Linux.
const WHEEL_PIXELS_PER_LINE: f64 = 53.0;

/// Image cursors kept built; a page that cycles through more starts over.
const IMAGE_CURSOR_CACHE: usize = 32;

pub(crate) struct UiOverlay {
    // The page goes before the runtime that hosts it.
    page: WebOverlay,
    runtime: WebRuntime,
    cursor: (i32, i32),
    modifiers: ModifiersState,
    buttons: [bool; 3],
    grabbed: bool,
    /// The visible cursor is confined to the window.
    confined: bool,
    /// The lock keeps the cursor in the window by warping it back to the
    /// centre, not by a pointer grab (X11).
    recentring: bool,
    focused: bool,
    /// Windows creates the browser asynchronously: the page takes focus
    /// once it exists.
    focus_pending: bool,
    motion: (f64, f64),
    /// What the page asks the native cursor to show; `None` hides it. A lock
    /// hides the cursor over this and shows it again on release.
    page_cursor: Option<Cursor>,
    /// What the UI engine last reported, which an image cursor from the
    /// Servo shim stands in front of.
    reported_cursor: Option<Cursor>,
    /// The Servo shim's image cursors, by its id, and the one it shows.
    shim_cursors: HashMap<u64, CursorImage>,
    shim_cursor: Option<CursorImage>,
    /// Image cursors built so far, by content and window scale.
    image_cursors: HashMap<(CursorImage, u64), CustomCursor>,
    /// Gameplay input the window hands the runtime while it holds the lock.
    gameplay: GameplayRoute,
    /// Live-debug scripts' answers from the page, not yet passed on.
    page_answers: Vec<(u64, String)>,
}

impl UiOverlay {
    pub(crate) fn open(
        config: &WebRuntimeConfig,
        gpu: &Gpu,
        url: &str,
        window: &Arc<Window>,
        scene: Arc<dyn DesktopScene>,
        controller: bool,
        proxy: EventLoopProxy<()>,
    ) -> Result<Self, String> {
        let runtime = WebRuntime::initialize(config.clone(), move || {
            let _ = proxy.send_event(());
        })?;
        // Servo takes the shell's scripts at the start of each document.
        #[cfg(feature = "servo-overlay")]
        let document_script = matches!(config.engine, render_wgpu::web::WebEngine::Servo { .. })
            .then(|| format!("{PAGE_SHIM}\n{SERVO_SHIM}"));
        #[cfg(not(feature = "servo-overlay"))]
        let document_script: Option<String> = None;
        let size = window.inner_size();
        let mut page = WebOverlay::new(
            &runtime,
            gpu,
            url,
            size.width,
            size.height,
            window.scale_factor() as f32,
            document_script.as_deref(),
        )?;
        let focus_pending = page.focus().is_err();
        Ok(Self {
            page,
            runtime,
            cursor: (0, 0),
            modifiers: ModifiersState::empty(),
            buttons: [false; 3],
            grabbed: false,
            confined: false,
            recentring: false,
            focused: window.has_focus(),
            focus_pending,
            motion: (0.0, 0.0),
            page_cursor: Some(CursorIcon::Default.into()),
            reported_cursor: Some(CursorIcon::Default.into()),
            shim_cursors: HashMap::new(),
            shim_cursor: None,
            image_cursors: HashMap::new(),
            gameplay: GameplayRoute::new(scene, controller),
            page_answers: Vec::new(),
        })
    }

    /// Evaluate live debug's script `id` in the page, in its global scope.
    /// The page answers on the console with the script's completion value,
    /// awaited when it is a promise and serialized as soon as it settles, as
    /// `{"value": ...}` in JSON (`undefined` reads `null`), or what it threw
    /// or what failed to serialize as `{"error": ...}`. Either carries
    /// `renderedAtUnixMs`: when the page had laid out what the finished script
    /// changed (two animation frames later), or `null` when no frame came
    /// within 250 ms or the page unloaded first (the script navigated away).
    /// An error here means the page could not take the script.
    pub(crate) fn evaluate(&mut self, id: u64, source: &str) -> Result<(), String> {
        // A Rust string's debug form is a valid JavaScript string literal.
        let script = format!(
            "(async () => {{ let body; \
             try {{ const value = await (0, eval)({source:?}); \
             body = JSON.stringify({{ value: value === undefined ? null : value }}); }} \
             catch (error) {{ body = JSON.stringify({{ error: String(error) }}); }} \
             let sent = false; \
             const send = (rendered) => {{ if (sent) return; sent = true; \
               removeEventListener('pagehide', unload); \
               const field = '\"renderedAtUnixMs\":' + JSON.stringify(rendered); \
               console.info('{PAGE_ANSWER}{id} ' + \
                 (body === '{{}}' ? '{{' + field + '}}' : body.slice(0, -1) + ',' + field + '}}')); }}; \
             const unload = () => send(null); \
             addEventListener('pagehide', unload); \
             requestAnimationFrame(() => requestAnimationFrame(() => send(Date.now()))); \
             setTimeout(() => send(null), 250); }})();"
        );
        self.page.execute_script(&script)
    }

    /// The page's answers to live-debug scripts since the last call.
    pub(crate) fn take_page_answers(&mut self) -> Vec<(u64, String)> {
        std::mem::take(&mut self.page_answers)
    }

    /// Whether the engine wakes the event loop for its work, rather than
    /// being pumped on a timer.
    pub(crate) fn wakes(&self) -> bool {
        self.runtime.wakes()
    }

    pub(crate) fn page(&mut self) -> &mut WebOverlay {
        &mut self.page
    }

    pub(crate) fn stats(&self) -> WebOverlayStats {
        self.page.stats()
    }

    pub(crate) fn last_error(&self) -> Option<String> {
        self.page.last_error().map(str::to_owned)
    }

    /// Chromium's work and the page's requests.
    pub(crate) fn pump(&mut self, window: &Window, event_loop: &ActiveEventLoop) {
        self.runtime.pump();
        if self.focus_pending {
            self.focus_pending = self.page.focus().is_err();
        }
        while let Some(event) = self.page.poll_event() {
            match event {
                NavigationEvent::LoadStart { .. } | NavigationEvent::LoadEnd { .. } => {
                    // Before the page's scripts ask for a lock, and again
                    // after, since a load start may still see the old page.
                    let _ = self.page.execute_script(PAGE_SHIM);
                    if !self.focused {
                        self.report_focus();
                    }
                    if matches!(event, NavigationEvent::LoadStart { .. }) {
                        self.release(window, false, InputClearReason::PointerLockLoss);
                        self.unconfine(window, false);
                    } else if self.focused {
                        // A page from another origin loads in a new Chromium
                        // renderer, which starts without focus.
                        let _ = self.page.focus();
                    }
                }
                NavigationEvent::DragStarted { x, y, .. } => {
                    let _ = self.page.refuse_drag(x, y);
                }
                // welding denies the new window; the page hears why.
                NavigationEvent::NewWindowRequested { url, .. } => {
                    let warning = format!(
                        "rusty desktop shell: a new window for {url} is not opened in the \
                         desktop window. Keep game UI in this page. \
                         (docs/desktop-shell.md#browser-features-in-the-window)"
                    );
                    // A Rust string's debug form is a valid JavaScript string literal.
                    let _ = self
                        .page
                        .execute_script(&format!("console.warn({warning:?});"));
                }
                NavigationEvent::ConsoleMessage { message, .. } if message == LOCK_REQUEST => {
                    self.grab(window);
                }
                NavigationEvent::ConsoleMessage { message, .. } if message == LOCK_EXIT => {
                    self.release(window, false, InputClearReason::PointerLockLoss);
                }
                NavigationEvent::ConsoleMessage { message, .. } if message == CONFINE_REQUEST => {
                    self.confine(window);
                }
                NavigationEvent::ConsoleMessage { message, .. } if message == CONFINE_EXIT => {
                    self.unconfine(window, false);
                }
                NavigationEvent::ConsoleMessage { message, .. }
                    if message.starts_with(SELECT_CHOSEN) =>
                {
                    let ids: Vec<usize> = message[SELECT_CHOSEN.len()..]
                        .split(',')
                        .filter_map(|id| id.parse().ok())
                        .collect();
                    self.page.answer_select(Some(&ids));
                }
                NavigationEvent::ConsoleMessage { message, .. }
                    if message.starts_with(CURSOR_IMAGE) =>
                {
                    if let Some((id, image)) = shim_cursor_image(&message[CURSOR_IMAGE.len()..]) {
                        self.shim_cursors.insert(id, image);
                    }
                }
                NavigationEvent::ConsoleMessage { message, .. }
                    if message.starts_with(CURSOR_SHOWN) =>
                {
                    let shown = &message[CURSOR_SHOWN.len()..];
                    self.shim_cursor = shown
                        .parse()
                        .ok()
                        .and_then(|id| self.shim_cursors.get(&id).cloned());
                    self.show_cursor(window, event_loop);
                }
                NavigationEvent::ConsoleMessage { message, .. } if message == SELECT_CANCELLED => {
                    self.page.answer_select(None);
                }
                NavigationEvent::ConsoleMessage { message, .. }
                    if message.starts_with(PAGE_ANSWER) =>
                {
                    if let Some((id, answer)) = message[PAGE_ANSWER.len()..].split_once(' ') {
                        if let Ok(id) = id.parse() {
                            self.page_answers.push((id, answer.to_owned()));
                        }
                    }
                }
                _ => {}
            }
        }
        while let Some(control) = self.page.poll_control() {
            let script = match control {
                PageControl::Select {
                    rect,
                    options,
                    multiple,
                } => {
                    // A Rust string's debug form is a valid JavaScript string literal.
                    let options: Vec<String> = options
                        .iter()
                        .map(|option| {
                            format!(
                                "{{id:{},label:{:?},disabled:{},selected:{},group:{}}}",
                                option.id,
                                option.label,
                                option.disabled,
                                option.selected,
                                option
                                    .group
                                    .as_ref()
                                    .map_or("null".to_owned(), |group| format!("{group:?}"))
                            )
                        })
                        .collect();
                    format!(
                        "window.__rustyDesktopSelect([{},{},{},{}],[{}],{multiple});",
                        rect.x,
                        rect.y,
                        rect.width,
                        rect.height,
                        options.join(",")
                    )
                }
                PageControl::Closed => "window.__rustyDesktopSelectClose();".to_owned(),
            };
            let _ = self.page.execute_script(&script);
        }
        while let Some(shape) = self.page.poll_cursor() {
            self.reported_cursor = match shape {
                CursorShape::Custom(name) if name == "none" => None,
                CursorShape::Image(image) => Some(
                    self.image_cursor(image, window.scale_factor(), event_loop)
                        .map_or(CursorIcon::Default.into(), Cursor::Custom),
                ),
                shape => Some(cursor_icon(&shape).into()),
            };
            self.show_cursor(window, event_loop);
        }
    }

    /// The Servo shim's image cursor while it shows one, else what the UI
    /// engine reported.
    fn show_cursor(&mut self, window: &Window, event_loop: &ActiveEventLoop) {
        self.page_cursor = match self.shim_cursor.clone() {
            Some(image) => Some(
                self.image_cursor(image, window.scale_factor(), event_loop)
                    .map_or(CursorIcon::Default.into(), Cursor::Custom),
            ),
            None => self.reported_cursor.clone(),
        };
        if !self.grabbed {
            self.show_page_cursor(window);
        }
    }

    fn show_page_cursor(&self, window: &Window) {
        match &self.page_cursor {
            Some(cursor) => {
                window.set_cursor(cursor.clone());
                window.set_cursor_visible(true);
            }
            None => window.set_cursor_visible(false),
        }
    }

    /// Chromium draws an image cursor at its CSS size; the window shows it at
    /// the window's scale. `None` if the platform refuses the image.
    fn image_cursor(
        &mut self,
        image: CursorImage,
        window_scale: f64,
        event_loop: &ActiveEventLoop,
    ) -> Option<CustomCursor> {
        let key = (image, window_scale.to_bits());
        if let Some(cursor) = self.image_cursors.get(&key) {
            return Some(cursor.clone());
        }
        let image = &key.0;
        let factor = window_scale / f64::from(image.scale);
        let source = if (factor - 1.0).abs() < 0.01 {
            CustomCursor::from_rgba(
                image.rgba.to_vec(),
                u16::try_from(image.width).ok()?,
                u16::try_from(image.height).ok()?,
                u16::try_from(image.hotspot_x).ok()?,
                u16::try_from(image.hotspot_y).ok()?,
            )
        } else {
            let scaled = |n: u32| ((f64::from(n) * factor).round() as u32).max(1);
            let (width, height) = (scaled(image.width), scaled(image.height));
            CustomCursor::from_rgba(
                resample(image, width, height),
                u16::try_from(width).ok()?,
                u16::try_from(height).ok()?,
                u16::try_from((f64::from(image.hotspot_x) * factor) as u32)
                    .ok()?
                    .min(width as u16 - 1),
                u16::try_from((f64::from(image.hotspot_y) * factor) as u32)
                    .ok()?
                    .min(height as u16 - 1),
            )
        }
        .ok()?;
        let cursor = event_loop.create_custom_cursor(source);
        if self.image_cursors.len() >= IMAGE_CURSOR_CACHE {
            self.image_cursors.clear();
        }
        self.image_cursors.insert(key, cursor.clone());
        Some(cursor)
    }

    /// Hand the page this frame's locked mouse motion, once per frame.
    pub(crate) fn flush_motion(&mut self) {
        let (x, y) = std::mem::take(&mut self.motion);
        if self.grabbed && (x != 0.0 || y != 0.0) {
            let _ = self
                .page
                .execute_script(&format!("window.__rustyDesktopMotion({x}, {y});"));
        }
    }

    pub(crate) fn raw_motion(&mut self, x: f64, y: f64) {
        if self.grabbed {
            self.motion.0 += x;
            self.motion.1 += y;
            self.gameplay.motion(x, y);
        }
    }

    /// Hand the runtime the locked motion since the last wake.
    pub(crate) fn flush_gameplay_motion(&mut self) {
        self.gameplay.flush_motion();
    }

    /// The gamepad's state changed.
    pub(crate) fn gamepad(&mut self, state: crate::gamepad::PadState) {
        self.page
            .send_gamepad(state.connected.then_some((state.axes, state.values)));
        self.gameplay.pad(state);
    }

    fn grab(&mut self, window: &Window) {
        let recentring = match window.set_cursor_grab(CursorGrabMode::Locked) {
            Ok(()) => Some(false),
            // X11 has no locked grab, and a confining grab makes XInput
            // deliver every raw motion twice (to the root window's selection
            // and to the grabbing client), doubling the turn rate. Keep the
            // hidden cursor in the window by recentring it instead.
            Err(_) if is_x11(window) => Some(true),
            Err(_) => window
                .set_cursor_grab(CursorGrabMode::Confined)
                .ok()
                .map(|_| false),
        };
        let Some(recentring) = recentring else {
            // No grab on this platform: tell the page the lock is gone.
            let _ = self.page.execute_script("window.__rustyDesktopUnlocked();");
            return;
        };
        window.set_cursor_visible(false);
        self.grabbed = true;
        self.recentring = recentring;
        self.motion = (0.0, 0.0);
        self.recentre(window);
        self.gameplay.take();
    }

    /// Warp a recentring lock's cursor back to the window's centre.
    fn recentre(&mut self, window: &Window) {
        if !self.recentring {
            return;
        }
        let size = window.inner_size();
        let centre = (size.width as i32 / 2, size.height as i32 / 2);
        if self.cursor != centre {
            let _ = window.set_cursor_position(PhysicalPosition::new(centre.0, centre.1));
        }
    }

    /// Release the cursor. `tell_page` when the shell, not the page, ended
    /// the lock (Escape, focus loss), as a browser reports an ended lock.
    /// The page takes gameplay input back, after a clear for `reason`.
    fn release(&mut self, window: &Window, tell_page: bool, reason: InputClearReason) {
        if !self.grabbed {
            return;
        }
        self.gameplay.flush_motion();
        self.gameplay.give_back(reason);
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        self.show_page_cursor(window);
        self.grabbed = false;
        self.recentring = false;
        // The page asked for confinement before the lock's end arrived: the
        // lock and the confinement are one grab, so it starts now.
        if std::mem::take(&mut self.confined) {
            self.confine(window);
        }
        if tell_page {
            let _ = self.page.execute_script("window.__rustyDesktopUnlocked();");
        }
    }

    /// Keep the visible cursor in the window. Platforms without a confining
    /// grab (macOS) refuse, and the page draws its own cursor instead. A held
    /// lock is already a grab; the page ends it next.
    fn confine(&mut self, window: &Window) {
        if self.confined {
            return;
        }
        if !self.grabbed && window.set_cursor_grab(CursorGrabMode::Confined).is_err() {
            let _ = self
                .page
                .execute_script("window.__rustyDesktopConfinementEnded(true);");
            return;
        }
        self.confined = true;
    }

    /// End a confinement. `tell_page` when the shell, not the page, ended it.
    fn unconfine(&mut self, window: &Window, tell_page: bool) {
        if !self.confined {
            return;
        }
        self.confined = false;
        if !self.grabbed {
            let _ = window.set_cursor_grab(CursorGrabMode::None);
        }
        if tell_page {
            let _ = self
                .page
                .execute_script("window.__rustyDesktopConfinementEnded(false);");
        }
    }

    /// Tell the page whether the native window has focus.
    fn report_focus(&mut self) {
        let _ = self
            .page
            .execute_script(&format!("window.__rustyDesktopFocus({});", self.focused));
    }

    pub(crate) fn window_event(&mut self, window: &Window, event: &WindowEvent) {
        match event {
            WindowEvent::Resized(size) => {
                let _ = self.page.resize(size.width, size.height);
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                let _ = self.page.set_scale_factor(*scale_factor as f32);
            }
            WindowEvent::Focused(false) => {
                self.release(window, true, InputClearReason::FocusLoss);
                self.gameplay.forget_keys();
                self.unconfine(window, true);
                // Buttons and modifiers released while another window has
                // focus never reach this one; the page clears its own held
                // input on the blur.
                self.buttons = [false; 3];
                self.modifiers = ModifiersState::empty();
                self.focused = false;
                self.report_focus();
            }
            WindowEvent::Focused(true) => {
                self.focused = true;
                let _ = self.page.focus();
                self.report_focus();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = (position.x as i32, position.y as i32);
                if !self.grabbed {
                    self.mouse(MouseButton::Left, MouseAction::Moved);
                } else {
                    self.recentre(window);
                }
            }
            WindowEvent::CursorLeft { .. } if !self.grabbed => {
                // Chromium treats a move outside the view as leaving it. Keep
                // the last position: a pointer that re-enters without moving
                // (a window mapped under it) reports no new one.
                let inside = self.cursor;
                self.cursor = (-1, -1);
                self.mouse(MouseButton::Left, MouseAction::Moved);
                self.cursor = inside;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let (button, index) = match button {
                    winit::event::MouseButton::Left => (MouseButton::Left, 0),
                    winit::event::MouseButton::Middle => (MouseButton::Middle, 1),
                    winit::event::MouseButton::Right => (MouseButton::Right, 2),
                    _ => return,
                };
                let pressed = *state == ElementState::Pressed;
                self.buttons[index] = pressed;
                if self.grabbed {
                    self.gameplay.flush_motion();
                    self.gameplay.button(index, pressed);
                }
                let action = if pressed {
                    MouseAction::Pressed
                } else {
                    MouseAction::Released
                };
                self.mouse(button, action);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (x, y) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (
                        f64::from(*x) * WHEEL_PIXELS_PER_LINE,
                        f64::from(*y) * WHEEL_PIXELS_PER_LINE,
                    ),
                    MouseScrollDelta::PixelDelta(position) => (position.x, position.y),
                };
                if self.grabbed {
                    self.gameplay.wheel(x.round() as i32, y.round() as i32);
                }
                self.mouse(
                    MouseButton::Left,
                    MouseAction::WheelScrolled {
                        delta_x: x.round() as i32,
                        delta_y: y.round() as i32,
                    },
                );
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else {
                    return;
                };
                let pressed = event.state == ElementState::Pressed;
                // As in a browser, Escape ends a pointer lock and the page
                // sees the lock end, not the key. A confinement ends the same way.
                if code == KeyCode::Escape && (self.grabbed || self.confined) {
                    if pressed {
                        self.release(window, true, InputClearReason::PointerLockLoss);
                        self.unconfine(window, true);
                    }
                    return;
                }
                if self.grabbed {
                    self.gameplay.flush_motion();
                }
                self.gameplay.key(code, pressed);
                let Some(windows_key_code) = keys::windows_key_code(code) else {
                    return;
                };
                // winit names keys and codes as the W3C does, except Super
                // (the W3C's Meta) and Space.
                let code_name = match code {
                    KeyCode::SuperLeft => "MetaLeft".to_owned(),
                    KeyCode::SuperRight => "MetaRight".to_owned(),
                    code => format!("{code:?}"),
                };
                let named;
                let key = match &event.logical_key {
                    Key::Character(text) => text.as_str(),
                    Key::Named(NamedKey::Space) => " ",
                    Key::Named(NamedKey::Super) => "Meta",
                    Key::Named(name) => {
                        named = format!("{name:?}");
                        &named
                    }
                    _ => "Unidentified",
                };
                let _ = self.page.send_key(&KeyInput {
                    pressed,
                    repeat: event.repeat,
                    code: &code_name,
                    key,
                    text: event.text.as_deref(),
                    windows_key_code,
                    native_key_code: keys::native_key_code(code),
                    modifiers: self.event_modifiers(),
                });
            }
            _ => {}
        }
    }

    fn mouse(&mut self, button: MouseButton, action: MouseAction) {
        let (x, y) = self.cursor;
        let modifiers = self.event_modifiers();
        let _ = self.page.send_mouse(MouseEvent {
            x,
            y,
            button,
            action,
            modifiers,
        });
    }

    fn event_modifiers(&self) -> EventModifiers {
        EventModifiers {
            shift: self.modifiers.shift_key(),
            ctrl: self.modifiers.control_key(),
            alt: self.modifiers.alt_key(),
            meta: self.modifiers.super_key(),
            left_mouse_button: self.buttons[0],
            middle_mouse_button: self.buttons[1],
            right_mouse_button: self.buttons[2],
        }
    }
}

/// A `rusty-desktop:cursor-image` line's id and image, after its prefix.
fn shim_cursor_image(line: &str) -> Option<(u64, CursorImage)> {
    let mut fields = line.split(' ');
    let mut number = || fields.next()?.parse::<u32>().ok();
    let id = u64::from(number()?);
    let (width, height, hotspot_x, hotspot_y) = (number()?, number()?, number()?, number()?);
    let scale: f32 = fields.next()?.parse().ok()?;
    let hex = fields.next()?.as_bytes();
    if hex.len() != width as usize * height as usize * 8 || scale.is_nan() || scale <= 0.0 {
        return None;
    }
    let nibble = |byte: u8| (byte as char).to_digit(16).map(|digit| digit as u8);
    let rgba: Option<Vec<u8>> = hex
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&[high, low]| Some(nibble(high)? << 4 | nibble(low)?))
        .collect();
    Some((
        id,
        CursorImage {
            rgba: rgba?.into(),
            width,
            height,
            hotspot_x,
            hotspot_y,
            scale,
        },
    ))
}

fn cursor_icon(shape: &CursorShape) -> CursorIcon {
    match shape {
        CursorShape::Pointer => CursorIcon::Pointer,
        CursorShape::Text => CursorIcon::Text,
        CursorShape::Wait => CursorIcon::Wait,
        CursorShape::Crosshair => CursorIcon::Crosshair,
        CursorShape::Move => CursorIcon::Move,
        CursorShape::NotAllowed => CursorIcon::NotAllowed,
        CursorShape::Help => CursorIcon::Help,
        CursorShape::Progress => CursorIcon::Progress,
        CursorShape::ResizeNs => CursorIcon::NsResize,
        CursorShape::ResizeEw => CursorIcon::EwResize,
        CursorShape::ResizeNeSw => CursorIcon::NeswResize,
        CursorShape::ResizeNwSe => CursorIcon::NwseResize,
        CursorShape::ResizeAll => CursorIcon::AllScroll,
        CursorShape::Grab => CursorIcon::Grab,
        CursorShape::Grabbing => CursorIcon::Grabbing,
        CursorShape::ZoomIn => CursorIcon::ZoomIn,
        CursorShape::ZoomOut => CursorIcon::ZoomOut,
        _ => CursorIcon::Default,
    }
}

/// Nearest-neighbour scaling of an image cursor's RGBA pixels.
fn resample(image: &CursorImage, width: u32, height: u32) -> Vec<u8> {
    let mut rgba = Vec::with_capacity(width as usize * height as usize * 4);
    for y in 0..height {
        let source_y = (y * image.height / height) as usize;
        for x in 0..width {
            let source_x = (x * image.width / width) as usize;
            let at = (source_y * image.width as usize + source_x) * 4;
            rgba.extend_from_slice(&image.rgba[at..at + 4]);
        }
    }
    rgba
}

fn is_x11(window: &Window) -> bool {
    window.window_handle().is_ok_and(|handle| {
        matches!(
            handle.as_raw(),
            RawWindowHandle::Xlib(_) | RawWindowHandle::Xcb(_)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::shim_cursor_image;

    #[test]
    fn a_shim_cursor_line_is_its_image() {
        // Two pixels, red then half-transparent white, hotspot at the second, two image pixels per CSS pixel.
        let (id, image) = shim_cursor_image("7 2 1 1 0 2 ff0000ffffffff80").unwrap();
        assert_eq!(id, 7);
        assert_eq!(
            (image.width, image.height, image.hotspot_x, image.hotspot_y),
            (2, 1, 1, 0)
        );
        assert_eq!(image.scale, 2.0);
        assert_eq!(&image.rgba[..], &[255, 0, 0, 255, 255, 255, 255, 128]);
        // Pixels that do not fill the size, or are not hex, are no image.
        assert!(shim_cursor_image("7 2 1 1 0 2 ff0000ff").is_none());
        assert!(shim_cursor_image("7 2 1 1 0 2 ff0000ffffffffzz").is_none());
    }
}
