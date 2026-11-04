#!/usr/bin/env python3
"""Download and hash-check the UFD Byleth hitbox GIFs, then write per-frame montages for inspection.

`ultimateframedata.com/hitboxes/byleth/<name>.gif` is the real per-move hitbox visualisation. Two of the names
linked from the character page redirect (`BylethJapRapidFinisher`, `BylethAymrSuperArmor`), so they are fetched
from their corrected spellings. GIF frame 0 is source gameplay frame 1 for the clips whose frame count equals
the UFD total.

    python3 tools/import-verdant/visuals.py /tmp/verdant-reference
"""
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/verdant'
MANIFEST = SOURCES / 'visuals.json'
BASE = 'https://ultimateframedata.com/hitboxes/byleth/'

# Destination slot / variant -> UFD file stem.
GIFS = {
    'jab1': ['BylethJab1'],
    'jab2': ['BylethJab2'],
    'jab3': ['BylethJab3'],
    'rapid': ['BylethJabRapid'],
    'rapidEnd': ['BylethJabRapidFinisher'],
    'ftilt': ['BylethFTilt'],
    'utilt': ['BylethUTilt'],
    'dtilt': ['BylethDTilt'],
    'dashatk': ['BylethDashAttack'],
    'fsmash': ['BylethFSmash', 'BylethFSmashDown', 'BylethFSmashUp'],
    'usmash': ['BylethUSmash'],
    'dsmash': ['BylethDSmash'],
    'nair': ['BylethNAir', 'BylethNAirLanding'],
    'fair': ['BylethFAir'],
    'bair': ['BylethBAir'],
    'uair': ['BylethUAir'],
    'dair': ['BylethDAir'],
    'grab': ['BylethGrab', 'BylethDashGrab', 'BylethPivotGrab'],
    'pummel': ['BylethPummel'],
    'throws': ['BylethFThrow', 'BylethBThrow', 'BylethUThrow', 'BylethDThrow'],
    'nspec': ['BylethFailnaught', 'BylethFailnaughtMax'],
    'sspec': ['BylethAreadbharGrounded', 'BylethAreadbharAerial'],
    'uspec': ['BylethSwordoftheCreatorGrounded', 'BylethSwordoftheCreatorAerial'],
    'dspec': ['BylethAymr', 'BylethAymrSuperArmour'],
}

# Frames kept per file for the montage: GIF frame 0 is source frame 1, so an active window of source frames
# a-b means GIF frames a-1 .. b-1.
FRAMES = {
    'BylethJab1': [0, 2, 3, 4, 8, 16],
    'BylethJab2': [0, 3, 4, 5, 9, 18],
    'BylethJab3': [0, 4, 5, 6, 9, 20],
    'BylethJabRapid': [0, 10, 11, 13, 16],
    'BylethJabRapidFinisher': [0, 4, 5, 6, 10, 30],
    'BylethFTilt': [0, 5, 6, 7, 9, 14],
    'BylethUTilt': [0, 6, 7, 9, 14, 20],
    'BylethDTilt': [0, 10, 11, 12, 14, 20],
    'BylethDashAttack': [0, 6, 8, 9, 10, 20],
    'BylethFSmash': [0, 12, 20, 22, 24, 30],
    'BylethFSmashDown': [22, 24],
    'BylethFSmashUp': [22, 24],
    'BylethUSmash': [0, 11, 12, 14, 16, 27],
    'BylethDSmash': [0, 14, 15, 17, 25, 28],
    'BylethNAir': [0, 5, 8, 14, 22, 27, 30],
    'BylethNAirLanding': [0, 1, 2, 4, 8],
    'BylethFAir': [0, 8, 11, 13, 16],
    'BylethBAir': [0, 10, 12, 14, 16, 19],
    'BylethUAir': [0, 7, 9, 12, 19, 22],
    'BylethDAir': [0, 15, 20, 21, 23, 28],
    'BylethGrab': [0, 4, 5, 6, 10, 20],
    'BylethDashGrab': [0, 8, 9, 10, 14, 24],
    'BylethPivotGrab': [0, 9, 10, 11, 16],
    'BylethFThrow': [0, 10, 12, 13, 20],
    'BylethBThrow': [0, 10, 12, 20, 24, 29],
    'BylethUThrow': [0, 10, 13, 14, 20],
    'BylethDThrow': [0, 10, 11, 12, 20],
    'BylethFailnaught': [0, 20, 40, 43, 44, 46, 60],
    'BylethFailnaughtMax': [20, 60, 100, 113, 115, 140],
    'BylethAreadbharGrounded': [0, 15, 18, 19, 21, 24, 40],
    'BylethAreadbharAerial': [0, 15, 19, 20, 22, 26, 40],
    'BylethSwordoftheCreatorGrounded': [0, 5, 7, 9, 14, 20, 30, 45],
    'BylethSwordoftheCreatorAerial': [0, 5, 7, 9, 14, 20, 30, 45],
    'BylethAymr': [0, 30, 50, 60, 63, 66, 80, 120],
    'BylethAymrSuperArmour': [0, 30, 40, 60, 100],
}
# The character page links these misspelled names, which redirect.
REDIRECTS = {'BylethJapRapidFinisher': 'BylethJabRapidFinisher', 'BylethAymrSuperArmor': 'BylethAymrSuperArmour'}


def fetch(name: str) -> bytes:
    url = BASE + REDIRECTS.get(name, name) + '.gif'
    with urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})) as r:
        return r.read()


def main() -> int:
    out = Path(sys.argv[1] if len(sys.argv) > 1 else '/tmp/verdant-reference')
    out.mkdir(parents=True, exist_ok=True)
    refresh = '--refresh' in sys.argv[2:]
    old = json.loads(MANIFEST.read_text()) if MANIFEST.exists() else {'base': BASE, 'files': {}}
    try:
        from PIL import Image
    except ImportError:
        print('Pillow is required for the frame montages: pip install pillow', file=sys.stderr)
        return 1
    files = {}
    for slot, names in GIFS.items():
        for name in names:
            gif = out / f'{name}.gif'
            if not gif.exists():
                gif.write_bytes(fetch(name))
            data = gif.read_bytes()
            digest = hashlib.sha256(data).hexdigest()
            before = old['files'].get(name, {}).get('sha256')
            if before and before != digest and not refresh:
                print(f'changed upstream: {name} {before} -> {digest}; rerun with --refresh to re-pin',
                      file=sys.stderr)
                return 1
            im = Image.open(gif)
            frames = []
            for i in range(getattr(im, 'n_frames', 1)):
                im.seek(i)
                frames.append(im.convert('RGBA'))
            keep = [f for f in FRAMES.get(name, [0, len(frames) // 2, len(frames) - 1]) if f < len(frames)]
            sheet = Image.new('RGBA', (frames[0].width * len(keep), frames[0].height), (18, 18, 24, 255))
            for k, f in enumerate(keep):
                sheet.alpha_composite(frames[f], (frames[0].width * k, 0))
            (out / f'{name}-frames.png').unlink(missing_ok=True)
            sheet.save(out / f'{name}-frames.png')
            files[name] = {'slot': slot, 'size': len(data), 'sha256': digest,
                           'frames': len(frames), 'montage': f'{name}-frames.png', 'kept': keep}
            print(f'{name:34} {len(data):>9} bytes  {len(frames):3} frames  kept {keep}')
    MANIFEST.write_text(json.dumps({'base': BASE, 'files': dict(sorted(files.items()))}, indent=1) + '\n')
    print(f'wrote {MANIFEST.relative_to(ROOT)} and montages in {out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
