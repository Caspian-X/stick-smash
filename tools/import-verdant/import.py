#!/usr/bin/env python3
"""Rebuild Bleth (SSBU Byleth) from pinned 13.0.1 scripts and documented authoring rules.

Numbers come from the pinned attack scripts (damage, launch fields, masks, hitlag, rehit, armour) and the
pinned Ultimate Frame Data rows (startup, active windows, totals, landing lag, autocancel, charge frames,
movement). Root-bone (`top`) geometry converts at 5 px per source unit; every weapon-bone hitbox is *fitted*
to the per-move UFD hitbox visualisations and the stick-figure poses authored below. Run `just import-verdant`.
"""
import copy, importlib, json, math, re, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
units = importlib.import_module('units')
SCALE = units.SCALE  # px per source unit: geometry, speeds (units/frame) and accelerations (units/frame²)

# Phase drift (`airAccelMult`) is written as a multiple of the old shared air accels (aerial 0.45, attack 0.25,
# upSpecial 0.3, px/tick² at the retired 3.5 px/unit speed scale, hence × SCALE / 3.5). Under `movement.ultimate`
# it multiplies airAccel × the kind's multiplier instead, so drift() stores the value that keeps the same
# effective acceleration.
AIR_ACCEL = round(0.053 * SCALE, 5)  # UFD air acceleration 0.053
def drift(legacy, kind):
    old = {'aerial': 0.45, 'attack': 0.25, 'upSpecial': 0.3}[kind] * SCALE / 3.5
    mult = {'aerial': 1.0, 'attack': 0.55, 'upSpecial': 0.65}[kind]
    return round(legacy * old / (AIR_ACCEL * mult), 4)

SOURCES = ROOT / 'docs/imports/sources/verdant'
source = json.loads((SOURCES / 'scripts.json').read_text())
ufd = json.loads((SOURCES / 'ufd.json').read_text())
scripts = {art: {s['Name']: s['Data'] for s in ss} for art, ss in source['scripts'].items()}
ledger = {'sourceCommit': source['commit'], 'sourceSha256': source['sha256'], 'geometryScale': 5, 'rawAttacks': {},
          'phases': {}}


# ---------------------------------------------------------------- script parsing
def expand(s):
    while (m := re.search(r'for\((\d+) Iterations\)\{', s)):
        pos = m.end(); depth = 1
        while depth:
            depth += (s[pos] == '{') - (s[pos] == '}'); pos += 1
        s = s[:m.start()] + expand(s[m.end():pos - 1]) * int(m[1]) + s[pos:]
    return s


def args(s):
    return {k: v.strip() for k, v in re.findall(r'(\w+)=(.*?)(?=, \w+=|$)', s)}


def attacks(article, name):
    """Attack records with their animation-frame windows (`start`..`end-1`)."""
    src = expand(scripts[article][name])
    t, live, out, motion = 0, {}, [], []

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
        if m := re.search(r'FT_MOTION_RATE\(FSM=([\d.]+)\)', line):
            motion.append([t, float(m[1])])
        if 'clear_all()' in line:
            for i in list(live):
                clear(i)
        elif m := re.search(r'AttackModule::clear\(ID=(\d+)\)', line):
            clear(int(m[1]))
        if m := re.search(r'\bATTACK\((.*)\)', line):
            a = args(m[1]); i = int(a['ID']); clear(i)
            live[i] = {'start': t, 'end': t + 1, 'source': a}
    for i in list(live):
        clear(i)
    return {'motion': motion, 'attacks': sorted(out, key=lambda b: (b['start'], int(b['source']['ID'])))}


def raw(slot, article, name):
    rows = attacks(article, name)
    ledger['rawAttacks'][slot] = {'article': article, 'script': name, **rows}
    return rows['attacks']


def take(rows, damage, nth=0):
    found = [b for b in rows if abs(float(b['source']['Damage']) - damage) < 1e-6]
    return found[nth]


def take_bone(rows, bone, damage, nth=0):
    found = [b for b in rows if bone in b['source'].get('Bone', '') and abs(float(b['source']['Damage']) - damage) < 1e-6]
    return found[nth]


def num(a, k, default=0):
    try:
        return float(a.get(k, default))
    except ValueError:
        return default


# ---------------------------------------------------------------- conversion

def frame(source_frame):
    """Source gameplay frame -> phase frame. The move's first tick is source frame 1 = phase frame 0."""
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
        b['fkb'] = num(a, 'FKB')  # Ultimate WBKB (`fkb`); bkb/kbg stay the source values
    if num(a, 'Rehit'):
        b['rehit'] = int(num(a, 'Rehit'))
    if num(a, 'Hitlag', 1) != 1:
        b['hitlagMult'] = num(a, 'Hitlag', 1)
    if num(a, 'ShieldDamage') and b['damage'] > 0:
        b['shieldDmgMult'] = round(max(0, (b['damage'] + num(a, 'ShieldDamage')) / b['damage']), 4)
    b.update(kw)
    return b


def root(a, start, end, **kw):
    """Root-bone (`top`) attack: source Z is forward and Y is up, 5 px per unit. A capsule becomes overlapping
    circles no farther apart than their radius."""
    x, y, r = num(a, 'Z') * SCALE, num(a, 'Y') * SCALE, num(a, 'Size') * SCALE
    if a.get('Z2', 'LUA_VOID') == 'LUA_VOID':
        return [hit(a, start, end, x, y, r, **kw)]
    x2, y2 = num(a, 'Z2') * SCALE, num(a, 'Y2') * SCALE
    steps = max(1, math.ceil(math.hypot(x2 - x, y2 - y) / max(r, 1)))
    return [hit(a, start, end, x + (x2 - x) * i / steps, y + (y2 - y) * i / steps, r, **kw) for i in range(steps + 1)]


def swept(a, pts, **kw):
    """Fitted per-frame track: `pts` is [(phase frame, x, y, r)] over the active window."""
    b = hit(a, pts[0][0], pts[-1][0], pts[0][1], pts[0][2], pts[0][3], **kw)
    b['frames'] = [[t, round(px, 2), round(py, 2), round(r, 2)] for t, px, py, r in pts]
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


def swing(pose, start, end, total, air=False):
    rest = 'air' if air else 'stand'
    return [{'at': 0, 'pose': rest},
            {'at': max(0, start - 3), 'pose': pose, 'lerp': True}, {'at': end, 'pose': pose},
            {'at': min(max(1, total - 1), end + 6), 'pose': rest, 'lerp': True}]


