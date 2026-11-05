#!/usr/bin/env python3
"""Download and hash-check the UFD Diddy Kong hitbox visuals, then write per-move montages.

`ultimateframedata.com/hitboxes/diddy_kong/<Name>.gif` is the real per-move hitbox visualisation (the
character page only links the generic getup/ledge/trip clips). GIF frame 0 is the move's first frame.

    python3 tools/import-bongo/visuals.py /tmp/bongo-reference
"""
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/bongo'
MANIFEST = SOURCES / 'visuals.json'
BASE = 'https://ultimateframedata.com/hitboxes/diddy_kong/'

# Destination slot / variant -> UFD file stem.
GIFS = {
    'jab': ['DiddyKongJab1', 'DiddyKongJab2', 'DiddyKongJab3'],
    'ftilt': ['DiddyKongFTilt', 'DiddyKongFTiltUp', 'DiddyKongFTiltDown'],
    'utilt': ['DiddyKongUTilt'],
    'dtilt': ['DiddyKongDTilt'],
    'dashatk': ['DiddyKongDashAttack'],
    'fsmash': ['DiddyKongFSmash'],
    'usmash': ['DiddyKongUSmash'],
    'dsmash': ['DiddyKongDSmash'],
    'nair': ['DiddyKongNAir'],
    'fair': ['DiddyKongFAir'],
    'bair': ['DiddyKongBAir'],
    'uair': ['DiddyKongUAir'],
    'dair': ['DiddyKongDAir'],
    'grab': ['DiddyKongGrab', 'DiddyKongDashGrab', 'DiddyKongPivotGrab'],
    'throws': ['DiddyKongPummel', 'DiddyKongFThrow', 'DiddyKongBThrow', 'DiddyKongUThrow', 'DiddyKongDThrow'],
    'nspec': ['DiddyKongPeanutPopgunBoom', 'DiddyKongPeanut'],
    'sspec': ['DiddyKongMonkeyFlip', 'DiddyKongMonkeyFlipKick', 'DiddyKongMonkeyFlipStick',
              'DiddyKongMonkeyFlipStickAttack'],
    'uspec': ['DiddyKongRocketBarrels', 'DiddyKongRocketBarrelsExplosion.png'],
    'dspec': ['DiddyKongBanana'],
}

# Frames kept per file for the montage: source frames worth inspecting (0-based GIF frame = source frame - 1).
FRAMES = {
    'DiddyKongJab1': [0, 2, 4, 9, 18], 'DiddyKongJab2': [0, 4, 5, 9, 19], 'DiddyKongJab3': [0, 4, 5, 9, 33],
    'DiddyKongFTilt': [0, 9, 10, 12, 16, 35], 'DiddyKongFTiltUp': [0, 9, 10, 16, 35],
    'DiddyKongFTiltDown': [0, 9, 10, 16, 35], 'DiddyKongUTilt': [0, 5, 6, 11, 28],
    'DiddyKongDTilt': [0, 3, 4, 5, 18], 'DiddyKongDashAttack': [0, 7, 8, 15, 16, 21, 22, 39],
    'DiddyKongFSmash': [0, 3, 4, 11, 12, 20, 21, 22, 49],
    'DiddyKongUSmash': [0, 4, 5, 8, 11, 12, 15, 18, 19, 22, 52],
    'DiddyKongDSmash': [0, 6, 7, 8, 11, 12, 20, 49],
    'DiddyKongNAir': [0, 6, 7, 8, 19, 20, 26, 47],
    'DiddyKongFAir': [0, 5, 8, 9, 15, 25, 45], 'DiddyKongBAir': [0, 4, 5, 7, 9, 19, 29],
    'DiddyKongUAir': [0, 3, 4, 6, 7, 8, 26, 35], 'DiddyKongDAir': [0, 13, 14, 15, 16, 18, 34, 45],
    'DiddyKongGrab': [0, 6, 7, 8, 19, 35], 'DiddyKongDashGrab': [0, 9, 10, 11, 19, 43],
    'DiddyKongPivotGrab': [0, 10, 11, 12, 38],
    'DiddyKongPeanutPopgunBoom': [0, 4, 8, 12, 20, 40, 80, 108],
    'DiddyKongMonkeyFlip': [0, 6, 12, 17, 18, 25, 30, 36, 40, 46],
    'DiddyKongMonkeyFlipKick': [0, 5, 8, 12, 18, 24, 31],
    'DiddyKongMonkeyFlipStick': [0, 10, 20, 30, 44],
    'DiddyKongMonkeyFlipStickAttack': [0, 5, 10, 15, 19],
    'DiddyKongRocketBarrels': [0, 7, 20, 40, 55, 60],
    'DiddyKongBanana': [0, 5, 10, 15, 19, 20, 25],
}


def fetch(name: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(BASE + name, headers={'User-Agent': 'Mozilla/5.0'})) as r:
        return r.read()


def main() -> int:
    refresh = '--refresh' in sys.argv
    args = [a for a in sys.argv[1:] if a != '--refresh']
    out = Path(args[0] if args else '/tmp/bongo-reference')
    out.mkdir(parents=True, exist_ok=True)
    old = json.loads(MANIFEST.read_text()) if MANIFEST.exists() else {'base': BASE, 'files': {}}
    try:
        from PIL import Image, ImageDraw
    except ImportError:
        print('Pillow is required for the frame montages: pip install pillow', file=sys.stderr)
        return 1
    files = {}
    for slot, names in GIFS.items():
        for name in names:
            suffix = '.png' if name.endswith('.png') else '.gif'
            local = out / (name if suffix in name else name + suffix)
            if not local.exists():
                local.write_bytes(fetch(local.name))
            data = local.read_bytes()
            digest = hashlib.sha256(data).hexdigest()
            before = old['files'].get(name, {}).get('sha256')
            if before and before != digest:
                print(f'changed upstream: {name} {before} -> {digest}', file=sys.stderr)
                if not refresh:
                    print('refusing to overwrite the pinned visual manifest; pass --refresh to accept', file=sys.stderr)
                    return 1
            im = Image.open(local)
            frames = []
            for i in range(getattr(im, 'n_frames', 1)):
                im.seek(i)
                frames.append(im.convert('RGBA'))
            keep = [f for f in FRAMES.get(name, [0, len(frames) // 2, len(frames) - 1]) if f < len(frames)]
            sheet = Image.new('RGBA', (frames[0].width * len(keep), frames[0].height), (18, 18, 24, 255))
            dr = ImageDraw.Draw(sheet)
            for k, f in enumerate(keep):
                sheet.alpha_composite(frames[f], (frames[0].width * k, 0))
                dr.text((frames[0].width * k + 4, 4), str(f), fill=(255, 220, 80, 255))
            montage = f'{name}-frames.png'
            sheet.save(out / montage)
            files[name] = {'slot': slot, 'size': len(data), 'sha256': digest, 'frames': len(frames),
                           'montage': montage, 'kept': keep}
            print(f'{name:32} {len(data):>9} bytes  {len(frames):3} frames  kept {keep}')
    MANIFEST.write_text(json.dumps({'base': BASE, 'files': dict(sorted(files.items()))}, indent=1) + '\n')
    print(f'wrote {MANIFEST.relative_to(ROOT)} and montages in {out}')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
