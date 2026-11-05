#!/usr/bin/env python3
"""Download and hash-check the UFD Robin hitbox GIFs, then write per-frame montages for inspection.

`ultimateframedata.com/hitboxes/robin/<name>.gif` is the real per-move hitbox visualisation (the character page
only links the generic getup/ledge/trip ones). GIF frame 0 is the first pictured frame of the move and, for the
normal attacks, corresponds to source frame 1.

    python3 tools/import-robin/visuals.py /tmp/robin-reference
"""
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/throbbin'
MANIFEST = SOURCES / 'visuals.json'
BASE = 'https://ultimateframedata.com/hitboxes/robin/'

# Destination slot / variant -> UFD file stem.
GIFS = {
    'jab1': ['RobinJab1'],
    'jab2': ['RobinJab2'],
    'jab3': ['RobinJab3'],
    'rapid': ['RobinJabRapid'],
    'ftilt': ['RobinFTilt'],
    'utilt': ['RobinUTilt'],
    'dtilt': ['RobinDTilt'],
    'dashatk': ['RobinDashAttack'],
    'fsmash': ['RobinFSmashLevin', 'RobinFSmashBronze'],
    'usmash': ['RobinUSmashLevin', 'RobinUSmashBronze'],
    'dsmash': ['RobinDSmashLevin', 'RobinDSmashBronze'],
    'nair': ['RobinNAirLevin', 'RobinNAirBronze'],
    'fair': ['RobinFAirLevin', 'RobinFAirBronze'],
    'bair': ['RobinBAirLevin', 'RobinBAirBronze'],
    'uair': ['RobinUAirLevin', 'RobinUAirBronze'],
    'dair': ['RobinDAirLevin', 'RobinDAirBronze'],
    'grab': ['RobinGrab', 'RobinDashGrab'],
    'throws': ['RobinFThrow', 'RobinBThrow', 'RobinUThrow', 'RobinDThrow'],
    'specials': ['RobinThunder', 'RobinElthunder', 'RobinArcthunder', 'RobinThoron', 'RobinArcfire',
                 'RobinElwind', 'RobinNosferatu'],
}

# Frames kept per file for the montage: the source's hit windows (0-based GIF frame = source frame - 1).
FRAMES = {
    'RobinFSmashLevin': [9, 12, 15, 17, 20, 26],
    'RobinFSmashBronze': [9, 12, 15, 17, 20, 26],
    'RobinUSmashLevin': [6, 10, 12, 14, 18, 28],
    'RobinDSmashLevin': [10, 14, 16, 18, 21, 26],
    'RobinNAirLevin': [4, 6, 9, 12, 20, 24],
    'RobinFAirLevin': [8, 11, 13, 15, 18, 24],
    'RobinFAirBronze': [8, 11, 13, 15, 18, 24],
    'RobinBAirLevin': [5, 8, 9, 12, 15, 20],
    'RobinUAirLevin': [6, 9, 11, 13, 17, 22],
    'RobinDAirLevin': [8, 12, 14, 16, 20, 26],
    'RobinUTilt': [3, 5, 8, 11, 13, 18],
    'RobinDTilt': [4, 6, 7, 9, 12, 16],
    'RobinJab1': [2, 3, 4, 6, 10, 16],
    'RobinJab3': [3, 5, 6, 8, 12, 18],
    'RobinThunder': [6, 8, 10, 14],
    'RobinThoron': [20, 24, 30, 40],
    'RobinArcfire': [10, 16, 20, 30],
    'RobinElwind': [6, 9, 14, 26],
    'RobinNosferatu': [12, 14, 16, 20, 26],
}


def fetch(name: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(BASE + name + '.gif', headers={'User-Agent': 'Mozilla/5.0'})) as r:
        return r.read()


def main() -> int:
    out = Path(sys.argv[1] if len(sys.argv) > 1 else '/tmp/robin-reference')
    out.mkdir(parents=True, exist_ok=True)
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
            if before and before != digest:
                print(f'changed upstream: {name} {before} -> {digest}', file=sys.stderr)
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
            print(f'{name:20} {len(data):>9} bytes  {len(frames):3} frames  kept {keep}')
    MANIFEST.write_text(json.dumps({'base': BASE, 'files': dict(sorted(files.items()))}, indent=1) + '\n')
    print(f'wrote {MANIFEST.relative_to(ROOT)} and montages in {out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
