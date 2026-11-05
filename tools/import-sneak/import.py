#!/usr/bin/env python3
"""Rebuild Sneak (SSBU Snake, patch 13.0.1) from pinned Rubendal scripts and UFD rows.

Numbers come from the pinned attack scripts (geometry, damage, launch fields, masks, hitlag, rehit) and the
pinned Ultimate Frame Data rows (startup, active windows, totals, landing lag, autocancel, charge frames, gadget
timings). Hits on the root bone (`top`) convert root-space (source Z forward, Y up, 5 px/unit); limb-bone hits
are fitted to the UFD hitbox visuals because their animated bone transforms are not in the script dump. Poses
and gadget visuals are authored here.

    just import-sneak        # python3 tools/import-sneak/import.py, then just bless
"""
import copy
import json
import math
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/sneak'
source = json.loads((SOURCES / 'scripts.json').read_text())
ufd = json.loads((SOURCES / 'ufd.json').read_text())
scripts = {art: {s['Name']: s['Data'] for s in ss} for art, ss in source['scripts'].items()}
ledger = {'sourceCommit': source['commit'], 'sourceSha256': source['sha256'],
          'ufdSha256': ufd['pageSha256'], 'geometryScale': 5, 'rawAttacks': {}, 'fitted': {}}

SCALE = 5.0


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
    return {k: v.strip() for k, v in re.findall(r'(\w+)=(.*?)(?=, \w+=|$)', s)}


def attacks(article, name):
    """Attack records with the animation frame they start on and the tick they are cleared."""
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
        if m := re.search(r'\bCATCH\((.*)\)', line):
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


def row(rows, damage, nth=0):
    found = [b for b in rows if abs(num(b['source'], 'Damage') - damage) < 1e-6]
    return found[nth]


def num(a, k, default=0):
    try:
        return float(a.get(k, default))
    except (TypeError, ValueError):
        return default


def src(f):
    """Source gameplay frame -> phase frame. The move's first tick is source frame 1 = phase frame 0."""
    return max(0, int(f) - 1)


def hit(a, start, end, x, y, r, **kw):
    """Hitbox from one script ATTACK row. Geometry defaults to the converted root-space circle."""
    if x is None:
        x, y, r = num(a, 'Z') * SCALE, num(a, 'Y') * SCALE, num(a, 'Size') * SCALE
    angle = num(a, 'Angle')
    kind = 'normal'
    if angle == 361:
        kind, angle = 'sakurai', 45
    elif angle in (366, 367, 368):
        kind, angle = 'autolink', 90
    elif angle in (269, 270):
        kind, angle = 'spike', {269: -89, 270: -90}[int(angle)]
    elif angle > 180:
        angle -= 360
        kind = 'spike' if angle <= -60 else 'normal'
    b = {'start': start, 'end': end, 'x': round(x, 2), 'y': round(y, 2), 'r': round(r, 2),
         'damage': num(a, 'Damage'), 'bkb': num(a, 'BKB'), 'kbg': round(num(a, 'KBG') / 100, 4), 'angle': angle}
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


def capsule(a, start, end, x1, y1, x2, y2, r, **kw):
    """Source `X2`/`Y2`/`Z2` capsules become circles no farther apart than their radius."""
    steps = max(1, math.ceil(math.hypot(x2 - x1, y2 - y1) / max(r, 1)))
    return [hit(a, start, end, x1 + (x2 - x1) * i / steps, y1 + (y2 - y1) * i / steps, r, **kw)
            for i in range(steps + 1)]


def root(a, start, end, x=None, y=None, r=None, **kw):
    """Root-bone (`top`) attack: source Z forward, Y up, 5 px per unit; capsules become circle chains.
    Explicit coordinates replace the converted centre (a fitted height or radius)."""
    if x is None:
        x = num(a, 'Z') * SCALE
    if y is None:
        y = num(a, 'Y') * SCALE
    if r is None:
        r = num(a, 'Size') * SCALE
    if a.get('Z2', 'LUA_VOID') == 'LUA_VOID' or a.get('Y2', 'LUA_VOID') == 'LUA_VOID':
        return [hit(a, start, end, x, y, r, **kw)]
    return capsule(a, start, end, x, y, num(a, 'Z2') * SCALE, num(a, 'Y2') * SCALE, r, **kw)


def fitted(a, start, end, x, y, r, **kw):
    """A limb-bone hit with 2D geometry fitted to the UFD visuals (recorded in the ledger)."""
    return [hit(a, start, end, x, y, r, **kw)]


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


def phase(name, frames=None, boxes=(), **kw):
    p = {'name': name}
    if frames is not None:
        p['frames'] = frames
    if boxes:
        p['hitboxes'] = [{k: v for k, v in b.items() if k != '_group'} for b in assign_groups(boxes)]
    p.update(kw)
    return p


def land(lag):
    return {'on': 'land', 'to': 'landing', 'lag': lag}


def snap(pose, end_pose, contact, total, air=False):
    """Windup -> contact hold -> recovery pose keys around the active window."""
    return [{'at': 0, 'pose': end_pose if air else 'guard'},
            {'at': max(0, contact - 3), 'pose': pose, 'lerp': True},
            {'at': contact + 1, 'pose': pose},
            {'at': min(total - 1, contact + 7), 'pose': end_pose, 'lerp': True}]


# ---------------------------------------------------------------- poses and body
stand = {'head': [2, 64], 'shoulder': [0, 50], 'hip': [0, 30], 'elbowF': [9, 42], 'handF': [15, 36],
         'elbowB': [-9, 42], 'handB': [-15, 35], 'kneeF': [4, 16], 'footF': [6, 1], 'kneeB': [-5, 16],
         'footB': [-8, 1]}
