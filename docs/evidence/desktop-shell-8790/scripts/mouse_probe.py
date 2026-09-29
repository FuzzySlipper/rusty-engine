"""An unlocked held mouse button across native focus loss (the #8790 review case).

usage: mouse_probe.py <display> <cdp-origin>

The right button does not request pointer lock, so the pointer stays unlocked.
1. Record window blur/focus and pointer events in the page.
2. Press the right button on the canvas and hold it.
3. Map an xterm, which takes focus; release the button over it.
4. Refocus the shell by its title bar and move the pointer over the canvas.
5. The page must have seen a blur, `document.hasFocus()` must have been false
   while the xterm had focus, and the next move must carry no held button.
"""
import ctypes, json, subprocess, sys, time

display, cdp = sys.argv[1:3]
here = sys.path[0]
x = ctypes.cdll.LoadLibrary('libX11.so.6'); xt = ctypes.cdll.LoadLibrary('libXtst.so.6')
x.XOpenDisplay.restype = ctypes.c_void_p
d = ctypes.c_void_p(x.XOpenDisplay(display.encode()))


def flush():
    x.XFlush(d); time.sleep(0.05)


def move(px, py):
    xt.XTestFakeMotionEvent(d, -1, px, py, 0); flush()


def button(number, down):
    xt.XTestFakeButtonEvent(d, number, down, 0); flush()


def page(expression):
    out = subprocess.run(['node', f'{here}/cdp.mjs', cdp, expression], capture_output=True, text=True)
    return json.loads(out.stdout.strip().splitlines()[-1])


page("""(() => {
  const log = window.__probe = { blur: 0, focus: 0, down: [], up: [], moves: [] };
  window.addEventListener('blur', () => { log.blur += 1; });
  window.addEventListener('focus', () => { log.focus += 1; });
  document.addEventListener('pointerdown', (e) => log.down.push(e.button));
  document.addEventListener('pointerup', (e) => log.up.push(e.button));
  document.addEventListener('pointermove', (e) => { log.moves.push(e.buttons); if (log.moves.length > 50) log.moves.shift(); });
  return true;
})()""")
move(640, 450); time.sleep(0.2)
button(3, True); time.sleep(0.3)
print('pressed:', page('({ hasFocus: document.hasFocus(), probe: window.__probe })'))
term = subprocess.Popen(['xterm', '-display', display, '-geometry', '40x10+900+600', '-title', 'focus-thief'])
time.sleep(1.5)
move(1000, 700); button(3, False); time.sleep(0.3)   # released over the xterm
unfocused = page('({ hasFocus: document.hasFocus(), probe: window.__probe })')
print('unfocused:', unfocused)
term.terminate(); term.wait(); time.sleep(0.8)
move(640, 15); button(1, True); button(1, False); time.sleep(0.5)   # title bar
page('(window.__probe.moves = [], true)')
move(600, 420); move(620, 430); move(640, 440); time.sleep(0.3)
refocused = page('({ hasFocus: document.hasFocus(), probe: window.__probe })')
print('refocused:', refocused)
held_after = any(buttons != 0 for buttons in refocused['probe']['moves'])
print(f"RESULT blur_seen={unfocused['probe']['blur'] > 0} "
      f"hasFocus_while_unfocused={unfocused['hasFocus']} "
      f"moves_after_refocus={refocused['probe']['moves']} held_button_after_refocus={held_after}")