# ---------------------------------------------------------------- poses and bodies
stand = {'head': [2, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [12, 45], 'handF': [22, 41],
         'elbowB': [-12, 45], 'handB': [-20, 39], 'kneeF': [4, 17], 'footF': [6, 1], 'kneeB': [-5, 17],
         'footB': [-8, 1]}
poses = {
    'stand': stand,
    # Areadbhar / sword thrust: weight forward, both hands on the haft.
    'thrust': {'head': [6, 60], 'shoulder': [3, 50], 'hip': [-2, 30], 'elbowF': [20, 50], 'handF': [38, 50],
               'elbowB': [-8, 46], 'handB': [-14, 44], 'kneeF': [14, 16], 'footF': [24, 1], 'kneeB': [-10, 16],
               'footB': [-16, 1]},
    # Downward slash after an overhead wind-up.
    'slashDown': {'head': [4, 58], 'shoulder': [2, 48], 'hip': [-2, 28], 'elbowF': [16, 44], 'handF': [34, 38],
                  'elbowB': [-10, 42], 'handB': [-16, 38], 'kneeF': [12, 15], 'footF': [20, 1], 'kneeB': [-8, 15],
                  'footB': [-14, 1]},
    # Sword or whip held overhead.
    'sweepUp': {'head': [-4, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [8, 64], 'handF': [10, 82],
                'elbowB': [-12, 46], 'handB': [-18, 40], 'kneeF': [4, 17], 'footF': [6, 1], 'kneeB': [-6, 17],
                'footB': [-9, 1]},
    # Crouched low sweep.
    'sweepLow': {'head': [8, 48], 'shoulder': [4, 38], 'hip': [-2, 20], 'elbowF': [22, 32], 'handF': [42, 24],
                 'elbowB': [-10, 32], 'handB': [-18, 22], 'kneeF': [20, 11], 'footF': [32, 1], 'kneeB': [-8, 10],
                 'footB': [-14, 1]},
    # Aymr raised above the head.
    'overhead': {'head': [-6, 66], 'shoulder': [-4, 53], 'hip': [2, 33], 'elbowF': [2, 64], 'handF': [0, 82],
                 'elbowB': [-14, 48], 'handB': [-22, 42], 'kneeF': [8, 17], 'footF': [12, 1], 'kneeB': [-8, 17],
                 'footB': [-14, 1]},
    # Aymr buried in the ground in front.
    'slam': {'head': [10, 52], 'shoulder': [6, 42], 'hip': [0, 22], 'elbowF': [20, 36], 'handF': [38, 26],
             'elbowB': [-8, 34], 'handB': [-14, 26], 'kneeF': [18, 12], 'footF': [28, 1], 'kneeB': [-10, 12],
             'footB': [-16, 1]},
    # Failnaught drawn.
    'draw': {'head': [2, 64], 'shoulder': [0, 50], 'hip': [-2, 30], 'elbowF': [18, 50], 'handF': [36, 52],
             'elbowB': [-16, 52], 'handB': [-26, 54], 'kneeF': [6, 16], 'footF': [10, 1], 'kneeB': [-8, 16],
             'footB': [-12, 1]},
    # Sword of the Creator stabbed along the ground.
    'whipLow': {'head': [10, 44], 'shoulder': [6, 34], 'hip': [0, 18], 'elbowF': [22, 28], 'handF': [40, 16],
                'elbowB': [-8, 28], 'handB': [-16, 18], 'kneeF': [22, 10], 'footF': [34, 1], 'kneeB': [-8, 9],
                'footB': [-14, 1]},
    'air': {'head': [2, 64], 'shoulder': [0, 51], 'hip': [0, 32], 'elbowF': [12, 46], 'handF': [24, 44],
            'elbowB': [-12, 44], 'handB': [-22, 40], 'kneeF': [10, 19], 'footF': [14, 5], 'kneeB': [-8, 17],
            'footB': [-14, 4]},
    'airHigh': {'head': [0, 64], 'shoulder': [0, 51], 'hip': [-2, 32], 'elbowF': [8, 60], 'handF': [10, 78],
                'elbowB': [-10, 46], 'handB': [-18, 42], 'kneeF': [12, 18], 'footF': [18, 4], 'kneeB': [-6, 16],
                'footB': [-10, 6]},
    'airLow': {'head': [10, 58], 'shoulder': [4, 46], 'hip': [-2, 28], 'elbowF': [20, 32], 'handF': [34, 18],
               'elbowB': [-10, 36], 'handB': [-18, 26], 'kneeF': [20, 14], 'footF': [34, 6], 'kneeB': [-6, 14],
               'footB': [-12, 4]},
    'win': {'head': [0, 68], 'shoulder': [0, 54], 'hip': [0, 34], 'elbowF': [16, 48], 'handF': [26, 64],
            'elbowB': [-16, 52], 'handB': [-28, 60], 'kneeF': [5, 17], 'footF': [9, 1], 'kneeB': [-6, 17],
            'footB': [-10, 1]},
}


def line(a, b, w, c, **kw):
    return {'shape': 'line', 'from': list(a), 'to': list(b), 'width': w, 'color': c, **kw}


def poly(points, c, **kw):
    return {'shape': 'poly', 'points': [list(p) for p in points], 'fill': c, **kw}


def circle(at, r, fill=None, **kw):
    s = {'shape': 'circle', 'at': list(at), 'r': r, **kw}
    if fill:
        s['fill'] = fill
    return s


SWORD = 62  # Sword of the Creator blade length (fitted)
LANCE = 126  # Areadbhar haft + head reach from the grip
AXE = 62  # Aymr haft to the blade's near edge
attachments = [
    {'id': 'sword', 'joint': 'handF', 'angle': 95, 'hidden': True, 'glow': 'active',
     'hold': {'elbow': 14, 'hand': 28, 'bend': 16}, 'trail': {'inner': 18, 'outer': 72},
     'shapes': [
         line((-13, 0), (0, 0), 5, '#463a2c'),
         line((-2, -7), (2, 7), 4, '#b9a45a'),
         line((2, 0), (SWORD, 0), 3.4, '#c9cfd8'),
         line((2, 2.2), (SWORD - 4, 2.2), 1.1, '#f2f6ff'),
         line((0, 0), (SWORD, 0), 7, '#66ffb2', alpha=0.35, when='glow'),
         line((2, 0), (SWORD, 0), 2.2, '#d8ffe9', when='glow'),
     ]},
    {'id': 'lance', 'joint': 'handF', 'angle': 90, 'hidden': True, 'glow': 'active',
     'hold': {'elbow': 16, 'hand': 32, 'bend': 8}, 'trail': {'inner': 40, 'outer': 140},
     'shapes': [
         line((-26, 0), (AXE + 50, 0), 4.4, '#4a4238'),
         circle((0, 0), 4.5, '#7a6c52'),
         poly([(104, 6), (LANCE, 1.5), (LANCE + 12, 0), (LANCE, -1.5), (104, -6)], '#cfd8e6'),
         line((100, 0), (LANCE, 0), 2, '#f2f6ff'),
         line((96, 0), (LANCE + 10, 0), 11, '#ffd27a', alpha=0.3, when='glow'),
         line((100, 0), (LANCE + 4, 0), 2.4, '#fff2c4', when='glow'),
     ]},
    {'id': 'axe', 'joint': 'handF', 'angle': 90, 'hidden': True, 'glow': 'active',
     'hold': {'elbow': 14, 'hand': 22, 'bend': 10}, 'trail': {'inner': 16, 'outer': 84},
     'shapes': [
         line((-22, 0), (AXE - 6, 0), 6, '#3a332c'),
         line((AXE - 12, 0), (AXE + 2, 0), 8, '#5c5148'),
         poly([(AXE - 4, 9), (AXE + 20, 17), (AXE + 30, 4), (AXE + 22, -13), (AXE - 4, -9), (AXE + 6, 0)],
              '#c8b06a'),
         poly([(AXE + 20, 17), (AXE + 30, 4), (AXE + 22, -13), (AXE + 16, 0)], '#8d7c4a'),
         poly([(AXE - 4, 9), (AXE + 18, 16), (AXE + 26, 4), (AXE + 18, -12), (AXE - 4, -9), (AXE + 4, 0)],
              '#ffd27a', alpha=0.35, when='glow'),
     ]},
    {'id': 'bow', 'joint': 'handF', 'angle': 90, 'hidden': True, 'glow': 'active',
     'hold': {'elbow': 12, 'hand': 26, 'bend': 14}, 'trail': {'inner': 14, 'outer': 56},
     'shapes': [
         poly([(26, 38), (14, 40), (4, 26), (0, 0), (4, -26), (14, -40), (26, -38), (16, -24), (10, 0), (16, 24)],
              '#5a4a33'),
         line((30, 39), (24, 38), 3.4, '#c8b06a'),
         line((30, -39), (24, -38), 3.4, '#c8b06a'),
         line((28, 39), (6, 0), 1.3, '#e8e2c8'),
         line((6, 0), (28, -39), 1.3, '#e8e2c8'),
         line((4, 28), (2, 0), 3.6, '#66ffb2', alpha=0.4, when='glow'),
         line((2, 0), (4, -28), 3.6, '#66ffb2', alpha=0.4, when='glow'),
     ]},
]
style = {'headRadius': 11, 'torsoWidth': 7, 'frontLimbWidth': 6, 'backLimbWidth': 5.5, 'glowWidth': 2.4,
         'torsoGlowWidth': 3, 'headGlowRadius': 2.6, 'headRing': 2, 'headFill': '#16281f',
         'eye': [2, 0, 4, 2]}

anims = {
    'idle': {'clock': 'world', 'loop': 44.0, 'keys': [
        {'at': 0, 'pose': 'stand'},
        {'at': 22, 'pose': {'from': 'stand', 'head': [2, 68], 'handB': [-20, 41]}, 'lerp': True}]},
    'walk': {'clock': 'world', 'loop': 28.0, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'kneeF': [6, 17], 'footF': [12, 1], 'kneeB': [-6, 17], 'footB': [-12, 1]}},
        {'at': 14, 'pose': {'from': 'stand', 'kneeF': [-2, 17], 'footF': [-6, 1], 'kneeB': [4, 17], 'footB': [8, 1]},
         'lerp': True}]},
    'run': {'clock': 'run', 'loop': 360.0, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'head': [4, 64], 'hip': [1, 32], 'kneeF': [16, 18], 'footF': [24, 6],
                           'kneeB': [-14, 16], 'footB': [-22, 2], 'elbowF': [16, 50], 'handF': [26, 52],
                           'elbowB': [-16, 46], 'handB': [-26, 42]}},
        {'at': 180, 'pose': {'from': 'stand', 'head': [4, 64], 'hip': [1, 32], 'kneeF': [-14, 16], 'footF': [-22, 2],
                             'kneeB': [16, 18], 'footB': [24, 6], 'elbowF': [16, 50], 'handF': [26, 52],
                             'elbowB': [-16, 46], 'handB': [-26, 42]}, 'lerp': True}]},
    'air.rise': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'kneeF': [14, 19], 'footF': [20, 8], 'kneeB': [-8, 17], 'footB': [-12, 10]}}]},
    'air.fall': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'kneeF': [8, 17], 'footF': [10, 2], 'kneeB': [-6, 16], 'footB': [-8, 4]}}]},
    'helpless': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'head': [-4, 60], 'elbowF': [8, 42], 'handF': [16, 30], 'elbowB': [-14, 42],
                           'handB': [-24, 32]}}]},
    'landing': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'sweepLow', 'head': [2, 44]}},
        {'at': 9, 'pose': 'stand', 'lerp': True}]},
    'crouch': {'clock': 'state', 'keys': [{'at': 0, 'pose': {'from': 'sweepLow', 'head': [4, 47]}}]},
    'jumpsquat': {'clock': 'state', 'keys': [{'at': 0, 'pose': {'from': 'sweepLow', 'head': [2, 50]}}]},
    'shield': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'elbowF': [14, 46], 'handF': [24, 46]}}]},
    'hitstun': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'head': [-8, 62], 'handF': [10, 36], 'handB': [-24, 32]}}]},
    'tumble': {'clock': 'spin', 'keys': [{'at': 0, 'pose': {'from': 'air', 'head': [-6, 62]}}]},
    'win': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'win'}, {'at': 22, 'pose': {'from': 'win', 'handF': [30, 74]}, 'lerp': True}]},
    'respawn': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'air'}]},
}


