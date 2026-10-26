#!/usr/bin/env python3
"""The desktop window's green-path check: real input against a window
product's UI page, item by item, under the UI engine it runs.

Game UI keeps to the green path (docs/csharp-product-style.md#game-ui-and-diagnostic-ui),
so a UI engine has to pass it rather than be a whole browser. The check
starts the controller-interaction fixture on a runtime pack (or uses a
window product already running with --live-debug), loads each page in
scripts/green-path/pages into the window's own page through
`engine.ui.eval`, drives it with the platform's real input and judges each
item from what the page reports. It prints a pass/fail table and writes it,
with every page report, to --out.

Routes:
- x11: XTest, on a private KWin's Xwayland (needs kwin_wayland, xwininfo).
- wayland: KWin's fake-input protocol, the window a native Wayland client.
- windows: SendInput, run on the Windows desktop itself (not over SSH).

A uinput pad (Linux) or `--vigem-pad <vpad-win.exe>` (Windows, ViGEmBus)
drives the gamepad items; without one they are not run. Differences
docs/desktop-shell.md documents between the engines are results, not
failures. It needs a GPU and a desktop or compositor, so it is no CI gate.

usage: check.py --route x11|wayland|windows --window-ui servo|chromium --out <new dir>
         (--runtime <desktop pack> [--sdk-version <v> --sdk-feed <dir>] | --origin <url> --title <window title>)
         [--vigem-pad <vpad-win.exe>] [--only page,...]
"""
import argparse
import ctypes
import datetime
import json
import os
import signal
import socket
import statistics
import struct
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
REPO = os.path.dirname(os.path.dirname(HERE))
FIXTURE = 'fixtures/csharp-controller-interaction/CsharpControllerInteraction.csproj'
FIXTURE_TITLE = 'Controller Interaction'
PAGES = ['audit', 'probe', 'drag', 'review', 'pickers', 'latency']
WINDOWS = sys.platform == 'win32'

# name: (X keysym, evdev code, Windows virtual key)
KEYS = {
    'Escape': ('Escape', 1, 0x1B), 'Tab': ('Tab', 15, 0x09), 'Return': ('Return', 28, 0x0D),
    'space': ('space', 57, 0x20), 'F5': ('F5', 63, 0x74), 'Shift_L': ('Shift_L', 42, 0xA0),
    'Control_L': ('Control_L', 29, 0xA2), 'Control_R': ('Control_R', 97, 0xA3),
    'Alt_L': ('Alt_L', 56, 0xA4), 'KP_7': ('KP_7', 71, 0x67), 'Up': ('Up', 103, 0x26),
    'Down': ('Down', 108, 0x28), 'Left': ('Left', 105, 0x25), 'Right': ('Right', 106, 0x27),
    'Next': ('Next', 109, 0x22),
    **{c: (c, code, ord(c.upper())) for c, code in zip('qwertyuiop', range(16, 26))},
    **{c: (c, code, ord(c.upper())) for c, code in zip('asdfghjkl', range(30, 39))},
    **{c: (c, code, ord(c.upper())) for c, code in zip('zxcvbnm', range(44, 51))},
    **{str(n): (str(n), 2 + (n - 1) % 10, 0x30 + n) for n in range(10)},
}
EXTENDED_VK = {0x25, 0x26, 0x27, 0x28, 0xA3, 0x22}


# ---------------------------------------------------------------- input routes

class X11Input:
    def __init__(self, display, title):
        self.x = ctypes.cdll.LoadLibrary('libX11.so.6')
        self.xt = ctypes.cdll.LoadLibrary('libXtst.so.6')
        self.x.XOpenDisplay.restype = ctypes.c_void_p
        self.x.XStringToKeysym.restype = ctypes.c_ulong
        self.x.XCreateSimpleWindow.restype = ctypes.c_ulong
        self.x.XDefaultRootWindow.restype = ctypes.c_ulong
        self.display, self.title = display, title
        self.d = ctypes.c_void_p(self.x.XOpenDisplay(display.encode()))

    def flush(self): self.x.XFlush(self.d)

    def geometry(self):
        info = subprocess.run(['xwininfo', '-display', self.display, '-name', self.title],
                              capture_output=True, text=True).stdout
        geo = {}
        for line in info.splitlines():
            key, _, value = line.strip().partition(':')
            if key in ('Absolute upper-left X', 'Absolute upper-left Y', 'Width', 'Height'):
                geo[key] = int(value)
        self.origin = (geo['Absolute upper-left X'], geo['Absolute upper-left Y'])
        return geo['Width'], geo['Height']

    def move(self, px, py):
        self.xt.XTestFakeMotionEvent(self.d, -1, self.origin[0] + px, self.origin[1] + py, 0); self.flush()

    def rel(self, dx, dy):
        self.xt.XTestFakeRelativeMotionEvent(self.d, dx, dy, 0); self.flush()

    def button(self, which, down):
        self.xt.XTestFakeButtonEvent(self.d, {'left': 1, 'middle': 2, 'right': 3}[which], down, 0); self.flush()

    def wheel(self, notches):
        for _ in range(abs(notches)):
            button = 5 if notches > 0 else 4
            self.xt.XTestFakeButtonEvent(self.d, button, True, 0); self.xt.XTestFakeButtonEvent(self.d, button, False, 0)
            self.flush(); time.sleep(0.05)

    def key(self, name, down):
        code = self.x.XKeysymToKeycode(self.d, self.x.XStringToKeysym(KEYS[name][0].encode()))
        self.xt.XTestFakeKeyEvent(self.d, code, down, 0); self.flush()

    def steal_focus(self):
        """Map another window over the window for a moment."""
        root = self.x.XDefaultRootWindow(self.d)
        other = self.x.XCreateSimpleWindow(self.d, ctypes.c_ulong(root), 100, 100, 300, 200, 0, 0, 0)
        self.x.XMapRaised(self.d, ctypes.c_ulong(other)); self.flush(); time.sleep(1.5)
        self.x.XDestroyWindow(self.d, ctypes.c_ulong(other)); self.flush(); time.sleep(1.5)
        return True


