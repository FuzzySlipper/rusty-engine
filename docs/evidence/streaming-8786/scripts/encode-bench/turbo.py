import io, sys, time
from PIL import Image
for path in sys.argv[1:]:
    rgb = Image.open(path).convert("RGB")
    for q in (70, 80, 90):
        times = []
        for i in range(23):
            buf = io.BytesIO(); t = time.perf_counter(); rgb.save(buf, "JPEG", quality=q, subsampling="4:2:0"); dt = (time.perf_counter()-t)*1000
            if i >= 3: times.append(dt)
        times.sort(); n = buf.tell()
        print(f"{rgb.width}x{rgb.height}\tlibjpeg-turbo (Pillow) q{q}\t{times[len(times)//2]:.2f} ms median\t{n} bytes\t{n*60/1e6:.1f} MB/s at 60fps")