# ---------------------------------------------------------------- weapon wielding
def wield(weapon, keys, scale=None, show=(0, 999), offset=None):
    """One weapon's attachment keys for a phase. Attachments are hidden by default, so every wielding phase
    shows exactly the weapon the source gives the move."""
    a = {'show': [int(show[0]), int(show[1])], 'angle': [[float(f), float(d)] for f, d in keys]}
    if offset is not None:
        a['offset'] = [[float(f), float(x), float(y)] for f, x, y in offset]
    if scale is not None:
        a['scale'] = [[float(f), float(s)] for f, s in (scale if isinstance(scale, list) else [[0, scale]])]
    return {weapon: a}


SWORD_HOLD, LANCE_HOLD, AXE_HOLD, BOW_HOLD = 'sword', 'lance', 'axe', 'bow'


def key_at(keys, t):
    """The renderer's linear (non-wrapping) attachment-angle sample at frame `t`."""
    if t <= keys[0][0]:
        return keys[0][1]
    for (f0, d0), (f1, d1) in zip(keys, keys[1:]):
        if t <= f1:
            return d0 if f1 == f0 else d0 + (d1 - d0) * (t - f0) / (f1 - f0)
    return keys[-1][1]


def rebase(keys, lo, hi):
    """A phase's slice of a move-level angle track: its value at the phase's first frame, then the later keys.
    Without this every sub-phase would restart the swing at the move's first key."""
    out = [[0.0, round(key_at(keys, lo), 4)]]
    out += [[f - lo, d] for f, d in keys if lo < f <= hi]
    return out