poses = {
    'stand': stand,
    'guard': {'head': [2, 63], 'shoulder': [0, 49], 'hip': [0, 30], 'elbowF': [10, 44], 'handF': [15, 54],
              'elbowB': [-8, 43], 'handB': [-3, 48], 'kneeF': [5, 16], 'footF': [9, 1], 'kneeB': [-6, 16],
              'footB': [-11, 1]},
    'punch': {'head': [4, 62], 'shoulder': [2, 49], 'hip': [-1, 29], 'elbowF': [18, 48], 'handF': [38, 47],
              'elbowB': [-10, 44], 'handB': [-16, 40], 'kneeF': [10, 16], 'footF': [16, 1], 'kneeB': [-6, 15],
              'footB': [-11, 1]},
    'bodyBlow': {'head': [6, 58], 'shoulder': [3, 45], 'hip': [-2, 26], 'elbowF': [14, 38], 'handF': [30, 33],
                 'elbowB': [-10, 40], 'handB': [-14, 34], 'kneeF': [14, 14], 'footF': [22, 1], 'kneeB': [-8, 14],
                 'footB': [-14, 1]},
    'knee': {'head': [2, 62], 'shoulder': [0, 48], 'hip': [0, 29], 'elbowF': [10, 44], 'handF': [16, 52],
             'elbowB': [-10, 42], 'handB': [-14, 34], 'kneeF': [22, 30], 'footF': [30, 12], 'kneeB': [-6, 16],
             'footB': [-10, 1]},
    'axeKick': {'head': [-4, 66], 'shoulder': [-2, 51], 'hip': [2, 31], 'elbowF': [8, 46], 'handF': [14, 40],
                'elbowB': [-12, 46], 'handB': [-18, 40], 'kneeF': [16, 36], 'footF': [26, 62], 'kneeB': [-4, 16],
                'footB': [-8, 1]},
    'lowKick': {'head': [6, 54], 'shoulder': [2, 42], 'hip': [-2, 24], 'elbowF': [12, 36], 'handF': [20, 28],
                'elbowB': [-12, 36], 'handB': [-18, 28], 'kneeF': [24, 18], 'footF': [42, 8], 'kneeB': [-8, 12],
                'footB': [-14, 1]},
    'crouchKick': {'head': [8, 46], 'shoulder': [4, 34], 'hip': [0, 20], 'elbowF': [14, 26], 'handF': [22, 16],
                   'elbowB': [-12, 28], 'handB': [-18, 20], 'kneeF': [22, 16], 'footF': [36, 10], 'kneeB': [-10, 12],
                   'footB': [-18, 1]},
    'crouchKickBack': {'head': [-8, 46], 'shoulder': [-4, 34], 'hip': [0, 20], 'elbowF': [12, 28], 'handF': [18, 20],
                       'elbowB': [-14, 26], 'handB': [-22, 16], 'kneeF': [-22, 16], 'footF': [-36, 10],
                       'kneeB': [10, 12], 'footB': [18, 1]},
    'shoulder': {'head': [12, 56], 'shoulder': [6, 44], 'hip': [2, 26], 'elbowF': [12, 38], 'handF': [18, 30],
                 'elbowB': [-8, 38], 'handB': [-14, 32], 'kneeF': [18, 14], 'footF': [28, 4], 'kneeB': [-12, 14],
                 'footB': [-20, 2]},
    'kneel': {'head': [2, 50], 'shoulder': [0, 38], 'hip': [-2, 22], 'elbowF': [14, 38], 'handF': [26, 44],
              'elbowB': [-10, 34], 'handB': [-18, 30], 'kneeF': [12, 12], 'footF': [18, 2], 'kneeB': [-12, 10],
              'footB': [-20, 2]},
    'mortar': {'head': [4, 52], 'shoulder': [2, 40], 'hip': [0, 24], 'elbowF': [14, 40], 'handF': [26, 56],
               'elbowB': [-12, 34], 'handB': [-20, 28], 'kneeF': [10, 12], 'footF': [16, 2], 'kneeB': [-10, 12],
               'footB': [-16, 2]},
    'plant': {'head': [10, 44], 'shoulder': [6, 32], 'hip': [0, 18], 'elbowF': [16, 24], 'handF': [28, 10],
              'elbowB': [-12, 26], 'handB': [-20, 16], 'kneeF': [10, 10], 'footF': [16, 1], 'kneeB': [-12, 10],
              'footB': [-20, 1]},
    'bust': {'head': [0, 64], 'shoulder': [0, 51], 'hip': [0, 31], 'elbowF': [14, 54], 'handF': [22, 66],
             'elbowB': [-10, 44], 'handB': [-16, 38], 'kneeF': [5, 16], 'footF': [9, 1], 'kneeB': [-6, 16],
             'footB': [-10, 1]},
    'pullPin': {'head': [2, 62], 'shoulder': [0, 49], 'hip': [0, 30], 'elbowF': [12, 52], 'handF': [16, 62],
                'elbowB': [-10, 44], 'handB': [-16, 40], 'kneeF': [5, 16], 'footF': [9, 1], 'kneeB': [-6, 16],
                'footB': [-10, 1]},
    'throwArm': {'head': [4, 62], 'shoulder': [2, 49], 'hip': [-1, 29], 'elbowF': [18, 50], 'handF': [34, 54],
                 'elbowB': [-10, 44], 'handB': [-16, 40], 'kneeF': [10, 16], 'footF': [16, 1], 'kneeB': [-6, 15],
                 'footB': [-11, 1]},
    'air': {'head': [2, 62], 'shoulder': [0, 49], 'hip': [0, 30], 'elbowF': [10, 44], 'handF': [18, 40],
            'elbowB': [-10, 44], 'handB': [-18, 38], 'kneeF': [10, 18], 'footF': [16, 6], 'kneeB': [-8, 16],
            'footB': [-14, 5]},
    'airKick': {'head': [4, 58], 'shoulder': [2, 46], 'hip': [-2, 28], 'elbowF': [10, 40], 'handF': [18, 34],
                'elbowB': [-10, 42], 'handB': [-16, 34], 'kneeF': [22, 26], 'footF': [40, 16], 'kneeB': [-8, 16],
                'footB': [-14, 6]},
    'airBody': {'head': [-10, 58], 'shoulder': [-6, 46], 'hip': [2, 30], 'elbowF': [6, 40], 'handF': [14, 32],
                'elbowB': [-16, 44], 'handB': [-24, 38], 'kneeF': [14, 18], 'footF': [22, 8], 'kneeB': [-6, 16],
                'footB': [-12, 6]},
    'airRaise': {'head': [0, 64], 'shoulder': [0, 50], 'hip': [0, 31], 'elbowF': [8, 58], 'handF': [12, 74],
                 'elbowB': [-10, 56], 'handB': [-14, 70], 'kneeF': [8, 18], 'footF': [12, 6], 'kneeB': [-6, 16],
                 'footB': [-10, 5]},
    'dive': {'head': [0, 22], 'shoulder': [0, 34], 'hip': [0, 50], 'elbowF': [6, 18], 'handF': [8, 6],
             'elbowB': [-6, 18], 'handB': [-8, 6], 'kneeF': [8, 62], 'footF': [10, 74], 'kneeB': [-6, 62],
             'footB': [-8, 74]},
    'ride': {'head': [-2, 62], 'shoulder': [0, 49], 'hip': [2, 30], 'elbowF': [10, 56], 'handF': [14, 70],
             'elbowB': [-10, 54], 'handB': [-12, 68], 'kneeF': [12, 18], 'footF': [18, 8], 'kneeB': [-4, 16],
             'footB': [-6, 8]},
    'hold': {'head': [2, 62], 'shoulder': [0, 49], 'hip': [0, 30], 'elbowF': [14, 44], 'handF': [28, 44],
             'elbowB': [-10, 44], 'handB': [-8, 40], 'kneeF': [8, 16], 'footF': [14, 1], 'kneeB': [-8, 16],
             'footB': [-14, 1]},
    'helpless': {'from': 'air', 'head': [-4, 58], 'elbowF': [8, 40], 'handF': [14, 26], 'elbowB': [-14, 40],
                 'handB': [-22, 28]},
    'win': {'head': [0, 64], 'shoulder': [0, 51], 'hip': [0, 31], 'elbowF': [12, 58], 'handF': [10, 74],
            'elbowB': [-12, 44], 'handB': [-18, 38], 'kneeF': [5, 16], 'footF': [9, 1], 'kneeB': [-6, 16],
            'footB': [-10, 1]},
}
poses['crouch'] = {k: [v[0], v[1] * 0.72] for k, v in stand.items()}


