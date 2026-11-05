#!/usr/bin/env python3
"""Download and hash-check the UFD Snake hitbox visualisations, then write per-frame montages.

`ultimateframedata.com/hitboxes/snake/<name>` holds the per-move hitbox overlays. The normal attacks, throws,
grabs, the grenade burst and the getup attacks are animated `.gif` files; the grenade, Cypher, C4, mortar and
Nikita hitboxes are single-frame `.png` images, so a "montage" for those is just the image.

    python3 tools/import-sneak/visuals.py /tmp/sneak-reference
    python3 tools/import-sneak/visuals.py --refresh /tmp/sneak-reference   # accept changed upstream bytes
"""
import hashlib
import json
import sys
import urllib.parse
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/sneak'
MANIFEST = SOURCES / 'visuals.json'
BASE = 'https://ultimateframedata.com/hitboxes/snake/'

# Ledger key -> UFD file names.
FILES = {
    'jab1': ['SnakeJab1.gif'],
    'jab2': ['SnakeJab2.gif'],
    'jab3': ['SnakeJab3.gif'],
    'ftilt1': ['SnakeFTilt1.gif'],
    'ftilt2': ['SnakeFTilt2.gif'],
    'utilt': ['SnakeUTilt.gif'],
    'dtilt': ['SnakeDTilt.gif'],
    'dashatk': ['SnakeDashAttack.gif'],
    'fsmash': ['SnakeFSmash.gif'],
    'usmash': ['SnakeUSmash.gif', 'SnakeUSmashSearch.png', 'SnakeUSmashExplosion.png'],
    'dsmash': ['SnakeDSmash.gif'],
    'nair': ['SnakeNAir.gif'],
    'fair': ['SnakeFAir.gif'],
    'bair': ['SnakeBAir.gif'],
    'uair': ['SnakeUAir.gif'],
    'dair': ['SnakeDAir.gif'],
    'grab': ['SnakeGrab.gif', 'SnakeDashGrab.gif', 'SnakePivotGrab.gif'],
    'pummel': ['SnakePummel.gif'],
    'throws': ['SnakeFThrow.gif', 'SnakeBThrow.gif', 'SnakeUThrow.gif', 'SnakeDThrow.gif'],
    'grenade': ['SnakeGrenade.png', 'SnakeGrenadeBurst.gif'],
    'nikita': ['Nikita Hurtbox (Weak).png', 'NikitaSearch.png', 'Nikita Explosion (Weak).png',
               'Nikita Explosion (Strong).png'],
    'cypher': ['SnakeCypher.png'],
    'c4': ['SnakeC4.png', 'C4 Explosion (Snake for Scale).png'],
    'getup': ['snakeGetupAttackU.gif', 'snakeGetupAttackD.gif', 'snakeTripAttack.gif', 'snakeLedgeAttack.gif'],
}

