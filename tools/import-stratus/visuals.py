#!/usr/bin/env python3
"""Download and hash-check the UFD Cloud hitbox GIFs, then write per-frame montages for inspection.

`ultimateframedata.com/hitboxes/cloud/<name>.gif` is the real per-move hitbox visualisation (the character page
only links the generic getup/ledge/trip clips). GIF frame 0 is the move's first pictured frame; for the normal
attacks that is source frame 1. The GIF frame count is only a duration cross-check: it matches the UFD total for
many clips (neutral air 38, back air 42, dash attack 45, up/down smash 45/49, grabs 36/44, throws 35/31/50/43)
and disagrees for animated or charge-skipping ones (forward smash 45 against a 66-frame move).

    python3 tools/import-stratus/visuals.py /tmp/stratus-reference
"""
import hashlib
import json
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/stratus'
MANIFEST = SOURCES / 'visuals.json'
BASE = 'https://ultimateframedata.com/hitboxes/cloud/'

# Destination slot / variant -> UFD file stems (both facing variants are kept where UFD supplies them).
GIFS = {
    'jab1': ['CloudJab1'],
    'jab2': ['CloudJab2'],
    'jab3': ['CloudJab3'],
    'ftilt': ['CloudFTilt'],
    'utilt': ['CloudUTilt'],
    'dtilt': ['CloudDTilt'],
    'dashatk': ['CloudDashAttack'],
    'fsmash': ['CloudFSmash'],
    'usmash': ['CloudUSmash'],
    'dsmash': ['CloudDSmash'],
    'nair': ['CloudNAir'],
    'fair': ['CloudFAir'],
    'bair': ['CloudBAir'],
    'uair': ['CloudUAir'],
    'dair': ['CloudDAir'],
    'grab': ['CloudGrab', 'CloudDashGrab', 'CloudPivotGrab', 'CloudPummel'],
    'throws': ['CloudFThrow', 'CloudBThrow', 'CloudUThrow', 'CloudDThrow'],
    'nspec': ['CloudBladeBeamG', 'CloudBladeBeamA', 'CloudBladeBeamLimitG', 'CloudBladeBeamLimitA'],
    'sspec': ['CloudCrossSlash1Left', 'CloudCrossSlash1Right', 'CloudCrossSlash2Left', 'CloudCrossSlash2Right',
              'CloudCrossSlash3Left', 'CloudCrossSlash3Right', 'CloudCrossSlashLimitLeft',
              'CloudCrossSlashLimitRight'],
    'uspec': ['CloudClimhazzard', 'CloudClimhazzardLimit', 'CloudClimhazzardFall', 'CloudClimhazzardLanding'],
    'dspec': ['CloudFinishingTouchG', 'CloudFinishingTouchA'],
}

# Frames kept per file for the montage: source hit windows (0-based GIF frame = source frame - 1).
FRAMES = {
    'CloudJab3': [0, 4, 5, 6, 9, 14],
    'CloudFTilt': [0, 6, 8, 9, 12, 18],
    'CloudUTilt': [0, 4, 5, 7, 9, 14],
    'CloudDTilt': [0, 5, 6, 8, 12, 18],
    'CloudDashAttack': [0, 6, 8, 10, 13, 16],
    'CloudFSmash': [0, 9, 12, 15, 18, 23, 27, 34],
    'CloudUSmash': [0, 9, 11, 12, 13, 15, 20],
    'CloudDSmash': [0, 5, 7, 8, 20, 22, 30],
    'CloudNAir': [3, 5, 8, 11, 14, 20],
    'CloudFAir': [0, 15, 17, 18, 20, 22, 28],
    'CloudBAir': [0, 8, 10, 11, 12, 16, 24],
    'CloudUAir': [0, 5, 7, 9, 12, 20],
    'CloudDAir': [0, 8, 10, 12, 15, 25],
    'CloudGrab': [0, 7, 8, 9, 12, 20],
    'CloudDashGrab': [0, 10, 11, 12, 16, 24],
    'CloudCrossSlash1Right': [0, 8, 9, 10, 12, 16],
    'CloudCrossSlash2Right': [0, 1, 2, 3, 6, 10],
    'CloudCrossSlash3Right': [0, 1, 2, 10, 11, 24, 25, 34],
    'CloudCrossSlashLimitRight': [0, 5, 9, 10, 14, 20, 27, 37],
    'CloudClimhazzard': [0, 5, 6, 8, 10, 14],
    'CloudClimhazzardLimit': [0, 5, 6, 8, 10, 14, 22],
    'CloudClimhazzardFall': [0, 5, 10],
    'CloudClimhazzardLanding': [0, 2, 6],
    'CloudFinishingTouchG': [0, 10, 12, 14, 16, 22, 30],
    'CloudBladeBeamG': [0, 10, 15, 17, 20, 30],
    'CloudBladeBeamLimitG': [0, 10, 15, 17, 20, 30],
    'CloudNair': [3, 5, 8, 11, 14, 20],
}


def fetch(name: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(BASE + name + '.gif', headers={'User-Agent': 'Mozilla/5.0'})) as r:
        return r.read()


def main() -> int:
    out = Path(sys.argv[1] if len(sys.argv) > 1 else '/tmp/stratus-reference')
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
            width, height = frames[0].size
            sheet = Image.new('RGBA', (width * len(keep), height), (18, 18, 24, 255))
            for k, f in enumerate(keep):
                sheet.alpha_composite(frames[f], (width * k, 0))
                ImageDraw.Draw(sheet).text((width * k + 8, 8), str(f), fill=(255, 255, 0, 255))
            (out / f'{name}-frames.png').unlink(missing_ok=True)
            sheet.save(out / f'{name}-frames.png')
            files[name] = {'slot': slot, 'size': len(data), 'sha256': digest,
                           'frames': len(frames), 'montage': f'{name}-frames.png', 'kept': keep}
            print(f'{name:26} {len(data):>9} bytes  {len(frames):3} frames  kept {keep}')
    MANIFEST.write_text(json.dumps({'base': BASE, 'files': dict(sorted(files.items()))}, indent=1) + '\n')
    print(f'wrote {MANIFEST.relative_to(ROOT)} and montages in {out}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