def line(a, b, w, c, **kw):
    return {'shape': 'line', 'from': list(a), 'to': list(b), 'width': w, 'color': c, **kw}


def circle(at, r, fill=None, **kw):
    s = {'shape': 'circle', 'at': list(at), 'r': r, **kw}
    if fill:
        s['fill'] = fill
    return s


attachments = [
    {'id': 'bandana', 'joint': 'head', 'behind': True, 'shapes': [
        line((-10, 1), (10, 1), 4, '#2f6c46'),
        line((-10, 1), (-20, -6), 3, '#2f6c46', alpha=0.9),
        line((-10, 1), (-19, 6), 2.4, '#24513a', alpha=0.8),
    ]},
    {'id': 'pack', 'joint': 'shoulder', 'behind': True, 'shapes': [
        line((-4, 2), (-12, -14), 12, '#3c4038'),
        line((-3, 0), (-11, -13), 5, '#5b6152', alpha=0.9),
        line((-10, -12), (6, -4), 2, '#8c7a52'),
    ]},
    {'id': 'grenade', 'joint': 'handF', 'hidden': True, 'angle': 80, 'shapes': [
        circle((0, 0), 7, '#3f5233', stroke='#8c9a6a', width=1.6),
        line((-2, 6), (2, 9), 3, '#b9a06a'),
        line((0, 9), (1, 13), 2.4, '#8c9a6a'),
    ]},
    {'id': 'c4', 'joint': 'handF', 'hidden': True, 'angle': 0, 'shapes': [
        {'shape': 'rect', 'x': -4, 'y': -5, 'w': 9, 'h': 10, 'fill': '#4a5340', 'stroke': '#8f9a7a',
         'width': 1.2},
        circle((1, 2), 1.8, '#e0442c'),
    ]},
    {'id': 'rpg', 'joint': 'shoulder', 'hidden': True, 'angle': 20, 'shapes': [
        line((-14, 0), (34, 0), 9, '#4d5a3c'),
        line((34, -3), (40, -3), 5, '#333b28'),
        line((-14, 4), (-6, 12), 5, '#3c4630'),
        line((10, 6), (14, 14), 3, '#5d6a48'),
    ]},
    {'id': 'mortar', 'joint': 'shoulder', 'hidden': True, 'angle': 55, 'shapes': [
        line((-6, 0), (24, 0), 11, '#57604a'),
        line((24, -4), (30, -4), 7, '#2f3627'),
        line((-4, 6), (8, 6), 6, '#3d4534'),
    ]},
    {'id': 'cypher', 'joint': 'head', 'hidden': True, 'behind': True, 'angle': 90, 'shapes': [
        circle((0, 18), 11, '#8e9b86', stroke='#d3dbc8', width=2),
        line((-24, 18), (24, 18), 4, '#c3ccb6'),
        line((-24, 18), (-16, 24), 2.5, '#a7b29c'),
        line((24, 18), (16, 24), 2.5, '#a7b29c'),
        circle((0, 18), 3.5, '#2c332a'),
        circle((0, 6), 2.4, '#e0442c'),
        line((-6, 29), (6, 29), 2.5, '#e8eede'),
    ]},
]
style = {'headRadius': 10, 'torsoWidth': 7, 'frontLimbWidth': 6, 'backLimbWidth': 5.5, 'glowWidth': 13,
         'torsoGlowWidth': 15, 'headGlowRadius': 15, 'headRing': 3.4, 'headFill': '#1d2a1c', 'eye': [3, -1, 4, 2]}
anims = {
    'idle': {'clock': 'world', 'loop': 44.0, 'keys': [
        {'at': 0, 'pose': 'guard'},
        {'at': 22, 'pose': {'from': 'guard', 'head': [2, 65], 'handF': [15, 56], 'handB': [-3, 50]}, 'lerp': True}]},
    'run': {'clock': 'run', 'loop': 360.0, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'head': [6, 60], 'hip': [1, 30], 'elbowF': [14, 46], 'handF': [24, 44],
                           'elbowB': [-14, 42], 'handB': [-24, 38], 'kneeF': [16, 16], 'footF': [24, 4],
                           'kneeB': [-14, 14], 'footB': [-22, 2]}},
        {'at': 180, 'pose': {'from': 'stand', 'head': [6, 60], 'hip': [1, 30], 'elbowF': [14, 46], 'handF': [24, 44],
                             'elbowB': [-14, 42], 'handB': [-24, 38], 'kneeF': [-14, 14], 'footF': [-22, 2],
                             'kneeB': [16, 16], 'footB': [24, 4]}, 'lerp': True}]},
    'walk': {'clock': 'world', 'loop': 28.0, 'keys': [
        {'at': 0, 'pose': {'from': 'guard', 'kneeF': [7, 16], 'footF': [13, 1], 'kneeB': [-6, 16],
                           'footB': [-13, 1]}},
        {'at': 14, 'pose': {'from': 'guard', 'kneeF': [-3, 16], 'footF': [-7, 1], 'kneeB': [5, 16], 'footB': [9, 1]},
         'lerp': True}]},
    'crouch': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'crouch'}]},
    'jumpsquat': {'clock': 'state', 'keys': [{'at': 0, 'pose': {'from': 'crouch', 'head': [2, 50]}}]},
    'landing': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'crouch', 'head': [4, 48]}}, {'at': 8, 'pose': 'guard', 'lerp': True}]},
    'air.rise': {'clock': 'state', 'keys': [{'at': 0, 'pose': {'from': 'air', 'kneeF': [14, 18], 'footF': [20, 8]}}]},
    'air.fall': {'clock': 'state', 'keys': [{'at': 0, 'pose': {'from': 'air', 'kneeF': [8, 16], 'footF': [12, 2]}}]},
    'helpless': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'head': [-4, 58], 'elbowF': [8, 40], 'handF': [14, 26], 'elbowB': [-14, 40],
                           'handB': [-22, 28]}}]},
    'hitstun': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'head': [-10, 60], 'handF': [10, 32], 'handB': [-22, 30]}}]},
    'tumble': {'clock': 'spin', 'keys': [{'at': 0, 'pose': {'from': 'air', 'head': [-8, 60]}}]},
    'hold': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'hold'}]},
    'throw.f': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'hold'}, {'at': 8, 'pose': 'throwArm', 'lerp': True}]},
    'throw.b': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'hold'}, {'at': 8, 'pose': {'from': 'throwArm', 'handF': [-10, 50]}, 'lerp': True}]},
    'throw.u': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'hold'}, {'at': 8, 'pose': 'airRaise', 'lerp': True}]},
    'throw.d': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'hold'}, {'at': 8, 'pose': 'plant', 'lerp': True}]},
    'win': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'win'}, {'at': 24, 'pose': {'from': 'win', 'handF': [14, 78], 'handB': [-12, 60]},
                                   'lerp': True}]},
    'respawn': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'air'}]},
}


# ---------------------------------------------------------------- moves
moves = {}

