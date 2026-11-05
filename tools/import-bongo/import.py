#!/usr/bin/env python3
"""Rebuild Bongo Boy (SSBU Diddy Kong) from pinned 13.0.1 scripts and documented authoring rules.

Damage, launch fields, masks, hitlag and the gadget scripts come from the pinned Rubendal fighter and item
data. Startup/active/total windows, autocancels, landing lag, charge boundaries and movement stats come from
the pinned Ultimate Frame Data rows. Geometry is fitted to the pinned UFD per-move hitbox GIFs and the
stick-figure poses authored below; it is not a projection of Nintendo's skeleton. Run `just import-bongo`.
"""
import copy
import json
import math
import re
from pathlib import Path

# Phase drift (`airAccelMult`) is written as a multiple of the old shared air accels (aerial 0.45, attack 0.25,
# upSpecial 0.3). Under `movement.ultimate` it multiplies airAccel × the kind's multiplier instead, so drift()
# stores the value that keeps the same effective acceleration.
AIR_ACCEL = 0.30
def drift(legacy, kind):
    old = {'aerial': 0.45, 'attack': 0.25, 'upSpecial': 0.3}[kind]
    mult = {'aerial': 1.0, 'attack': 0.55, 'upSpecial': 0.65}[kind]
    return round(legacy * old / (AIR_ACCEL * mult), 4)

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/bongo'
source = json.loads((SOURCES / 'scripts.json').read_text())
items = json.loads((SOURCES / 'items.json').read_text())
ufd = json.loads((SOURCES / 'ufd.json').read_text())
scripts = {art: {s['Name']: s['Data'] for s in ss} for art, ss in source['scripts'].items()}
ledger = {'sourceCommit': source['commit'], 'sourceSha256': source['sha256'], 'itemsSha256': items['sha256'],
          'ufdPageSha256': ufd['pageSha256'], 'geometryScale': 5, 'rawAttacks': {}, 'rawItems': {}}


# ---------------------------------------------------------------- script parsing
def expand(s):
    while (m := re.search(r'for\((\d+) Iterations\)\{', s)):
        pos = m.end()
        depth = 1
        while depth:
            depth += (s[pos] == '{') - (s[pos] == '}')
            pos += 1
        s = s[:m.start()] + expand(s[m.end():pos - 1]) * int(m[1]) + s[pos:]
    return s


def args(s):
    out = {}
    for part in re.split(r',(?![^()]*\))', s):
        if '=' in part:
            k, v = part.split('=', 1)
            out[k.strip()] = v.strip()
    return out


def attacks(article, name):
    src = expand(scripts[article][name])
    t, live, out = 0, {}, []

    def clear(i):
        if i in live:
            b = live.pop(i)
            b['end'] = t
            if b['end'] >= b['start']:
                out.append(b)

    for raw in src.split('\n'):
        line = raw.strip()
        if m := re.search(r'frame\(Frame=(\d+)\)', line):
            t = int(m[1])
        elif m := re.search(r'frame\((\d+), (\d+)\)', line):
            t = int(m[1])
        if m := re.search(r'wait\(Frames=(\d+)\)', line):
            t += int(m[1])
        if 'AttackModule::clear_all()' in line:
            for i in list(live):
                clear(i)
        elif m := re.search(r'AttackModule::clear\(ID=(\d+)\)', line):
            clear(int(m[1]))
        if m := re.search(r'\bATTACK\((.*)\)', line):
            a = args(m[1])
            i = int(a['ID'])
            clear(i)
            live[i] = {'start': t, 'end': t + 1, 'source': a}
    for i in list(live):
        clear(i)
    return sorted(out, key=lambda b: (b['start'], int(b['source']['ID'])))


def raw(slot, article, name):
    rows = attacks(article, name)
    ledger['rawAttacks'][slot] = {'article': article, 'script': name, 'attacks': rows}
    return rows


def pick(rows, damage, nth=0, bone=None):
    found = [b for b in rows if abs(float(b['source']['Damage']) - damage) < 1e-6
             and (bone is None or bone in b['source'].get('Bone', ''))]
    return found[nth]


def num(a, k, default=0):
    try:
        return float(a.get(k, default))
    except ValueError:
        return default


# ---------------------------------------------------------------- conversion
SCALE = 5.0


def frame(source_frame):
    """Source frame -> phase frame. The move's first tick is source frame 1 = phase frame 0."""
    return max(0, int(source_frame) - 1)


def hit(a, start, end, x, y, r, **kw):
    angle = num(a, 'Angle')
    kind = 'normal'
    if angle == 361:
        kind, angle = 'sakurai', 45
    elif angle in (366, 367, 368):
        kind, angle = 'autolink', 90
    elif angle > 180:
        angle -= 360
        kind = 'spike' if angle <= -60 else 'normal'
    b = {'start': start, 'end': end, 'x': round(x, 2), 'y': round(y, 2), 'r': round(r, 2),
         'damage': num(a, 'Damage'), 'bkb': num(a, 'BKB'), 'kbg': round(num(a, 'KBG') / 100, 4),
         'angle': angle}
    if kind != 'normal':
        b['type'] = kind
    face = a.get('FacingRestrict', '')
    if face.endswith('_B'):
        b['dir'] = 'back'
    elif face.endswith('_POS'):
        b['dir'] = 'away'
    mask = a.get('Ground_or_Air', '')
    if mask.endswith('_G'):
        b['situation'] = 'ground'
    elif mask.endswith('_A'):
        b['situation'] = 'air'
    if num(a, 'FKB'):
        b['setKb'] = True
        b['bkb'] = round(((1 + num(a, 'FKB') / 2) * 1.4 + 18) * num(a, 'KBG') / 100 + num(a, 'BKB'), 4)
    if num(a, 'Rehit'):
        b['rehit'] = int(num(a, 'Rehit'))
    if num(a, 'Hitlag', 1) != 1:
        b['hitlagMult'] = num(a, 'Hitlag', 1)
    if num(a, 'ShieldDamage') and b['damage'] > 0:
        b['shieldDmgMult'] = round(max(0, (b['damage'] + num(a, 'ShieldDamage')) / b['damage']), 4)
    b.update(kw)
    return b


def root(a, start, end, **kw):
    """Root-bone (`top`) attack: source Z is forward and Y is up, 5 px per unit. Capsules become a line of
    overlapping circles no farther apart than their radius."""
    x, y, r = num(a, 'Z') * SCALE, num(a, 'Y') * SCALE, num(a, 'Size') * SCALE
    if a.get('Z2', 'LUA_VOID') == 'LUA_VOID':
        return [hit(a, start, end, x, y, r, **kw)]
    x2, y2 = num(a, 'Z2') * SCALE, num(a, 'Y2') * SCALE
    steps = max(1, math.ceil(math.hypot(x2 - x, y2 - y) / max(r, 1)))
    return [hit(a, start, end, x + (x2 - x) * i / steps, y + (y2 - y) * i / steps, r, **kw) for i in range(steps + 1)]


