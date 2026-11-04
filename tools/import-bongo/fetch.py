#!/usr/bin/env python3
"""Acquire the pinned Diddy Kong reference inputs and normalize the UFD rows.

Writes `docs/imports/sources/bongo/{scripts,items,ufd}.json` with source URLs, pinned revisions and
sha256 hashes. Run once (or to re-check upstream); `import.py` reads only the retained files.

    python3 tools/import-bongo/fetch.py
"""
import hashlib
import html
import json
import re
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / 'docs/imports/sources/bongo'
REPO = 'https://github.com/rubendal/ssbu'
COMMIT = '2e8f0d245481cb0c936524b6ffe1cdea867c4f68'
PATCH = '13.0.1'
RAW = f'https://raw.githubusercontent.com/rubendal/ssbu/{COMMIT}/public/data/patch/{PATCH}'
UFD = 'https://ultimateframedata.com/diddy_kong'


def get(url: str) -> bytes:
    with urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})) as r:
        return r.read()


def text(fragment: str) -> str:
    return re.sub(r'\s+', ' ', re.sub(r'<[^>]+>', '', html.unescape(fragment))).strip()


def rows(page: str) -> dict:
    out = {}
    for chunk in page.split('<div class="movecontainer">')[1:]:
        name = re.search(r'<div class="movename">(.*?)</div>', chunk, re.S)
        if not name:
            continue
        row = {'name': text(name[1])}
        for field in ('startup', 'totalframes', 'landinglag', 'notes', 'basedamage', 'shieldlag', 'shieldstun',
                      'whichhitbox', 'advantage', 'activeframes'):
            m = re.search(r'<div class="' + field + r'">(.*?)</div>', chunk, re.S)
            if m:
                row[field] = text(m[1])
        m = re.search(r'<div class="endlag">(.*?)</div>', chunk, re.S)
        if m:
            row['endlag'] = text(m[1])
        row['images'] = re.findall(r'data-src="(hitboxes/[^"]+)"', chunk)
        out[row['name']] = row
    return out


def stats(page: str) -> dict:
    block = page.split('Stats', 1)[1].split('Ledge Grab', 1)[0]
    flat = text(block)
    def num(label: str) -> float:
        return float(re.search(label + r' — ([\d.]+)', flat)[1])
    jump = re.search(r'SH / FH / SHFF / FHFF Frames — ([\d ]+) / ([\d ]+)', flat)
    fall = re.search(r'Fall Speed / Fast Fall Speed — ([\d.]+) / ([\d.]+)', flat)
    return {
        'weight': num('Weight'), 'gravity': num('Gravity'), 'walk': num('Walk Speed'),
        'run': num('Run Speed'), 'dash': num('Initial Dash'), 'air': num('Air Speed'),
        'airAccel': num('Total Air Acceleration'), 'shortHopFrames': int(jump[1]),
        'fullHopFrames': int(jump[2]), 'fall': float(fall[1]), 'fastFall': float(fall[2]),
        'jumpSquat': 3,
    }


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    raw = get(f'{RAW}/character/Diddy%20Kong/data.json')
    items_raw = get(f'{RAW}/items.json')
    page = get(UFD).decode()
    data = json.loads(raw)
    items = json.loads(items_raw)['Data']['item']['Items']
    (OUT / 'scripts.json').write_text(json.dumps({
        'repository': REPO, 'commit': COMMIT, 'patch': PATCH,
        'source': f'public/data/patch/{PATCH}/character/Diddy Kong/data.json',
        'sha256': hashlib.sha256(raw).hexdigest(), 'scripts': data['Scripts'],
    }, indent=1) + '\n')
    (OUT / 'items.json').write_text(json.dumps({
        'repository': REPO, 'commit': COMMIT, 'patch': PATCH, 'source': f'public/data/patch/{PATCH}/items.json',
        'sha256': hashlib.sha256(items_raw).hexdigest(),
        'items': {k: items[k] for k in ('banana', 'diddypeanuts')},
    }, indent=1) + '\n')
    parsed = rows(page)
    (OUT / 'ufd.json').write_text(json.dumps({
        'url': UFD, 'accessed': '2026-11-04', 'pageSha256': hashlib.sha256(page.encode()).hexdigest(),
        'note': 'Per-move hitbox GIFs live under https://ultimateframedata.com/hitboxes/diddy_kong/ '
                '(the character page only links generic getup/ledge/trip clips); see visuals.json.',
        'rows': parsed, 'stats': stats(page),
    }, indent=1) + '\n')
    print(f'wrote scripts.json ({len(raw)} bytes), items.json ({len(items_raw)} bytes), ufd.json ({len(parsed)} rows)')
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