# --- neutral attack: two jabs then a straight kick (Attack11/12/13)
j1 = raw('jab1', 'snake', 'Attack11')
jab1 = [b for n in (0, 1, 3) for b in root(row(j1, 2.5, n)['source'], src(3), src(4))]
j2 = raw('jab2', 'snake', 'Attack12')
jab2 = [b for n in (0, 2, 4) for b in root(row(j2, 2.5, n)['source'], src(4), src(5))]
j3 = raw('jab3', 'snake', 'Attack13')
jab3 = root(row(j3, 6.0, 0)['source'], src(8), src(9)) + root(row(j3, 6.0, 1)['source'], src(8), src(9)) + \
    root(row(j3, 6.0, 2)['source'], src(8), src(9))
moves['jab'] = {'name': 'Snake Jab', 'phases': [
    phase('jab1', 15, jab1, anim=snap('punch', 'guard', src(3), 15),
          events=[{'at': src(3), 'sfx': 'swing'}],
          transitions=[{'on': 'attackHeld', 'from': src(6), 'to': 'jab2'}, {'on': 'timeout', 'to': 'end'}]),
    phase('jab2', 24, jab2, anim=snap('punch', 'guard', src(4), 24),
          events=[{'at': src(4), 'sfx': 'swing'}],
          transitions=[{'on': 'attackHeld', 'from': src(7), 'to': 'jab3'}, {'on': 'timeout', 'to': 'end'}]),
    phase('jab3', 46, jab3, anim=snap('knee', 'guard', src(8), 46),
          events=[{'at': src(8), 'sfx': 'swing'}], transitions=[{'on': 'timeout', 'to': 'end'}]),
]}

# --- forward tilt: knee then a straight body blow (AttackS3, AttackS3S2)
ft1 = raw('ftilt1', 'snake', 'AttackS3')
knee_a = row(ft1, 4.0, 0)['source']
ftilt1 = fitted(row(ft1, 4.0, 0)['source'], src(4), src(5), 44, 38, 22, situation='ground') + \
    fitted(row(ft1, 4.0, 1)['source'], src(4), src(5), 44, 38, 22, situation='air')
ft2 = raw('ftilt2', 'snake', 'AttackS3S2')
ftilt2 = root(row(ft2, 9.0)['source'], src(8), src(8)) + root(row(ft2, 11.0)['source'], src(8), src(8)) + \
    root(row(ft2, 10.0)['source'], src(8), src(8))
ledger['fitted']['ftilt1'] = {'bone': 'legr', 'pos': [44, 38, 22], 'note': 'knee strike fitted to SnakeFTilt1'}
moves['ftilt'] = {'name': 'Knee / Body Blow', 'phases': [
    phase('knee', 31, ftilt1, anim=snap('knee', 'guard', src(4), 31), events=[{'at': src(4), 'sfx': 'swing'}],
          transitions=[{'on': 'attackHeld', 'from': src(10), 'to': 'blow'}, {'on': 'timeout', 'to': 'end'}]),
    phase('blow', 39, ftilt2, anim=snap('bodyBlow', 'guard', src(8), 39), events=[{'at': src(8), 'sfx': 'smash'}],
          transitions=[{'on': 'timeout', 'to': 'end'}]),
]}

# --- up tilt: rising axe kick (AttackHi3); UFD windows 6-8 early, 9-13 late
ut = raw('utilt', 'snake', 'AttackHi3')
rows = ut
u_early = root(row(rows, 14.5, 0)['source'], src(6), src(6)) + root(row(rows, 14.5, 1)['source'], src(6), src(6)) + \
    root(row(rows, 14.5, 2)['source'], src(6), src(6))
u_mid = root(row(rows, 14.5, 3)['source'], src(7), src(7)) + root(row(rows, 14.5, 4)['source'], src(7), src(7)) + \
    root(row(rows, 14.5, 5)['source'], src(7), src(7))
u_high = fitted(row(rows, 14.5, 6)['source'], src(8), src(8), 22, 30, 25) + \
    fitted(row(rows, 14.5, 7)['source'], src(8), src(8), 8, 22, 15) + \
    fitted(row(rows, 14.5, 8)['source'], src(8), src(8), 5, 20, 15)
u_late = fitted(row(rows, 13.5, 0)['source'], src(9), src(13), 22, 30, 25) + \
    fitted(row(rows, 13.5, 1)['source'], src(9), src(13), 8, 22, 15) + \
    fitted(row(rows, 13.5, 2)['source'], src(9), src(13), 5, 20, 15)
ledger['fitted']['utilt'] = {'bone': 'kneel/legl', 'pos': [[22, 30, 25], [8, 22, 15], [5, 20, 15]]}
moves['utilt'] = {'name': 'Rising Kick', 'phases': [phase(
    'main', 37, u_early + u_mid + u_high + u_late,
    anim=[{'at': 0, 'pose': 'guard'}, {'at': 2, 'pose': 'lowKick', 'lerp': True},
          {'at': src(6), 'pose': 'axeKick', 'lerp': True}, {'at': src(13), 'pose': 'axeKick'},
          {'at': 28, 'pose': 'guard', 'lerp': True}],
    events=[{'at': src(6), 'sfx': 'swing'}], transitions=[{'on': 'timeout', 'to': 'end'}])]}

# --- down tilt: low sweep (AttackLw3)
dt = raw('dtilt', 'snake', 'AttackLw3')
dtilt = root(row(dt, 12.0, 0)['source'], src(6), src(6)) + root(row(dt, 12.0, 1)['source'], src(6), src(6)) + \
    root(row(dt, 12.0, 2)['source'], src(6), src(6))
for n in (3, 4, 5):
    dtilt += root(row(dt, 12.0, n)['source'], src(7), src(7))
moves['dtilt'] = {'name': 'Low Sweep', 'phases': [phase(
    'main', 30, dtilt, anim=snap('lowKick', 'guard', src(6), 30, ), events=[{'at': src(6), 'sfx': 'swing'}],
    transitions=[{'on': 'timeout', 'to': 'end'}])]}

# --- dash attack: shoulder charge (AttackDash)
da = raw('dashatk', 'snake', 'AttackDash')
dash = fitted(row(da, 11.0)['source'], src(5), src(8), 22, 42, 22.5) + \
    root(row(da, 9.0)['source'], src(5), src(8))
dash += fitted(row(da, 8.0)['source'], src(9), src(12), 22, 40, 22.5) + \
    root(row(da, 6.0)['source'], src(9), src(12))
ledger['fitted']['dashatk'] = {'bone': 'neck', 'pos': [22, 42, 22.5]}
moves['dashatk'] = {'name': 'Shoulder Charge', 'phases': [phase(
    'main', 41, dash, slide=src(13), anim=snap('shoulder', 'guard', src(5), 41),
    events=[{'at': 0, 'set_vel': {'x': 4.6}}, {'at': src(5), 'sfx': 'smash'}],
    transitions=[{'on': 'timeout', 'to': 'end'}])]}

# --- forward smash: RPG-7 blast (AttackS4); UFD charge hold 38, hit 41-43, total 73
fs = raw('fsmash', 'snake', 'AttackS4')
fsmash = root(row(fs, 22.0, 0)['source'], src(41), src(43), y=26, r=60) + \
    root(row(fs, 22.0, 1)['source'], src(41), src(43), y=26, r=35)