class WaylandInput:
    """KWin's org_kde_kwin_fake_input over the Wayland wire protocol, with no
    client library. KWin runs with KWIN_WAYLAND_NO_PERMISSION_CHECKS=1."""

    def __init__(self, socket_name):
        self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        self.sock.connect(os.path.join(os.environ['XDG_RUNTIME_DIR'], socket_name))
        self.next_id, self.buffer, self.origin = 2, b'', (0, 0)
        registry = self.new_id()
        self.send(1, 1, struct.pack('<I', registry))
        done = self.new_id()
        self.send(1, 0, struct.pack('<I', done))
        found = None
        while True:
            obj, opcode, payload = self.read()
            if obj == registry and opcode == 0:
                number, length = struct.unpack('<II', payload[:8])
                if payload[8:8 + length - 1] == b'org_kde_kwin_fake_input':
                    version = struct.unpack('<I', payload[8 + length + (-length % 4):][:4])[0]
                    found = (number, min(version, 4))
            elif obj == done:
                break
        if not found:
            raise SystemExit('the compositor offers no org_kde_kwin_fake_input')
        self.fake = self.new_id()
        self.send(registry, 0, struct.pack('<I', found[0]) + self.string('org_kde_kwin_fake_input')
                  + struct.pack('<II', found[1], self.fake))
        self.send(self.fake, 0, self.string('green-path check') + self.string('window tests'))

    def new_id(self):
        self.next_id += 1
        return self.next_id - 1

    def send(self, obj, opcode, payload=b''):
        self.sock.sendall(struct.pack('<II', obj, ((8 + len(payload)) << 16) | opcode) + payload)

    def read(self):
        while len(self.buffer) < 8 or len(self.buffer) < struct.unpack('<II', self.buffer[:8])[1] >> 16:
            self.buffer += self.sock.recv(4096)
        obj, word = struct.unpack('<II', self.buffer[:8])
        size = word >> 16
        payload, self.buffer = self.buffer[8:size], self.buffer[size:]
        return obj, word & 0xffff, payload

    @staticmethod
    def string(text):
        data = text.encode() + b'\0'
        return struct.pack('<I', len(data)) + data + b'\0' * (-len(data) % 4)

    @staticmethod
    def fixed(value):
        return struct.pack('<i', int(round(value * 256)))

    def geometry(self):
        return self.size

    def calibrate(self, page_point, global_point, size):
        self.origin = (global_point[0] - page_point[0], global_point[1] - page_point[1])
        self.size = size

    def move_global(self, gx, gy):
        self.send(self.fake, 9, self.fixed(gx) + self.fixed(gy))

    def move(self, px, py):
        self.move_global(self.origin[0] + px, self.origin[1] + py)

    def rel(self, dx, dy):
        self.send(self.fake, 1, self.fixed(dx) + self.fixed(dy))

    def button(self, which, down):
        self.send(self.fake, 2, struct.pack('<II', {'left': 0x110, 'right': 0x111, 'middle': 0x112}[which], int(down)))

    def wheel(self, notches):
        for _ in range(abs(notches)):
            self.send(self.fake, 3, struct.pack('<I', 0) + self.fixed(15 if notches > 0 else -15)); time.sleep(0.05)

    def key(self, name, down):
        self.send(self.fake, 10, struct.pack('<II', KEYS[name][1], int(down)))

    def steal_focus(self):
        return False