def swept(a, pts, r, **kw):
    """Fitted per-frame track: `pts` is `[(phase frame, x, y)]` over the active window."""
    b = hit(a, pts[0][0], pts[-1][0], pts[0][1], pts[0][2], r, **kw)
    b['frames'] = [[t, round(px, 2), round(py, 2), round(r, 2)] for t, px, py in pts]
    return b


def assign_groups(boxes):
    """Cluster inclusive source creation/clear windows; engine ends exclude the clear tick.
    Keep identities assigned before a timeline is split into phases.
    """
    boxes = copy.deepcopy(list(boxes))
    if not all('_group' in b for b in boxes):
        group, end, rehits = -1, -1, set()
        for b in sorted(boxes, key=lambda b: b['start']):
            rehit = b.get('rehit', 0)
            if b['start'] > end or b['start'] == end and any(rehits) and rehit not in rehits:
                group += 1
                rehits = set()
            rehits.add(rehit)
            end = max(end, (b['end'] + 1 if b.get('end') is not None else float('inf')))
            b['_group'] = group
    for b in boxes:
        b.pop('group', None)
        if b['_group']:
            b['group'] = b['_group']
    return boxes


def phase(name, frames, boxes=(), **kw):
    p = {'name': name, 'frames': frames}
    if boxes:
        p['hitboxes'] = [{k: v for k, v in b.items() if k != '_group'} for b in assign_groups(boxes)]
    p.update(kw)
    return p


def land(lag):
    return {'on': 'land', 'to': 'landing', 'lag': lag}


def wait(to):
    return {'on': 'timeout', 'to': to}


def swing(pose, start, end, total, air=False, before=3):
    p = 'air' if air else 'stand'
    return [{'at': 0, 'pose': p}] + \
        [{'at': max(0, start - before), 'pose': pose, 'lerp': True}, {'at': end, 'pose': pose}] + \
        [{'at': min(max(1, total - 1), end + 6), 'pose': p, 'lerp': True}]


def aerial(slot, total, landing, ac_start, ac_end, boxes, pose, attach=None, air_pose='air'):
    """Split an aerial into start (initial autocancel), main (landing lag) and late (ending autocancel)."""
    start, late = frame(ac_start), frame(ac_end)
    boxes = assign_groups(boxes)
    spans = [(0, start, 'startup'), (start, late, 'main'), (late, total, 'late')]
    parts = []
    for lo, hi, tag in spans:
        if hi <= lo:
            continue
        cut = []
        for b in boxes:
            b = copy.deepcopy(b)
            s, e = b['start'], b.get('end', total)
            if s >= hi or e < lo:
                continue
            b['start'], b['end'] = max(lo, s) - lo, min(hi - 1, e) - lo
            b['frames'] = [[t - lo, x, y, r] for t, x, y, r in b.get('frames', []) if lo <= t < hi]
            if not b['frames']:
                b.pop('frames', None)
            cut.append(b)
        ph = phase(tag, hi - lo, cut)
        ph['transitions'] = [land(landing) if tag == 'main' else {'on': 'land', 'to': 'end'}]
        ph['anim'] = [{'at': 0, 'pose': air_pose}]
        if cut:
            lo_hit = min(b['start'] for b in cut)
            hi_hit = max(b.get('end', 0) for b in cut)
            ph['anim'] += [{'at': max(0, lo_hit - 2), 'pose': pose, 'lerp': True},
                           {'at': max(1, hi_hit), 'pose': pose},
                           {'at': min(hi - lo - 1, hi_hit + 3), 'pose': air_pose, 'lerp': True}]
        if attach:
            ph['attach'] = copy.deepcopy(attach)
        parts.append(ph)
    for i in range(len(parts) - 1):
        parts[i]['transitions'].append({**wait(parts[i + 1]['name']), 'preserveHits': True})
    return parts


# ---------------------------------------------------------------- poses and body
stand = {'head': [3, 60], 'shoulder': [0, 47], 'hip': [0, 27], 'elbowF': [7, 36], 'handF': [13, 26],
         'elbowB': [-7, 36], 'handB': [-13, 26], 'kneeF': [6, 13], 'footF': [8, 0], 'kneeB': [-6, 13],
         'footB': [-9, 0]}
