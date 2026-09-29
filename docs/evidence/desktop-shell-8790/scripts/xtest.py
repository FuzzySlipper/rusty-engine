"""Drive the X11 display with XTest: click the canvas, move while locked, keys, menu click."""
import ctypes, sys, time
x = ctypes.cdll.LoadLibrary('libX11.so.6'); xt = ctypes.cdll.LoadLibrary('libXtst.so.6')
x.XOpenDisplay.restype = ctypes.c_void_p
x.XStringToKeysym.restype = ctypes.c_ulong
d = ctypes.c_void_p(x.XOpenDisplay(sys.argv[1].encode()))
def flush(): x.XFlush(d); time.sleep(0.05)
def move(px, py):
    xt.XTestFakeMotionEvent(d, -1, px - 3, py - 3, 0); flush()
    xt.XTestFakeMotionEvent(d, -1, px, py, 0); flush()
def rel(dx, dy): xt.XTestFakeRelativeMotionEvent(d, dx, dy, 0); flush()
def click(b=1): xt.XTestFakeButtonEvent(d, b, True, 0); flush(); xt.XTestFakeButtonEvent(d, b, False, 0); flush()
def key(name, hold=0.05):
    kc = x.XKeysymToKeycode(d, x.XStringToKeysym(name.encode()))
    xt.XTestFakeKeyEvent(d, kc, True, 0); flush(); time.sleep(hold); xt.XTestFakeKeyEvent(d, kc, False, 0); flush()
step = sys.argv[2] if len(sys.argv) > 2 else 'all'
move(640, 450); time.sleep(0.2); click()   # canvas: pointer lock
time.sleep(0.3)
for _ in range(10): rel(3, -1)          # locked motion (below the X accel threshold)
for k in ['w', 'a', 's', 'd', 'space', 'Control_L', '1']: key(k)
xt.XTestFakeButtonEvent(d, 4, True, 0); flush(); xt.XTestFakeButtonEvent(d, 4, False, 0); flush()  # wheel up
time.sleep(0.2)
key('Escape')                            # ends the lock
time.sleep(0.3)
move(1176, 102); click()                # the Menu button (client origin near 0,58)
print('done')