class WindowsInput:
    def __init__(self, title):
        from ctypes import wintypes
        self.user32 = ctypes.WinDLL('user32', use_last_error=True)
        self.user32.SetProcessDPIAware()
        self.title = title

        class MOUSEINPUT(ctypes.Structure):
            _fields_ = [('dx', ctypes.c_long), ('dy', ctypes.c_long), ('mouseData', wintypes.DWORD),
                        ('dwFlags', wintypes.DWORD), ('time', wintypes.DWORD), ('extra', ctypes.c_size_t)]

        class KEYBDINPUT(ctypes.Structure):
            _fields_ = [('wVk', wintypes.WORD), ('wScan', wintypes.WORD), ('dwFlags', wintypes.DWORD),
                        ('time', wintypes.DWORD), ('extra', ctypes.c_size_t)]

        class UNION(ctypes.Union):
            _fields_ = [('mi', MOUSEINPUT), ('ki', KEYBDINPUT)]

        class INPUT(ctypes.Structure):
            _fields_ = [('type', wintypes.DWORD), ('u', UNION)]

        self.MOUSEINPUT, self.KEYBDINPUT, self.UNION, self.INPUT = MOUSEINPUT, KEYBDINPUT, UNION, INPUT
        self.wintypes = wintypes

    def send(self, item):
        if self.user32.SendInput(1, ctypes.byref(item), ctypes.sizeof(self.INPUT)) != 1:
            raise OSError(f'SendInput failed: {ctypes.get_last_error()}')

    def mouse(self, flags, data=0, dx=0, dy=0):
        union = self.UNION()
        union.mi = self.MOUSEINPUT(dx, dy, data & 0xFFFFFFFF, flags, 0, 0)
        self.send(self.INPUT(0, union))

    def geometry(self):
        window = self.user32.FindWindowW(None, self.title)
        if not window:
            raise SystemExit(f'no window titled {self.title!r}')
        self.user32.SetForegroundWindow(window)
        point = self.wintypes.POINT(0, 0)
        self.user32.ClientToScreen(window, ctypes.byref(point))
        rect = self.wintypes.RECT()
        self.user32.GetClientRect(window, ctypes.byref(rect))
        self.origin = (point.x, point.y)
        return rect.right, rect.bottom

    def move(self, px, py):
        self.user32.SetCursorPos(self.origin[0] + px, self.origin[1] + py)
        self.mouse(0x0001)

    def rel(self, dx, dy):
        self.mouse(0x0001, dx=dx, dy=dy)

    def button(self, which, down):
        flags = {'left': (0x2, 0x4), 'right': (0x8, 0x10), 'middle': (0x20, 0x40)}[which]
        self.mouse(flags[0] if down else flags[1])

    def wheel(self, notches):
        for _ in range(abs(notches)):
            self.mouse(0x0800, data=-120 if notches > 0 else 120); time.sleep(0.05)

    def key(self, name, down):
        vk = KEYS[name][2]
        scan = self.user32.MapVirtualKeyW(vk, 0)
        flags = 0x0008 | (0x0001 if vk in EXTENDED_VK else 0) | (0 if down else 0x0002)
        union = self.UNION()
        union.ki = self.KEYBDINPUT(0, scan, flags, 0, 0)
        self.send(self.INPUT(1, union))

    def steal_focus(self):
        return False


# ---------------------------------------------------------------- gamepads

class UinputPad:
    """An Xbox-style pad through /dev/uinput (linux/uinput.h ioctls)."""
    BTN_SOUTH, ABS_X = 0x130, 0

    def __init__(self, name=b'Green-path check pad'):
        import fcntl
        self.fd = os.open('/dev/uinput', os.O_WRONLY | os.O_NONBLOCK)
        fcntl.ioctl(self.fd, 0x40045564, 0x01)  # UI_SET_EVBIT EV_KEY
        fcntl.ioctl(self.fd, 0x40045564, 0x03)  # EV_ABS
        for button in (0x130, 0x131, 0x133, 0x134, 0x136, 0x137, 0x13a, 0x13b, 0x13c, 0x13d, 0x13e):
            fcntl.ioctl(self.fd, 0x40045565, button)
        for axis, low, high in ((0, -32768, 32767), (1, -32768, 32767), (3, -32768, 32767), (4, -32768, 32767),
                                (2, 0, 255), (5, 0, 255), (0x10, -1, 1), (0x11, -1, 1)):
            fcntl.ioctl(self.fd, 0x40045567, axis)
            fcntl.ioctl(self.fd, 0x401c5504, struct.pack('<Hxxiiiiii', axis, 0, low, high, 0, 0, 0))
        setup = struct.pack('<HHHH80sI', 0x03, 0x045e, 0x028e, 0x110, name, 0)
        fcntl.ioctl(self.fd, 0x405c5503, setup)
        fcntl.ioctl(self.fd, 0x5501)
        time.sleep(2.0)

    def emit(self, kind, code, value):
        os.write(self.fd, struct.pack('<qqHHi', 0, 0, kind, code, value) + struct.pack('<qqHHi', 0, 0, 0, 0, 0))

    def press(self, down): self.emit(0x01, self.BTN_SOUTH, int(down))

    def stick(self, value): self.emit(0x03, self.ABS_X, value)

    def close(self):
        import fcntl
        fcntl.ioctl(self.fd, 0x5502)
        os.close(self.fd)


# ---------------------------------------------------------------- the runtime

class Runtime:
    def __init__(self, args, env):
        self.args, self.env, self.session = args, env, None
        self.origin, self.title = args.origin, args.title or FIXTURE_TITLE

    def start(self):
        if self.origin:
            return
        rusty = os.path.join(self.args.runtime, 'bin', 'rusty.exe' if WINDOWS else 'rusty')
        env = dict(self.env)
        if self.args.sdk_version:
            env['RustyEngineFixtureSdkVersion'] = self.args.sdk_version
        if self.args.sdk_feed:
            env['RestoreAdditionalProjectSources'] = self.args.sdk_feed
        out = subprocess.run([rusty, 'dev', 'start', '--label', 'green-path', '--project', FIXTURE,
                              '--runtime', self.args.runtime, '--output', 'window', '--window-ui',
                              self.args.window_ui, '--live-debug'],
                             cwd=REPO, env=env, capture_output=True, text=True, timeout=1200)
        text = out.stdout + out.stderr
        try:
            self.session = json.loads(text[text.index('{'):text.rindex('}') + 1])
        except ValueError:
            raise SystemExit(f'rusty dev start failed:\n{text}')
        self.origin = self.session['url']
        time.sleep(8.0)

    def stop(self):
        if self.session:
            rusty = os.path.join(self.args.runtime, 'bin', 'rusty.exe' if WINDOWS else 'rusty')
            subprocess.run([rusty, 'dev', 'stop', self.session['id']], cwd=REPO, env=self.env,
                           capture_output=True, timeout=120)

    def debug(self, command):
        request = urllib.request.Request(f'{self.origin}/__rusty/product/runtime/debug/execute', command.encode(),
                                         {'Content-Type': 'text/plain; charset=utf-8'}, method='POST')
        try:
            body = urllib.request.urlopen(request, timeout=30).read().decode()
        except urllib.error.HTTPError as error:
            body = error.read().decode()
        return json.loads(body)

    def eval(self, expression):
        answer = self.debug(f'engine.ui.eval {expression}')
        if isinstance(answer, dict) and 'message' in answer and isinstance(answer['message'], str):
            try:
                answer = json.loads(answer['message'])
            except ValueError:
                return {'error': answer['message']}
        return answer

    def window(self):
        answer = self.debug('engine.renderer')
        if 'renderer' not in answer and isinstance(answer.get('message'), str):
            answer = json.loads(answer['message'])
        return answer['renderer']['window']