poses = {
    'stand': stand,
    'crouch': {'head': [4, 44], 'shoulder': [2, 33], 'hip': [0, 16], 'elbowF': [9, 25], 'handF': [15, 16],
               'elbowB': [-8, 25], 'handB': [-13, 16], 'kneeF': [10, 10], 'footF': [14, 0], 'kneeB': [-8, 9],
               'footB': [-11, 0]},
    'air': {'head': [3, 58], 'shoulder': [0, 46], 'hip': [0, 27], 'elbowF': [10, 50], 'handF': [18, 58],
            'elbowB': [-10, 50], 'handB': [-18, 56], 'kneeF': [8, 20], 'footF': [12, 10], 'kneeB': [-6, 18],
            'footB': [-10, 8]},
    'airTuck': {'from': 'air', 'kneeF': [12, 26], 'footF': [18, 22], 'kneeB': [-4, 24], 'footB': [-8, 18]},
    'airKick': {'from': 'air', 'kneeF': [18, 17], 'footF': [36, 12], 'kneeB': [-6, 20], 'footB': [-12, 4]},
    'airBack': {'from': 'air', 'kneeF': [-14, 17], 'footF': [-32, 14], 'kneeB': [-4, 20], 'footB': [-10, 6]},
    'airUp': {'from': 'air', 'kneeF': [6, 30], 'footF': [10, 48], 'kneeB': [-6, 20], 'footB': [-12, 8]},
    'airLow': {'from': 'air', 'elbowF': [8, 40], 'handF': [12, 28], 'elbowB': [-8, 40], 'handB': [-12, 28],
               'kneeF': [6, 20], 'footF': [8, 6], 'kneeB': [-6, 18], 'footB': [-9, 4]},
    'punch': {'head': [5, 58], 'shoulder': [3, 45], 'hip': [0, 27], 'elbowF': [18, 44], 'handF': [34, 44],
              'elbowB': [14, 42], 'handB': [30, 42], 'kneeF': [10, 13], 'footF': [16, 0], 'kneeB': [-6, 13],
              'footB': [-10, 0]},
    'doubleArm': {'from': 'punch', 'handF': [38, 40], 'handB': [34, 38]},
    'clap': {'head': [6, 42], 'shoulder': [3, 32], 'hip': [0, 16], 'elbowF': [16, 26], 'handF': [34, 16],
             'elbowB': [12, 24], 'handB': [30, 12], 'kneeF': [12, 10], 'footF': [18, 0], 'kneeB': [-8, 9],
             'footB': [-13, 0]},
    'overhead': {'head': [1, 62], 'shoulder': [0, 49], 'hip': [0, 28], 'elbowF': [8, 58], 'handF': [12, 74],
                 'elbowB': [-8, 54], 'handB': [-12, 68], 'kneeF': [5, 14], 'footF': [7, 0], 'kneeB': [-6, 14],
                 'footB': [-9, 0]},
    'spinLow': {'head': [8, 40], 'shoulder': [4, 32], 'hip': [0, 17], 'elbowF': [18, 28], 'handF': [34, 18],
                'elbowB': [-12, 26], 'handB': [-26, 14], 'kneeF': [16, 10], 'footF': [30, 4], 'kneeB': [-14, 8],
                'footB': [-28, 2]},
    'spinHigh': {'head': [0, 62], 'shoulder': [0, 48], 'hip': [0, 28], 'elbowF': [12, 58], 'handF': [18, 76],
                 'elbowB': [-12, 54], 'handB': [-18, 68], 'kneeF': [10, 16], 'footF': [14, 4], 'kneeB': [-8, 14],
                 'footB': [-14, 2]},
    'charge': {'head': [2, 50], 'shoulder': [0, 38], 'hip': [0, 20], 'elbowF': [12, 30], 'handF': [20, 20],
               'elbowB': [-12, 30], 'handB': [-20, 20], 'kneeF': [12, 11], 'footF': [18, 0], 'kneeB': [-8, 10],
               'footB': [-14, 0]},
    'gun': {'head': [4, 58], 'shoulder': [2, 46], 'hip': [0, 27], 'elbowF': [16, 44], 'handF': [30, 46],
            'elbowB': [-10, 42], 'handB': [-16, 36], 'kneeF': [8, 13], 'footF': [12, 0], 'kneeB': [-6, 13],
            'footB': [-10, 0]},
    'flip': {'head': [0, 56], 'shoulder': [0, 44], 'hip': [-2, 26], 'elbowF': [12, 52], 'handF': [20, 64],
             'elbowB': [-12, 50], 'handB': [-20, 60], 'kneeF': [14, 20], 'footF': [22, 18], 'kneeB': [-8, 16],
             'footB': [-14, 10], 'rot': -18, 'pivot': 34},
    'flipTight': {'from': 'flip', 'rot': 160, 'offsetY': -20, 'kneeF': [16, 28], 'footF': [10, 22],
                  'kneeB': [-10, 24], 'footB': [-16, 18]},
    'kick': {'head': [-2, 58], 'shoulder': [0, 46], 'hip': [0, 27], 'elbowF': [10, 50], 'handF': [16, 60],
             'elbowB': [-10, 48], 'handB': [-16, 56], 'kneeF': [18, 20], 'footF': [38, 18], 'kneeB': [-6, 16],
             'footB': [-12, 6]},
    'throwBack': {'head': [-4, 58], 'shoulder': [-2, 46], 'hip': [0, 27], 'elbowF': [-8, 44], 'handF': [-20, 50],
                  'elbowB': [-10, 40], 'handB': [-24, 34], 'kneeF': [8, 13], 'footF': [12, 0], 'kneeB': [-8, 13],
                  'footB': [-14, 0]},
    'throwF': {'head': [6, 58], 'shoulder': [4, 46], 'hip': [0, 27], 'elbowF': [18, 48], 'handF': [36, 48],
               'elbowB': [-8, 44], 'handB': [-14, 36], 'kneeF': [12, 13], 'footF': [20, 0], 'kneeB': [-8, 13],
               'footB': [-14, 0]},
    'throwU': {'head': [0, 62], 'shoulder': [0, 49], 'hip': [0, 28], 'elbowF': [8, 58], 'handF': [12, 76],
               'elbowB': [-8, 56], 'handB': [-12, 72], 'kneeF': [6, 14], 'footF': [8, 0], 'kneeB': [-6, 14],
               'footB': [-9, 0]},
    'throwD': {'head': [8, 42], 'shoulder': [4, 32], 'hip': [0, 16], 'elbowF': [16, 24], 'handF': [32, 10],
               'elbowB': [-8, 24], 'handB': [-16, 10], 'kneeF': [14, 10], 'footF': [22, 0], 'kneeB': [-8, 9],
               'footB': [-14, 0]},
    'latch': {'head': [6, 58], 'shoulder': [4, 46], 'hip': [2, 27], 'elbowF': [16, 52], 'handF': [30, 58],
              'elbowB': [12, 48], 'handB': [24, 52], 'kneeF': [8, 13], 'footF': [12, 4], 'kneeB': [-6, 12],
              'footB': [-10, 2]},
    'hurt': {'head': [-8, 58], 'shoulder': [-4, 45], 'hip': [0, 27], 'elbowF': [8, 52], 'handF': [14, 64],
             'elbowB': [-12, 50], 'handB': [-20, 60], 'kneeF': [8, 14], 'footF': [14, 4], 'kneeB': [-6, 14],
             'footB': [-12, 2]},
    'stun': {'head': [10, 36], 'shoulder': [5, 28], 'hip': [0, 14], 'elbowF': [14, 20], 'handF': [26, 14],
             'elbowB': [-8, 20], 'handB': [-20, 12], 'kneeF': [16, 8], 'footF': [28, 2], 'kneeB': [-10, 7],
             'footB': [-20, 1]},
    'win': {'head': [0, 62], 'shoulder': [0, 49], 'hip': [0, 28], 'elbowF': [10, 58], 'handF': [16, 76],
            'elbowB': [-10, 56], 'handB': [-16, 74], 'kneeF': [8, 14], 'footF': [12, 0], 'kneeB': [-8, 14],
            'footB': [-14, 0]},
}

shapes = list


def line(a, b, w, c, **kw):
    return {'shape': 'line', 'from': list(a), 'to': list(b), 'width': w, 'color': c, **kw}


def poly(points, c, **kw):
    s = {'shape': 'poly', 'points': [list(p) for p in points]}
    if c:
        s['fill'] = c
    s.update(kw)
    return s


def circle(at, r, fill=None, **kw):
    s = {'shape': 'circle', 'at': list(at), 'r': r}
    if fill:
        s['fill'] = fill
    s.update(kw)
    return s


BROWN = '#8a5a32'
BROWN_DARK = '#5c3a1e'
RED = '#c0392b'
RED_DARK = '#82251b'
CREAM = '#e8d5a7'