# ---------------------------------------------------------------- aerials
def aerial(slot, name, total, landing, ac_start, ac_end, boxes, pose, weapon, attach, air_pose='air', split=None):
    """Split an aerial into start (initial autocancel), main (landing lag) and late (ending autocancel)."""
    start, late = frame(ac_start), frame(ac_end)
    boxes = assign_groups(boxes)
    spans = [(0, start, 'startup'), (start, late, 'main'), (late, total, 'late')]
    if split is not None:
        spans = [(0, start, 'startup'), (start, split, 'swing1'), (split, late, 'swing2'), (late, total, 'late')]
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
        ph['transitions'] = [land(landing) if tag in ('main', 'swing1', 'swing2') else {'on': 'land', 'to': 'end'}]
        if cut:
            ph['anim'] = [{'at': 0, 'pose': air_pose}] + \
                [{'at': max(0, cut[0]['start'] - 2), 'pose': pose, 'lerp': True},
                 {'at': max(1, cut[-1].get('end', 0)), 'pose': pose},
                 {'at': min(hi - lo - 1, cut[-1].get('end', 0) + 3), 'pose': air_pose, 'lerp': True}]
        else:
            ph['anim'] = [{'at': 0, 'pose': air_pose}]
        ph['attach'] = wield(weapon, rebase(attach, lo, hi))
        parts.append(ph)
    for i in range(len(parts) - 1):
        parts[i]['transitions'].append({**wait(parts[i + 1]['name']), 'preserveHits': True})
    return {'name': name, 'air': 'aerial', 'phases': parts}


moves = {}

# ---------------------------------------------------------------- jab: three sword pokes, then the whip flurry
j = raw('jab', 'master', 'Attack11')
jab1 = root(take(j, 1.5, 0)['source'], frame(4), frame(5)) + \
    root(take(j, 1.5, 1)['source'], frame(4), frame(5)) + root(take(j, 1.5, 2)['source'], frame(4), frame(5))
moves['jab'] = {'name': 'Creator Jab', 'phases': [
    phase('jab1', 22, jab1, anim=swing('thrust', frame(4), frame(5), 22),
          attach=wield(SWORD_HOLD, [[0, 95], [2, 40], [4, 0], [18, 60]]),
          transitions=[{'on': 'attackHeld', 'from': 10, 'to': 'jab2'}, wait('end')])]}
j2 = raw('jab2', 'master', 'Attack12')
jab2 = root(take(j2, 2.0, 0)['source'], frame(4), frame(5)) + root(take(j2, 2.0, 1)['source'], frame(4), frame(5))
moves['jab']['phases'][1:1] = [phase(
    'jab2', 24, jab2, anim=swing('thrust', frame(4), frame(5), 24),
    attach=wield(SWORD_HOLD, [[0, 60], [2, 20], [4, -5], [20, 60]]),
    transitions=[{'on': 'attackHeld', 'from': 8, 'to': 'jab3'}, wait('end')])]
j3 = raw('jab3', 'master', 'Attack13')
jab3 = root(take(j3, 4.5, 0)['source'], frame(5), frame(7)) + root(take(j3, 4.5, 1)['source'], frame(5), frame(7))
moves['jab']['phases'][2:2] = [phase(
    'jab3', 34, jab3, anim=swing('thrust', frame(5), frame(7), 34),
    attach=wield('sword', [[0, 120], [3, 30], [5, 0], [30, 70]], scale=1.25),
    transitions=[{'on': 'attackHeld', 'from': 10, 'to': 'rapidStart'}, wait('end')])]
rr = raw('rapid', 'master', 'Attack100Sub')
rapid = [hit(rr[0]['source'], 0, 0, 45, 35, 30, rehit=3), hit(rr[1]['source'], 0, 0, 90, 35, 30, rehit=3)]
rapid[0].pop('end'); rapid[1].pop('end')
moves['jab']['phases'].append(phase(
    'rapidStart', 4, anim=[{'at': 0, 'pose': 'thrust'}],
    attach=wield('sword', [[0, 95], [3, 0]], scale=1.8), transitions=[wait('rapid')]))
moves['jab']['phases'].append(phase(
    'rapid', 60, rapid, anim=[{'at': 0, 'pose': 'thrust'}],
    attach=wield('sword', [[0, 0]], scale=1.8), events=[{'at': 0, 'sfx': 'swing'}],
    transitions=[{'on': 'release', 'to': 'rapidEnd'}, wait('rapid')]))
re_ = raw('rapidEnd', 'master', 'Attack100End')
rapid_end = [swept(re_[0]['source'], [(frame(6), 80, 40, 30), (frame(7), 92, 40, 30)])]
moves['jab']['phases'].append(phase(
    'rapidEnd', 50, rapid_end, anim=swing('thrust', frame(6), frame(7), 50),
    attach=wield('sword', [[0, 60], [4, 0], [6, -10], [40, 60]], scale=1.8),
    events=[{'at': frame(6), 'sfx': 'slash'}]))

# ---------------------------------------------------------------- tilts and dash attack
ft = raw('ftilt', 'master', 'AttackS3')
ftilt = root(take_bone(ft, 'top', 11.0)['source'], frame(8), frame(10)) + \
    [swept(take_bone(ft, 'sword1', 11.0, 0)['source'], [(frame(8), 40, 58, 17.5), (frame(9), 56, 48, 17.5),
                                                       (frame(10), 72, 38, 17.5)])]
moves['ftilt'] = {'name': 'Creator Slash', 'phases': [phase(
    'main', 34, ftilt, anim=swing('slashDown', frame(8), frame(10), 34),
    attach=wield('sword', [[0, 140], [5, 70], [8, 5], [10, -15], [34, 60]]),
    events=[{'at': frame(8), 'sfx': 'slash'}])]}
ut = raw('utilt', 'master', 'AttackHi3')
utilt = [swept(take_bone(ut, 'sword1', 10.0, 0)['source'],
               [(frame(9), 42, 58, 17.5), (frame(11), 44, 80, 17.5), (frame(13), 34, 100, 17.5),
                (frame(15), 24, 114, 17.5)]),
         hit(take_bone(ut, 'sword1', 10.0, 2)['source'], frame(9), frame(15), 30, 72, 18)]
moves['utilt'] = {'name': 'Rising Creator', 'phases': [phase(
    'main', 34, utilt, anim=swing('sweepUp', frame(9), frame(15), 34),
    attach=wield('sword', [[0, 160], [4, 340], [8, 390], [12, 455], [34, 455]]),
    events=[{'at': frame(9), 'sfx': 'slash'}])]}
dt = raw('dtilt', 'master', 'AttackLw3')
dtilt = [swept(take(dt, 8.0, 0)['source'], [(frame(13), 52, 14, 14), (frame(15), 62, 14, 14)]),
         swept(take(dt, 8.0, 1)['source'], [(frame(13), 86, 14, 14), (frame(15), 126, 14, 14)])]
moves['dtilt'] = {'name': 'Whip Sweep', 'phases': [phase(
    'main', 35, dtilt, anim=swing('whipLow', frame(13), frame(15), 35),
    attach=wield('sword', [[0, 40], [10, 5], [13, 0], [35, 40]], scale=2.1),
    events=[{'at': frame(13), 'sfx': 'slash'}])]}
da = raw('dashatk', 'master', 'AttackDash')
dashatk = [swept(take_bone(da, 'sword1', 13.0, 0)['source'],
                 [(frame(9), 55, 32, 18), (frame(10), 80, 32, 20), (frame(11), 96, 32, 20)]),
           hit(take_bone(da, 'sword1', 9.0)['source'], frame(9), frame(11), 40, 32, 17.5),
           hit(take_bone(da, 'top', 13.0)['source'], frame(9), frame(11), 70, 32, 25)]