moves['fsmash'] = {'name': 'RPG-7', 'phases': [phase(
    'main', 73, fsmash, charge={'frame': src(38)}, anim=[
        {'at': 0, 'pose': 'guard'}, {'at': 6, 'pose': 'kneel', 'lerp': True},
        {'at': src(41), 'pose': 'kneel'}, {'at': 55, 'pose': 'guard', 'lerp': True}],
    attach={'rpg': {'show': [0, 70]}},
    events=[{'at': src(41), 'sfx': 'quake', 'fx': [{'type': 'burst', 'x': 80, 'y': 26, 'color': '#ffb066', 'n': 18,
                                                    'speed': 5.5, 'life': 20, 'size': 4.5}]}],
    transitions=[{'on': 'timeout', 'to': 'end'}])]}

# --- up smash: mortar launcher + shell (AttackHi4); UFD charge hold 24, launcher 11-12, shell 35
us = raw('usmash', 'snake', 'AttackHi4')
launcher = root(row(us, 4.0, 0)['source'], src(11), src(12), y=25, r=25) + \
    root(row(us, 4.0, 1)['source'], src(11), src(12), y=25, r=25)
moves['usmash'] = {'name': 'Mortar', 'phases': [phase(
    'main', 54, launcher, charge={'frame': src(24)}, anim=[
        {'at': 0, 'pose': 'guard'}, {'at': 4, 'pose': 'mortar', 'lerp': True},
        {'at': src(35), 'pose': 'mortar'}, {'at': 46, 'pose': 'guard', 'lerp': True}],
    attach={'mortar': {'show': [0, 50]}},
    events=[{'at': src(35), 'spawn': 'mortarShell', 'sfx': 'shoot'}],
    transitions=[{'on': 'timeout', 'to': 'end'}])]}

# --- down smash: front then back low kick (AttackLw4); UFD 8-10 and 20-22, charge hold 5
ds = raw('dsmash', 'snake', 'AttackLw4')
dsmash = fitted(row(ds, 12.0, 0)['source'], src(8), src(10), 52, 24, 20) + \
    fitted(row(ds, 12.0, 1)['source'], src(8), src(10), 42, 20, 16.5) + \
    fitted(row(ds, 14.0, 0)['source'], src(20), src(22), -52, 24, 20) + \
    fitted(row(ds, 14.0, 1)['source'], src(20), src(22), -42, 20, 16.5)
ledger['fitted']['dsmash'] = {'bone': 'kneel/legl', 'pos': [[52, 24, 20], [-52, 24, 20]]}
moves['dsmash'] = {'name': 'Double Kick', 'phases': [phase(
    'main', 44, dsmash, charge={'frame': src(5)}, anim=[
        {'at': 0, 'pose': 'guard'}, {'at': 3, 'pose': 'crouchKick', 'lerp': True},
        {'at': src(10), 'pose': 'crouchKick'}, {'at': src(16), 'pose': 'crouchKickBack', 'lerp': True},
        {'at': src(22), 'pose': 'crouchKickBack'}, {'at': 34, 'pose': 'guard', 'lerp': True}],
    events=[{'at': src(8), 'sfx': 'swing'}, {'at': src(20), 'sfx': 'swing'}],
    transitions=[{'on': 'timeout', 'to': 'end'}])]}


def aerial(slot, total, landing, ac_early, ac_late, boxes, contacts, pose, loop_pose='air'):
    """Split an aerial at its autocancel boundaries so startup and ending windows land without lag."""
    boxes = assign_groups(boxes)
    spans = [(0, ac_early, 'startup', None), (ac_early, ac_late, 'main', landing), (ac_late, total, 'late', None)]
    spans = [s for s in spans if s[1] > s[0]]
    parts = []
    for k, (lo, hi, tag, lag) in enumerate(spans):
        following = spans[k + 1][2] if k + 1 < len(spans) else 'end' 
        cut = []
        for b in boxes:
            b = copy.deepcopy(b)
            s, e = b['start'], b.get('end', total)
            if s >= hi or e < lo:
                continue
            b['start'], b['end'] = max(lo, s) - lo, min(hi - 1, e) - lo
            cut.append(b)
        ph = phase(tag, hi - lo, cut, transitions=[
            {'on': 'land', 'to': 'landing', 'lag': lag} if lag else {'on': 'land', 'to': 'end'},
            {'on': 'timeout', 'to': following, 'preserveHits': True}])
        keys = [{'at': 0, 'pose': loop_pose}]
        for c in contacts:
            if lo <= c < hi:
                keys.append({'at': max(0, c - lo - 2), 'pose': pose, 'lerp': True})
                keys.append({'at': min(hi - lo - 1, c - lo + 2), 'pose': pose})
        keys.append({'at': hi - lo - 1, 'pose': loop_pose, 'lerp': True})
        ph['anim'] = keys
        parts.append(ph)
    return {'name': slot, 'air': 'aerial', 'phases': parts}


# --- neutral air: three autolink hits then a final swing (AttackAirN); total 59, land 16, AC 1-3 / 50+
an = raw('nair', 'snake', 'AttackAirN')
nair = []
for (start, end, y) in [(10, 11, 35), (18, 19, 45), (26, 27, 45)]:
    nair += capsule(row(an, 3.0, 0)['source'], src(start), src(end), 20, y, 55, y, 20)
    nair += capsule(row(an, 3.0, 1)['source'], src(start), src(end), 20, y, 55, y, 20)
nair += root(row(an, 12.0)['source'], src(36), src(38))
moves['nair'] = aerial('Leaping Kick', 59, 16, 3, 50, nair, [src(10), src(18), src(26), src(36)], 'airKick')

# --- forward air: knee drop with a meteor (AttackAirF); total 69, land 19, AC 1-3 / 63+
af = raw('fair', 'snake', 'AttackAirF')
fair = fitted(row(af, 15.0)['source'], src(23), src(26), 52, 26, 27.5) + \
    fitted(row(af, 14.0)['source'], src(23), src(26), 36, 34, 23.5)
ledger['fitted']['fair'] = {'bone': 'kneel/legl', 'pos': [[52, 26, 27.5], [36, 34, 23.5]]}
moves['fair'] = aerial('Knee Drop', 69, 19, 3, 62, fair, [src(23)], 'airKick')

# --- back air: body slam (AttackAirB); total 42, land 19, AC 40+
ab = raw('bair', 'snake', 'AttackAirB')
bair = fitted(row(ab, 16.0)['source'], src(7), src(9), -50, 30, 27) + \
    fitted(row(ab, 14.0)['source'], src(7), src(9), -26, 28, 22.5) + \
    fitted(row(ab, 10.0)['source'], src(10), src(26), -50, 28, 25) + \
    fitted(row(ab, 9.0)['source'], src(10), src(26), -24, 26, 21.5)
ledger['fitted']['bair'] = {'bone': 'top', 'pos': [[-50, 30, 27], [-26, 28, 22.5]],
                            'note': 'source Z offsets kept; height raised to the visual body centre'}
moves['bair'] = aerial('Belly Flop', 42, 19, 0, 39, bair, [src(7), src(10)], 'airBody')

# --- up air: overhead double kick (AttackAirHi); total 47, land 15, AC 1-4 / 36+
au = raw('uair', 'snake', 'AttackAirHi')
uair = []
for y in (75, 90, 105):
    uair += fitted(row(au, 14.0)['source'], src(10), src(12), 0, y, 20)
    uair += fitted(row(au, 10.0)['source'], src(13), src(23), 0, y, 20)