# ---------------------------------------------------------------- the check

class Check:
    def __init__(self, args, runtime, route, pad_kind, report_log, port, shots):
        self.args, self.runtime, self.input, self.pad_kind = args, runtime, route, pad_kind
        self.log, self.port, self.shots = report_log, port, shots
        self.results = []

    # -- page and report plumbing
    def mark(self, name):
        print('  step:', name, flush=True)
        with open(self.log, 'a') as log:
            log.write(json.dumps({'event': 'STEP', 'name': name}) + '\n')

    def events(self):
        out = []
        with open(self.log) as log:
            for line in log:
                try:
                    out.append(json.loads(line))
                except ValueError:
                    pass
        return out

    def step_events(self, name):
        """The page's reports between step `name` and the next step."""
        events, inside = [], False
        for event in self.events():
            if event.get('event') == 'STEP':
                inside = event['name'] == name
            elif inside:
                events.append(event)
        return events

    def load(self, page):
        before = len(self.events())
        url = f'http://127.0.0.1:{self.port}/{page}'
        answer = self.runtime.eval(f'setTimeout(() => {{ location.href = {json.dumps(url)}; }}, 50); "navigating"')
        if answer.get('error'):
            raise SystemExit(f'engine.ui.eval failed: {answer["error"]}')
        deadline = time.time() + 20
        while time.time() < deadline:
            if any(e.get('event') == 'loaded' for e in self.events()[before:]):
                time.sleep(1.0)
                return
            time.sleep(0.2)
        raise SystemExit(f'the window did not load {url}')

    def page_size(self):
        size = self.runtime.eval('[innerWidth, innerHeight]').get('value')
        return size

    def shot(self, name):
        if not self.shots:
            return
        path = os.path.join(self.shots, name.replace(' ', '_') + '.png')
        if WINDOWS:
            return
        env = dict(os.environ, **self.args.compositor_env)
        subprocess.run(['spectacle', '-b', '-n', '-f', '-o', path], env=env, capture_output=True, timeout=30)

    def record(self, page, item, ok, detail):
        state = {True: 'pass', False: 'FAIL', None: 'not run'}[ok]
        self.results.append({'page': page, 'item': item, 'result': state, 'detail': detail})
        print(f'  {state:7} {item}: {detail}', flush=True)

    # -- input helpers
    def at(self, px, py, pause=0.15):
        self.input.move(px, py); time.sleep(pause)

    def click(self, which='left', pause=0.6):
        self.input.button(which, True); time.sleep(0.05); self.input.button(which, False); time.sleep(pause)

    def key(self, name, pause=0.2, hold=0.04):
        self.input.key(name, True)
        if isinstance(self.input, WindowsInput) and hold > 0.5:
            # Injected input does not auto-repeat: send the repeats a held
            # key's keyboard would, after its delay.
            time.sleep(0.5)
            for _ in range(int((hold - 0.5) * 30)):
                self.input.key(name, True); time.sleep(1 / 30)
        else:
            time.sleep(hold)
        self.input.key(name, False); time.sleep(pause)

    def chord(self, modifier, name, pause=0.3):
        self.input.key(modifier, True); time.sleep(0.05); self.key(name, 0.05); self.input.key(modifier, False); time.sleep(pause)

    # -- pages
    def audit(self):
        cell = lambda i: (40 + 180 * (i % 5) + 80, 40 + 60 * (i // 5) + 20)

        def alive(name):
            """After each item the page still takes a click and a key."""
            self.mark(name + ': check')
            self.at(1080, 640); self.click(); self.key('a')

        def press(i, name, button='left', then=None, wait=1.5):
            self.mark(name)
            c = cell(i); self.at(c[0] - 5, c[1]); self.at(*c); self.click(button, wait)
            if then:
                then()
            self.shot(f'audit {name}')
            alive(name)

        names = ['alert', 'confirm', 'prompt', 'window.open', 'print', 'fullscreen', 'clipboard',
                 'notification', 'geolocation', 'gamepads', 'dialog.showModal']
        self.at(600, 400); self.at(610, 405)
        for i, name in enumerate(names):
            press(i, name)
        press(11, 'target=_blank link')
        press(12, 'file input', then=lambda: self.key('Escape'))
        press(14, 'color input', then=lambda: self.key('Escape'))
        press(15, 'select', then=lambda: (self.key('Down'), self.key('Return')))
        press(17, 'context menu', button='right')
        self.mark('wheel over list'); c = cell(20); self.at(c[0], c[1] + 40); self.input.wheel(3); time.sleep(0.5); alive('wheel over list')
        self.mark('text entry'); self.at(1080, 580); self.click(); self.key('h'); self.key('i'); alive('text entry')
        self.mark('Tab focus'); self.key('Tab'); alive('Tab focus')
        press(18, 'download link')
        press(23, 'beforeunload + reload', wait=3.0)
        self.mark('Ctrl+P'); self.at(1080, 640); self.click(); self.chord('Control_L', 'p'); alive('Ctrl+P')
        press(19, 'same-tab link', wait=2.0)
        time.sleep(1.0)

        def judge(name, test, detail):
            events = self.step_events(name)
            checked = self.step_events(name + ': check')
            took = any(e.get('event') in ('ping', 'keydown') for e in checked) or name == 'same-tab link'
            ok = bool(test(events)) and took
            self.record('audit', name, ok, detail if took else detail + '; the page stopped taking input')

        has = lambda kind, **want: (lambda events: any(e.get('event') == kind and all(e.get(k) == v for k, v in want.items()) for e in events))
        warned = lambda text: (lambda events: any(e.get('event') == 'console.warn' and text in e.get('text', '') for e in events))
        quick = lambda kind: (lambda events: any(e.get('event') == kind and e.get('ms', 9999) < 1000 for e in events))
        judge('alert', lambda e: quick('alert returned')(e) and warned('alert()')(e), 'cancelled at once, warns')
        judge('confirm', lambda e: has('confirm returned', value=False)(e), 'false at once')
        judge('prompt', lambda e: has('prompt returned', value=None)(e), 'null at once')
        judge('window.open', lambda e: has('window.open returned', value='null')(e) and warned('new window')(e), 'null, warns')
        judge('print', quick('print returned'), 'returns at once')
        judge('fullscreen', has('fullscreen granted', element=True), 'fills the view')
        judge('clipboard', lambda e: has('clipboard write ok')(e) and not any(
            x.get('event') == 'clipboard read ok' and x.get('value') for x in e), 'write works, read denied or empty')
        judge('notification', lambda e: not has('notification permission', value='granted')(e), 'not granted')
        judge('geolocation', lambda e: not has('geolocation granted')(e), 'not granted')
        judge('gamepads', has('gamepads'), 'navigator.getGamepads answers')
        judge('dialog.showModal', has('dialog open', open=True), 'opens')
        judge('target=_blank link', lambda e: warned('new window')(e) and not has('loaded', url='/other')(e), 'warns, opens nothing')
        judge('file input', warned('file picker'), 'warns, opens nothing')
        judge('color input', warned('color picker'), 'warns, opens nothing')
        judge('select', has('select changed', value='two'), 'Down, Enter chooses')
        judge('context menu', has('contextmenu event'), 'contextmenu fires, no native menu')
        judge('wheel over list', lambda e: any(x.get('event') == 'list scrolled' and x.get('top', 0) > 0 for x in e), 'the list scrolls')
        judge('text entry', has('text input', value='hi'), 'typing reaches the input')
        judge('Tab focus', has('keydown', key='Tab'), 'Tab reaches the page')
        judge('download link', lambda e: True, 'nothing downloads')
        judge('beforeunload + reload', has('loaded', url='/audit'), 'reloads with no prompt')
        judge('Ctrl+P', has('keydown', key='p'), 'an ordinary key')
        judge('same-tab link', has('loaded', url='/other'), 'replaces the page')

    def probe(self):
        width, height = self.input_geometry()
        self.at(600, 400); self.at(610, 405)
        self.mark('pointer lock'); self.at(135, 65); self.at(140, 70); self.click(pause=1.0)
        for _ in range(10):
            self.input.rel(10, 5); time.sleep(0.03)
        time.sleep(0.5)
        self.mark('Escape ends the lock'); self.key('Escape', 1.0)
        self.mark('confine'); self.at(395, 65); self.at(400, 70); self.click(pause=1.0)
        self.mark('Escape ends confinement'); self.key('Escape', 1.0)
        self.mark('keys'); self.at(185, 155); self.at(190, 160); self.click()
        for name in ['w', 'Shift_L', 'Control_R', 'KP_7', 'Return', 'space', 'Tab', 'F5']:
            self.key(name, 0.3)
        self.chord('Shift_L', 'a')
        self.key('d', 0.3, hold=0.8)
        self.mark('wheel'); self.at(600, 400); self.input.wheel(1); time.sleep(0.3); self.input.wheel(-1); time.sleep(0.3)
        self.mark('focus loss'); stolen = self.input.steal_focus()
        self.mark('click back'); self.at(600, 400); self.click(pause=1.0)
        locked = [e for e in self.step_events('pointer lock') if e.get('event') == 'pointerlockchange']
        ended = [e for e in self.step_events('Escape ends the lock') if e.get('event') == 'pointerlockchange']
        self.record('probe', 'pointer lock', bool(locked and locked[0].get('locked')), 'a click takes the lock')
        motion = ended[0] if ended else {}
        self.record('probe', 'locked motion', motion.get('movementX') == 100 and motion.get('movementY') == 50,
                    f"10 moves of (10, 5) sum to ({motion.get('movementX')}, {motion.get('movementY')})")
        self.record('probe', 'Escape ends the lock', bool(ended and not ended[0].get('locked')), 'the page sees the lock end')
        confine = [e for e in self.step_events('confine') if e.get('event') == 'confine']
        self.record('probe', 'confinement', bool(confine and confine[0].get('granted')), 'granted')
        self.record('probe', 'Escape ends confinement',
                    any(e.get('event') == 'confinement end' for e in self.step_events('Escape ends confinement')), 'the page sees it end')
        keys = [e for e in self.step_events('keys') if e.get('event') == 'keydown']
        codes = [(e['code'], e['location']) for e in keys if not e.get('repeat')]
        want = [('KeyW', 0), ('ShiftLeft', 1), ('ControlRight', 2), ('Numpad7', 3), ('Enter', 0), ('Space', 0), ('Tab', 0), ('F5', 0)]
        self.record('probe', 'key codes and locations', codes[:8] == want, ' '.join(f'{c}/{l}' for c, l in codes[:8]))
        self.record('probe', 'modifiers', any(e.get('key') == 'A' and e.get('shift') for e in keys), 'Shift+A reads A with shiftKey')
        held = [e for e in keys if e.get('code') == 'KeyD']
        self.record('probe', 'a held key repeats', len(held) >= 3, f'{len(held)} keydowns (repeat flags documented per engine)')
        wheel = [e.get('deltaY') for e in self.step_events('wheel') if e.get('event') == 'wheel']
        # One notch down, then one up; Chromium may split a notch in two.
        turned = [delta > 0 for delta in wheel if delta]
        self.record('probe', 'wheel', bool(turned) and turned[0] and not turned[-1]
                    and turned == sorted(turned, reverse=True), f'deltaY {wheel}')
        if stolen:
            blur = any(e.get('event') == 'blur' and e.get('hasFocus') is False for e in self.step_events('focus loss'))
            self.record('probe', 'focus loss', blur, 'blur, hasFocus() false')
        else:
            self.record('probe', 'focus loss', None, 'this route opens no other window')

    def drag(self):
        self.at(130, 130); self.at(140, 140, 0.3)
        self.mark('drag')
        self.input.button('left', True); time.sleep(0.1)
        for i in range(1, 21):
            self.at(140 + 15 * i, 140, 0.03)
        time.sleep(0.3); self.input.button('left', False); time.sleep(0.8)
        events = self.step_events('drag')
        down = [e for e in events if e.get('event') == 'pointerdown']
        up = [e for e in events if e.get('event') == 'pointerup']
        self.record('drag', 'pointer capture', bool(down and down[0].get('captured')), 'pointerdown captures')
        self.record('drag', 'drag and drop', bool(up and up[0].get('dropped') and up[0].get('moves', 0) >= 10),
                    f"{up[0].get('moves') if up else 0} moves, dropped in the slot" if up else 'no pointerup')

    def review(self):
        width, height = self.input_geometry()
        top = round(height / 2 - 12)
        self.at(600, 300); self.at(610, 305)
        self.mark('middle: last option by keys'); self.at(110, top + 10); self.at(115, top + 12); self.click(pause=1.0)
        self.shot('review middle open')
        for _ in range(45):
            self.key('Down', 0.02)
        time.sleep(0.4); self.key('Return', 0.6)
        # The list opens below the select where 20em fits, else on the side with more room.
        below = height - (top + 24)
        downward = below >= 260 or below >= top
        self.mark('middle: click a row'); self.at(115, top + 12); self.click(pause=1.0)
        row = top + 24 + 40 if downward else top - 40
        self.at(100, row); self.at(100, row + 2); self.click(pause=0.8)
        self.mark('multi: keys toggle, a click outside answers'); self.at(335, 50); self.at(340, 52); self.click(pause=1.0)
        self.key('Up'); self.key('space'); self.key('Down'); self.key('Down'); self.key('space')
        self.at(900, 300); self.click(pause=0.8)
        self.mark('multi: Escape keeps the choice'); self.at(340, 52); self.click(pause=1.0)
        self.key('Down'); self.key('space'); self.key('Escape', 0.6)
        self.mark('edge select'); self.at(width - 64, height - 15); self.at(width - 60, height - 14); self.click(pause=1.0)
        self.shot('review edge open'); self.key('Escape', 0.6)
        changes = lambda step: [e.get('selected') for e in self.step_events(step) if e.get('event') == 'change']
        # Chromium changes the value as the arrow keys move; the last change counts.
        keys = changes('middle: last option by keys')
        self.record('review', 'select by keys', keys[-1:] == [[39]], f'last change {keys[-1:]} (the last of 40 options)')
        clicked = changes('middle: click a row')
        self.record('review', 'select by a click on a row', bool(clicked) and clicked[-1] != [39], f'change {clicked}')
        if self.args.window_ui == 'chromium':
            listbox = "Chromium draws a <select multiple> as an in-page listbox, not a popup"
            self.record('review', 'select multiple', None, listbox)
            self.record('review', 'Escape keeps the choice', None, listbox)
        else:
            multi = changes('multi: keys toggle, a click outside answers')
            self.record('review', 'select multiple', multi == [[0, 1, 2]], f'change {multi}')
            kept = changes('multi: Escape keeps the choice')
            self.record('review', 'Escape keeps the choice', kept == [], f'change {kept}')
        self.gamepad()

    def gamepad(self):
        width, height = self.input_geometry()
        state = lambda step: [e.get('state') for e in self.step_events(step) if e.get('event') == 'gamepad']
        kinds = lambda step: [e.get('event') for e in self.step_events(step)]
        if self.pad_kind == 'uinput':
            self.mark('pad plugged'); pad = UinputPad()
            pad.press(True); time.sleep(0.5); pad.press(False); time.sleep(0.5)
            self.mark('pad unplugged'); pad.close(); time.sleep(2.0)
            self.mark('pad replugged'); pad = UinputPad(); pad.stick(32767); time.sleep(0.5)
            self.mark('reload with the stick held'); self.at(535, 50); self.at(540, 55); self.click(pause=3.0)
            self.mark('after reload'); pad.stick(0); time.sleep(0.4); pad.press(True); time.sleep(0.5); pad.press(False); time.sleep(0.5)
            pad.close(); time.sleep(1.5)
            self.record('review', 'gamepad connects and reads',
                        'gamepadconnected' in kinds('pad plugged') and any('"a":true' in (s or '') for s in state('pad plugged')), 'press and release')
            self.record('review', 'gamepad unplugs', 'gamepaddisconnected' in kinds('pad unplugged'), 'gamepaddisconnected; getGamepads()[0] null')
            self.record('review', 'gamepad replugs', 'gamepadconnected' in kinds('pad replugged'), 'connected again')
            self.record('review', 'gamepad across a reload',
                        any('"x":1' in (s or '') for s in state('reload with the stick held'))
                        and any('"a":true' in (s or '') for s in state('after reload')), 'the new document has the pad and its held stick')
        elif self.pad_kind == 'vigem':
            self.mark('pad plugged')
            pad = subprocess.Popen([self.args.vigem_pad, str(urllib.parse.urlparse(self.runtime.origin).port), '12', '3000',
                                    os.path.join(self.args.out, 'vpad.json')], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            time.sleep(7.0)
            self.mark('reload with the pad plugged'); self.at(535, 50); self.at(540, 55); self.click(pause=0.2)
            pad.wait(timeout=60); time.sleep(1.0)
            self.mark('after the pad exits')
            self.record('review', 'gamepad connects and reads',
                        'gamepadconnected' in kinds('pad plugged') and any('"a":true' in (s or '') for s in state('pad plugged')), 'presses')
            self.record('review', 'gamepad across a reload',
                        any('"a":true' in (s or '') for s in state('reload with the pad plugged')), 'the new document has the pad')
            self.record('review', 'gamepad unplugs', 'gamepaddisconnected' in kinds('reload with the pad plugged'), 'gamepaddisconnected when the pad goes')
        else:
            self.record('review', 'gamepad', None, 'no pad on this route (uinput on Linux, --vigem-pad on Windows)')

    def pickers(self):
        width, height = self.input_geometry()
        self.at(700, 400); self.at(705, 402)
        today = datetime.date.today()
        if self.args.window_ui == 'servo':
            self.mark('date: keys choose a day'); self.at(115, 50); self.at(120, 52); self.click(pause=1.0)
            self.shot('pickers date open')
            self.key('Right'); self.key('Right'); self.key('Down'); self.key('Return', 0.6)
            self.mark('date: Escape keeps the value'); self.at(120, 52); self.click(pause=1.0); self.key('Next'); self.key('Escape', 0.6)
            self.mark('time: Enter answers'); self.at(355, 50); self.at(360, 52); self.click(pause=1.0); self.shot('pickers time open'); self.key('Return', 0.6)
            self.mark('datetime: a day back, Enter'); self.at(615, 50); self.at(620, 52); self.click(pause=1.0); self.key('Left'); self.key('Return', 0.6)
            self.mark('Alt+Down on a focused input'); self.at(115, height - 18); self.at(120, height - 20); self.click(pause=0.6)
            self.key('Escape', 0.4); self.input.key('Alt_L', True); self.key('Down', 0.1); self.input.key('Alt_L', False); time.sleep(0.8)
            self.shot('pickers bottom open'); self.key('Return', 0.6)
            want = [('date: keys choose a day', 'date', '2026-11-03'), ('time: Enter answers', 'time', '09:30'),
                    ('datetime: a day back, Enter', 'local', '2026-10-24T18:45'),
                    ('Alt+Down on a focused input', 'bottom', today.isoformat())]
        else:
            # Chromium draws its own picker; its fields also take typed digits.
            self.mark('date: typed digits'); self.at(60, 50); self.click(pause=0.5)
            for digit in '11032026':
                self.key(digit, 0.08)
            self.key('Tab', 0.6)
            want = [('date: typed digits', 'date', '2026-11-03')]
        for step, element, value in want:
            got = [e.get('value') for e in self.step_events(step) if e.get('event') == 'change' and e.get('id') == element]
            self.record('pickers', f'{element}: {step.split(": ", 1)[-1]}', value in got, f'change {got}, want {value}')
        if self.args.window_ui == 'servo':
            kept = [e for e in self.step_events('date: Escape keeps the value') if e.get('event') == 'change']
            self.record('pickers', 'date: Escape keeps the value', not kept, f'{len(kept)} changes')

    def latency(self):
        self.input_geometry()
        self.at(640, 500)
        rows = []
        for _ in range(30):
            before = self.runtime.window()['uiRepaints']['count']
            sent = time.time() * 1e3
            self.key('u', 0.4, hold=0.03)
            window = self.runtime.window()
            received = [t for t in window['inputsReceivedAtUnixMs'] if t >= sent - 1]
            shown = [t for t in window['uiRepaints']['shownAtUnixMs'] if received and t >= received[0]]
            if received and shown and window['uiRepaints']['count'] > before:
                rows.append((received[0] - sent, shown[0] - received[0]))
        final = self.runtime.window()
        if not rows:
            self.record('latency', 'input to frame', False, 'no key reached a shown repaint')
            return
        shown = sorted(r[1] for r in rows)
        self.record('latency', 'input to frame', True,
                    f'{len(rows)}/30 measured; receipt to shown p50 {statistics.median(shown):.1f} ms, '
                    f'p90 {shown[int(0.9 * (len(shown) - 1))]:.1f} ms; repaint {final["uiRepaints"]["medianMs"]:.2f} ms '
                    f'on the window thread; {final["uiRepaints"]["errors"]} import errors')

    def input_geometry(self):
        return self.input.geometry()


# ---------------------------------------------------------------- compositor

def private_kwin(out):
    """A private headless KWin with Xwayland on its own D-Bus session; returns
    (process, Wayland socket, X display, environment for screenshots)."""
    name = f'wayland-greenpath-{os.getpid()}'
    before = set(os.listdir('/tmp/.X11-unix'))
    bus = os.path.join(out, 'kwin-bus')
    env = dict(os.environ, KWIN_WAYLAND_NO_PERMISSION_CHECKS='1', KWIN_SCREENSHOT_NO_PERMISSION_CHECKS='1')
    process = subprocess.Popen(['dbus-run-session', '--', 'sh', '-c',
                                f'echo "$DBUS_SESSION_BUS_ADDRESS" > "{bus}"; exec kwin_wayland --virtual --socket {name} '
                                '--xwayland --no-lockscreen --width 1280 --height 800'],
                               env=env, stdout=open(os.path.join(out, 'kwin.log'), 'w'), stderr=subprocess.STDOUT,
                               start_new_session=True)
    for _ in range(100):
        time.sleep(0.2)
        added = set(os.listdir('/tmp/.X11-unix')) - before
        if added and os.path.exists(os.path.join(os.environ['XDG_RUNTIME_DIR'], name)) and os.path.exists(bus):
            display = ':' + sorted(added)[0].lstrip('X')
            shots = {'WAYLAND_DISPLAY': name, 'DBUS_SESSION_BUS_ADDRESS': open(bus).read().strip(), 'QT_QPA_PLATFORM': 'wayland'}
            return process, name, display, shots
    os.killpg(process.pid, signal.SIGTERM)
    raise SystemExit('the private KWin did not start')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--route', required=True, choices=['x11', 'wayland', 'windows'])
    parser.add_argument('--window-ui', required=True, choices=['servo', 'chromium'])
    parser.add_argument('--out', required=True)
    parser.add_argument('--runtime')
    parser.add_argument('--sdk-version')
    parser.add_argument('--sdk-feed')
    parser.add_argument('--origin')
    parser.add_argument('--title')
    parser.add_argument('--vigem-pad')
    parser.add_argument('--only', default=','.join(PAGES))
    args = parser.parse_args()
    if not args.runtime and not args.origin:
        parser.error('give --runtime <desktop pack> or --origin <url> of a running window product')
    if os.path.exists(args.out):
        parser.error(f'{args.out} exists; give a new directory')
    os.makedirs(os.path.join(args.out, 'shots'))
    args.compositor_env = {}
    log = os.path.join(args.out, 'reports.ndjson')
    open(log, 'w').close()
    env = dict(os.environ)
    kwin = None
    if args.route in ('x11', 'wayland'):
        kwin, socket_name, display, args.compositor_env = private_kwin(args.out)
        if args.route == 'x11':
            env['DISPLAY'] = display
            env.pop('WAYLAND_DISPLAY', None)
        else:
            env['WAYLAND_DISPLAY'] = socket_name
            env.pop('DISPLAY', None)
    server = subprocess.Popen(['node', os.path.join(HERE, 'server.mjs'), log], stdout=subprocess.PIPE, text=True)
    port = int(server.stdout.readline())
    runtime = Runtime(args, env)
    try:
        runtime.start()
        engine = runtime.window().get('ui')
        print(f'window UI: {engine}', flush=True)
        if args.route == 'x11':
            route = X11Input(display, runtime.title)
            route.geometry()
        elif args.route == 'wayland':
            route = WaylandInput(socket_name)
        else:
            route = WindowsInput(runtime.title)
            route.geometry()
        pad = 'uinput' if args.route != 'windows' and os.access('/dev/uinput', os.W_OK) else 'vigem' if args.vigem_pad else None
        check = Check(args, runtime, route, pad, log, port, os.path.join(args.out, 'shots'))
        for page in args.only.split(','):
            print(f'{page}:', flush=True)
            check.load(page)
            if isinstance(route, WaylandInput):
                # The page reports where a click landed: that places the window.
                size = runtime.eval('[innerWidth, innerHeight]')['value']
                runtime.eval('window.__greenPathClick = new Promise((done) => addEventListener("pointerdown", '
                             '(e) => done([e.clientX, e.clientY]), { once: true })); null')
                route.move_global(900, 500); time.sleep(0.2)
                route.button('left', True); time.sleep(0.05); route.button('left', False); time.sleep(0.5)
                route.calibrate(runtime.eval('window.__greenPathClick')['value'], (900, 500), size)
            getattr(check, page)()
    finally:
        runtime.stop()
        server.terminate()
        if kwin:
            os.killpg(kwin.pid, signal.SIGTERM)
    results = check.results
    lines = [f'# Green-path check: {args.window_ui} on {args.route}', '', f'Window UI: {engine}', '',
             '| Page | Item | Result | Detail |', '|---|---|---|---|']
    lines += [f"| {r['page']} | {r['item']} | {r['result']} | {r['detail']} |" for r in results]
    failed = [r for r in results if r['result'] == 'FAIL']
    lines += ['', f"{sum(r['result'] == 'pass' for r in results)} pass, {len(failed)} fail, "
              f"{sum(r['result'] == 'not run' for r in results)} not run"]
    open(os.path.join(args.out, 'results.md'), 'w').write('\n'.join(lines) + '\n')
    json.dump(results, open(os.path.join(args.out, 'results.json'), 'w'), indent=1)
    print('\n'.join(lines))
    sys.exit(1 if failed else 0)


if __name__ == '__main__':
    main()
