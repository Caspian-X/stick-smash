#!/usr/bin/env python3
"""Fetch and pin the Snake (SSBU 13.0.1) sources the Sneak importer reads.

Writes `docs/imports/sources/sneak/scripts.json` (Rubendal attack scripts) and `ufd.json` (the parsed Ultimate
Frame Data rows plus the raw page hash). Both files are retained; `import.py` never touches the network.

    python3 tools/import-sneak/fetch.py
"""
import hashlib
import html
import json
import re
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/sneak'
SCRIPTS = ('https://raw.githubusercontent.com/rubendal/ssbu/'
           '2e8f0d245481cb0c936524b6ffe1cdea867c4f68/public/data/patch/13.0.1/character/Snake/data.json')
REPOSITORY = 'https://github.com/rubendal/ssbu'
COMMIT = '2e8f0d245481cb0c936524b6ffe1cdea867c4f68'
UFD = 'https://ultimateframedata.com/snake/'

# UFD move-container title -> the ledger key the importer uses.
KEYS = {
    'Jab 1': 'jab1', 'Jab 2': 'jab2', 'Jab 3': 'jab3',
    'Forward Tilt 1': 'ftilt1', 'Forward Tilt 2': 'ftilt2', 'Up Tilt': 'utilt', 'Down Tilt': 'dtilt',
    'Dash Attack': 'dashatk', 'Forward Smash': 'fsmash', 'Up Smash': 'usmash', 'Down Smash': 'dsmash',
    'Neutral Air': 'nair', 'Forward Air': 'fair', 'Back Air': 'bair', 'Up Air': 'uair', 'Down Air': 'dair',
    'Neutral B (Grenade)': 'grenadePull', 'Grenade (Neutral Throw, Smash Throw, Underhand Throw)': 'grenadeThrows',
    'Side B (Nikita)': 'nikita', 'Up B (Cypher)': 'cypher', 'Down B (C4)': 'c4', 'C4 Detonate/Explosion': 'c4Blast',
    'Grab': 'grab', 'Dash Grab': 'dashgrab', 'Pivot Grab': 'pivotgrab', 'Pummel': 'pummel',
    'Forward Throw': 'fthrow', 'Backward Throw': 'bthrow', 'Up Throw': 'uthrow', 'Down Throw': 'dthrow',
}


def get(url: str) -> bytes:
    return urllib.request.urlopen(urllib.request.Request(url, headers={'User-Agent': 'Mozilla/5.0'})).read()


def number(text):
    try:
        return float(text)
    except (TypeError, ValueError):
        return text


def rows(page: str) -> dict:
    out = {}
    for block in re.split(r'<div class="movecontainer', page)[1:]:
        block = block.split('<div class="movecontainer')[0]

        def field(name):
            m = re.search(r'<div class="%s">(.*?)</div>' % name, block, re.S)
            return re.sub(r'\s+', ' ', html.unescape(re.sub('<[^>]+>', '', m.group(1)))).strip() if m else None

        name = field('movename')
        if name not in KEYS:
            continue
        gifs = sorted(set(re.findall(r'hitboxes/snake/([^"]+\.(?:gif|png))', block)))
        out[KEYS[name]] = {
            'name': name,
            'startup': field('startup'),
            'active': field('activeframes'),
            'total': field('totalframes'),
            'landing': field('landinglag'),
            'damage': field('basedamage'),
            'hitbox': field('whichhitbox'),
            'shieldlag': field('shieldlag'),
            'shieldstun': field('shieldstun'),
            'advantage': field('advantage'),
            'notes': field('notes'),
            'hitboxFiles': gifs,
        }
    return out


def stats(page: str) -> dict:
    i = page.find('id="misc"')
    text = re.sub(r'\s+', ' ', html.unescape(re.sub('<[^>]+>', ' ', page[i:i + 4000])))
    out = {}
    for label, key in [('Weight', 'weight'), ('Gravity', 'gravity'), ('Walk Speed', 'walkSpeed'),
                       ('Run Speed', 'runSpeed'), ('Initial Dash', 'dashSpeed'), ('Air Speed', 'airSpeed'),
                       ('Total Air Acceleration', 'airAccel'), ('Fall Speed / Fast Fall Speed', 'fallSpeed')]:
        m = re.search(re.escape(label) + r'\s*[—-]\s*([0-9./ ]+)', text)
        if m:
            out[key] = [number(v) for v in m.group(1).split('/') if v.strip()]
    m = re.search(r'SH / FH / SHFF / FHFF Frames\s*[—-]\s*([0-9/ ]+)', text)
    if m:
        out['jumpFrames'] = [int(v) for v in m.group(1).split('/') if v.strip()]
    return out


def main() -> int:
    SOURCES.mkdir(parents=True, exist_ok=True)
    data = get(SCRIPTS)
    page = get(UFD).decode('utf-8')
    parsed = json.loads(data)
    scripts = {
        'repository': REPOSITORY, 'commit': COMMIT, 'patch': '13.0.1', 'source': SCRIPTS,
        'sha256': hashlib.sha256(data).hexdigest(),
        'scripts': {k: v for k, v in parsed['Scripts'].items()},
    }
    (SOURCES / 'scripts.json').write_text(json.dumps(scripts, indent=1) + '\n')
    ufd = {
        'url': UFD, 'accessed': '2026-11-04', 'pageSha256': hashlib.sha256(page.encode()).hexdigest(),
        'note': 'Per-move hitbox visualisations live under `https://ultimateframedata.com/hitboxes/snake/<File>`; '
                'normal attacks are `.gif`, the grenade, Cypher, C4, mortar and Nikita hitboxes are single-frame '
                '`.png` files. Spaces in those names are URL-encoded; `just sneak-visuals` reacquires and hashes them.',
        'rows': rows(page), 'stats': stats(page),
    }
    (SOURCES / 'ufd.json').write_text(json.dumps(ufd, indent=1) + '\n')
    print(f'wrote {SOURCES.relative_to(ROOT)}/scripts.json ({len(scripts["scripts"])} articles) '
          f'and ufd.json ({len(ufd["rows"])} rows)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
