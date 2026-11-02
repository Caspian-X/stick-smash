#!/usr/bin/env python3
"""Fetch the recorded UFD animations and extract inspection sheets (requires Pillow)."""
import hashlib
import json
import sys
import urllib.request
from pathlib import Path
from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[2]
manifest = json.loads((root / 'docs/imports/sources/wizard/visuals.json').read_text())
out = Path(sys.argv[1]); out.mkdir(parents=True, exist_ok=True)
for entry in manifest['animations']:
    path = out / entry['filename']
    urllib.request.urlretrieve(entry['url'], path)
    if hashlib.sha256(path.read_bytes()).hexdigest() != entry['sha256']:
        raise SystemExit(f'{path}: source animation changed; review it before updating the manifest')
    im = Image.open(path)
    sheet = Image.new('RGB', (250 * len(entry['inspect']), 265), (45, 45, 45))
    draw = ImageDraw.Draw(sheet)
    for col, frame in enumerate(entry['inspect']):
        im.seek(frame); image = im.convert('RGB'); image.thumbnail((248, 235))
        sheet.paste(image, (col * 250, 25))
        draw.text((col * 250 + 4, 5), f'{path.stem}: GIF index {frame}', fill='white')
    sheet.save(out / (path.stem + '-frames.png'))
print(f'Wrote {len(manifest["animations"])} original animations and frame sheets to {out}')
