"""Deterministic foam and ripple textures for the water shore (#9540): foam.png, a tiling mottle of
bright patches over dark (read as the foam mask), and ripples.png, a tiling tangent-space normal map
of crossed low waves. No runtime image generation."""
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

SIZE=128
def waves(u,v,terms):
    return sum(a*math.sin(2*math.pi*(fx*u+fy*v)+p) for a,fx,fy,p in terms)

FOAM=[(1.0,3,1,0.3),(0.7,-2,4,1.9),(0.5,5,3,4.1),(0.4,1,-6,2.6),(0.3,7,2,0.9)]
def foam(x,y):
    u,v=(x+0.5)/SIZE,(y+0.5)/SIZE
    value=waves(u,v,FOAM)/2.9
    level=int(255*min(max(0.5+0.5*value,0.0),1.0)**1.6)
    return (level,level,level,255)

RIPPLE=[(0.5,2,1,0.0),(0.35,-1,3,2.2),(0.2,4,-2,1.1)]
def ripples(x,y):
    u,v=(x+0.5)/SIZE,(y+0.5)/SIZE
    e=1.0/SIZE
    h=lambda a,b: 0.06*waves(a,b,RIPPLE)
    dx=(h(u+e,v)-h(u-e,v))/(2*e)
    dy=(h(u,v+e)-h(u,v-e))/(2*e)
    n=(-dx,-dy,1.0); l=math.sqrt(sum(c*c for c in n)); n=[c/l for c in n]
    return (int(127.5+127.5*n[0]),int(127.5+127.5*n[1]),int(127.5+127.5*n[2]),255)

out=Path(__file__).resolve().parent/'content'
png(out/'foam.png',SIZE,SIZE,foam)
png(out/'ripples.png',SIZE,SIZE,ripples)
print('wrote foam.png and ripples.png')