moves['dashatk'] = {'name': 'Dash Thrust', 'phases': [phase(
    'main', 44, dashatk, slide=frame(20), anim=swing('thrust', frame(9), frame(11), 44),
    attach=wield('sword', [[0, 120], [6, 20], [9, 0], [20, 30], [44, 70]]),
    events=[{'at': 0, 'setVel': {'x': 6.0}}, {'at': frame(9), 'sfx': 'slash'}])]}

# ---------------------------------------------------------------- smashes
fs = raw('fsmash', 'master', 'AttackS4')
fsmash = [
    hit(take_bone(fs, 'haver', 18.0, 0)['source'], frame(23), frame(25), 132, 44, 13),
    hit(take_bone(fs, 'haver', 18.0, 1)['source'], frame(23), frame(25), 150, 46, 10),
    hit(take_bone(fs, 'haver', 12.0)['source'], frame(23), frame(25), 70, 44, 16),
] + root(take_bone(fs, 'top', 12.0)['source'], frame(23), frame(25))
moves['fsmash'] = {'name': 'Areadbhar', 'phases': [phase(
    'main', 63, fsmash, charge={'frame': frame(14)},
    anim=swing('thrust', frame(16), frame(25), 63),
    attach=wield('lance', [[0, 150], [frame(14), 120], [frame(20), 40], [frame(23), 0], [frame(40), 20],
                           [63, 90]]),
    events=[{'at': frame(14), 'sfx': 'smash'}, {'at': frame(23), 'sfx': 'slash'}])]}
us = raw('usmash', 'master', 'AttackHi4')
usmultihit = [
    swept(take(us, 2.0, 0)['source'], [(frame(13), 28, 52, 35), (frame(14), 22, 78, 35), (frame(16), 14, 100, 35)]),
    # Source frames 17-27 are the second multihit window with its own (4-frame) rehit.
    hit(take(us, 2.0, 2)['source'], frame(17), frame(27), 0, 120, 27.5),
    hit(take(us, 2.0, 2)['source'], frame(17), frame(27), -32, 130, 25),
    hit(take(us, 2.0, 2)['source'], frame(17), frame(27), 50, 130, 25),
]
usfinal = [hit(take(us, 10.0)['source'], 0, 1, 10, 122, 32.5)]
moves['usmash'] = {'name': 'Creator Pillar', 'phases': [
    phase('windup', 27, usmultihit, charge={'frame': frame(7)},
          anim=swing('sweepUp', frame(11), 26, 27),
          attach=wield('sword', [[0, 200], [frame(7), 180], [frame(11), 90], [27, 90]], scale=2.2),
          events=[{'at': frame(7), 'sfx': 'smash'}, {'at': frame(13), 'sfx': 'slash'}],
          transitions=[wait('final')]),
    phase('final', 2, usfinal, anim=[{'at': 0, 'pose': 'sweepUp'}],
          attach=wield('sword', [[0, 90]], scale=2.2),
          transitions=[wait('recover')]),
    phase('recover', 27, anim=[{'at': 0, 'pose': 'sweepUp'}, {'at': 20, 'pose': 'stand', 'lerp': True}],
          attach=wield('sword', [[0, 90], [20, 95]], scale=2.2)),
]}
ds = raw('dsmash', 'master', 'AttackLw4')
dsfront = [
    hit(take_bone(ds, 'top', 23.0, 0)['source'], frame(19), frame(21), 60, 18, 18.5),
    hit(take_bone(ds, 'haver', 23.0, 0)['source'], frame(19), frame(21), 78, 42, 22),
]
dsback = [
    hit(take_bone(ds, 'top', 23.0, 1)['source'], 0, 2, -55, 18, 18.5),
    hit(take_bone(ds, 'haver', 23.0, 1)['source'], 0, 2, -74, 42, 22),
]
moves['dsmash'] = {'name': 'Aymr Sweep', 'phases': [
    phase('front', 28, dsfront, charge={'frame': frame(3)},
          anim=[{'at': 0, 'pose': 'stand'}, {'at': 8, 'pose': 'overhead', 'lerp': True},
                {'at': frame(19), 'pose': 'sweepLow', 'lerp': True}, {'at': frame(21), 'pose': 'sweepLow'}],
          attach=wield('axe', [[0, 120], [8, 150], [frame(19), 10], [frame(21), -5]]),
          events=[{'at': frame(3), 'sfx': 'smash'}, {'at': frame(19), 'sfx': 'slash'}],
          transitions=[wait('back')]),
    phase('back', 46, dsback,
          anim=[{'at': 0, 'pose': 'sweepLow'}, {'at': 25, 'pose': 'stand', 'lerp': True}],
          attach=wield('axe', [[0, -5], [2, 190], [4, 170], [20, 120], [46, 95]]),
          events=[{'at': 0, 'sfx': 'slash'}]),
]}

# ---------------------------------------------------------------- aerials
an = raw('nair', 'master', 'AttackAirN')
nair = [
    hit(take(an, 2.5, 0)['source'], frame(6), frame(26), 34, 32, 17),
    hit(take(an, 2.5, 1)['source'], frame(6), frame(26), -20, 20, 18),
    hit(take(an, 2.5, 4)['source'], frame(6), frame(26), 0, 46, 15),
    hit(take(an, 3.0)['source'], frame(28), frame(28), 0, 46, 50),
]
n = aerial('nair', 'Quiver Spin', 51, 13, 6, 39, nair, 'air', BOW_HOLD,
           [[0, 95], [5, 20], [12, 120], [22, 220], [30, 320], [51, 95]], split=27)
# The landing hitbox runs before the shared landing lag (2 frames + 11 = the source's 13).
n['phases'].insert(1, phase('landhit', 2, [{'start': 1, 'end': 1, 'x': 0, 'y': 20, 'r': 34, 'damage': 1.0,
                                            'bkb': 60.0, 'kbg': 0.5, 'angle': 70}],
                            anim=[{'at': 0, 'pose': 'sweepLow'}],
                            attach=wield('bow', [[0, 95]]),
                            transitions=[{'on': 'timeout', 'to': 'landing', 'lag': 11}]))
# Both swings land into the landing hit; the autocancel spans still end on landing.
for ph in n['phases']:
    if ph['name'] in ('swing1', 'swing2'):
        ph['transitions'] = [{'on': 'land', 'to': 'landhit'}] + \
            [tr for tr in ph['transitions'] if tr.get('on') != 'land']
moves['nair'] = n
af = raw('fair', 'master', 'AttackAirF')
fair = [
    swept(take_bone(af, 'top', 12.75)['source'], [(frame(12), 96, 44, 15), (frame(13), 122, 44, 15)]),
    swept(take_bone(af, 'top', 8.5)['source'], [(frame(12), 44, 42, 15), (frame(13), 74, 42, 13)]),
]
moves['fair'] = aerial('fair', 'Lance Cut', 39, 11, 3, 36, fair, 'thrust', LANCE_HOLD,
                       [[0, 120], [8, 40], [12, -5], [39, 40]])