attachments = [
    {'id': 'tail', 'joint': 'hip', 'angle': 180, 'behind': True,
     'sway': {'deg': 8, 'period': 46},
     'shapes': [
         line((0, 0), (16, -3), 6, BROWN),
         line((16, -3), (25, -12), 5, BROWN),
         line((25, -12), (28, -23), 4, BROWN_DARK),
         circle((28, -23), 3.2, BROWN_DARK),
     ]},
    {'id': 'cap', 'joint': 'head', 'angle': 0, 'shapes': [
        poly(((-10, 5), (-8, 12), (2, 14), (9, 11), (10, 5)), RED),
        line((-11, 5), (11, 5), 4, RED_DARK),
        line((7, 5), (21, 3), 3.6, RED_DARK),
        circle((2, 10), 2.2, CREAM, alpha=0.9),
    ]},
    {'id': 'popgun', 'joint': 'handF', 'angle': 0, 'hidden': True,
     'hold': {'elbow': 16, 'hand': 30, 'bend': 8},
     'shapes': [
         line((-6, 0), (24, 0), 7, '#6b4a2b'),
         line((18, -5), (18, 5), 6, '#8f6a3e'),
         line((22, -4), (22, 4), 3, '#d9c28c'),
         circle((26, 0), 3.5, '#d9c28c'),
         line((-6, 0), (-12, -5), 5, '#4a3320'),
         line((-2, 3), (20, 3), 1.4, '#a97a48', alpha=0.8),
     ]},
    {'id': 'barrels', 'joint': 'shoulder', 'angle': 0, 'behind': True, 'hidden': True, 'glow': 'active',
     'shapes': [
         poly(((-22, 4), (-12, 0), (-12, 18), (-22, 22)), '#7a5a34'),
         poly(((-8, 4), (2, 0), (2, 18), (-8, 22)), '#7a5a34'),
         line((-22, 10), (-12, 10), 2.4, '#4c351d'),
         line((-8, 10), (2, 10), 2.4, '#4c351d'),
         line((-22, 14), (-12, 14), 1.4, '#b08a58'),
         line((-8, 14), (2, 14), 1.4, '#b08a58'),
         circle((-2, 2), 4, '#ff9a3c', when='glow'),
         circle((-20, 2), 4, '#ff9a3c', when='glow'),
     ]},
]

style = {'headRadius': 11.5, 'torsoWidth': 7, 'frontLimbWidth': 6, 'backLimbWidth': 5.5, 'glowWidth': 2.4,
         'torsoGlowWidth': 3, 'headGlowRadius': 2.8, 'headRing': 2.2, 'headFill': '#3a2417',
         'eye': [4, -2, 4, 4]}

anims = {
    'idle': {'clock': 'world', 'loop': 48, 'keys': [
        {'at': 0, 'pose': 'stand'},
        {'at': 12, 'pose': {'from': 'stand', 'head': [3, 62], 'handF': [14, 28], 'handB': [-14, 28]},
         'lerp': True},
        {'at': 24, 'pose': 'stand', 'lerp': True},
        {'at': 36, 'pose': {'from': 'stand', 'head': [2, 58]}, 'lerp': True}]},
    'walk': {'clock': 'world', 'loop': 22, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'kneeF': [8, 13], 'footF': [14, 0], 'kneeB': [-8, 13],
                           'footB': [-14, 0], 'elbowF': [10, 38], 'handF': [18, 32]}},
        {'at': 11, 'pose': {'from': 'stand', 'kneeF': [-2, 13], 'footF': [-6, 0], 'kneeB': [4, 13],
                            'footB': [8, 0], 'elbowB': [-10, 38], 'handB': [-18, 32]}, 'lerp': True}]},
    'run': {'clock': 'run', 'loop': 360, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'head': [8, 56], 'hip': [2, 27], 'kneeF': [18, 14], 'footF': [30, 8],
                           'kneeB': [-14, 12], 'footB': [-22, 2], 'elbowF': [14, 42], 'handF': [26, 44],
                           'elbowB': [-14, 40], 'handB': [-26, 36]}},
        {'at': 180, 'pose': {'from': 'stand', 'head': [8, 56], 'hip': [2, 27], 'kneeF': [-14, 12],
                             'footF': [-22, 2], 'kneeB': [18, 14], 'footB': [30, 8], 'elbowF': [14, 42],
                             'handF': [26, 44], 'elbowB': [-14, 40], 'handB': [-26, 36]}, 'lerp': True}]},
    'air.rise': {'keys': [{'at': 0, 'pose': {'from': 'air', 'kneeF': [10, 22], 'footF': [16, 14]}}]},
    'air.fall': {'keys': [{'at': 0, 'pose': {'from': 'air', 'kneeF': [6, 18], 'footF': [8, 4]}}]},
    'helpless': {'keys': [{'at': 0, 'pose': {'from': 'air', 'head': [-2, 54], 'elbowF': [6, 42],
                                               'handF': [10, 30], 'elbowB': [-12, 42], 'handB': [-18, 30]}}]},
    'landing': {'keys': [{'at': 0, 'pose': {'from': 'crouch', 'head': [4, 38]}},
                         {'at': 7, 'pose': 'stand', 'lerp': True}]},
    'crouch': {'clock': 'world', 'loop': 50, 'keys': [{'at': 0, 'pose': 'crouch'},
                                                      {'at': 25, 'pose': {'from': 'crouch', 'head': [4, 46]},
                                                       'lerp': True},
                                                      {'at': 50, 'pose': 'crouch', 'lerp': True}]},
    'jumpsquat': {'keys': [{'at': 0, 'pose': {'from': 'crouch', 'head': [4, 40]}},
                           {'at': 2, 'pose': {'from': 'stand', 'head': [2, 56]}}]},
    'shield': {'keys': [{'at': 0, 'pose': {'from': 'stand', 'elbowF': [12, 42], 'handF': [4, 52],
                                           'elbowB': [10, 38], 'handB': [2, 48]}}]},
    'hitstun': {'keys': [{'at': 0, 'pose': 'hurt'}, {'at': 6, 'pose': {'from': 'hurt', 'rot': -10}},
                         {'at': 16, 'pose': 'stand', 'lerp': True}]},
    'tumble': {'clock': 'spin', 'keys': [{'at': 0, 'pose': 'air'},
                                         {'at': 360, 'pose': {'from': 'air', 'rot': 360}, 'lerp': True}]},
    'win': {'clock': 'world', 'loop': 40, 'keys': [
        {'at': 0, 'pose': 'win'},
        {'at': 10, 'pose': {'from': 'win', 'head': [0, 64], 'handF': [20, 82], 'handB': [-20, 80], 'offsetY': 4},
         'lerp': True},
        {'at': 20, 'pose': 'win', 'lerp': True},
        {'at': 30, 'pose': {'from': 'win', 'offsetY': 2}, 'lerp': True}]},
    'respawn': {'keys': [{'at': 0, 'pose': 'air'}]},
    'hold': {'keys': [{'at': 0, 'pose': 'latch'}]},
    'pummel': {'keys': [{'at': 0, 'pose': 'latch'},
                        {'at': 4, 'pose': {'from': 'latch', 'head': [8, 56]}},
                        {'at': 10, 'pose': 'latch'}]},
    'throw.f': {'keys': [{'at': 0, 'pose': 'latch'}, {'at': 8, 'pose': 'throwF'}]},
    'throw.b': {'keys': [{'at': 0, 'pose': 'latch'}, {'at': 12, 'pose': 'throwBack'}]},
    'throw.u': {'keys': [{'at': 0, 'pose': 'latch'}, {'at': 8, 'pose': 'throwU'}]},
    'throw.d': {'keys': [{'at': 0, 'pose': 'latch'}, {'at': 10, 'pose': 'throwD'}]},
    'grabbed': {'clock': 'world', 'loop': 12, 'keys': [
        {'at': 0, 'pose': {'from': 'hurt', 'head': [-8, 56]}},
        {'at': 6, 'pose': {'from': 'hurt', 'head': [-6, 58], 'handF': [10, 58]}, 'lerp': True},
        {'at': 12, 'pose': {'from': 'hurt', 'head': [-8, 56]}, 'lerp': True}]},
}

