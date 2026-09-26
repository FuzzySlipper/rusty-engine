"""Deterministic authored fixture panoramas; no runtime image generation."""
import struct, zlib
from pathlib import Path
WIDTH, HEIGHT = 256, 128

def png(path, night):
    rows=[]
    for y in range(HEIGHT):
        row=bytearray()
        for x in range(WIDTH):
            height=abs(y-HEIGHT/2)/(HEIGHT/2)
            if night:
                rgb=[int(4+5*(1-height)),int(6+8*(1-height)),int(16+15*(1-height))]
                if y<HEIGHT//2 and (x*431+y*173)%991<3:rgb=[210,220,255]
                if (x-180)**2+(y-30)**2<7**2:rgb=[205,218,240]
            else:
                rgb=[int(65+100*(1-height)),int(125+70*(1-height)),int(220+20*(1-height))]
                if (x-90)**2+(y-30)**2<9**2:rgb=[255,235,150]
            row.extend([*rgb,255])
        rows.append(b'\0'+row)
    def chunk(kind,data):return struct.pack('>I',len(data))+kind+data+struct.pack('>I',zlib.crc32(kind+data)&0xffffffff)
    path.write_bytes(b'\x89PNG\r\n\x1a\n'+chunk(b'IHDR',struct.pack('>IIBBBBB',WIDTH,HEIGHT,8,6,0,0,0))+chunk(b'IDAT',zlib.compress(b''.join(rows)))+chunk(b'IEND',b''))
for name,night in [('day',False),('night',True)]:png(Path(__file__).parent/'content'/f'{name}.png',night)