ab = raw('bair', 'master', 'AttackAirB')
bair = [
    hit(take(ab, 15.0)['source'], frame(13), frame(14), -100, 46, 17.5),
    hit(take(ab, 10.0)['source'], frame(13), frame(14), -40, 46, 11.5),
    hit(take(ab, 7.5)['source'], frame(15), frame(17), -98, 50, 15),
    hit(take(ab, 5.0)['source'], frame(15), frame(17), -38, 50, 10),
]
moves['bair'] = aerial('bair', 'Reverse Lance', 46, 13, 6, 40, bair, 'thrust', LANCE_HOLD,
                       [[0, 20], [5, 150], [7, 180], [16, 190], [46, 95]], air_pose='airHigh')
au = raw('uair', 'master', 'AttackAirHi')
uair = [
    swept(take(au, 3.0, 1)['source'], [(frame(10), 10, 96, 23.5), (frame(13), 0, 120, 25), (frame(19), 0, 142, 25)]),
    hit(take(au, 3.0, 0)['source'], frame(10), frame(19), 0, 132, 25),
    hit(take(au, 3.0, 2)['source'], frame(10), frame(19), -34, 142, 25),
    hit(take(au, 3.0, 3)['source'], frame(10), frame(19), 34, 142, 25),
    hit(take(au, 5.5, 1)['source'], frame(20), frame(23), 0, 142, 27.5),
    hit(take(au, 5.5, 2)['source'], frame(20), frame(23), -34, 142, 27.5),
    hit(take(au, 5.5, 3)['source'], frame(20), frame(23), 34, 142, 27.5),
]
moves['uair'] = aerial('uair', 'Creator Arc', 48, 14, 4, 48, uair, 'sweepUp', SWORD_HOLD,
                       [[0, 150], [4, 30], [6, 90], [48, 90]], air_pose='airHigh', split=19)
for ph in moves['uair']['phases']:
    ph['attach']['sword']['scale'] = [[0, 2.3]]
ad = raw('dair', 'master', 'AttackAirLw')
# The UFD frames put the whole Aymr swing below the fighter: the blade is the spike in the air.
dair = [
    swept(take_bone(ad, 'haver', 19.0, 0)['source'], [(frame(22), 14, -18, 21), (frame(24), 10, -30, 22)]),
    hit(take_bone(ad, 'top', 19.0, 0)['source'], frame(22), frame(24), 8, 8, 20),
    hit(take_bone(ad, 'top', 19.0, 2)['source'], frame(22), frame(24), 4, -14, 20),
    hit(take_bone(ad, 'haver', 19.0, 1)['source'], frame(22), frame(24), 14, -18, 21),
    hit(take_bone(ad, 'top', 19.0, 1)['source'], frame(22), frame(24), 8, 8, 20),
    hit(take_bone(ad, 'top', 19.0, 3)['source'], frame(22), frame(24), 4, -14, 20),
]
moves['dair'] = aerial('dair', 'Aymr Drop', 59, 28, 2, 52, dair, 'slam', AXE_HOLD,
                       [[0, 60], [12, 160], [20, -60], [24, -95], [59, -60]], air_pose='airLow')

# ---------------------------------------------------------------- grabs
cg = raw('grab', 'master', 'Catch')
grab_box = [{'start': frame(6), 'end': frame(7), 'x': 25, 'y': 40, 'r': 16.5, 'damage': 0.0, 'bkb': 0.0,
             'kbg': 0.0, 'angle': 0, 'type': 'grab'},
            {'start': frame(6), 'end': frame(7), 'x': 35, 'y': 40, 'r': 16.5, 'damage': 0.0, 'bkb': 0.0,
             'kbg': 0.0, 'angle': 0, 'type': 'grab'},
            {'start': frame(6), 'end': frame(7), 'x': 45, 'y': 40, 'r': 16.5, 'damage': 0.0, 'bkb': 0.0,
             'kbg': 0.0, 'angle': 0, 'type': 'grab'}]
# The source grab uses the free hand (`CATCH` on the top bone, no article), so no weapon is drawn.
moves['grab'] = {'name': 'Grab', 'phases': [phase(
    'main', 39, grab_box, anim=swing('thrust', frame(6), frame(7), 39))]}
dg = raw('dashgrab', 'master', 'CatchDash')
dash_grab_box = [{'start': frame(10), 'end': frame(11), 'x': 22, 'y': 33, 'r': 15, 'damage': 0.0, 'bkb': 0.0,
                  'kbg': 0.0, 'angle': 0, 'type': 'grab'},
                 {'start': frame(10), 'end': frame(11), 'x': 35, 'y': 33, 'r': 15, 'damage': 0.0, 'bkb': 0.0,
                  'kbg': 0.0, 'angle': 0, 'type': 'grab'},
                 {'start': frame(10), 'end': frame(11), 'x': 48, 'y': 33, 'r': 15, 'damage': 0.0, 'bkb': 0.0,
                  'kbg': 0.0, 'angle': 0, 'type': 'grab'}]
moves['dashgrab'] = {'name': 'Dash Grab', 'phases': [phase(
    'main', 48, dash_grab_box, slide=frame(18), anim=swing('thrust', frame(10), frame(11), 48))]}

# ---------------------------------------------------------------- specials
SN = scripts['master']['SpecialN']
sn = take(attacks('master_arrow1', 'Fly')['attacks'], 12.0, 0)['source']
smax = take(attacks('master_arrow2', 'Search')['attacks'], 29.0, 0)['source']
arrow_shapes = [
    line((-14, 0), (10, 0), 2.4, '#e8e2c8'),
    poly([(10, 4), (20, 0), (10, -4)], '#c9cfd8'),
    line((-14, 4), (-9, 0), 2, '#66ffb2'),
    line((-14, -4), (-9, 0), 2, '#66ffb2'),
]
arrow_timing = ufd['moves']['nspec']
arrow_charge = arrow_timing['startup'][1] - arrow_timing['startup'][0]
arrow_recovery = arrow_timing['total'][1] - (arrow_timing['startup'][1] - 1)
moves['nspec'] = {'name': 'Failnaught', 'bReverse': True, 'phases': [
    phase('draw', 44, anim=[{'at': 0, 'pose': 'draw'}],
          attach=wield('bow', [[0, 95], [8, 0]]),
          charge={'frame': 43, 'max': arrow_charge, 'button': 'special'},
          events=[{'at': 0, 'sfx': 'charge'}],
          transitions=[
              {'on': 'release', 'from': 44, 'minCharge': arrow_charge, 'to': 'looseMax'},
              {'on': 'release', 'from': 44, 'to': 'loose'},
              {'on': 'shieldPress', 'from': 20, 'to': 'end'},
              {'on': 'timeout', 'minCharge': arrow_charge, 'to': 'looseMax'},
              wait('loose'),
          ]),
    phase('loose', 36, anim=[{'at': 0, 'pose': 'draw'}, {'at': 4, 'pose': 'thrust', 'lerp': True}],
          attach=wield('bow', [[0, 0]]),
          events=[{'at': 0, 'spawn': 'arrow', 'sfx': 'shoot'}]),
    phase('looseMax', arrow_recovery, anim=[{'at': 0, 'pose': 'draw'}, {'at': 4, 'pose': 'thrust', 'lerp': True}],
          attach=wield('bow', [[0, 0]]),
          events=[{'at': 0, 'spawn': 'arrowMax', 'sfx': 'shoot'}]),
]}

