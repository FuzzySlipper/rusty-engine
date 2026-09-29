"""Held input across native focus loss, through XTest on the shell's X11 window.

usage: focus_probe.py <display> <unlocked|locked> <live-debug> <origin>

1. Click the canvas (the page takes pointer lock); for `unlocked`, Escape ends it.
2. Hold W and sample the player position while it walks.
3. Map an xterm, which takes focus; release W there.
4. Sample the position: it must stop changing.
5. Refocus the shell by its title bar and sample again: no stuck key.
"""
import ctypes, subprocess, sys, time

display, mode, live, origin = sys.argv[1:5]
x = ctypes.cdll.LoadLibrary('libX11.so.6'); xt = ctypes.cdll.LoadLibrary('libXtst.so.6')
x.XOpenDisplay.restype = ctypes.c_void_p
x.XStringToKeysym.restype = ctypes.c_ulong
d = ctypes.c_void_p(x.XOpenDisplay(display.encode()))


def flush():
    x.XFlush(d); time.sleep(0.05)


def move(px, py):
    xt.XTestFakeMotionEvent(d, -1, px, py, 0); flush()


def click(button=1):
    xt.XTestFakeButtonEvent(d, button, True, 0); flush(); xt.XTestFakeButtonEvent(d, button, False, 0); flush()


def key(name, down):
    code = x.XKeysymToKeycode(d, x.XStringToKeysym(name.encode()))
    xt.XTestFakeKeyEvent(d, code, down, 0); flush()


def position():
    out = subprocess.run([live, '--origin', origin, '--command', 'loading-bay.readout'],
                         capture_output=True, text=True).stdout
    fields = dict(part.split('=', 1) for part in out.strip().split(';') if '=' in part)
    return tuple(round(float(v), 2) for v in fields['position'].split(','))


def active():
    out = subprocess.run(['xprop', '-display', display, '-root', '_NET_ACTIVE_WINDOW'], capture_output=True, text=True).stdout
    window = out.strip().split()[-1]
    name = subprocess.run(['xprop', '-display', display, '-id', window, 'WM_NAME'], capture_output=True, text=True).stdout
    return name.strip().split('=', 1)[-1].strip()


def sample(label, seconds=0.6):
    before = position(); time.sleep(seconds); after = position()
    print(f'{label}: active={active()} {before} -> {after} moved={before != after}')
    return before != after


move(640, 450); click(); time.sleep(0.4)
if mode == 'unlocked':
    key('Escape', True); key('Escape', False); time.sleep(0.4)
key('w', True)
walking = sample('holding W')
term = subprocess.Popen(['xterm', '-display', display, '-geometry', '40x10+900+600', '-title', 'focus-thief'])
time.sleep(1.5)
key('w', False)          # released while another window has focus
time.sleep(0.3)
stopped = not sample('after focus loss', 1.0)
term.terminate(); term.wait(); time.sleep(0.8)
move(640, 15); click(); time.sleep(0.6)   # the title bar gives focus back without a canvas click
resumed_still = not sample('after refocus', 1.0)
print(f'RESULT mode={mode} walking={walking} stopped_after_focus_loss={stopped} still_after_refocus={resumed_still}')