# ---------------------------------------------------------------- moves
moves = {}

# --- jab: two inward slaps and an outward sidekick; the source's held-hit loop is folded into the chain
j1 = raw('jab', 'diddy', 'Attack11')
jab1 = [hit(pick(j1, 2.0, n)['source'], frame(3), frame(3), x, 27, r)
        for n, (x, r) in enumerate([(20, 9), (35, 9), (50, 10)])]
j2 = raw('jab2', 'diddy', 'Attack12')
jab2 = [hit(pick(j2, 1.5, n)['source'], frame(5), frame(5), x, 27, r)
        for n, (x, r) in enumerate([(20, 11.5), (35, 13), (56, 14)])]
j3 = raw('jab3', 'diddy', 'Attack13')
jab3 = [hit(pick(j3, 4.0, n)['source'], frame(5), frame(5), x, y, r)
        for n, (x, y, r) in enumerate([(20, 30, 13), (37, 31, 15), (60, 34, 19)])]
moves['jab'] = {'name': 'Flap Flap Kick', 'phases': [
    phase('jab1', 19, jab1, anim=swing('punch', 2, 2, 19),
          transitions=[{'on': 'attackHeld', 'from': frame(6), 'to': 'jab2'}, wait('end')]),
    phase('jab2', 20, jab2, anim=swing('punch', 4, 4, 20),
          transitions=[{'on': 'attackHeld', 'from': frame(6), 'to': 'jab3'}, wait('end')]),
    phase('jab3', 34, jab3, anim=swing('kick', 4, 4, 34), events=[{'at': 0, 'sfx': 'swing'}]),
]}

# --- tilts
ft = raw('ftilt', 'diddy', 'AttackS3')
ftilt_clean = [hit(pick(ft, 10.0, n)['source'], frame(10), frame(11), x, 30, r)
               for n, (x, r) in enumerate([(42, 19), (50, 19), (36, 20)])]
ftilt_late = [hit(pick(ft, 10.0, n)['source'], frame(12), frame(16), x, 29, r, damage=7.0)
              for n, (x, r) in enumerate([(38, 16), (44, 16), (34, 17)])]
moves['ftilt'] = {'name': 'Double Arm', 'phases': [phase(
    'main', 36, ftilt_clean + ftilt_late, anim=swing('doubleArm', 9, 10, 36),
    events=[{'at': frame(10), 'sfx': 'swing'}])]}
ut = raw('utilt', 'diddy', 'AttackHi3')
utilt = [swept(pick(ut, 6.0, n)['source'], [(frame(6), 12 + n * 6, 60 + n * 8), (frame(9), 16 + n * 4, 78 + n * 8),
                                            (frame(11), 6, 96 + n * 6)], 18 - n * 2) for n in range(2)]
utilt.append(hit(pick(ut, 6.0, 2)['source'], frame(6), frame(11), 8, 62, 17))
moves['utilt'] = {'name': 'Overhead Slap', 'phases': [phase(
    'main', 29, utilt, anim=swing('overhead', 5, 10, 29), events=[{'at': frame(6), 'sfx': 'swing'}])]}
dt = raw('dtilt', 'diddy', 'AttackLw3')
moves['dtilt'] = {'name': 'Hand Clap', 'phases': [phase(
    'main', 18, [hit(pick(dt, 5.5, 0)['source'], frame(4), frame(5), 66, 12, 18),
                 hit(pick(dt, 5.5, 1)['source'], frame(4), frame(5), 42, 17, 17)],
    anim=swing('clap', 3, 4, 18), events=[{'at': frame(4), 'sfx': 'swing'}])]}
da = raw('dashatk', 'diddy', 'AttackDash')
moves['dashatk'] = {'name': 'Rolling Slap', 'phases': [phase(
    'main', 40, [hit(pick(da, 2.0, 2)['source'], frame(8), frame(9), 38, 46, 18),
                 hit(pick(da, 2.0, 5)['source'], frame(16), frame(17), 38, 47, 18),
                 hit(pick(da, 3.0)['source'], frame(22), frame(23), 40, 44, 26)],
    slide=20, anim=[{'at': 0, 'pose': 'stand'}, {'at': frame(7), 'pose': 'spinLow', 'lerp': True},
                    {'at': frame(16), 'pose': {'from': 'spinLow', 'rot': 180}, 'lerp': True},
                    {'at': frame(24), 'pose': 'spinLow'}, {'at': 39, 'pose': 'stand', 'lerp': True}],
    events=[{'at': frame(8), 'sfx': 'swing'}, {'at': 0, 'setVel': {'x': 4.2}}])]}

# --- smashes
fs = raw('fsmash', 'diddy', 'AttackS4')
fsmash = [
    hit(pick(fs, 5.0, 1)['source'], frame(12), frame(13), 44, 48, 23),
    hit(pick(fs, 11.0, 0)['source'], frame(21), frame(23), 42, 50, 22),
    hit(pick(fs, 9.0)['source'], frame(21), frame(23), 16, 50, 17),
]
moves['fsmash'] = {'name': 'Slap & Beat', 'phases': [phase(
    'main', 50, fsmash, charge={'frame': frame(4)}, slide=30, anim=[{'at': 0, 'pose': 'stand'},
                                                          {'at': frame(4), 'pose': 'charge'},
                                                          {'at': frame(11), 'pose': 'spinLow', 'lerp': True},
                                                          {'at': frame(21), 'pose': 'spinLow'},
                                                          {'at': frame(24), 'pose': 'spinHigh', 'lerp': True},
                                                          {'at': frame(35), 'pose': 'stand', 'lerp': True}],
    events=[{'at': frame(12), 'sfx': 'swing', 'setVel': {'x': 6.5}},
            {'at': frame(21), 'sfx': 'smash', 'setVel': {'x': 4.0}}])]}
us = raw('usmash', 'diddy', 'AttackHi4')
usmash = [
    hit(pick(us, 2.5, 2)['source'], frame(5), frame(8), 28, 38, 22),
    hit(pick(us, 2.5, 8)['source'], frame(12), frame(15), 24, 45, 23),
    hit(pick(us, 6.0)['source'], frame(19), frame(23), 18, 65, 28),
]
moves['usmash'] = {'name': 'Triple Attack', 'phases': [phase(
    'main', 53, usmash, charge={'frame': frame(1)}, anim=[{'at': 0, 'pose': 'crouch'},
                                                         {'at': frame(5), 'pose': 'spinLow', 'lerp': True},
                                                         {'at': frame(12), 'pose': {'from': 'spinLow', 'rot': 180}, 'lerp': True},
                                                         {'at': frame(19), 'pose': 'spinHigh', 'lerp': True},
                                                         {'at': frame(30), 'pose': 'stand', 'lerp': True}],
    events=[{'at': 0, 'setVel': {'y': 8.0}, 'leaveGround': True},
            {'at': frame(5), 'sfx': 'swing'}, {'at': frame(19), 'sfx': 'smash'}],
    transitions=[{'on': 'land', 'from': frame(23), 'to': 'landing', 'lag': 30}, wait('end')])]}