ss = raw('sspec', 'master', 'SpecialSF')
ssair = raw('sspecAir', 'master', 'SpecialAirSF')
# Grounded: the source spawns the sour `top` boxes on frame 20 and upgrades them to the 17.25 sweetspot one
# frame later (UFD: "sweetspot spawns a frame later... and gains priority for that one frame").
sspec = [
    hit(take_bone(ss, 'haver', 17.25, 0)['source'], 1, 4, 118, 34, 15),
    hit(take_bone(ss, 'haver', 17.25, 1)['source'], 1, 4, 142, 34, 12),
    hit(take_bone(ss, 'top', 17.25, 0)['source'], 1, 4, 136, 42, 27),
    hit(take_bone(ss, 'haver', 11.5, 0)['source'], 0, 4, 62, 34, 20),
    hit(take_bone(ss, 'haver', 11.5, 1)['source'], 0, 4, 92, 34, 15),
    hit(take_bone(ss, 'top', 11.5, 0)['source'], 0, 4, 54, 36, 25),
]
# Aerial: 14.25/9.5 down-forward, one frame later startup and a 61-frame total.
sspec_air = [
    hit(take_bone(ssair, 'haver', 14.25, 0)['source'], 0, 3, 116, 16, 15),
    hit(take_bone(ssair, 'haver', 14.25, 1)['source'], 0, 3, 138, 12, 13),
    hit(take_bone(ssair, 'haver', 9.5, 0)['source'], 0, 3, 60, 24, 20),
    hit(take_bone(ssair, 'haver', 9.5, 1)['source'], 0, 3, 88, 20, 16),
]
moves['sspec'] = {'name': 'Areadbhar Rush', 'bReverse': True, 'phases': [
    phase('wind', 19, anim=[{'at': 0, 'pose': 'stand'}, {'at': 8, 'pose': 'thrust', 'lerp': True}],
          attach=wield('lance', [[0, 95], [10, 130]]),
          events=[{'at': 0, 'sfx': 'swing'}],
          transitions=[{'on': 'grounded', 'from': 19, 'to': 'thrust'},
                       {'on': 'airborne', 'from': 19, 'to': 'airThrust'}, wait('thrust')]),
    phase('thrust', 5, sspec, slide=frame(24), anim=[{'at': 0, 'pose': 'thrust'}],
          attach=wield('lance', [[0, 10], [4, 0]]),
          events=[{'at': 0, 'setVel': {'x': 8.5714}, 'sfx': 'slash'}],
          transitions=[{'on': 'grounded', 'from': 5, 'to': 'recover'},
                       {'on': 'airborne', 'from': 5, 'to': 'airRecover'}, wait('recover')]),
    phase('recover', 41, anim=[{'at': 0, 'pose': 'thrust'}, {'at': 10, 'pose': 'stand', 'lerp': True}],
          attach=wield('lance', [[0, 0], [10, 95]])),
    phase('airThrust', 5, sspec_air, anim=[{'at': 0, 'pose': 'airLow'}, {'at': 4, 'pose': 'thrust'}],
          attach=wield('lance', [[0, 30], [4, 10]]),
          events=[{'at': 0, 'setVel': {'x': 5.0}, 'sfx': 'slash'}],
          transitions=[{'on': 'land', 'to': 'landing', 'lag': 16}, wait('airRecover')]),
    phase('airRecover', 37, airAccelMult=drift(0.4, 'attack'), anim=[{'at': 0, 'pose': 'thrust'}, {'at': 12, 'pose': 'air', 'lerp': True}],
          attach=wield('lance', [[0, 10], [12, 60]]),
          transitions=[{'on': 'land', 'to': 'landing', 'lag': 16}]),
]}

sw_hi = raw('uspec', 'master', 'SpecialHi')
# The whip phase starts on source frame 9; its nine frames cover UFD 9-17. The 3.0 rows are one hit per victim
# in the source; a 3-tick rehit lets the travelling whip connect more than once (the 1.0 autolink is folded in).
whip = [swept(take(sw_hi, 3.0, 0)['source'], [(1, 62, 82, 22), (4, 94, 122, 22), (8, 116, 152, 20)],
              rehit=3)]
moves['uspec'] = {'name': 'Creator Tether', 'air': 'upSpecial', 'phases': [
    # UFD: the tether reaches from source frame 7, so it opens during the startup phase.
    phase('startup', 8, airAccelMult=drift(0.2, 'upSpecial'), ledgeGrab=frame(7),
          tether={'x1': 30, 'x2': 210, 'y1': 20, 'y2': 160},
          anim=[{'at': 0, 'pose': 'sweepUp'}],
          attach=wield('sword', [[0, 95], [6, 60]], scale=1.4),
          events=[{'at': 0, 'setVel': {'x': 0}, 'sfx': 'swing'}],
          transitions=[wait('whip')]),
    phase('whip', 9, whip, airAccelMult=drift(0.3, 'upSpecial'), ledgeGrab=0,
          tether={'x1': 30, 'x2': 210, 'y1': 20, 'y2': 160},
          anim=[{'at': 0, 'pose': 'sweepUp'}],
          attach=wield('sword', [[0, 60], [8, 45]], scale=2.4),
          events=[{'at': 0, 'sfx': 'slash'}],
          transitions=[wait('rise')]),
    phase('rise', 29, airAccelMult=drift(0.4, 'upSpecial'), ledgeGrab=0, anim=[{'at': 0, 'pose': 'airHigh'}],
          attach=wield('sword', [[0, 60], [10, 95]], scale=1.6),
          events=[{'at': 0, 'setVel': {'y': 9.5851}}],
          transitions=[{'on': 'land', 'to': 'landing', 'lag': 24}, wait('helpless')]),
]}