ledger['fitted']['uair'] = {'bone': 'top', 'pos': [[0, 75, 20], [0, 90, 20], [0, 105, 20]]}
moves['uair'] = aerial('Sky Kick', 47, 15, 4, 35, uair, [src(10), src(13)], 'airRaise')

# --- down air: headstand drill (AttackAirLw); total 59, land 20, AC 54+
ad = raw('dair', 'snake', 'AttackAirLw')
dair = fitted(row(ad, 10.0, 0)['source'], src(25), src(26), 15, -5, 35) + \
    fitted(row(ad, 10.0, 1)['source'], src(25), src(26), 7, 25, 30)
for k, (start, end) in enumerate([(3, 4), (10, 11), (17, 18)]):
    dmg, base = (4.0, 0) if start == 3 else (3.0, 3)
    for n in range(3):
        dair += root(row(ad, dmg, base + n)['source'], src(start), src(end))
moves['dair'] = aerial('Drill Kick', 59, 20, 0, 53, dair, [src(3), src(10), src(17), src(25)], 'dive')

# --- grabs (Catch / CatchDash)
cg = raw('grab', 'snake', 'Catch')
grab_a = cg[0]['source']
grab = capsule(grab_a, src(8), src(9), 35, 41, 57.5, 41, 20, type='grab', situation='ground', damage=0)
grab += capsule(cg[1]['source'], src(8), src(9), 25, 41, 67.5, 41, 10, type='grab', situation='air', damage=0)
for b in grab:
    b['damage'] = 0
moves['grab'] = {'name': 'Grab', 'phases': [phase(
    'main', 34, grab, anim=snap('hold', 'guard', src(8), 34, ), transitions=[{'on': 'timeout', 'to': 'end'}])]}
dg = raw('dashgrab', 'snake', 'CatchDash')
dash_grab = capsule(dg[0]['source'], src(11), src(12), 35, 41, 57.5, 41, 20, type='grab', damage=0) + \
    capsule(dg[1]['source'], src(11), src(12), 25, 41, 67.5, 41, 10, type='grab', damage=0)
for b in dash_grab:
    b['damage'] = 0
moves['dashgrab'] = {'name': 'Dash Grab', 'phases': [phase(
    'main', 42, dash_grab, slide=src(20), anim=snap('hold', 'guard', src(11), 42),
    transitions=[{'on': 'timeout', 'to': 'end'}])]}

# --- neutral special: pull the pin, hold it, then throw it as an arc chosen by the stick
GRENADE_FUSE = 129  # 150 frames from unpin minus the 21-frame pull
HOLD_SPECIAL = {'frame': 0, 'max': 0, 'hold_after_max': 0, 'button': 'special'}
# The held tick count is the cooking clock: the fuse starts at the unpin (see the grenade's chargeScale).
COOK_HOLD = {'frame': 0, 'max': GRENADE_FUSE, 'hold_after_max': 0, 'button': 'special'}
pull = phase('pull', 21, charge=HOLD_SPECIAL, anim=[
    {'at': 0, 'pose': 'guard'}, {'at': 3, 'pose': 'pullPin', 'lerp': True}, {'at': 20, 'pose': 'pullPin'}],
    attach={'grenade': {'show': [0, 20]}},
    events=[{'at': 0, 'sfx': 'swing'}],
    transitions=[{'on': 'releaseAway', 'from': src(21), 'to': 'throwLob'},
                 {'on': 'releaseToward', 'from': src(21), 'to': 'throwSmash'},
                 {'on': 'release', 'from': src(21), 'to': 'throwMid'},
                 {'on': 'timeout', 'to': 'hold'}])
hold = phase('hold', None, ground_speed=1.7, charge=COOK_HOLD,
             anim=[{'at': 0, 'pose': 'pullPin'}], attach={'grenade': {'show': [0, 400]}},
             transitions=[{'on': 'shieldPress', 'to': 'drop'},
                          {'on': 'releaseAway', 'to': 'throwLob'},
                          {'on': 'releaseToward', 'to': 'throwSmash'},
                          {'on': 'release', 'to': 'throwMid'},
                          {'on': 'attackPress', 'to': 'throwMid'}])


def throw_phase(name, total, start, spawn, pose_after):
    return phase(name, total, anim=[
        {'at': 0, 'pose': 'pullPin'}, {'at': max(1, start - 4), 'pose': 'throwArm', 'lerp': True},
        {'at': start, 'pose': 'throwArm'}, {'at': min(total - 1, start + 8), 'pose': pose_after, 'lerp': True}],
        events=[{'at': start, 'spawn': spawn, 'sfx': 'swing'}],
        transitions=[{'on': 'timeout', 'to': 'end'}])
moves['nspec'] = {'name': 'Hand Grenade', 'b_reverse': True, 'phases': [
    pull, hold,
    throw_phase('throwMid', 27, src(8), 'grenadeMid', 'guard'),
    throw_phase('throwSmash', 31, src(10), 'grenadeStrong', 'guard'),
    throw_phase('throwLob', 25, src(9), 'grenadeLob', 'guard'),
    phase('drop', 21, anim=[{'at': 0, 'pose': 'pullPin'}, {'at': 4, 'pose': 'plant', 'lerp': True},
                            {'at': 16, 'pose': 'guard', 'lerp': True}],
          events=[{'at': 0, 'spawn': 'grenadeDrop', 'sfx': 'swing'}],
          transitions=[{'on': 'timeout', 'to': 'end'}]),
]}

# --- side special: shoulder the launcher, fly the Nikita, release it (SpecialSStart / SpecialSAway)
ss = raw('nikita', 'snake', 'SpecialSStart')
moves['sspec'] = {'name': 'Nikita', 'b_reverse': True, 'phases': [
    phase('startup', 41, anim=[
        {'at': 0, 'pose': 'guard'}, {'at': 8, 'pose': 'kneel', 'lerp': True}, {'at': 40, 'pose': 'kneel'}],
        attach={'mortar': {'show': [0, 40]}}, events=[{'at': src(7), 'sfx': 'swing'}],
        transitions=[{'on': 'timeout', 'to': 'guide'}]),
    phase('guide', 200, air_accel_mult=0.15, anim=[{'at': 0, 'pose': 'kneel'}],
          events=[{'at': 0, 'spawn': 'nikita', 'sfx': 'shoot'}],
          transitions=[{'on': 'shieldPress', 'to': 'away'},
                       {'on': 'articleRelease', 'to': 'fire'},
                       {'on': 'land', 'to': 'fire'},
                       {'on': 'timeout', 'to': 'away'}]),
    phase('fire', 36, anim=[{'at': 0, 'pose': 'kneel'}, {'at': 12, 'pose': 'bust', 'lerp': True},
                            {'at': 24, 'pose': 'guard', 'lerp': True}],
          events=[{'at': 0, 'sfx': 'shoot'}], transitions=[{'on': 'timeout', 'to': 'end'}]),
    phase('away', 31, anim=[{'at': 0, 'pose': 'kneel'}, {'at': 10, 'pose': 'guard', 'lerp': True}],
          transitions=[{'on': 'timeout', 'to': 'end'}]),
]}

