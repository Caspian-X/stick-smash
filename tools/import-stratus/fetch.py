#!/usr/bin/env python3
"""Pin the Stratus (SSBU Cloud 13.0.1) sources: the Rubendal attack scripts and the Ultimate Frame Data rows.

Run `just import-stratus` (which calls this, then import.py, then `just bless`). Re-running verifies the pinned
commit and reports any changed byte or page hash before rewriting the retained copies.
"""
import hashlib
import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/stratus'
REPOSITORY = 'https://github.com/rubendal/ssbu'
COMMIT = '2e8f0d245481cb0c936524b6ffe1cdea867c4f68'
PATCH = '13.0.1'
SCRIPT_PATH = f'public/data/patch/{PATCH}/character/Cloud/data.json'
UFD_URL = 'https://ultimateframedata.com/cloud/'
ACCESSED = '2026-11-04'


def fetch(url: str) -> bytes:
    request = urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})
    with urllib.request.urlopen(request, timeout=60) as response:
        return response.read()


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def field(block: str, name: str) -> str:
    m = re.search(rf'<div class="{name}">\s*(.*?)\s*</div>', block, re.S)
    if not m:
        return ''
    return re.sub(r'\s+', ' ', re.sub(r'<[^>]+>', ' ', m[1])).strip()


def parse_ufd(page: str) -> dict:
    rows = {}
    for block in page.split('<div class="movecontainer">')[1:]:
        name = field(block, 'movename')
        if not name:
            continue
        gif = re.search(r'hitboxes/cloud/([A-Za-z0-9_]+)\.gif', block)
        numbers = lambda key: [float(v) for v in re.findall(r'-?\d+(?:\.\d+)?', field(block, key))]
        rows[name] = {
            'startup': field(block, 'startup'),
            'total': field(block, 'totalframes'),
            'landing': field(block, 'landinglag'),
            'notes': field(block, 'notes'),
            'damage': numbers('basedamage'),
            'shieldlag': numbers('shieldlag'),
            'shieldstun': numbers('shieldstun'),
            'hits': field(block, 'whichhitbox'),
            'advantage': field(block, 'advantage'),
            'active': field(block, 'activeframes'),
            'endlag': field(block, 'endlag'),
            'gif': gif[1] if gif else None,
        }
    stats = re.search(r'Misc Info(.*?)Ledge Grab', re.sub(r'<[^>]+>', ' ', page), re.S)
    if stats:
        text = re.sub(r'\s+', ' ', stats[1])
        rows['stats'] = {'text': text.strip()}
    return rows


def main() -> int:
    SOURCES.mkdir(parents=True, exist_ok=True)
    raw = fetch(f'https://raw.githubusercontent.com/rubendal/ssbu/{COMMIT}/{SCRIPT_PATH}')
    digest = sha(raw)
    data = json.loads(raw)
    old = SOURCES / 'scripts.json'
    if old.exists():
        before = json.loads(old.read_text())
        if before.get('sha256') != digest:
            print(f'changed upstream: Cloud scripts {before.get("sha256")} -> {digest}', file=sys.stderr)
    (old).write_text(json.dumps({
        'repository': REPOSITORY,
        'commit': COMMIT,
        'patch': PATCH,
        'source': SCRIPT_PATH,
        'sha256': digest,
        'articles': data['Articles'],
        'scripts': data['Scripts'],
        'motions': data['Motions'],
    }, indent=1) + '\n')

    page = fetch(UFD_URL).decode('utf-8', 'replace')
    page_digest = sha(page.encode())
    previous = SOURCES / 'ufd.json'
    if previous.exists():
        before = json.loads(previous.read_text()).get('pageSha256')
        if before != page_digest:
            print(f'changed upstream: UFD page {before} -> {page_digest}', file=sys.stderr)
    previous.write_text(json.dumps({
        'url': UFD_URL,
        'accessed': ACCESSED,
        'pageSha256': page_digest,
        'note': 'Extracted move rows. Per-move hitbox GIFs live under '
                'https://ultimateframedata.com/hitboxes/cloud/<Name>.gif; the character page only links the '
                'generic getup/ledge/trip clips. `just stratus-visuals` downloads and hashes the 41 move GIFs.',
        'rows': parse_ufd(page),
    }, indent=1) + '\n')
    print(f'pinned Cloud scripts sha256 {digest} and UFD page sha256 {page_digest}')
    return 0


if __name__ == '__main__':
    sys.exit(main())