ax = raw('dspec', 'master_axe', 'SpecialLw')
axair = raw('dspecAir', 'master_axe', 'SpecialAirLw')
axhit = raw('dspecHit', 'master_axe', 'SpecialLwHit')
slam_ground = hit(take(ax, 30.0)['source'], 0, 5, 58, 40, 26)
slam_air = hit(take(axair, 25.0)['source'], 0, 5, 52, 34, 24)
shock = hit(take(axhit, 8.0)['source'], 0, 3, 16, 12, 40)
shock.pop('situation', None)
shock['situation'] = 'ground'
moves['dspec'] = {'name': 'Aymr', 'phases': [
    phase('raise', 33, anim=[{'at': 0, 'pose': 'stand'}, {'at': 16, 'pose': 'overhead', 'lerp': True}],
          attach=wield('axe', [[0, 60], [16, 120]]),
          events=[{'at': 0, 'sfx': 'swing'}],
          transitions=[wait('slam')]),
    # Source frames 34-61: the damage-based armour window (34-63) starts on this phase's first frame.
    phase('slam', 28, armorThreshold=36.0, anim=[{'at': 0, 'pose': 'overhead'}],
          attach=wield('axe', [[0, 120], [20, 150], [27, 170]]),
          transitions=[{'on': 'grounded', 'from': 28, 'to': 'impact'},
                       {'on': 'airborne', 'from': 28, 'to': 'impactAir'}, wait('impact')]),
    phase('impact', 6, [slam_ground], armorThreshold=36.0, anim=[{'at': 0, 'pose': 'slam'}],
          attach=wield('axe', [[0, 30], [4, -20]]),
          events=[{'at': 0, 'sfx': 'smash'}],
          transitions=[wait('wave')]),
    phase('wave', 4, [shock], anim=[{'at': 0, 'pose': 'slam'}],
          attach=wield('axe', [[0, -20]]),
          events=[{'at': 0, 'sfx': 'quake'}],
          transitions=[wait('recover')]),
    phase('recover', 62, anim=[{'at': 0, 'pose': 'slam'}, {'at': 24, 'pose': 'stand', 'lerp': True}],
          attach=wield('axe', [[0, -10], [24, 95]])),
    phase('impactAir', 6, [slam_air], anim=[{'at': 0, 'pose': 'slam'}],
          attach=wield('axe', [[0, 30], [4, -20]]),
          events=[{'at': 0, 'sfx': 'smash'}],
          transitions=[wait('recoverAir')]),
    phase('recoverAir', 66, anim=[{'at': 0, 'pose': 'slam'}, {'at': 20, 'pose': 'air', 'lerp': True}],
          attach=wield('axe', [[0, -10], [20, 60]]),
          transitions=[{'on': 'land', 'to': 'landing', 'lag': 24}]),
]}

# ---------------------------------------------------------------- throws
def throw(slot, script, total, release, damage, angle, bkb, kbg):
    s = scripts['master'][script]
    rows = re.findall(r'ATTACK_ABS\(Kind=FIGHTER_ATTACK_ABSOLUTE_KIND_THROW,(.*)\)', s)
    a = args(rows[0]) if rows else {'Damage': str(damage), 'Angle': str(angle), 'BKB': str(bkb), 'KBG': str(kbg)}
    ledger['rawAttacks'][slot] = {'article': 'master', 'script': script, 'abs': a}
    return {'release': frame(release), 'total': total,
            'hit': hit(a, 0, 0, 70, 40, 32, damage=damage, angle=angle, bkb=bkb, kbg=kbg / 100)}


throws = {
    'f': throw('fthrow', 'ThrowF', 35, 13, 6.0, 38, 60, 100),
    'b': throw('bthrow', 'ThrowB', 43, 30, 7.0, 40, 50, 80),
    'u': throw('uthrow', 'ThrowHi', 37, 15, 9.0, 76, 60, 110),
    'd': throw('dthrow', 'ThrowLw', 31, 13, 5.0, 74, 50, 150),
}

# ---------------------------------------------------------------- projectiles
def arrow(name, a, vel, r, life, shapes):
    return {'name': name, 'spawn': [36, 52], 'vel': [vel, 0], 'r': r, 'life': life, 'max': 2,
            'hit': hit(a, 0, 0, 0, 0, r), 'spawnSfx': 'shoot', 'shapes': copy.deepcopy(shapes)}


projectiles = {
    'arrow': arrow('Failnaught', sn, 20, 8, 90, arrow_shapes),
    'arrowMax': arrow('Failnaught (full)', smax, 28.5714, 11, 100, arrow_shapes),
}

# ---------------------------------------------------------------- fighter document
GRAVITY = round(0.089 * SCALE, 5)  # UFD gravity 0.089
JUMP_HEIGHTS = (26.5, 14, 28.5)  # SSBU full hop, short hop, double jump (units; ssbwiki "Jump")
doc = {
    '$schema': '../../schema/fighter.schema.json',
    'schemaVersion': 1,
    'extends': 'base',
    'name': 'Bleth',
    'tagline': 'Wielder of four relics',
    'blurb': 'Very long reach and heavy reads at the cost of speed. Areadbhar stabs and Aymr slams, Failnaught '
             'charges a piercing arrow and the Sword of the Creator tethers a ledge.',
    'stats': {
        'weight': round(100 / 97, 8), 'pummelDamage': 1.5,
        # UFD movement × SCALE; runAccel is fitted (0.62 at the old 3.5 px/unit speed scale).
        'walkSpeed': round(0.97 * SCALE, 5), 'runSpeed': round(1.43 * SCALE, 5),
        'dashSpeed': round(1.8 * SCALE, 5), 'dashFrames': 15, 'runAccel': 0.8857,
        'groundFriction': 0.85,
        **units.jumps(*JUMP_HEIGHTS, GRAVITY), 'airJumpDrift': round(0.89 * SCALE, 5), 'airJumps': 1,
        'airSpeed': round(0.89 * SCALE, 5), 'airAccel': AIR_ACCEL, 'gravity': GRAVITY,
        'fallSpeed': round(1.6 * SCALE, 5), 'fastFallSpeed': round(2.56 * SCALE, 5),
        'traction': round(0.125 * SCALE, 5), 'airFriction': round(0.008 * SCALE, 5),  # Ultimate ground_brake / air_brake_x (fighter_param dump; ssbwiki Traction, Air friction)
    },
    'ai': {
        'reach': 90, 'counterChance': 0,
        'specials': [
            {'minDistance': 200, 'height': 50, 'grounded': True, 'noProjectile': True, 'chance': 0.5,
             'neutral': True, 'down': False, 'hold': [20, 60]},
            {'minDistance': 110, 'maxDistance': 260, 'height': 50, 'grounded': True, 'noProjectile': False,
             'chance': 0.4, 'neutral': False, 'down': False, 'hold': None},
            {'minDistance': 0, 'maxDistance': 120, 'height': 40, 'grounded': True, 'noProjectile': False,
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

# ---------------------------------------------------------------- ledger and write
for slot, m in doc['moves'].items():
    for p in m['phases']:
        for b in p.get('hitboxes', []):
            assert isinstance(b, dict), (slot, p['name'], b)
    row = {'weapon': {'jab': 'sword', 'ftilt': 'sword', 'utilt': 'sword', 'dtilt': 'sword', 'dashatk': 'sword',
                      'fsmash': 'lance', 'usmash': 'sword', 'dsmash': 'axe', 'nair': 'bow', 'fair': 'lance',
                      'bair': 'lance', 'uair': 'sword', 'dair': 'axe', 'grab': 'sword', 'dashgrab': 'sword',
                      'nspec': 'bow', 'sspec': 'lance', 'uspec': 'sword', 'dspec': 'axe'}[slot],
           'phases': [{'name': p['name'], 'frames': p.get('frames'),
                       'hitframes': sorted({(b['start'], b.get('end', p.get('frames'))) for b in p.get('hitboxes', [])}),
                       'attach': sorted((p.get('attach') or {}).keys())} for p in m['phases']]}
    ledger['phases'][slot] = row

out = ROOT / 'assets/fighters/verdant/fighter.json'
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(doc, indent=2) + '\n')
(SOURCES / 'ledger.json').write_text(json.dumps(ledger, indent=1) + '\n')
print(f'wrote {out.relative_to(ROOT)} ({out.stat().st_size} bytes)')
