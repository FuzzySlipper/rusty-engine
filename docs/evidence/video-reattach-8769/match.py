# Finds the anim0000 time that best matches each screenshot's letterboxed clip.
import json, os
from PIL import Image
OUT = os.environ.get('OUT', '.')
# ffmpeg -i anim0000.webm -vf fps=4,scale=160:100,format=gray -f rawvideo ref-gray.raw
ref = open(f'{OUT}/ref-gray.raw', 'rb').read()
W, H, FPS = 160, 100, 4
frames = [ref[i*W*H:(i+1)*W*H] for i in range(len(ref)//(W*H))]
run = json.load(open(f'{OUT}/reattach.json'))
for shot in run['shots']:
    img = Image.open(f"{OUT}/{shot['label']}.png").convert('L')
    # 320x200 letterboxed in 1280x720: 1152x720 at x=64.
    img = img.crop((64, 0, 1216, 720)).resize((W, H), Image.BILINEAR).tobytes()
    scores = sorted((sum((a-b)**2 for a, b in zip(img, f)) / (W*H), i) for i, f in enumerate(frames))
    best = scores[0]
    shot['bestMatchSeconds'] = best[1] / FPS
    shot['bestMse'] = round(best[0], 1)
    shot['runnerUp'] = [(round(s, 1), i / FPS) for s, i in scores[1:4]]
    shot['matchAtZeroMse'] = round(next(s for s, i in scores if i == 0), 1)
json.dump(run, open(f'{OUT}/reattach-matched.json', 'w'), indent=2)
print(json.dumps(run, indent=2))
