#!/usr/bin/env python3
"""Fetch the recorded UFD Piranha Plant animations and write per-frame inspection sheets (Pillow)."""
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

from PIL import Image, ImageDraw

root = Path(__file__).resolve().parents[2]
manifest = json.loads((root / "docs/imports/sources/plant/visuals.json").read_text())
out = Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/plant-reference")
out.mkdir(parents=True, exist_ok=True)
for entry in manifest["animations"]:
    path = out / entry["filename"]
    urllib.request.urlretrieve(entry["url"], path)
    if hashlib.sha256(path.read_bytes()).hexdigest() != entry["sha256"]:
        raise SystemExit(f"{path}: source animation changed; review it before updating the manifest")
    frames = entry["inspect"]
    im = Image.open(path)
    w, h = im.size
    scale = min(1.0, 260 / w)
    tw, th = int(w * scale), int(h * scale)
    cols = min(5, len(frames))
    rows = (len(frames) + cols - 1) // cols
    sheet = Image.new("RGB", (tw * cols, th * rows + 22), (38, 38, 38))
    draw = ImageDraw.Draw(sheet)
    for i, frame in enumerate(frames):
        im.seek(min(frame, im.n_frames - 1))
        x, y = (i % cols) * tw, 22 + (i // cols) * th
        sheet.paste(im.convert("RGB").resize((tw, th)), (x, y))
        draw.text((x + 4, y + 4), f"{frame}", fill="white")
    sheet.save(out / (path.stem + "-frames.png"))
print(f"Wrote {len(manifest['animations'])} original animations and frame sheets to {out}")