ds = raw('dsmash', 'diddy', 'AttackLw4')
dsmash = [
    hit(pick(ds, 12.0, 0)['source'], frame(7), frame(8), 54, 18, 20),
    hit(pick(ds, 12.0, 1)['source'], frame(7), frame(8), 36, 18, 17),
    hit(pick(ds, 15.0, 0)['source'], frame(12), frame(13), -50, 18, 20),
    hit(pick(ds, 15.0, 1)['source'], frame(12), frame(13), -32, 18, 17),
]
moves['dsmash'] = {'name': 'Turning Leg', 'phases': [phase(
    'main', 50, dsmash, charge={'frame': frame(2)}, anim=[{'at': 0, 'pose': 'stand'},
                                                         {'at': frame(2), 'pose': 'crouch'},
                                                         {'at': frame(7), 'pose': {'from': 'spinLow', 'rot': -90}, 'lerp': True},
                                                         {'at': frame(12), 'pose': {'from': 'spinLow', 'rot': 90}},
                                                         {'at': frame(20), 'pose': 'stand', 'lerp': True}],
    events=[{'at': frame(7), 'sfx': 'swing'}, {'at': frame(12), 'sfx': 'smash'}])]}

# --- aerials
an = raw('nair', 'diddy', 'AttackAirN')
nair = [swept(pick(an, 6.0, 0)['source'], [(frame(8), 18, 36), (frame(11), 4, 22), (frame(14), -14, 38),
                                          (frame(17), -4, 52), (frame(20), 12, 46)], 21),
        swept(pick(an, 6.0, 3)['source'], [(frame(8), 12, 34), (frame(11), -2, 20), (frame(14), -18, 34),
                                          (frame(17), -8, 48), (frame(20), 8, 42)], 18)]
moves['nair'] = {'name': 'Diddy Roll', 'air': 'aerial',
                 'phases': aerial('nair', 48, 8, 4, 27, nair, 'airLow')}
af = raw('fair', 'diddy', 'AttackAirF')
fair = [hit(pick(af, 10.0, 0)['source'], frame(6), frame(9), 54, 32, 24),
        hit(pick(af, 10.0, 1)['source'], frame(6), frame(9), 36, 30, 20),
        hit(pick(af, 8.0, 0)['source'], frame(10), frame(16), 60, 30, 13),
        hit(pick(af, 8.0, 1)['source'], frame(10), frame(16), 38, 28, 12)]
moves['fair'] = {'name': 'Screw Kick', 'air': 'aerial',
                 'phases': aerial('fair', 46, 16, 3, 26, fair, 'airKick')}
ab = raw('bair', 'diddy', 'AttackAirB')
bair = [hit(pick(ab, 9.0, 0)['source'], frame(5), frame(8), -50, 38, 24),
        hit(pick(ab, 9.0, 1)['source'], frame(5), frame(8), -30, 34, 20),
        # Identity addition fitted from the spin animation: the tail sweeps the space above/behind the kick.
        hit(pick(ab, 9.0, 2)['source'], frame(5), frame(8), -34, 58, 17)]
moves['bair'] = {'name': 'Quick Backspin', 'air': 'aerial',
                 'phases': aerial('bair', 30, 7, 3, 20, bair, 'airBack')}
au = raw('uair', 'diddy', 'AttackAirHi')
uair = [hit(pick(au, 7.0, 0)['source'], frame(4), frame(7), -10, 76, 24),
        hit(pick(au, 7.0, 1)['source'], frame(4), frame(7), -26, 62, 20)]
moves['uair'] = {'name': 'Turn Kick', 'air': 'aerial',
                 'phases': aerial('uair', 36, 9, 1, 27, uair, 'airUp')}
ad = raw('dair', 'diddy', 'AttackAirLw')
dair = [hit(pick(ad, 13.0)['source'], frame(15), frame(16), 0, -32, 22),
        hit(pick(ad, 10.0)['source'], frame(15), frame(16), 0, -14, 32)]
moves['dair'] = {'name': 'Hammer Meteor Knuckle', 'air': 'aerial',
                 'phases': aerial('dair', 46, 17, 15, 35, dair, 'airLow')}

# --- grabs
moves['grab'] = {'name': 'Grab', 'phases': [phase(
    'main', 36, [{'start': frame(7), 'end': frame(8), 'x': 40, 'y': 44, 'r': 20, 'damage': 0, 'bkb': 0,
                  'kbg': 0, 'angle': 0, 'type': 'grab'}],
    anim=swing('latch', 6, 7, 36))]}
moves['dashgrab'] = {'name': 'Dash Grab', 'phases': [phase(
    'main', 44, [{'start': frame(10), 'end': frame(11), 'x': 45, 'y': 44, 'r': 22, 'damage': 0, 'bkb': 0,
                  'kbg': 0, 'angle': 0, 'type': 'grab'}],
    slide=18, anim=swing('latch', 9, 10, 44))]}

# --- specials
# Neutral special: charge frame 16, max at 121, misfire at 124. The UFD total is 48 from press.
moves['nspec'] = {
    'name': 'Peanut Popgun', 'bReverse': True, 'chargeDamage': 0.0,
    'phases': [
        phase('charge', 16, anim=[{'at': 0, 'pose': 'gun'}, {'at': frame(4), 'pose': 'gun'}],
              charge={'frame': frame(16), 'max': 105, 'holdAfterMax': 3, 'button': 'special'},
              attach={'popgun': {'show': [0, 15], 'angle': [[0, 0]]}},
              events=[{'at': frame(16), 'sfx': 'charge'}],
              transitions=[{'on': 'release', 'from': frame(16), 'to': 'shot'}, wait('overcharge')]),
        phase('shot', 33, [{'start': 0, 'end': 1, 'x': 34, 'y': 44, 'r': 16, 'damage': 2.0, 'bkb': 40,
                            'kbg': 0.7, 'angle': 45, 'type': 'sakurai'},
                           ],
              anim=[{'at': 0, 'pose': 'gun'}, {'at': 6, 'pose': {'from': 'gun', 'head': [4, 60]}},
                    {'at': 18, 'pose': 'stand', 'lerp': True}],
              attach={'popgun': {'show': [0, 31], 'angle': [[0, 0]]}},
              events=[{'at': 0, 'spawn': 'peanut', 'sfx': 'shoot'}]),
        phase('overcharge', 104, [{'start': 0, 'end': 1, 'x': 0, 'y': 42, 'r': 41, 'damage': 23.0,
                                   'bkb': 30.0, 'kbg': 0.92, 'angle': 45, 'type': 'sakurai',
                                   'hitlagMult': 1.5}],
              anim=[{'at': 0, 'pose': 'gun'}, {'at': 8, 'pose': 'hurt'}, {'at': 24, 'pose': 'stun'},
                    {'at': 70, 'pose': 'crouch'}, {'at': 100, 'pose': 'stand', 'lerp': True}],
              events=[{'at': 0, 'sfx': 'quake', 'fx': [{'type': 'burst', 'color': '#ffb14a', 'n': 18,
                                                        'speed': 5.5, 'life': 22, 'size': 4.5}]},
                      {'at': 0, 'fx': [{'type': 'shake', 'k': 6}]}]),
    ]}

