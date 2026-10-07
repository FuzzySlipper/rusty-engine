"""Deterministic authored torch particle sprites: an eight-frame flame
flipbook strip, an ember dot and a smoke puff; no runtime image generation."""
import math, struct, zlib
from pathlib import Path

def png(path, width, height, pixel):
    rows=[]
    for y in range(height):
        row=bytearray()
        for x in range(width):
            row.extend(pixel(x,y))
        rows.append(b'\0'+row)
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',width,height,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(rows)))+chunk(b'IEND',b''))

def smooth(t):
    t=min(max(t,0.0),1.0)
    return t*t*(3-2*t)

FRAMES, FRAME = 8, 32
def flame(x,y):
    frame, x = divmod(x, FRAME)
    phase = frame*2*math.pi/FRAMES
    u = (x+0.5)/FRAME-0.5
    v = 1-(y+0.5)/FRAME
    # The tongue leans and its tip breathes with the frame.
    u -= math.sin(phase+v*5)*0.07*v
    tip = 0.82+0.14*math.cos(phase*2)
    v /= tip
    if v>=1: return (0,0,0,0)
    radius = 0.34*math.sqrt(1-v)*(0.35+0.65*math.sqrt(v*2.2 if v<0.45 else 1))
    core = 1-(abs(u)/max(radius,1e-3))**2
    if core<=0: return (0,0,0,0)
    alpha = smooth(core*1.3)
    hot = smooth((core-0.35)/0.65)*(1-smooth((v-0.45)/0.5))
    r = 255
    g = int(60+180*hot)
    b = int(10+170*hot*hot)
    return (r,g,b,int(255*alpha))

EMBER=16
def ember(x,y):
    d = math.hypot((x+0.5)/EMBER-0.5,(y+0.5)/EMBER-0.5)*2
    glow = max(0.0,1-d*d)
    return (255,int(170+70*glow),int(80+120*glow*glow),int(255*glow*glow))

SMOKE=64
def smoke(x,y):
    u,v = (x+0.5)/SMOKE-0.5,(y+0.5)/SMOKE-0.5
    d = math.hypot(u,v)*2
    lumps = 0.78+0.11*math.cos(u*17+v*5)+0.11*math.sin(v*13-u*7)
    alpha = max(0.0,1-d*d)*lumps
    return (210,205,200,int(255*smooth(alpha)))

content = Path(__file__).parent/'content'
png(content/'flame.png', FRAMES*FRAME, FRAME, flame)
png(content/'ember.png', EMBER, EMBER, ember)
png(content/'smoke.png', SMOKE, SMOKE, smoke)