# --- up special: Cypher (SpecialHiStart / SpecialAirHiHang / SpecialAirHiCut); armor 7-94, cancel 47
moves['uspec'] = {'name': 'Cypher', 'air': 'upSpecial', 'phases': [
    phase('startup', 3, anim=[{'at': 0, 'pose': 'guard'}, {'at': 3, 'pose': 'ride'}],
          events=[{'at': 0, 'sfx': 'swing'}], transitions=[{'on': 'timeout', 'to': 'rise'}]),
    # The rise phase begins on source frame 4, so its frame = source frame - 4.
    phase('rise', 86, armor_threshold=7, ledge_grab=src(14) - 3, air_accel_mult=0.4,
          attach={'cypher': {'show': [0, 85]}},
          anim=[{'at': 0, 'pose': 'ride'}],
          events=[{'at': 0, 'leave_ground': True, 'sfx': 'jet'},
                  {'from': 0, 'to': 85, 'set_vel': {'y': 3.4}}],
          transitions=[{'on': 'attackPress', 'from': src(47) - 3, 'to': 'cut'},
                       {'on': 'shieldPress', 'from': src(47) - 3, 'to': 'cut'},
                       {'on': 'land', 'to': 'landing', 'lag': 20},
                       {'on': 'timeout', 'to': 'cut'}]),
    phase('cut', 12, anim=[{'at': 0, 'pose': 'air'}, {'at': 10, 'pose': 'helpless', 'lerp': True}],
          events=[{'at': 0, 'spawn': 'cypherShot', 'sfx': 'slash'}],
          transitions=[{'on': 'land', 'to': 'landing', 'lag': 20}, {'on': 'timeout', 'to': 'helpless'}]),
]}

# --- down special: plant the C4, then press the switch (SpecialLwStart / SpecialLwBlast)
moves['dspec'] = {
    'name': 'C4', 'variants': [{'needs': {'charge': 'c4', 'uses': 1}, 'phases': [
        phase('plant', 24, cost={'charge': 'c4', 'uses': 1}, attach={'c4': {'show': [0, 23]}},
              anim=[{'at': 0, 'pose': 'guard'}, {'at': 3, 'pose': 'plant', 'lerp': True},
                    {'at': 12, 'pose': 'plant'}, {'at': 22, 'pose': 'guard', 'lerp': True}],
              events=[{'at': src(2), 'spawn': 'c4', 'sfx': 'swing'}],
              transitions=[{'on': 'timeout', 'to': 'end'}])]}],
    'phases': [phase('detonate', 32, anim=[
        {'at': 0, 'pose': 'guard'}, {'at': 4, 'pose': 'bust', 'lerp': True}, {'at': src(25), 'pose': 'bust'},
        {'at': 31, 'pose': 'guard', 'lerp': True}],
        events=[{'at': src(25), 'detonate': ['c4'], 'restore': {'charge': 'c4', 'uses': 1}, 'sfx': 'quake'}],
        transitions=[{'on': 'timeout', 'to': 'end'}])],
}

# ---------------------------------------------------------------- throws
def throw(slot, script, total, release, **kw):
    s = scripts['snake'][script]
    line = next(x for x in s.splitlines() if 'ATTACK_ABS(Kind=FIGHTER_ATTACK_ABSOLUTE_KIND_THROW' in x)
    a = args(line.split('THROW,', 1)[1].rstrip(')'))
    ledger['rawAttacks'][slot] = {'article': 'snake', 'script': script, 'attacks': [a]}
    b = hit(a, 0, 0, 0, 0, 60)
    b.update(kw)
    return {'release': release, 'total': total, 'hit': b, 'ticks': []}


throws = {
    'f': throw('fthrow', 'ThrowF', 39, src(21), x=0, y=0, r=60),
    # The engine launches a back throw along `-face`, so the source's 140 degree angle becomes 40.
    'b': throw('bthrow', 'ThrowB', 30, src(20), x=0, y=0, r=60, angle=40.0),
    # Source: a 7% catch-only contact on frame 22, then the 4% absolute throw on frame 23.
    'u': throw('uthrow', 'ThrowHi', 47, src(23), x=0, y=0, r=60),
    # Source angle 270 with no knockback lays the victim down; a small downward launch approximates the
    # knockdown in this engine's throw model.
    'd': throw('dthrow', 'ThrowLw', 41, src(35), x=0, y=0, r=60, bkb=25.0, kbg=0.35, angle=-90, type='spike'),
}
throws['u']['ticks'] = [{'at': src(22), 'damage': 7.0}]

# ---------------------------------------------------------------- projectiles
def proj(name, spawn, vel, r, life, hitb, **kw):
    p = {'name': name, 'spawn': list(spawn), 'vel': list(vel), 'r': r, 'life': life, 'max': 1, 'hit': hitb}
    p.update(kw)
    return {k: v for k, v in p.items() if v is not None}


def blast_hit(**kw):
    b = {'start': 0, 'x': 0, 'y': 0, 'r': 80, 'damage': 9.0, 'bkb': 60, 'kbg': 0.69, 'angle': 80,
         'shieldDmgMult': 0.5}
    b.update(kw)
    return b


def grenade(name, vel, life, maxn=1, spawn=(22, 52)):
    return proj(name, spawn, vel, 9, life, blast_hit(),
                max=maxn, gravity=0.30, maxFall=13.0,
                bounce={'restitution': 0.45, 'minSpeed': 1.2, 'friction': 0.72, 'wallRestitution': 0.5,
                        'maxBounces': 3, 'fizzleLife': life, 'sfx': 'land'},
                explosion={'armTicks': 0, 'radius': 80, 'sfx': 'quake',
                           'fx': [{'type': 'burst', 'color': '#ffd27a', 'n': 16, 'speed': 5.0, 'life': 18,
                                   'size': 4.5}]},
                spawnSfx='shoot', spin=8.0,
                # Cooking: the source fuse starts at the unpin, so every held tick removes one tick of life.
                chargeScale={'speed': 0.0, 'damage': 0.0, 'life': -1.0 / GRENADE_FUSE},
                shapes=[circle((0, 0), 9, '#3f5233', stroke='#9aa878', width=1.6),
                        circle((0, 0), 8.4, '#4c6140'),
                        line((-4, 2), (4, 2), 1.6, '#2b3624'),
                        line((2, 6), (5, 10), 3, '#b9a06a')])


