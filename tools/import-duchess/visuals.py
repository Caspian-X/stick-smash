#!/usr/bin/env python3
"""Download and hash-check the UFD Peach hitbox clips, then write per-frame montages for inspection.

The character page links every per-move visualisation (paths under `hitboxes/peach`); `ufd.json` keeps those
paths, so this tool has no separate file list. GIF frame 0 is the move's first pictured frame, which for the
normal attacks is source frame 1.

    just duchess-visuals /tmp/duchess-reference
"""
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/duchess'
MANIFEST = SOURCES / 'visuals.json'
BASE = 'https://ultimateframedata.com/'
UA = {'User-Agent': 'Mozilla/5.0'}

# Frames kept per clip: around each move's source hit windows (0-based GIF frame = source frame - 1).
FRAMES = {
    'PeachJab1': [0, 1, 2, 5, 10, 18],
    'PeachJab2': [0, 1, 2, 3, 8, 16],
    'PeachFTilt': [3, 6, 7, 8, 9, 14],
    'PeachUTilt': [4, 8, 10, 12, 18, 26],
    'PeachDTilt': [3, 7, 8, 9, 14, 20],
    'PeachDashAttack': [2, 5, 6, 8, 16, 18],
    'PeachFSmashGolfClub': [4, 10, 14, 15, 16, 22],
    'PeachFSmashFryingPan': [4, 10, 14, 15, 16, 22],
    'PeachFSmashTennisRacket': [4, 10, 14, 15, 16, 22],
    'PeachUSmash': [8, 12, 13, 15, 17, 19],
    'PeachDSmash': [2, 5, 6, 9, 17, 29],
    'PeachNAir': [4, 5, 7, 8, 12, 18],
    'PeachFAir': [12, 15, 16, 18, 20, 26],
    'PeachBAir': [4, 5, 6, 7, 10, 16],
    'PeachUAir': [6, 9, 10, 12, 14, 16],
    'PeachDAir': [8, 11, 12, 17, 23, 29],
    'PeachGrab': [3, 5, 6, 7, 10, 16],
    'PeachDashGrab': [6, 8, 9, 10, 14, 20],
    'PeachPivotGrab': [7, 9, 10, 11, 16, 24],
    'PeachPummel': [0, 1, 2, 3, 8, 14],
    'PeachFThrow': [12, 13, 14, 15, 20, 28],
    'PeachBThrow': [17, 19, 20, 21, 26, 34],
    'PeachUThrow': [16, 18, 19, 20, 24, 32],
    'PeachDThrow': [30, 33, 34, 35, 40, 48],
    'PeachPeachBomber.png': [0],
    'PeachPeachBomberHit': [0, 1, 2, 3, 6, 12],
    'PeachPeachParasol': [5, 6, 8, 10, 15, 26],
    'PeachPeachParasolFall': [0, 4, 8, 12, 20, 30],
    'PeachTurnip.png': [0],
}


def fetch(rel: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(BASE + rel, headers=UA)) as r:
        return r.read()


def main() -> int:
    out = Path(sys.argv[1] if len(sys.argv) > 1 else '/tmp/duchess-reference')
    out.mkdir(parents=True, exist_ok=True)
    ufd = json.loads((SOURCES / 'ufd.json').read_text())
    clips = sorted({path for row in ufd['moves'] for path in row['hitboxes']})
    old = json.loads(MANIFEST.read_text()) if MANIFEST.exists() else {'base': BASE, 'files': {}}
    try:
        from PIL import Image
    except ImportError:
        print('Pillow is required for the frame montages: pip install pillow', file=sys.stderr)
        return 1
    files = {}
    for rel in clips:
        name = Path(rel).name
        stem = Path(rel).stem
        local = out / name
        if not local.exists():
            local.write_bytes(fetch(rel))
        data = local.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        before = old['files'].get(name, {}).get('sha256')
        if before and before != digest:
            print(f'changed upstream: {name} {before} -> {digest}', file=sys.stderr)
        im = Image.open(local)
        frames = []
        for i in range(getattr(im, 'n_frames', 1)):
            im.seek(i)
            frames.append(im.convert('RGBA'))
        keep = [f for f in FRAMES.get(stem, [0, len(frames) // 2, len(frames) - 1]) if f < len(frames)]
        sheet = Image.new('RGBA', (frames[0].width * len(keep), frames[0].height), (18, 18, 24, 255))
        for k, f in enumerate(keep):
            sheet.alpha_composite(frames[f], (frames[0].width * k, 0))
        (out / f'{Path(name).stem}-frames.png').unlink(missing_ok=True)
        sheet.save(out / f'{Path(name).stem}-frames.png')
        files[name] = {'url': BASE + rel, 'size': len(data), 'sha256': digest, 'frames': len(frames),
                       'montage': f'{Path(name).stem}-frames.png', 'kept': keep}
        print(f'{name:28} {len(data):>9} bytes  {len(frames):3} frames  kept {keep}')
    MANIFEST.write_text(json.dumps({'base': BASE, 'files': dict(sorted(files.items()))}, indent=1) + '\n')
    print(f'wrote {MANIFEST.relative_to(ROOT)} and montages in {out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