# Side special: leap (frames 1-17), flip grab window (18-36) and the attack-button kick branch.
moves['sspec'] = {
    'name': 'Monkey Flip', 'bReverse': True, 'oncePerAirtime': True,
    'phases': [
        phase('leap', 17, anim=[{'at': 0, 'pose': 'crouch'}, {'at': 3, 'pose': 'flip', 'lerp': True},
                                {'at': 12, 'pose': 'flip'}],
              events=[{'at': 0, 'setVel': {'x': 4.2, 'y': 13.0}, 'leaveGround': True, 'sfx': 'swing'}],
              transitions=[{'on': 'attackPress', 'to': 'kick'}, land(26), wait('flip')]),
        phase('flip', 19, [{'start': 0, 'end': 18, 'x': 30, 'y': 42, 'r': 26, 'damage': 0, 'bkb': 0, 'kbg': 0,
                            'angle': 0, 'type': 'grab'}],
              airAccelMult=drift(0.5, 'attack'), ledgeGrab=0,
              anim=[{'at': 0, 'pose': 'flipTight'}, {'at': 12, 'pose': 'flip', 'lerp': True}],
              transitions=[{'on': 'attackPress', 'to': 'kick'}, land(26), wait('recover')]),
        phase('kick', 32, [{'start': frame(6), 'end': frame(10), 'x': 36, 'y': 34, 'r': 25, 'damage': 14.0,
                            'bkb': 80.0, 'kbg': 0.5, 'angle': 45, 'type': 'sakurai'},
                           {'start': frame(11), 'end': frame(25), 'x': 34, 'y': 32, 'r': 22, 'damage': 12.0,
                            'bkb': 80.0, 'kbg': 0.5, 'angle': 45, 'type': 'sakurai'}],
              airAccelMult=drift(0.6, 'attack'), ledgeGrab=0,
              anim=[{'at': 0, 'pose': 'flipTight'}, {'at': frame(6), 'pose': 'kick', 'lerp': True},
                    {'at': frame(25), 'pose': 'kick'}, {'at': 31, 'pose': 'air', 'lerp': True}],
              events=[{'at': frame(6), 'sfx': 'slash'}],
              transitions=[land(18), wait('end')]),
        phase('recover', 11, airAccelMult=drift(0.5, 'attack'), ledgeGrab=0,
              anim=[{'at': 0, 'pose': 'flipTight'}, {'at': 8, 'pose': 'air', 'lerp': True}],
              transitions=[land(26), wait('end')]),
    ]}

# Up special: charge frames 1-8 (held to 68), then the boost and the surface/landing explosion.
moves['uspec'] = {
    'name': 'Rocketbarrel Boost', 'air': 'upSpecial', 'bReverse': True, 'chargeDamage': 0.0,
    'phases': [
        phase('charge', 8, aim=True, fallCap=2.5, airAccelMult=drift(0.25, 'upSpecial'),
              charge={'frame': frame(8), 'max': 60, 'button': 'special', 'jet': True},
              anim=[{'at': 0, 'pose': 'charge'}],
              attach={'barrels': {'show': [0, 67], 'angle': [[0, 0]]}},
              events=[{'at': frame(8), 'sfx': 'jet'}],
              transitions=[{'on': 'release', 'from': frame(8), 'to': 'boost'}, wait('boost')]),
        phase('boost', 34,
              [{'start': 0, 'end': 1, 'x': 0, 'y': 40, 'r': 20, 'damage': 10.0, 'bkb': 40.0, 'kbg': 0.85,
                'angle': 45, 'type': 'sakurai'},
               {'start': 2, 'end': 8, 'x': 0, 'y': 40, 'r': 16, 'damage': 8.0, 'bkb': 40.0, 'kbg': 0.9,
                'angle': 80},
               {'start': 9, 'end': 27, 'x': 0, 'y': 40, 'r': 14, 'damage': 6.0, 'bkb': 40.0, 'kbg': 0.9,
                'angle': 70}],
              airAccelMult=drift(0.9, 'upSpecial'), ledgeGrab=0,
              anim=[{'at': 0, 'pose': 'charge'}, {'at': 3, 'pose': 'airTuck', 'lerp': True},
                    {'at': 20, 'pose': 'air'}],
              attach={'barrels': {'show': [0, 29], 'angle': [[0, 0]]}},
              events=[{'at': 0, 'setVel': {'x': 3.2, 'y': 14.0}, 'chargeVelocity': 0.006, 'leaveGround': True,
                       'sfx': 'jet'}],
              transitions=[{'on': 'land', 'to': 'explosion'}, wait('fall')]),
        phase('explosion', 59,
              [{'start': 0, 'end': 1, 'x': 0, 'y': 40, 'r': 100, 'damage': 18.0, 'bkb': 50.0, 'kbg': 0.7,
                'angle': 70, 'hitlagMult': 1.0}],
              anim=[{'at': 0, 'pose': 'stun'}, {'at': 30, 'pose': 'crouch'}, {'at': 55, 'pose': 'stand', 'lerp': True}],
              attach={'barrels': {'show': [0, 2], 'angle': [[0, 0]]}},
              events=[{'at': 0, 'sfx': 'quake', 'fx': [{'type': 'burst', 'color': '#ff7a3c', 'n': 22,
                                                        'speed': 6.5, 'life': 24, 'size': 5.5},
                                                       {'type': 'shake', 'k': 7}]}]),
        phase('fall', 30, airAccelMult=drift(0.5, 'upSpecial'), ledgeGrab=0,
              anim=[{'at': 0, 'pose': 'air'}],
              transitions=[{'on': 'land', 'to': 'landing', 'lag': 30}, wait('helpless')]),
    ]}

# Down special: one peel at a time; the 85-tick pool is the source's regeneration cooldown.
banana_throw = phase('throw', 39,
                     anim=[{'at': 0, 'pose': 'stand'}, {'at': 3, 'pose': 'throwBack', 'lerp': True},
                           {'at': frame(14), 'pose': 'throwBack'}, {'at': frame(20), 'pose': 'stand', 'lerp': True}],
                     cost={'charge': 'banana', 'uses': 1},
                     events=[{'at': frame(20), 'spawn': 'banana', 'sfx': 'swing'}])
moves['dspec'] = {'name': 'Banana Peel', 'bReverse': True, 'variants': [
    {'needs': {'charge': 'banana', 'uses': 1}, 'phases': [banana_throw]}],
    'phases': [phase('empty', 39,
                     anim=[{'at': 0, 'pose': 'stand'}, {'at': 3, 'pose': 'throwBack', 'lerp': True},
                           {'at': frame(20), 'pose': {'from': 'throwBack', 'head': [-2, 58]}},
                           {'at': frame(28), 'pose': 'stand', 'lerp': True}])]}