projectiles = {
    'grenadeMid': grenade('Grenade', [4.6, 5.2], GRENADE_FUSE, maxn=2),
    'grenadeStrong': grenade('Grenade (smash throw)', [10.5, 3.0], GRENADE_FUSE),
    'grenadeLob': grenade('Grenade (underhand)', [1.8, 10.6], GRENADE_FUSE),
    'grenadeDrop': grenade('Grenade (drop)', [0.0, 0.0], GRENADE_FUSE, spawn=(0, 30)),
    'c4': proj('C4', (0, 16), (0, 0), 10, 1600,
               {'start': 0, 'x': 0, 'y': 0, 'r': 85, 'damage': 17.0, 'bkb': 40, 'kbg': 0.9, 'angle': 80},
               gravity=0.4, maxFall=14.0,
               bounce={'restitution': 0.0, 'minSpeed': 0.0, 'friction': 0.5, 'wallRestitution': 0.3,
                       'maxBounces': 8, 'fizzleLife': 1600},
               explosion={'manual': True, 'armTicks': 0, 'radius': 85, 'sfx': 'quake',
                          'fx': [{'type': 'burst', 'color': '#ffcf6a', 'n': 24, 'speed': 7.0, 'life': 22,
                                  'size': 5.5}]},
               spawnSfx='swing',
               shapes=[{'shape': 'rect', 'x': -9, 'y': -6, 'w': 18, 'h': 14, 'fill': '#4a5340',
                        'stroke': '#98a37c', 'width': 1.6},
                       circle((4, 4), 2.4, '#e0442c')]),
    'cypherShot': proj('Cypher', (0, 60), (0, 15), 15, 24,
                       {'start': 0, 'x': 0, 'y': 0, 'r': 15, 'damage': 6.0, 'bkb': 63, 'kbg': 0.17, 'angle': 42},
                       spawnSfx='slash',
                       shapes=[line((-8, 0), (8, 0), 5, '#8a9680'), circle((0, 0), 7, '#5d6a58', stroke='#9aa88f',
                                                                           width=1.4)]),
    'mortarShell': proj('Mortar Shell', (34, 66), (2.4, 11.5), 9, 100,
                        {'start': 0, 'x': 0, 'y': 0, 'r': 60, 'damage': 14.0, 'bkb': 87, 'kbg': 0.6, 'angle': 65},
                        gravity=0.34, maxFall=13.0,
                        bounce={'restitution': 0.2, 'minSpeed': 0.0, 'friction': 0.8, 'wallRestitution': 0.6,
                                'maxBounces': 0, 'fizzleLife': 3, 'sfx': 'land'},
                        explosion={'armTicks': 0, 'radius': 60, 'sfx': 'quake',
                                   'fx': [{'type': 'burst', 'color': '#ffc46a', 'n': 14, 'speed': 5.0, 'life': 16,
                                           'size': 4.0}]},
                        chargeScale={'speed': 0.00571, 'damage': 0.0},
                        spawnSfx='shoot', spin=10.0,
                        shapes=[circle((0, 0), 8, '#4d5343', stroke='#9aa08a', width=1.4),
                                line((0, -4), (0, 6), 2, '#c9b98a')]),
}
projectiles['nikita'] = proj(
    'Nikita', (34, 46), (2.4, 0), 12, 200,
    {'start': 0, 'x': 0, 'y': 0, 'r': 45, 'damage': 14.0, 'bkb': 75, 'kbg': 0.7, 'angle': 50},
    spawnSfx='shoot',
    article={'slot': 'sspec', 'minRelease': 0, 'autoRelease': 150, 'releaseOnUp': True, 'steer': 2.6,
             'stages': [{'from': 0, 'startup': 0, 'frames': 100, 'vel': [4.4, 0],
                         'hitboxes': [
                             # One box: the source has a single 14% explosion, and a second outer ring
                             # would steal straight hits from the strong box.
                             {'start': 0, 'x': 0, 'y': 0, 'r': 65, 'damage': 14.0, 'bkb': 75, 'kbg': 0.7,
                              'angle': 50}],
                         'shapes': [circle((0, 0), 11, '#5a604c', stroke='#99a082', width=1.6),
                                    line((-10, 0), (12, 0), 5, '#7d8a6a'),
                                    line((12, -5), (18, -5), 4, '#3b4133'),
                                    line((12, 5), (18, 5), 4, '#3b4133'),
                                    circle((0, 0), 3.4, '#e0a24c')],
                         'attackShapes': [circle((0, 0), 45, '#ff9a4a', alpha=0.25),
                                          circle((0, 0), 65, '#ffd18a', alpha=0.16),
                                          circle((0, 0), 20, '#fff0c4', alpha=0.4)]}]},
    shapes=[circle((0, 0), 11, '#5a604c', stroke='#99a082', width=1.6), line((-10, 0), (12, 0), 5, '#7d8a6a'),
            line((12, -5), (18, -5), 4, '#3b4133'), line((12, 5), (18, 5), 4, '#3b4133'),
            circle((0, 0), 3.4, '#e0a24c')],
)

# ---------------------------------------------------------------- fighter document
stats = {
    'weight': round(100 / 106, 8), 'pummelDamage': 1.3,
    'walkSpeed': 3.087, 'runSpeed': 5.5825, 'dashSpeed': 6.16, 'dashFrames': 14, 'runAccel': 0.62,
    'groundFriction': 0.82,
    'jumpSpeed': 13.0, 'shortHopSpeed': 9.0, 'airJumpSpeed': 12.6, 'airJumpDrift': 3.455, 'airJumps': 1,
    'airSpeed': 3.455, 'airAccel': 0.105, 'gravity': 0.48, 'fallSpeed': 10.38, 'fastFallSpeed': 16.608,
}
doc = {
    '$schema': '../../schema/fighter.schema.json',
    'schemaVersion': 1,
    'extends': 'base',
    'name': 'Sneak',
    'tagline': 'Mercenary with a grenade in each hand',
    'blurb': 'A slow, heavy tactical fighter. Cooks and lobs grenades, steers a remote missile, plants a '
             'remote C4 and recovers on the Cypher. Everything he throws explodes, including at his feet.',
    'stats': stats,
    'charges': [{'id': 'c4', 'uses': 1, 'recharge': 0, 'emptySfx': 'toss', 'refillSfx': 'ready'}],
    'ai': {
        'reach': 74, 'counterChance': 0,
        'specials': [
            {'minDistance': 170, 'height': 60, 'grounded': True, 'noProjectile': True, 'chance': 0.5,
             'neutral': True, 'down': False, 'hold': None},
            {'minDistance': 90, 'maxDistance': 260, 'height': 80, 'grounded': True, 'noProjectile': True,
             'chance': 0.4, 'neutral': False, 'down': False, 'hold': None},
            {'minDistance': 0, 'maxDistance': 120, 'height': 60, 'grounded': True, 'noProjectile': True,
             'chance': 0.25, 'neutral': False, 'down': True, 'hold': None},
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

def camel(v):
    """Snake_case dict keys become the camelCase the content schema expects; joint/pose names are untouched."""
    if isinstance(v, dict):
        return {(re.sub(r'_([a-z])', lambda m: m.group(1).upper(), k) if '_' in k else k): camel(x) for k, x in v.items()}
    if isinstance(v, list):
        return [camel(x) for x in v]
    return v


doc = camel(doc)
ledger['moves'] = {slot: [{'name': p['name'], 'frames': p.get('frames'),
                           'hits': [[h['start'], h.get('end'), h['damage'], h.get('angle')]
                                    for h in p.get('hitboxes', [])]} for p in m['phases']]
                   for slot, m in doc['moves'].items()}

out = ROOT / 'assets/fighters/sneak/fighter.json'
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(doc, indent=2) + '\n')
(SOURCES / 'ledger.json').write_text(json.dumps(ledger, indent=1) + '\n')
print(f'wrote {out.relative_to(ROOT)} ({out.stat().st_size} bytes)')
