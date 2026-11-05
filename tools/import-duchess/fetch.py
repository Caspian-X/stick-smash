#!/usr/bin/env python3
"""Download and pin the SSBU 13.0.1 Peach sources used by the Duchess import.

Writes (all under `docs/imports/sources/duchess/`):

- `scripts.json`  Rubendal Peach script dump at a pinned commit (both articles).
- `items.json`    the `peachdaikon` (Vegetable) item attack row from the same commit.
- `ufd.html`      the Ultimate Frame Data Peach page, retained verbatim.
- `ufd.json`      the move rows parsed out of that page.

    python3 tools/import-duchess/fetch.py
"""
import hashlib
import html
import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/duchess'
COMMIT = '2e8f0d245481cb0c936524b6ffe1cdea867c4f68'
REPO = 'https://github.com/rubendal/ssbu'
PATCH = '13.0.1'
ACCESSED = '2026-11-04'
UFD = 'https://ultimateframedata.com/peach/'
UA = {'User-Agent': 'Mozilla/5.0'}


def get(url: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(url, headers=UA)) as r:
        return r.read()


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def fetch_scripts() -> None:
    path = f'public/data/patch/{PATCH}/character/Peach/data.json'
    url = f'https://raw.githubusercontent.com/rubendal/ssbu/{COMMIT}/{path}'
    data = get(url)
    doc = json.loads(data)
    out = {
        'repository': REPO,
        'commit': COMMIT,
        'patch': PATCH,
        'source': path,
        'url': url,
        'accessed': ACCESSED,
        'sha256': sha(data),
        'articles': doc['Articles'],
        'scripts': {name: scripts for name, scripts in doc['Scripts'].items()},
        'motions': {name: [m['MotionKind'] for m in rows] for name, rows in doc['Motions'].items()},
    }
    (SOURCES / 'scripts.json').write_text(json.dumps(out, indent=1) + '\n')
    print(f'scripts.json  sha256 {out["sha256"]}  {len(data)} bytes')


def fetch_item() -> None:
    path = f'public/data/patch/{PATCH}/items.json'
    url = f'https://raw.githubusercontent.com/rubendal/ssbu/{COMMIT}/{path}'
    data = get(url)
    doc = json.loads(data)
    items = doc['Data']['item']['Items']
    keep = {name: items[name] for name in ('peachdaikon', 'daisydaikon')}
    out = {'repository': REPO, 'commit': COMMIT, 'patch': PATCH, 'source': path, 'url': url,
           'accessed': ACCESSED, 'sha256': sha(data), 'note':
           'Only the two daikon (Vegetable) item entries are retained; the upstream file holds every item.',
           'items': keep}
    (SOURCES / 'items.json').write_text(json.dumps(out, indent=1) + '\n')
    print(f'items.json    sha256 {out["sha256"]}  (full upstream {len(data)} bytes)')


def cell(block: str, name: str) -> str:
    m = re.search(rf'<div class="{name}">(.*?)</div>', block, re.S)
    if not m:
        return ''
    text = re.sub(r'<[^>]+>', ' ', m[1])
    return re.sub(r'\s+', ' ', html.unescape(text)).strip()


def fetch_ufd() -> None:
    data = get(UFD)
    (SOURCES / 'ufd.html').write_bytes(data)
    page = data.decode('utf-8', 'replace')
    headings = [(m.start(), re.sub(r'<[^>]+>', '', m[2]).strip())
                for m in re.finditer(r'<h2 class="movecategory" id="([^"]+)">(.*?)</h2>', page, re.S)]
    starts = [m.start() for m in re.finditer(r'<div class="movecontainer">', page)]
    rows = []
    for n, start in enumerate(starts):
        end = starts[n + 1] if n + 1 < len(starts) else len(page)
        block = page[start:end]
        name = cell(block, 'movename')
        if not name:
            continue
        category = next((name for pos, name in reversed(headings) if pos < start), None)
        rows.append({
            'category': category,
            'movename': name,
            'startup': cell(block, 'startup'),
            'total': cell(block, 'totalframes'),
            'landinglag': cell(block, 'landinglag'),
            'notes': cell(block, 'notes'),
            'damage': cell(block, 'basedamage'),
            'shieldlag': cell(block, 'shieldlag'),
            'shieldstun': cell(block, 'shieldstun'),
            'whichhitbox': cell(block, 'whichhitbox'),
            'advantage': cell(block, 'advantage'),
            'active': cell(block, 'activeframes'),
            'endlag': cell(block, 'endlag'),
            'hitboxes': [u for u in re.findall(r'data-featherlight="(hitboxes/[^"]+)"', block)],
        })
    stats = {}
    text = re.sub(r'<[^>]+>', '\n', page)
    text = html.unescape(re.sub(r'\n{2,}', '\n', text))
    for key, pattern in [('weight', r'Weight — ([\d.]+)'), ('gravity', r'Gravity — ([\d.]+)'),
                         ('walk', r'Walk Speed — ([\d.]+)'), ('run', r'Run Speed — ([\d.]+)'),
                         ('dash', r'Initial Dash — ([\d.]+)'), ('air', r'Air Speed — ([\d.]+)'),
                         ('airAccel', r'Total Air Acceleration — ([\d.]+)'),
                         ('airFrames', r'SH / FH / SHFF / FHFF Frames — ([\d /]+)'),
                         ('fall', r'Fall Speed / Fast Fall Speed — ([\d.]+ / [\d.]+)')]:
        m = re.search(pattern, text)
        stats[key] = m[1].strip() if m else None
    out = {'url': UFD, 'accessed': ACCESSED, 'pageSha256': sha(data),
           'note': 'Hitbox clips are linked from this page (paths under hitboxes/peach). '
                   'The character page lists every per-move visualisation; they are downloaded by visuals.py.',
           'stats': stats, 'moves': rows}
    (SOURCES / 'ufd.json').write_text(json.dumps(out, indent=1) + '\n')
    print(f'ufd.html      sha256 {out["pageSha256"]}  ({len(rows)} rows)')


def main() -> int:
    SOURCES.mkdir(parents=True, exist_ok=True)
    fetch_scripts()
    fetch_item()
    fetch_ufd()
    return 0


if __name__ == '__main__':
    sys.exit(main())