# ---------------------------------------------------------------- throws
def throw(slot, script, total, release, damage, angle, bkb, kbg, ticks=()):
    line = next(x for x in scripts['diddy'][script].splitlines()
                if 'ATTACK_ABS(Kind=FIGHTER_ATTACK_ABSOLUTE_KIND_THROW' in x)
    a = args(line.split('THROW,', 1)[1].rstrip(')'))
    ledger['rawAttacks'][slot] = {'article': 'diddy', 'script': script, 'attacks': [a]}
    box = hit(a, 0, 0, 0, 0, 60, angle=angle, bkb=bkb, kbg=round(kbg / 100, 4))
    return {'release': frame(release), 'total': total, 'hit': box, 'ticks': list(ticks)}


throws = {
    'f': throw('fthrow', 'ThrowF', 31, 21, 9.0, 46, 69, 65),
    'b': throw('bthrow', 'ThrowB', 51, 18, 12.0, 42, 60, 55),
    'u': throw('uthrow', 'ThrowHi', 26, 19, 4.0, 87, 70, 72, [{'at': frame(17), 'damage': 1.0}]),
    'd': throw('dthrow', 'ThrowLw', 31, 22, 7.0, 106, 85, 59),
}

# ---------------------------------------------------------------- projectiles
banana_item = items['items']['banana']['Scripts'][0]
peanut_item = items['items']['diddypeanuts']['Scripts'][0]
ledger['rawItems'] = {'banana': banana_item, 'diddypeanuts': peanut_item}
peanut = next(a for a in re.findall(r'ATTACK\((.*?)\)', peanut_item['Data'].split('wait(Frames=1)')[0])
              for a in [args(a)] if a.get('Damage') == '3')
banana = args(re.search(r'ATTACK\((.*?)\)', banana_item['Data'])[1])

projectiles = {
    'peanut': {
        'name': 'Peanut', 'spawn': [30, 46], 'vel': [13.0, 1.5], 'r': 12, 'life': 55, 'max': 3,
        'gravity': 0.12, 'maxFall': 5.0, 'spawnSfx': 'shoot',
        'chargeScale': {'speed': 0.0062, 'damage': 0.0337662, 'angleTo': 45.0},
        'hit': {'start': 0, 'x': 0, 'y': 0, 'r': 0, 'damage': 3.3, 'bkb': 40.0, 'kbg': 0.7, 'angle': 70.0,
                'shieldDmgMult': 0.7, 'hitlagMult': 0.2},
        'shapes': [poly(((-8, -4), (-2, -8), (8, -3), (10, 4), (2, 8), (-7, 4)), '#c9a06a'),
                   poly(((-5, -2), (0, 4), (6, 2), (2, -5)), '#e6cf9c'),
                   line((-9, 3), (9, 5), 1.5, '#8a6338', alpha=0.7)],
    },
    'banana': {
        'name': 'Banana Peel', 'spawn': [-18, 40], 'vel': [-1.6, 2.4], 'r': 10, 'life': 600, 'max': 1,
        'gravity': 0.16, 'maxFall': 6.0, 'spawnSfx': 'swing',
        'bounce': {'restitution': 0.0, 'minSpeed': 0.0, 'friction': 0.0, 'maxBounces': 4, 'fizzleLife': 300,
                   'wallRestitution': 0.4, 'sfx': 'bonk'},
        'hit': {'start': 0, 'x': 0, 'y': 0, 'r': 0, 'damage': 0.0, 'bkb': float(banana['BKB']),
                'kbg': 0.0, 'angle': 0.0, 'type': 'trip'},
        'shapes': [poly(((-5, -8), (0, -9), (6, -5), (3, -2), (9, 2), (6, 7), (-2, 6), (-6, 1)), '#e8c43c'),
                   poly(((-4, -5), (2, -3), (4, 2), (0, 4), (-3, 1)), '#f6dd76'),
                   line((-5, -8), (-8, -10), 2.0, '#6b4a1e'),
                   line((5, 6), (8, 9), 2.0, '#6b4a1e')],
        'spin': 1.5,
    },
}

# ---------------------------------------------------------------- fighter document
st = ufd['stats']
doc = {
    '$schema': '../../schema/fighter.schema.json',
    'schemaVersion': 1,
    'extends': 'base',
    'name': 'Bongo Boy',
    'tagline': 'Small ape, big trouble',
    'blurb': 'A tiny, quick ape with a banana peel, a chargeable peanut popgun, a grabbing Monkey Flip '
             'and a rocketbarrel recovery that explodes on landing.',
    'stats': {
        'weight': round(100.0 / st['weight'], 8), 'pummelDamage': 1.0,
        'walkSpeed': round(st['walk'] * 3.5, 5), 'runSpeed': round(st['run'] * 3.5, 5),
        'dashSpeed': round(st['dash'] * 3.5, 5), 'dashFrames': 13, 'runAccel': 0.62,
        'groundFriction': 0.85,
        'jumpSpeed': 14.2, 'shortHopSpeed': 9.9, 'airJumpSpeed': 14.0, 'airJumpDrift': round(st['air'] * 3.5, 5),
        'airJumps': 1,
        # Source ×3.5 gives 3.234 / 0.175; fitted upward so the requested above-average air mobility is felt
        # in this engine's shared movement (recorded as adapted in the import record).
        'airSpeed': 4.3, 'airAccel': AIR_ACCEL,
        'gravity': round(st['gravity'] * 6, 5), 'fallSpeed': round(st['fall'] * 6, 5),
        'fastFallSpeed': round(st['fastFall'] * 6, 5),
    },
    'body': {'halfWidth': 13, 'height': 70},
    'hurtboxes': {'default': {'x1': -14, 'x2': 14, 'y1': 0, 'y2': 72},
                  'knockdown': {'x1': -29, 'x2': 29, 'y1': 0, 'y2': 15}},
    'charges': [{'id': 'banana', 'uses': 1, 'recharge': 85}],
    'ai': {
        'reach': 58, 'counterChance': 0,
        'specials': [
            {'minDistance': 140, 'height': 55, 'grounded': True, 'noProjectile': False, 'chance': 0.45,
             'neutral': True, 'down': False, 'hold': [8, 45]},
            {'minDistance': 70, 'maxDistance': 200, 'height': 50, 'grounded': False, 'noProjectile': False,
             'chance': 0.35, 'neutral': False, 'down': False, 'hold': None},
            {'minDistance': 40, 'maxDistance': 300, 'height': 40, 'grounded': True, 'noProjectile': True,
             'chance': 0.3, 'neutral': False, 'down': True, 'hold': None},
        ],
    },
    'moves': moves,
    'throws': throws,
    'projectiles': projectiles,
    'style': style,
    'poses': poses,
    'anims': anims,
    'attachments': attachments,
}

# ---------------------------------------------------------------- write
out = ROOT / 'assets/fighters/bongo/fighter.json'
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(doc, indent=2) + '\n')
(SOURCES / 'ledger.json').write_text(json.dumps(ledger, indent=1) + '\n')
print(f'wrote {out.relative_to(ROOT)} ({out.stat().st_size} bytes)')