# GIF frames kept for inspection (0-based; GIF frame 0 is source frame 1 for the normal attacks).
KEEP = {
    'SnakeJab1': [1, 2, 3, 6, 10],
    'SnakeJab2': [2, 3, 4, 8, 14],
    'SnakeJab3': [6, 7, 8, 12, 20],
    'SnakeFTilt1': [2, 3, 4, 10, 18],
    'SnakeFTilt2': [5, 6, 7, 10, 20],
    'SnakeUTilt': [4, 5, 6, 8, 11, 18],
    'SnakeDTilt': [4, 5, 6, 7, 12, 18],
    'SnakeDashAttack': [3, 4, 6, 8, 10, 16],
    'SnakeFSmash': [4, 20, 38, 40, 41, 44, 60],
    'SnakeUSmash': [4, 10, 11, 24, 34, 40, 50],
    'SnakeDSmash': [4, 6, 7, 9, 18, 21, 30],
    'SnakeNAir': [6, 9, 10, 17, 25, 35, 38],
    'SnakeFAir': [10, 18, 22, 24, 26, 34],
    'SnakeBAir': [5, 6, 8, 10, 14, 24],
    'SnakeUAir': [6, 9, 10, 13, 16, 22],
    'SnakeDAir': [2, 3, 9, 16, 24, 26, 40],
    'SnakeGrab': [6, 7, 8, 12],
    'SnakeDashGrab': [9, 10, 11, 16],
    'SnakePivotGrab': [7, 8, 9, 14],
    'SnakePummel': [0, 1, 3],
    'SnakeFThrow': [8, 15, 20, 21, 26],
    'SnakeBThrow': [8, 14, 19, 20, 26],
    'SnakeUThrow': [10, 18, 21, 22, 23, 30],
    'SnakeDThrow': [10, 20, 30, 34, 35, 40],
    'SnakeGrenadeBurst': [0, 1, 2, 4, 8],
    'snakeGetupAttackU': [0, 4, 8, 14],
    'snakeGetupAttackD': [0, 4, 8, 14],
    'snakeTripAttack': [0, 4, 8, 14],
    'snakeLedgeAttack': [0, 6, 12, 22],
}


def fetch(name: str) -> bytes:
    url = BASE + urllib.parse.quote(name)
    with urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})) as r:
        return r.read()


def main() -> int:
    args = [a for a in sys.argv[1:] if not a.startswith('-')]
    refresh = '--refresh' in sys.argv[1:]
    out = Path(args[0] if args else '/tmp/sneak-reference')
    out.mkdir(parents=True, exist_ok=True)
    old = json.loads(MANIFEST.read_text()) if MANIFEST.exists() else {'base': BASE, 'files': {}}
    try:
        from PIL import Image
    except ImportError:
        print('Pillow is required for the frame montages: pip install pillow', file=sys.stderr)
        return 1
    files = {}
    for slot, names in FILES.items():
        for name in names:
            path = out / name
            if not path.exists():
                path.write_bytes(fetch(name))
            data = path.read_bytes()
            digest = hashlib.sha256(data).hexdigest()
            before = old['files'].get(name, {}).get('sha256')
            if before and before != digest:
                # Pinned sources must not drift silently: fail unless the caller asks to refresh them.
                print(f'changed upstream: {name} {before} -> {digest}', file=sys.stderr)
                if not refresh:
                    print('refusing to rewrite the pinned manifest without --refresh', file=sys.stderr)
                    return 1
            im = Image.open(path)
            frames = []
            for i in range(getattr(im, 'n_frames', 1)):
                im.seek(i)
                frames.append(im.convert('RGBA'))
            keep = [f for f in KEEP.get(Path(name).stem, [0, len(frames) // 2, len(frames) - 1]) if f < len(frames)]
            if len(frames) == 1:
                sheet = frames[0]
                stem = f'{Path(name).stem}-frames.png'
            else:
                # Wide sprites fit in a row; tall ones wrap into two so the sheet stays inspectable.
                across = max(1, len(keep) // 2) if frames[0].height > frames[0].width * 1.4 else len(keep)
                rows = (len(keep) + across - 1) // across
                sheet = Image.new('RGBA', (frames[0].width * across, frames[0].height * rows), (18, 18, 24, 255))
                for k, f in enumerate(keep):
                    sheet.alpha_composite(frames[f], (frames[0].width * (k % across), frames[0].height * (k // across)))
                stem = f'{Path(name).stem}-frames.png'
            sheet.save(out / stem)
            files[name] = {'slot': slot, 'size': len(data), 'sha256': digest,
                           'frames': len(frames), 'montage': stem, 'kept': keep}
            print(f'{name:34} {len(data):>9} bytes  {len(frames):3} frames  kept {keep}')
    MANIFEST.write_text(json.dumps({'base': BASE, 'files': dict(sorted(files.items()))}, indent=1) + '\n')
    print(f'wrote {MANIFEST.relative_to(ROOT)} and montages in {out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
