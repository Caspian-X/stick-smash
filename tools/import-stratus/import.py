#!/usr/bin/env python3
"""Rebuild Stratus (SSBU Cloud, patch 13.0.1) from pinned Rubendal scripts and UFD rows.

Damage, launch fields, masks, hitlag and rehit come from the pinned attack scripts. Startup, active windows,
totals, landing lag, autocancels, charge frames and movement come from the pinned Ultimate Frame Data rows.
Sword geometry is *fitted* to the stick-figure poses and the per-move hitbox GIF frames, not projected from
Nintendo's skeleton. Limit is a one-use charge pool filled by the down special and spent by variant timelines.
Movement uses the shared 5 px/unit scale (`tools/units.py`): speeds units/frame → px/tick, accelerations
units/frame² → px/tick², jump speeds derived from the source jump heights.
Run `just import-stratus`.
"""
import copy, importlib, json, math, re, sys
from pathlib import Path

# Phase drift (`airAccelMult`) is written as a multiple of the old shared air accels (aerial 0.45, attack 0.25,
# upSpecial 0.3). Under `movement.ultimate` it multiplies airAccel × the kind's multiplier instead, so drift()
# stores the ratio that keeps the same effective acceleration relative to airAccel. The ratio is scale-free, so it
# is taken against the airAccel at the old 3.5 px/unit scale those legacy accels were written in (0.07 × 3.5).
LEGACY_AIR_ACCEL = 0.245
def drift(legacy, kind):
    old = {'aerial': 0.45, 'attack': 0.25, 'upSpecial': 0.3}[kind]
    mult = {'aerial': 1.0, 'attack': 0.55, 'upSpecial': 0.65}[kind]
    return round(legacy * old / (LEGACY_AIR_ACCEL * mult), 4)

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
units = importlib.import_module('units')
SOURCES = ROOT / 'docs/imports/sources/stratus'
source = json.loads((SOURCES / 'scripts.json').read_text())
ufd = json.loads((SOURCES / 'ufd.json').read_text())['rows']
scripts = {art: {s['Name']: s['Data'] for s in ss} for art, ss in source['scripts'].items()}
ledger = {'sourceCommit': source['commit'], 'sourceSha256': source['sha256'], 'scale': units.SCALE, 'rawAttacks': {}}


# ---------------------------------------------------------------- script parsing
def expand(s):
    while (m := re.search(r'for\((\d+) Iterations\)\{', s)):
        pos = m.end()
        depth = 1
        while depth:
            depth += (s[pos] == '{') - (s[pos] == '}'); pos += 1
        s = s[:m.start()] + expand(s[m.end():pos - 1]) * int(m[1]) + s[pos:]
    return s


def args(s):
    return {k: v.strip() for k, v in re.findall(r'(\w+)=(.*?)(?=, \w+=|$)', s)}


def attacks(article, name):
    """Attack rows with their source frame, id and argument map. The left/right (`0x1801b0`) duplicate branch is
    skipped: both branches describe the same hitbox mirrored by facing."""
    src = expand(scripts[article][name])
    t, live, out, depth, skip = 0, {}, [], 0, None

    def clear(i):
        if i in live:
            b = live.pop(i)
            b['end'] = t
            out.append(b)

    for raw in src.split('\n'):
        line = raw.strip()
        if skip is not None:
            depth += line.count('{') - line.count('}')
            if depth <= skip:
                skip = None
            continue
        if line.startswith('else{'):
            skip = depth
            depth += line.count('{') - line.count('}')
            continue
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
            a = args(m[1]); i = int(a['ID'])
            clear(i)
            live[i] = {'id': i, 'start': t, 'end': t, 'source': a}
        elif m := re.search(r'\bATTACK_ABS\((.*)\)', line):
            a = args(m[1]); i = int(a['ID'])
            clear(i)
            live[i] = {'id': i, 'start': t, 'end': t, 'source': a}
        depth += line.count('{') - line.count('}')
    for i in list(live):
        clear(i)
    rows, seen = [], set()
    for b in sorted(out, key=lambda b: (b['start'], b['id'])):
        key = (b['id'], b['start'], b['source']['Damage'])
        if b['end'] < b['start'] or key in seen:
            continue
        seen.add(key)
        rows.append(b)
    return rows


def raw(slot, article, name):
    rows = attacks(article, name)
    ledger['rawAttacks'][slot] = {'article': article, 'script': name, 'attacks': rows}
    return rows


def take(rows, damage, nth=0, at=None):
    """The `nth` row with this damage, optionally created on source frame `at` (re-created script boxes)."""
    found = [b for b in rows
             if abs(float(b['source']['Damage']) - damage) < 1e-6 and (at is None or b['start'] == at)]
    return found[nth]


def num(a, k, default=0):
    try:
        return float(a.get(k, default))
    except ValueError:
        return default


# ---------------------------------------------------------------- conversion
SCALE = units.SCALE


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
        b['fkb'] = num(a, 'FKB')  # Ultimate WBKB (`fkb`); bkb/kbg stay the source values
    if num(a, 'Rehit'):
        b['rehit'] = int(num(a, 'Rehit'))
    if num(a, 'Hitlag', 1) != 1:
        b['hitlagMult'] = num(a, 'Hitlag', 1)
    if num(a, 'ShieldDamage') and b['damage'] > 0:
        b['shieldDmgMult'] = round(max(0, (b['damage'] + num(a, 'ShieldDamage')) / b['damage']), 4)
    b.update(kw)
    return b


def capsule(a, start, end, p0, p1, r, **kw):
    """A line of overlapping circles no farther apart than their radius."""
    steps = max(1, math.ceil(math.hypot(p1[0] - p0[0], p1[1] - p0[1]) / max(r, 1)))
    return [hit(a, start, end, p0[0] + (p1[0] - p0[0]) * i / steps, p0[1] + (p1[1] - p0[1]) * i / steps, r, **kw)
            for i in range(steps + 1)]


def swept(a, pts, r, **kw):
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
    p = {'name': name}
    if frames is not None:
        p['frames'] = frames
    if boxes:
        p['hitboxes'] = [{k: v for k, v in b.items() if k != '_group'} for b in assign_groups(boxes)]
    p.update(kw)
    return p


def land(lag=None):
    t = {'on': 'land', 'to': 'landing'}
    if lag is not None:
        t['lag'] = lag
    return t


def wait(to):
    return {'on': 'timeout', 'to': to}


def swing(pose, start, end, total, rest='stand'):
    return [{'at': 0, 'pose': rest}] + \
        [{'at': max(0, start - 3), 'pose': pose, 'lerp': True}, {'at': end, 'pose': pose}] + \
        [{'at': min(max(1, total - 1), end + 6), 'pose': rest, 'lerp': True}]


def attach(keys, scale=None):
    """Sword angle keys `[frame, degrees]`; `scale` shortens the blade for a shorter visual (unused here)."""
    a = {'angle': [[float(f), float(d)] for f, d in keys]}
    if scale is not None:
        a['scale'] = [[0, scale]]
    return {'sword': a}


def angle_at(keys, t):
    if t <= keys[0][0]:
        return keys[0][1]
    for (f0, d0), (f1, d1) in zip(keys, keys[1:]):
        if t <= f1:
            return d0 + (d1 - d0) * (t - f0) / max(f1 - f0, 1e-9)
    return keys[-1][1]


def shift_keys(keys, lo):
    """A phase split from a longer timeline re-bases its attachment angle keys on the split frame."""
    if lo == 0:
        return keys
    out = [[0.0, angle_at(keys, lo)]] + [[float(f - lo), d] for f, d in keys if f > lo]
    return out


def aerial(name, total, landing, ac_early, ac_late, boxes, poses):
    """Split an aerial into the opening autocancel, the landing-lag body and the ending autocancel. `poses` is
    (pose, rest, attach keys)."""
    pose, rest, keys = poses
    boxes = assign_groups(boxes)
    spans = [(0, ac_early, 'startup', None), (ac_early, ac_late, 'main', landing), (ac_late, total, 'late', None)]
    parts = []
    for lo, hi, tag, lag in spans:
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
        ph['transitions'] = [land(lag) if lag is not None else {'on': 'land', 'to': 'end'}]
        lo_t = min((b['start'] for b in cut), default=1)
        hi_t = max((b.get('end', 0) for b in cut), default=1)
        ph['anim'] = [{'at': 0, 'pose': rest}] + \
            [{'at': max(0, lo_t - 2), 'pose': pose, 'lerp': True}, {'at': max(1, hi_t), 'pose': pose},
             {'at': min(hi - lo - 1, hi_t + 3), 'pose': rest, 'lerp': True}]
        ph['attach'] = attach(shift_keys(keys, lo))
        parts.append(ph)
    for i in range(len(parts) - 1):
        parts[i]['transitions'].append({**wait(parts[i + 1]['name']), 'preserveHits': True})
    return {'name': name, 'air': 'aerial', 'phases': parts}


# ---------------------------------------------------------------- poses and body
stand = {'head': [2, 66], 'shoulder': [0, 52], 'hip': [0, 30], 'elbowF': [11, 43], 'handF': [24, 40],
         'elbowB': [-11, 42], 'handB': [-20, 38], 'kneeF': [5, 16], 'footF': [9, 1], 'kneeB': [-6, 16],
         'footB': [-11, 1]}
poses = {
    'stand': stand,
    'guard': {'from': 'stand', 'head': [4, 60], 'shoulder': [2, 47], 'elbowF': [16, 40], 'handF': [30, 44]},
    'thrust': {'head': [5, 62], 'shoulder': [3, 49], 'hip': [-2, 29], 'elbowF': [20, 47], 'handF': [36, 50],
               'elbowB': [-10, 44], 'handB': [-16, 42], 'kneeF': [14, 15], 'footF': [22, 1], 'kneeB': [-7, 15],
               'footB': [-13, 1]},
    'chop': {'head': [8, 56], 'shoulder': [5, 44], 'hip': [0, 24], 'elbowF': [24, 36], 'handF': [40, 26],
             'elbowB': [-8, 36], 'handB': [-14, 28], 'kneeF': [18, 12], 'footF': [28, 1], 'kneeB': [-9, 12],
             'footB': [-16, 1]},
    'sweepLow': {'head': [8, 50], 'shoulder': [4, 38], 'hip': [-2, 20], 'elbowF': [21, 32], 'handF': [40, 22],
                 'elbowB': [-12, 31], 'handB': [-18, 22], 'kneeF': [19, 11], 'footF': [31, 1], 'kneeB': [-8, 10],
                 'footB': [-14, 1]},
    'sweepUp': {'head': [-2, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [8, 62], 'handF': [12, 80],
                'elbowB': [-10, 46], 'handB': [-16, 40], 'kneeF': [3, 17], 'footF': [4, 1], 'kneeB': [-6, 17],
                'footB': [-8, 1]},
    'overhead': {'head': [-6, 64], 'shoulder': [-4, 51], 'hip': [2, 31], 'elbowF': [4, 62], 'handF': [0, 82],
                 'elbowB': [-14, 46], 'handB': [-20, 40], 'kneeF': [8, 16], 'footF': [12, 1], 'kneeB': [-8, 16],
                 'footB': [-14, 1]},
    'cross': {'head': [6, 58], 'shoulder': [3, 46], 'hip': [-1, 26], 'elbowF': [22, 44], 'handF': [40, 38],
              'elbowB': [-8, 40], 'handB': [-14, 34], 'kneeF': [16, 13], 'footF': [26, 1], 'kneeB': [-8, 13],
              'footB': [-15, 1]},
    'cast': {'head': [3, 64], 'shoulder': [1, 50], 'hip': [-1, 30], 'elbowF': [16, 50], 'handF': [36, 56],
             'elbowB': [-12, 44], 'handB': [-20, 42], 'kneeF': [5, 16], 'footF': [8, 1], 'kneeB': [-6, 16],
             'footB': [-10, 1]},
    'spin': {'head': [-2, 62], 'shoulder': [0, 49], 'hip': [0, 30], 'elbowF': [16, 52], 'handF': [30, 62],
             'elbowB': [-14, 44], 'handB': [-22, 44], 'kneeF': [6, 16], 'footF': [10, 1], 'kneeB': [-7, 16],
             'footB': [-12, 1]},
    'plunge': {'head': [4, 54], 'shoulder': [2, 44], 'hip': [0, 24], 'elbowF': [12, 30], 'handF': [14, 8],
               'elbowB': [-10, 36], 'handB': [-16, 22], 'kneeF': [14, 14], 'footF': [18, 4], 'kneeB': [-8, 13],
               'footB': [-12, 3]},
    'rise': {'head': [0, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [9, 62], 'handF': [14, 82],
             'elbowB': [-12, 48], 'handB': [-20, 50], 'kneeF': [8, 18], 'footF': [10, 4], 'kneeB': [-6, 16],
             'footB': [-10, 6]},
    'air': {'head': [2, 64], 'shoulder': [0, 50], 'hip': [0, 30], 'elbowF': [12, 44], 'handF': [24, 42],
            'elbowB': [-12, 42], 'handB': [-22, 38], 'kneeF': [10, 18], 'footF': [14, 5], 'kneeB': [-8, 16],
            'footB': [-14, 4]},
    'airHigh': {'head': [-2, 64], 'shoulder': [0, 50], 'hip': [-2, 30], 'elbowF': [10, 60], 'handF': [12, 78],
                'elbowB': [-10, 44], 'handB': [-18, 42], 'kneeF': [12, 17], 'footF': [18, 4], 'kneeB': [-6, 15],
                'footB': [-10, 6]},
    'airLow': {'head': [10, 56], 'shoulder': [5, 44], 'hip': [-2, 26], 'elbowF': [20, 30], 'handF': [34, 16],
               'elbowB': [-10, 34], 'handB': [-18, 24], 'kneeF': [20, 12], 'footF': [34, 6], 'kneeB': [-6, 12],
               'footB': [-12, 4]},
    'airBack': {'head': [-6, 62], 'shoulder': [-3, 48], 'hip': [-2, 28], 'elbowF': [-14, 52], 'handF': [-30, 58],
                'elbowB': [-16, 44], 'handB': [-28, 46], 'kneeF': [8, 17], 'footF': [12, 4], 'kneeB': [-8, 15],
                'footB': [-14, 3]},
    'crouch': {'from': 'sweepLow', 'head': [6, 46], 'shoulder': [3, 34], 'hip': [-1, 17], 'handF': [30, 18],
               'handB': [-16, 18]},
    'hold': {'head': [0, 64], 'shoulder': [0, 50], 'hip': [0, 30], 'elbowF': [14, 44], 'handF': [28, 48],
             'elbowB': [10, 40], 'handB': [26, 38], 'kneeF': [5, 16], 'footF': [9, 1], 'kneeB': [-6, 16],
             'footB': [-11, 1]},
    'win': {'head': [0, 68], 'shoulder': [0, 53], 'hip': [0, 32], 'elbowF': [14, 50], 'handF': [22, 68],
            'elbowB': [-16, 50], 'handB': [-30, 60], 'kneeF': [5, 17], 'footF': [9, 1], 'kneeB': [-6, 17],
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


blade = 84  # Buster Sword blade length in px (fitted to the sword hitbox reach)
attachments = [
    {'id': 'sword', 'joint': 'handF', 'angle': 95, 'trail': {'inner': 26, 'outer': 128},
     'glow': 'active', 'hold': {'elbow': 18, 'hand': 30, 'bend': 20},
     'shapes': [
         line((-16, 0), (2, 0), 6, '#2c2b33'),
         circle((-17, 0), 4.5, '#3d3c46'),
         line((2, -7), (2, 7), 5, '#8a6a3a'),
         poly(((5, -8), (blade - 12, -7), (blade, -2), (blade, 2), (blade - 12, 7), (5, 8)), '#b6bdcb'),
         line((6, 6), (blade - 13, 6), 2.2, '#98a0b0'),
         line((6, -6), (blade - 13, -6), 2.2, '#98a0b0'),
         line((8, 0), (blade - 14, 0), 1.6, '#868d9c'),
         circle((18, 0), 1.8, '#4b4f5c'),
         circle((30, 0), 1.8, '#4b4f5c'),
         line((5, -8), (blade - 12, -7), 3, '#7fd4ff', alpha=0.55, when='glow'),
         line((5, 8), (blade - 12, 7), 3, '#7fd4ff', alpha=0.55, when='glow'),
         line((6, 0), (blade, 0), 5, '#cdefff', alpha=0.5, when='glow'),
     ]},
    {'id': 'bangle', 'joint': 'handB', 'angle': 40, 'behind': True,
     'shapes': [circle((0, 0), 3.4, '#9fb4d8', alpha=0.9)]},
]
style = {'headRadius': 11, 'torsoWidth': 7.5, 'frontLimbWidth': 6.5, 'backLimbWidth': 6, 'glowWidth': 2.6,
         'torsoGlowWidth': 3.2, 'headGlowRadius': 2.8, 'headRing': 2, 'headFill': '#2a3550', 'eye': [2, 0, 4, 2]}

anims = {
    'idle': {'clock': 'world', 'loop': 42.0, 'keys': [
        {'at': 0, 'pose': 'stand'},
        {'at': 21, 'pose': {'from': 'stand', 'head': [2, 68], 'handF': [25, 42]}, 'lerp': True}]},
    'walk': {'clock': 'world', 'loop': 26.0, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'kneeF': [6, 16], 'footF': [13, 1], 'kneeB': [-7, 16], 'footB': [-13, 1]}},
        {'at': 13, 'pose': {'from': 'stand', 'kneeF': [-3, 16], 'footF': [-7, 1], 'kneeB': [5, 16], 'footB': [9, 1]},
         'lerp': True}]},
    'run': {'clock': 'run', 'loop': 360.0, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'head': [5, 64], 'hip': [1, 30], 'kneeF': [17, 17], 'footF': [25, 6],
                           'kneeB': [-15, 15], 'footB': [-23, 2], 'elbowF': [17, 48], 'handF': [27, 50],
                           'elbowB': [-17, 44], 'handB': [-27, 40]}},
        {'at': 180, 'pose': {'from': 'stand', 'head': [5, 64], 'hip': [1, 30], 'kneeF': [-15, 15], 'footF': [-23, 2],
                             'kneeB': [17, 17], 'footB': [25, 6], 'elbowF': [17, 48], 'handF': [27, 50],
                             'elbowB': [-17, 44], 'handB': [-27, 40]}, 'lerp': True}]},
    'air.rise': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'kneeF': [14, 18], 'footF': [20, 8], 'kneeB': [-8, 16], 'footB': [-12, 10]}}]},
    'air.fall': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'kneeF': [8, 16], 'footF': [10, 2], 'kneeB': [-6, 15], 'footB': [-8, 4]}}]},
    'helpless': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'head': [-4, 58], 'elbowF': [8, 40], 'handF': [16, 28], 'elbowB': [-14, 40],
                           'handB': [-24, 30]}}]},
    'landing': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'sweepLow', 'head': [2, 52]}},
        {'at': 8, 'pose': 'stand', 'lerp': True}]},
    'crouch': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'crouch'}]},
    'jumpsquat': {'clock': 'state', 'keys': [{'at': 0, 'pose': {'from': 'sweepLow', 'head': [2, 56]}}]},
    'shield': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'hold', 'elbowF': [14, 44], 'handF': [24, 44]}}]},
    'hitstun': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'head': [-8, 60], 'handF': [10, 34], 'handB': [-24, 30]}}]},
    'tumble': {'clock': 'spin', 'keys': [{'at': 0, 'pose': {'from': 'air', 'head': [-6, 60]}}]},
    'win': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'win'}, {'at': 24, 'pose': {'from': 'win', 'handF': [26, 78]}, 'lerp': True}]},
    'respawn': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'air'}]},
}


# ---------------------------------------------------------------- moves
def rebase_anim(keys, lo):
    prior = [k for k in keys if k['at'] <= lo]
    base = dict(prior[-1]) if prior else {'at': 0, 'pose': 'stand'}
    base['at'] = 0
    base.pop('lerp', None)
    return [base] + [dict(k, at=k['at'] - lo) for k in keys if k['at'] > lo]


def rebase_events(events, lo):
    out = []
    for e in events:
        if 'at' in e:
            if e['at'] < lo:
                continue
            out.append(dict(e, at=e['at'] - lo))
        else:
            out.append(dict(e))
    return out


def segments(total, cuts, names, boxes=(), anim=(), keys=(), events=(), land_lag=None, land_ends=(), **first):
    """Cut presentation/landing phases while retaining the whole timeline's contact identities."""
    boxes = assign_groups(boxes)
    assert cuts[0] == 0 and cuts[-1] == total and len(cuts) == len(names) + 1, (total, cuts, names)
    phases = []
    for i, (lo, hi) in enumerate(zip(cuts, cuts[1:])):
        cut = []
        for b in boxes:
            s, e = b['start'], b.get('end', total)
            if s >= hi or e < lo:
                continue
            nb = copy.deepcopy(b)
            nb['start'], nb['end'] = max(lo, s) - lo, min(hi - 1, e) - lo
            nb['frames'] = [[t - lo, x, y, r] for t, x, y, r in nb.get('frames', []) if lo <= t < hi]
            if not nb['frames']:
                nb.pop('frames', None)
            cut.append(nb)
        ph = phase(names[i], hi - lo, cut)
        ph['anim'] = rebase_anim(anim, lo)
        if keys:
            ph['attach'] = attach(shift_keys(keys, lo))
        if events:
            ph['events'] = rebase_events(events, lo)
        if i == 0:
            ph.update(first)
        trans = []
        if land_lag is not None:
            trans.append({'on': 'land', 'to': 'end'} if i in land_ends else land(land_lag))
        if i + 1 < len(names):
            trans.append({**wait(names[i + 1]), 'preserveHits': True})
        if trans:
            ph['transitions'] = trans
        phases.append(ph)
    return phases


moves = {}

# --- neutral attack: three sword pokes (jab 2 from source 8, jab 3 from source 9)
j1 = raw('jab', 'cloud', 'Attack11')
a = take(j1, 2.5, 0)['source']
# The script lists a fourth 2.5 box at the same point with a 180-degree angle; it is shadowed by the earlier
# 361 box (first listed wins), so only the three forward hits are authored.
jab1 = [hit(a, frame(4), frame(5), 35, 50, 10), hit(take(j1, 2.5, 1)['source'], frame(4), frame(5), 52.5, 50, 10),
        hit(take(j1, 2.5, 3)['source'], frame(4), frame(5), 70, 50, 11.5)]
j2 = raw('jab2', 'cloud', 'Attack12')
jab2 = [hit(take(j2, 2.0, 0)['source'], frame(5), frame(6), 15, 47.5, 15),
        hit(take(j2, 2.0, 1)['source'], frame(5), frame(6), 35, 47.5, 15),
        hit(take(j2, 2.0, 2)['source'], frame(5), frame(6), 55, 47.5, 17.5)]
j3 = raw('jab3', 'cloud', 'Attack13')
moves['jab'] = {'name': 'Jab', 'phases': [
    phase('jab1', 29, jab1, anim=swing('thrust', frame(4), frame(5), 29),
          attach=attach([[0, 150], [2, 60], [4, 0], [28, 95]]),
          transitions=[{'on': 'attackHeld', 'from': frame(8), 'to': 'jab2'}]),
    phase('jab2', 32, jab2, anim=swing('thrust', frame(5), frame(6), 32),
          attach=attach([[0, 150], [3, 60], [6, 0], [31, 95]]),
          transitions=[{'on': 'attackHeld', 'from': frame(9), 'to': 'jab3'}]),
    phase('jab3', 37, capsule(take(j3, 3.5)['source'], frame(6), frame(7), (80, 45), (40, 45), 22.5),
          anim=swing('chop', frame(6), frame(7), 37), attach=attach([[0, 150], [5, 40], [8, 330], [36, 95]]),
          events=[{'at': frame(6), 'sfx': 'slash'}])]}

# --- tilts
ft = raw('ftilt', 'cloud', 'AttackS3')
moves['ftilt'] = {'name': 'Forward Tilt', 'phases': segments(
    35, [0, 35], ['main'], capsule(take(ft, 11.0)['source'], frame(9), frame(10), (72.5, 45), (35, 45), 30),
    anim=swing('thrust', frame(9), frame(10), 35), keys=[[0, 160], [5, 40], [9, 5], [34, 95]],
    events=[{'at': frame(9), 'sfx': 'slash'}])}
ut = raw('utilt', 'cloud', 'AttackHi3')
a = take(ut, 8.0, 0)['source']
utilt = [hit(a, frame(6), frame(9), 22, 108, 26), hit(a, frame(6), frame(9), 12, 96, 25),
         hit(take(ut, 8.0, 2)['source'], frame(6), frame(9), 6, 82, 30),
         *capsule(take(ut, 8.0, 3)['source'], frame(6), frame(9), (0, 105), (0, 80), 22.5)]
moves['utilt'] = {'name': 'Up Tilt', 'phases': segments(
    32, [0, 32], ['main'], utilt, anim=swing('sweepUp', frame(6), frame(9), 32),
    keys=[[0, 150], [3, 330], [6, 380], [9, 460], [31, 455]], events=[{'at': frame(6), 'sfx': 'slash'}])}
dt = raw('dtilt', 'cloud', 'AttackLw3')
# The 7.0 knee box is cleared on source 18 (active 7-17) and the 6.0 root box on source 9 (active 7-8); they
# share the opening window, and the already-hit list makes the front hit win inside it.
dtilt = [*capsule(take(dt, 7.0)['source'], frame(7), frame(18), (55, 12), (35, 12), 18),
         hit(take(dt, 6.0)['source'], frame(7), frame(8), -13, 12, 12.5)]
moves['dtilt'] = {'name': 'Down Tilt', 'phases': segments(
    40, [0, 40], ['main'], dtilt, anim=swing('sweepLow', frame(7), frame(8), 40),
    keys=[[0, 150], [4, 300], [7, 355], [39, 95]], events=[{'at': frame(7), 'sfx': 'slash'}])}
da = raw('dashatk', 'cloud', 'AttackDash')
dash = [hit(take(da, 11.0, 0)['source'], frame(9), frame(13), 40, 70, 25),
        hit(take(da, 11.0, 1)['source'], frame(9), frame(13), 30, 40, 20),
        hit(take(da, 11.0, 2)['source'], frame(9), frame(13), 25, 40, 15),
        hit(take(da, 8.0, 0)['source'], frame(14), frame(18), 45, 70, 20),
        hit(take(da, 8.0, 1)['source'], frame(14), frame(18), 32, 40, 16.5),
        hit(take(da, 8.0, 2)['source'], frame(14), frame(18), 27, 40, 14)]
moves['dashatk'] = {'name': 'Dash Attack', 'phases': segments(
    45, [0, 8, 13, 18, 45], ['windup', 'early', 'late', 'recovery'], dash,
    anim=swing('thrust', frame(9), frame(18), 45), keys=[[0, 130], [8, 5], [18, 35], [44, 95]],
    events=[{'at': 0, 'setVel': {'x': 6.0}}, {'at': frame(9), 'sfx': 'slash'}], slide=frame(20))}

# --- smashes: UFD charge holds are 5, 5 and 2
fs = raw('fsmash', 'cloud', 'AttackS4')
fs_early = [hit(take(fs, 3.0, 0)['source'], frame(19), frame(19), 50, 45, 22.5),
            hit(take(fs, 3.0, 1)['source'], frame(19), frame(19), 80, 45, 22.5),
            hit(take(fs, 3.0, 2)['source'], frame(19), frame(19), 50, 45, 22.5),
            hit(take(fs, 3.0, 3)['source'], frame(19), frame(19), 80, 45, 22.5)]
fs_mid = [hit(take(fs, 3.0, 4)['source'], frame(24), frame(24), 25, 45, 12.5),
          hit(take(fs, 2.0)['source'], frame(24), frame(24), 50, 45, 26),
          hit(take(fs, 4.0)['source'], frame(24), frame(24), 80, 45, 26)]
fs_clean = [*capsule(take(fs, 13.0)['source'], frame(28), frame(28), (32.5, 45), (50, 45), 27.5),
            *capsule(take(fs, 12.0)['source'], frame(28), frame(28), (32.5, 45), (80, 45), 27.5)]
moves['fsmash'] = {'name': 'Forward Smash', 'phases': segments(
    66, [0, 18, 19, 23, 24, 27, 28, 66], ['windup', 'drag1', 'link1', 'drag2', 'link2', 'clean', 'recovery'],
    fs_early + fs_mid + fs_clean,
    anim=[{'at': 0, 'pose': 'stand'}, {'at': 5, 'pose': 'overhead', 'lerp': True}, {'at': 18, 'pose': 'overhead'},
          {'at': 22, 'pose': 'thrust', 'lerp': True}, {'at': 24, 'pose': 'chop'}, {'at': 28, 'pose': 'chop'},
          {'at': 40, 'pose': 'stand', 'lerp': True}],
    keys=[[0, 130], [4, 190], [17, 60], [19, 10], [24, 20], [28, -10], [65, 95]],
    events=[{'at': frame(5), 'sfx': 'smash'}, {'at': frame(19), 'sfx': 'slash'}, {'at': frame(28), 'sfx': 'smash'}],
    charge={'frame': frame(5)})}
us = raw('usmash', 'cloud', 'AttackHi4')
# Script: the 13.0 boxes stay up while the 12.0/11.0 boxes are added (source 14-16), then every box becomes the
# 8.0 tier on source 17. UFD's gameplay windows win: 12(13-14/15-16).
usmash = [hit(take(us, 13.0, 0, at=14)['source'], frame(12), frame(14), 30, 105, 22),
          hit(take(us, 13.0, 1, at=14)['source'], frame(12), frame(14), 30, 122, 22),
          hit(take(us, 12.0, at=15)['source'], frame(13), frame(14), 18, 88, 18),
          hit(take(us, 11.0, at=15)['source'], frame(13), frame(14), 42, 100, 18),
          hit(take(us, 8.0, 0, at=17)['source'], frame(15), frame(16), 30, 105, 15),
          hit(take(us, 8.0, 1, at=17)['source'], frame(15), frame(16), 25, 92, 15),
          hit(take(us, 8.0, 2, at=17)['source'], frame(15), frame(16), 20, 80, 15)]
# UFD gameplay windows: 12(13-14/15-16). The 13.0 rows stay up through the
# 12.0/11.0 window (the script only replaces them with the 8.0 tier on source 17).
usmash[0]['start'] = usmash[1]['start'] = frame(12)
usmash[0]['end'] = usmash[1]['end'] = frame(15)
usmash[2]['start'] = usmash[3]['start'] = frame(13)
usmash[2]['end'] = usmash[3]['end'] = frame(15)
us_late_start, us_late_end = map(int, re.findall(r'\d+', ufd['Up Smash']['active'])[-2:])
usmash[4]['start'] = usmash[5]['start'] = usmash[6]['start'] = frame(us_late_start)
usmash[4]['end'] = usmash[5]['end'] = usmash[6]['end'] = frame(us_late_end)
moves['usmash'] = {'name': 'Up Smash', 'phases': segments(
    45, [0, 11, 14, 16, 45], ['windup', 'hits', 'late', 'recovery'], usmash,
    anim=[{'at': 0, 'pose': 'stand'}, {'at': 5, 'pose': 'overhead', 'lerp': True}, {'at': 11, 'pose': 'overhead'},
          {'at': 12, 'pose': 'sweepUp'}, {'at': 20, 'pose': 'stand', 'lerp': True}],
    keys=[[0, 130], [4, 170], [11, 90], [14, 20], [44, 95]],
    events=[{'at': frame(5), 'sfx': 'smash'}, {'at': frame(12), 'sfx': 'slash'}],
    charge={'frame': frame(5)})}
ds = raw('dsmash', 'cloud', 'AttackLw4')
front = [hit(take(ds, 3.0, 0)['source'], frame(8), frame(10), 50, 20, 12.5),
         hit(take(ds, 3.0, 1)['source'], frame(8), frame(10), 85, 20, 12.5),
         hit(take(ds, 3.0, 2)['source'], frame(8), frame(10), 50, 20, 17.5),
         hit(take(ds, 3.0, 3)['source'], frame(8), frame(10), 85, 20, 17.5)]
back = capsule(take(ds, 11.0)['source'], frame(21), frame(24), (-5, 24), (-70, 24), 22)
moves['dsmash'] = {'name': 'Down Smash', 'phases': segments(
    49, [0, 7, 10, 20, 24, 49], ['windup', 'front', 'link', 'back', 'recovery'], front + back,
    anim=[{'at': 0, 'pose': 'sweepLow'}, {'at': 8, 'pose': 'sweepLow', 'lerp': True},
          {'at': 19, 'pose': 'cross', 'lerp': True}, {'at': 21, 'pose': 'cross'},
          {'at': 34, 'pose': 'stand', 'lerp': True}],
    keys=[[0, 150], [1, 190], [6, 300], [8, 350], [19, 300], [21, 190], [48, 95]],
    events=[{'at': frame(2), 'sfx': 'smash'}, {'at': frame(8), 'sfx': 'slash'}, {'at': frame(21), 'sfx': 'slash'}],
    charge={'frame': frame(2)})}

# --- aerials: landing lag and autocancel boundaries from UFD; hits from the scripts
an = raw('nair', 'cloud', 'AttackAirN')
a = take(an, 8.0, 0)['source']
nair = [hit(a, frame(5), frame(8), 34, 52, 19), hit(a, frame(5), frame(9), 18, 78, 19),
        hit(a, frame(7), frame(11), -22, 56, 19), hit(a, frame(9), frame(15), -4, 18, 19)]
moves['nair'] = {'name': 'Neutral Air', 'air': 'aerial', 'phases': segments(
    38, [0, 30, 38], ['main', 'late'], nair,
    anim=swing('air', frame(5), frame(15), 38, 'air'),
    keys=[[0, 300], [4, 340], [6, 380], [9, 480], [12, 560], [15, 660], [37, 815]],
    land_lag=9, land_ends=(1,))}
af = raw('fair', 'cloud', 'AttackAirF')
fair = [hit(take(af, 14.0, 0, at=18)['source'], frame(18), frame(19), 72, 37, 16.5),
        hit(take(af, 14.0, 1, at=18)['source'], frame(18), frame(19), 72, 37, 16.5),
        hit(take(af, 13.0, 0, at=18)['source'], frame(18), frame(19), 70, 50, 39),
        hit(take(af, 13.0, 1, at=18)['source'], frame(18), frame(19), 25, 50, 20),
        hit(take(af, 11.0, at=20)['source'], frame(20), frame(25), 65, 12, 17.5)]
moves['fair'] = {'name': 'Forward Air', 'air': 'aerial', 'phases': segments(
    53, [0, 2, 17, 19, 25, 42, 53], ['startup', 'windup', 'clean', 'late', 'recovery', 'autocancel'], fair,
    anim=[{'at': 0, 'pose': 'air'}, {'at': 8, 'pose': 'airHigh', 'lerp': True}, {'at': 15, 'pose': 'airHigh'},
          {'at': 18, 'pose': 'airLow'}, {'at': 28, 'pose': 'air', 'lerp': True}],
    keys=[[0, 270], [10, 300], [16, 90], [18, -50], [25, -70], [52, 95]],
    land_lag=11, land_ends=(0, 5))}  # UFD autocancels on the opening frames too
ab = raw('bair', 'cloud', 'AttackAirB')
bair = [hit(take(ab, 13.0, 0, at=11)['source'], frame(11), frame(11), -55, 42, 35),
        hit(take(ab, 13.0, 0, at=12)['source'], frame(12), frame(12), -55, 80, 35)]
moves['bair'] = {'name': 'Back Air', 'air': 'aerial', 'phases': segments(
    42, [0, 4, 10, 11, 12, 36, 42], ['startup', 'windup', 'hit1', 'hit2', 'recovery', 'autocancel'], bair,
    anim=[{'at': 0, 'pose': 'air'}, {'at': 6, 'pose': 'airBack', 'lerp': True}, {'at': 12, 'pose': 'airBack'},
          {'at': 20, 'pose': 'air', 'lerp': True}],
    keys=[[0, 95], [5, 60], [9, 180], [12, 135], [41, 95]],
    land_lag=8, land_ends=(0, 5))}  # UFD autocancels on the opening frames too
au = raw('uair', 'cloud', 'AttackAirHi')
uair_end = int(re.findall(r'\d+', ufd['Up Air']['active'])[-1])
uair = [*capsule(take(au, 11.0, 0, at=8)['source'], frame(8), frame(9), (0, 120), (45, 90), 21),
        *capsule(take(au, 11.0, 1, at=8)['source'], frame(8), frame(9), (22, 120), (52, 82), 21),
        *capsule(take(au, 11.0, 2, at=8)['source'], frame(8), frame(9), (44, 115), (60, 74), 21),
        hit(take(au, 11.0, 0, at=9)['source'], frame(9), frame(9), 10, 30, 20),
        hit(take(au, 11.0, 1, at=9)['source'], frame(9), frame(9), 12, 50, 20),
        hit(take(au, 11.0, 2, at=9)['source'], frame(9), frame(9), 14, 70, 20),
        hit(take(au, 8.0, 0, at=10)['source'], frame(10), frame(uair_end), 8, 30, 15),
        hit(take(au, 8.0, 1, at=10)['source'], frame(10), frame(uair_end), 10, 50, 15),
        hit(take(au, 8.0, 2, at=10)['source'], frame(10), frame(uair_end), 12, 70, 15)]
moves['uair'] = {'name': 'Up Air', 'air': 'aerial', 'phases': segments(
    44, [0, 7, 9, 24, 31, 44], ['windup', 'early', 'late', 'recovery', 'autocancel'], uair,
    anim=[{'at': 0, 'pose': 'air'}, {'at': 6, 'pose': 'airHigh', 'lerp': True}, {'at': 23, 'pose': 'airHigh'},
          {'at': 32, 'pose': 'air', 'lerp': True}],
    keys=[[0, 150], [5, 250], [7, 330], [9, 400], [12, 460], [43, 470]],
    land_lag=9, land_ends=(4,))}
ad = raw('dair', 'cloud', 'AttackAirLw')
dair_end = int(re.findall(r'\d+', ufd['Down Air']['active'])[-1])
dair = [hit(take(ad, 15.0, at=11)['source'], frame(11), frame(13), 5, -55, 18),
        hit(take(ad, 13.0, at=11)['source'], frame(11), frame(13), 5, -60, 22.5),
        hit(take(ad, 8.0, at=14)['source'], frame(14), frame(dair_end), 5, -55, 17.5)]
moves['dair'] = {'name': 'Down Air', 'air': 'aerial', 'phases': segments(
    64, [0, 4, 10, 13, 40, 45, 64], ['startup', 'windup', 'early', 'late', 'recovery', 'autocancel'], dair,
    anim=[{'at': 0, 'pose': 'air'}, {'at': 8, 'pose': 'airHigh', 'lerp': True},
          {'at': 11, 'pose': 'airLow'}, {'at': 34, 'pose': 'airLow'}, {'at': 46, 'pose': 'air', 'lerp': True}],
    keys=[[0, 60], [8, 250], [11, 270], [40, 270], [63, 95]],
    land_lag=16, land_ends=(0, 5))}  # UFD autocancels on the opening frames too

# --- grabs
grab1 = frame(9)
grab_box = [*capsule({'Damage': '0', 'Angle': '0', 'KBG': '0', 'BKB': '0', 'Hitlag': '1'}, grab1, grab1 + 1,
                     (20, 33), (43.5, 33), 16.5, type='grab'),
            *capsule({'Damage': '0', 'Angle': '0', 'KBG': '0', 'BKB': '0', 'Hitlag': '1'}, grab1, grab1 + 1,
                     (11.75, 33), (51.75, 33), 8.25, type='grab')]
moves['grab'] = {'name': 'Grab', 'phases': [phase(
    'main', 36, grab_box, anim=swing('thrust', frame(9), frame(10), 36),
    attach=attach([[0, 95], [7, 60], [9, 20], [35, 95]]), events=[{'at': grab1, 'sfx': 'grab'}])]}
raw('grab', 'cloud', 'Catch')
dg1 = frame(12)
dash_grab_box = [*capsule({'Damage': '0', 'Angle': '0', 'KBG': '0', 'BKB': '0', 'Hitlag': '1'}, dg1, dg1 + 1,
                         (20, 33), (52, 33), 13, type='grab'),
                 *capsule({'Damage': '0', 'Angle': '0', 'KBG': '0', 'BKB': '0', 'Hitlag': '1'}, dg1, dg1 + 1,
                         (13.5, 33), (58.5, 33), 6.5, type='grab')]
moves['dashgrab'] = {'name': 'Dash Grab', 'phases': [phase(
    'main', 44, dash_grab_box, slide=frame(18), anim=swing('thrust', frame(12), frame(13), 44),
    attach=attach([[0, 95], [10, 60], [12, 20], [43, 95]]), events=[{'at': dg1, 'sfx': 'grab'}])]}
raw('dashgrab', 'cloud', 'CatchDash')

# --- specials
def beam_projectile(name, damage, kbg, vel, life, r, colour, glow):
    shapes = [circle((0, 0), r, colour, alpha=0.55), circle((0, 0), r * 0.66, glow, alpha=0.85),
              line((-r, 0), (r, 0), r * 0.25, glow), line((0, -r), (0, r), r * 0.2, colour)]
    return {'name': name, 'spawn': [46, 58], 'vel': [vel, 0], 'r': r, 'life': life, 'max': 1,
            'hit': {'start': 0, 'x': 0, 'y': 0, 'r': r, 'damage': damage, 'bkb': 38.0, 'kbg': kbg, 'angle': 55,
                    'hitlagMult': 0.5, 'shieldDmgMult': 0.5},
            'spawnSfx': 'shoot', 'shapes': shapes,
            'trail': [{'every': 2, 'fx': [
                {'type': 'part', 'x': 0, 'y': 0, 'vx': [-0.6, 0.6], 'vy': [-1.2, 1.2], 'life': [8, 16], 'size': 3.0,
                 'color': colour}]}]}


projectiles = {
    'bladeBeam': beam_projectile('Blade Beam', 8.0, 0.60, 15.7143, 44, 15, '#67d08a', '#d8ffe4'),
    'bladeBeamAir': beam_projectile('Blade Beam', 6.4, 0.60, 12.8571, 40, 15, '#67d08a', '#d8ffe4'),
    'limitBlade': beam_projectile('Limit Blade Beam', 19.0, 0.726, 20.0, 46, 40, '#3f7dff', '#d9ecff'),
    'limitBladeAir': beam_projectile('Limit Blade Beam', 15.0, 0.803, 15.7143, 42, 40, '#3f7dff', '#d9ecff'),
}
for key in ('limitBlade', 'limitBladeAir'):
    projectiles[key]['hit'].update({'bkb': 50.0, 'angle': 48, 'hitlagMult': 1.0, 'shieldDmgMult': 1.0})
    projectiles[key]['spin'] = 24


def blade_beam(limit):
    if limit:
        raw('nspecLimit', 'cloud', 'SpecialN_LB')
        return [phase('main', 60, cost={'charge': 'limit', 'uses': 1},
                      anim=[{'at': 0, 'pose': 'stand'}, {'at': 6, 'pose': 'overhead', 'lerp': True},
                            {'at': 15, 'pose': 'cast'}, {'at': 30, 'pose': 'stand', 'lerp': True}],
                      attach=attach([[0, 150], [8, 190], [14, 30], [16, 0], [59, 95]]),
                      events=[{'at': frame(16), 'if': 'grounded', 'spawn': 'limitBlade', 'sfx': 'shoot'},
                              {'at': frame(16), 'if': 'airborne', 'spawn': 'limitBladeAir', 'sfx': 'shoot'},
                              {'at': frame(10), 'invuln': 8},
                              {'at': frame(16), 'sfx': 'slash'}],
                      transitions=[land(7)])]
    raw('nspec', 'cloud', 'SpecialN')
    # Source: a 0-damage windbox on gameplay frames 16-17 in front of Cloud.
    wind = [{'start': frame(16), 'end': frame(17), 'x': 30, 'y': 33, 'r': 12, 'damage': 0.0, 'bkb': 6.0,
             'kbg': 0.0, 'angle': 20, 'type': 'windbox', 'situation': 'ground'}]
    startup = phase('startup', 18, wind,
                    anim=[{'at': 0, 'pose': 'stand'}, {'at': 6, 'pose': 'overhead', 'lerp': True},
                          {'at': 15, 'pose': 'cast'}],
                    attach=attach([[0, 150], [8, 190], [14, 40], [17, 0], [18, 0]]),
                    events=[{'at': frame(18), 'if': 'grounded', 'spawn': 'bladeBeam', 'sfx': 'shoot'},
                            {'at': frame(18), 'if': 'airborne', 'spawn': 'bladeBeamAir', 'sfx': 'shoot'}],
                    transitions=[{'on': 'timeout', 'if': 'airborne', 'to': 'airTail'},
                                 {'on': 'timeout', 'to': 'groundTail'}])
    ground = phase('groundTail', 35, anim=[{'at': 0, 'pose': 'cast'},
                                           {'at': 20, 'pose': 'stand', 'lerp': True}],
                   attach=attach([[0, 0], [34, 95]]))
    air = phase('airTail', 42, anim=[{'at': 0, 'pose': 'cast'},
                                     {'at': 26, 'pose': 'stand', 'lerp': True}],
                attach=attach([[0, 0], [41, 95]]), transitions=[land(7)])
    return [startup, ground, air]


moves['nspec'] = {
    'name': 'Blade Beam', 'bReverse': True,
    'variants': [{'needs': {'charge': 'limit', 'uses': 1}, 'phases': blade_beam(True)}],
    'phases': blade_beam(False)}

# --- side special: Cross Slash (normal chains three slashes; the Limit variant runs five hits in one timeline)
ss1 = raw('sspec', 'cloud', 'SpecialS1')
ss2 = raw('sspec2', 'cloud', 'SpecialS2')
ss3 = raw('sspec3', 'cloud', 'SpecialS3')
slash1 = [hit(take(ss1, 4.0, 0)['source'], frame(10), frame(11), 62, 45, 17.5),
          hit(take(ss1, 4.0, 1)['source'], frame(10), frame(11), 85, 88, 17.5),
          hit(take(ss1, 4.0, 2)['source'], frame(10), frame(11), 70, 96, 35)]
slash2 = [hit(take(ss2, 3.0, 0)['source'], frame(2), frame(3), 82, 66, 30),
          hit(take(ss2, 3.0, 1)['source'], frame(2), frame(3), 48, 42, 15)]
slash3 = [hit(take(ss3, 3.0, 0)['source'], frame(2), frame(3), 68, 30, 30),
          hit(take(ss3, 3.0, 1)['source'], frame(2), frame(3), 68, 64, 30),
          hit(take(ss3, 3.0, 0, at=11)['source'], frame(11), frame(12), 60, 55, 30),
          hit(take(ss3, 3.0, 1, at=11)['source'], frame(11), frame(12), 85, 55, 35),
          hit(take(ss3, 6.0)['source'], frame(25), frame(26), 84, 88, 40)]
lb = raw('sspecLimit', 'cloud', 'SpecialS1_LB')
lb2 = raw('sspecLimit2', 'cloud', 'SpecialS2_LB')
lb3 = raw('sspecLimit3', 'cloud', 'SpecialS3_LB')
limit_slash = [hit(take(lb, 5.0, 0)['source'], frame(10), frame(11), 75, 50, 20),
               hit(take(lb, 5.0, 1)['source'], frame(10), frame(11), 85, 96, 40),
               hit(take(lb2, 5.0, at=2)['source'], frame(15), frame(16), 82, 66, 40),
               hit(take(lb3, 3.0, 0, at=2)['source'], frame(21), frame(22), 68, 30, 35),
               hit(take(lb3, 3.0, 1, at=2)['source'], frame(21), frame(22), 68, 64, 35),
               hit(take(lb3, 3.0, 0, at=11)['source'], frame(28), frame(28), 62, 55, 36),
               hit(take(lb3, 3.0, 1, at=11)['source'], frame(28), frame(28), 62, 78, 36),
               hit(take(lb3, 10.0, at=25)['source'], frame(38), frame(39), 86, 90, 42)]
moves['sspec'] = {
    'name': 'Cross Slash', 'bReverse': True,
    'variants': [{'needs': {'charge': 'limit', 'uses': 1}, 'phases': segments(
        60, [0, 9, 11, 14, 16, 20, 22, 27, 28, 37, 39, 60],
        ['windup', 'hit1', 'link1', 'hit2', 'link2', 'hit3', 'link3', 'hit4', 'link4', 'hit5', 'recovery'],
        limit_slash,
        anim=[{'at': 0, 'pose': 'cross'}, {'at': 10, 'pose': 'chop'}, {'at': 16, 'pose': 'cross'},
              {'at': 22, 'pose': 'thrust'}, {'at': 33, 'pose': 'cross'}, {'at': 38, 'pose': 'chop'},
              {'at': 50, 'pose': 'stand', 'lerp': True}],
        keys=[[0, 150], [8, 60], [10, 20], [16, 120], [22, 60], [28, 20], [38, 80], [59, 95]],
        events=[{'at': frame(10), 'sfx': 'slash'}, {'at': frame(38), 'sfx': 'smash'},
                {'at': frame(6), 'invuln': 6}],
        cost={'charge': 'limit', 'uses': 1})}],
    'phases': [
        phase('slash1', 12, slash1, anim=swing('chop', frame(10), frame(11), 12),
              attach=attach([[0, 150], [8, 60], [10, 20], [11, -40]]),
              events=[{'at': frame(10), 'sfx': 'slash'}], transitions=[wait('slash2')]),
        phase('slash2', 14, slash2, anim=swing('cross', frame(2), frame(3), 14),
              attach=attach([[0, 60], [2, -40], [3, -70], [13, 95]]),
              events=[{'at': frame(2), 'sfx': 'slash'}], transitions=[wait('windup')]),
        *segments(55, [0, 1, 3, 10, 12, 24, 26, 55],
                  ['windup', 'hit1', 'link', 'hit2', 'link2', 'hit3', 'recovery'], slash3,
                  anim=[{'at': 0, 'pose': 'cross'}, {'at': 2, 'pose': 'sweepLow', 'lerp': True},
                        {'at': 11, 'pose': 'thrust'}, {'at': 25, 'pose': 'chop'},
                        {'at': 40, 'pose': 'stand', 'lerp': True}],
                  keys=[[0, 200], [2, 300], [11, 60], [25, 20], [40, 95]],
                  events=[{'at': frame(2), 'sfx': 'slash'}, {'at': frame(25), 'sfx': 'smash'}])]}

# --- up special: Climhazzard (rising slash, optional downward follow-up, landing lag 26)
hi = raw('uspec', 'cloud', 'SpecialHi')
thrust = [hit(take(hi, 3.0, 0)['source'], frame(7), frame(7), 20, 35, 10),
          hit(take(hi, 3.0, 1)['source'], frame(7), frame(7), 30, 35, 25),
          hit(take(hi, 3.0, 2)['source'], frame(7), frame(7), 60, 35, 17.5)]
# The rise starts on move frame 9 (source 10); these windows are rise-local.
rise = [hit(take(hi, 4.0, 0, at=10)['source'], 0, 0, 8, 30, 25),
        hit(take(hi, 4.0, 1, at=10)['source'], 0, 0, 8, 70, 25),
        hit(take(hi, 4.0, 0, at=11)['source'], 1, 1, 8, 70, 17.5),
        hit(take(hi, 4.0, 0, at=12)['source'], 2, 6, 10, 30, 20),
        hit(take(hi, 4.0, 1, at=12)['source'], 2, 6, 12, 55, 18),
        hit(take(hi, 4.0, 2, at=12)['source'], 2, 6, 14, 80, 18)]
hi2 = raw('uspec2', 'cloud', 'SpecialHi2')
plunge = [hit(take(hi2, 4.5, 0)['source'], frame(12), None, 30, 10, 15),
          hit(take(hi2, 4.5, 1)['source'], frame(12), None, 25, 20, 20)]
lhi = raw('uspecLimit', 'cloud', 'SpecialHi_LB')
lthrust = [hit(take(lhi, 6.0, 0)['source'], frame(7), frame(10), 20, 35, 10),
           hit(take(lhi, 6.0, 1)['source'], frame(7), frame(10), 30, 35, 25),
           hit(take(lhi, 6.0, 2)['source'], frame(7), frame(10), 60, 35, 17.5)]
lrise = [hit(take(lhi, 7.0, 0, at=10)['source'], 0, 1, 8, 30, 25),
         hit(take(lhi, 7.0, 1, at=10)['source'], 0, 1, 8, 70, 25),
         hit(take(lhi, 7.0, 0, at=11)['source'], 1, 5, 8, 30, 25),
         hit(take(lhi, 7.0, 1, at=11)['source'], 1, 5, 8, 70, 25),
         hit(take(lhi, 7.0, 2, at=11)['source'], 1, 5, 10, 55, 22)]
land_hit = [hit({'Damage': '3.5', 'Angle': '64', 'KBG': '128', 'BKB': '64', 'Hitlag': '1.3'},
                0, 2, 40, 40, 40)]


def climhazzard(limit):
    thrust_boxes = lthrust if limit else thrust
    rise_boxes = lrise if limit else rise
    speed = 19.1703 if limit else 15.5188  # fitted rise: 21 / 17 at the old scale × √(5/6), same height
    # Nine thrust ticks (move frames 0-8), so the rise starts on move frame 9 = source frame 10.
    thrust_phase = phase('thrust', 9, thrust_boxes, ledgeGrab=frame(7),
                         anim=[{'at': 0, 'pose': 'stand'}, {'at': 5, 'pose': 'thrust', 'lerp': True},
                               {'at': 8, 'pose': 'rise'}],
                         attach=attach([[0, 150], [6, 20], [8, 90], [9, 90]]),
                         events=[{'at': frame(7), 'sfx': 'slash'},
                                 {'at': frame(9), 'setVel': {'y': speed}, 'leaveGround': True},
                                 {'at': frame(5), 'invuln': 8}],
                         transitions=[wait('rise')])
    if limit:
        rise_cuts = [0, 1, 2, 6, 36]
        rise_names = ['rise', 'hit2', 'hit3', 'tail']
    else:
        thrust_phase['events'].pop()  # only Limit Climhazzard is intangible (UFD "Limit ... frame 5-12")
        rise_cuts = [0, 1, 2, 7, 24]
        rise_names = ['rise', 'hit2', 'hit3', 'tail']
    rise_phases = segments(36 if limit else 24, rise_cuts, rise_names, rise_boxes,
                           anim=[{'at': 0, 'pose': 'rise'}], keys=[[0, 90], [8, 120], [14, 95]],
                           ledgeGrab=0, airAccelMult=drift(0.5, 'upSpecial'))
    # A fresh special press starts the plunge from move frame 20 (normal) / 33 (Limit).
    follow = frame(33) if limit else frame(20)
    last = rise_phases[-1]
    last['transitions'] = [{'on': 'specialPress', 'from': max(0, follow - 9 - rise_cuts[-2]), 'to': 'plunge'},
                           wait('helpless')]
    return [thrust_phase] + rise_phases


plunge_phase = phase('plunge', 26, plunge, airAccelMult=drift(0.3, 'upSpecial'),
                     anim=[{'at': 0, 'pose': 'plunge'}],
                     attach=attach([[0, 90], [4, 180], [12, 270], [25, 95]]),
                     events=[{'at': 0, 'setVel': {'x': 0, 'y': -11.6667}}, {'at': frame(12), 'sfx': 'slash'}],
                     transitions=[{'on': 'land', 'to': 'landHit'}, wait('helpless')])
land_hit = phase('landHit', 26, land_hit,
                 anim=[{'at': 0, 'pose': 'plunge'}, {'at': 8, 'pose': 'stand', 'lerp': True}],
                 attach=attach([[0, 270], [9, 95]]), events=[{'at': 0, 'sfx': 'quake'}])
limit_climb = climhazzard(True)
limit_climb[0]['cost'] = {'charge': 'limit', 'uses': 1}
moves['uspec'] = {'name': 'Climhazzard', 'air': 'upSpecial', 'variants': [
    {'needs': {'charge': 'limit', 'uses': 1},
     'phases': limit_climb + [copy.deepcopy(plunge_phase), copy.deepcopy(land_hit)]}],
    'phases': climhazzard(False) + [copy.deepcopy(plunge_phase), copy.deepcopy(land_hit)]}

# --- down special: Limit Charge; with a full gauge it becomes Finishing Touch
lw = raw('dspec', 'cloud', 'SpecialLw')
charge_frames = 334  # 100 units at 0.3/frame since 7.0.0 (5.567 s); UFD and SmashWiki agree
finishing = [hit(take(lw, 1.0, 0, at=16)['source'], frame(12), frame(12), 45, 95, 30),
             hit(take(lw, 1.0, 0, at=17)['source'], frame(13), frame(14), 60, 100, 28),
             hit(take(lw, 1.0, 0, at=18)['source'], frame(22), frame(26), -10, 100, 28),
             hit(take(lw, 1.0, 0, at=19)['source'], frame(22), frame(26), -55, 80, 30),
             {'start': frame(22), 'end': frame(26), 'x': 0, 'y': 60, 'r': 70, 'damage': 0.0, 'bkb': 9.0,
              'kbg': 0.0, 'angle': 10, 'type': 'windbox', 'dir': 'away'}]
moves['dspec'] = {
    'name': 'Limit Charge', 'variants': [{'needs': {'charge': 'limit', 'uses': 1}, 'phases': segments(
        84, [0, 11, 15, 21, 26, 84], ['windup', 'front', 'link', 'rear', 'recovery'], finishing,
        anim=[{'at': 0, 'pose': 'spin'}, {'at': 10, 'pose': 'overhead', 'lerp': True},
              {'at': 14, 'pose': 'chop'}, {'at': 24, 'pose': 'cross'}, {'at': 40, 'pose': 'spin'},
              {'at': 60, 'pose': 'stand', 'lerp': True}],
        keys=[[0, 150], [6, 200], [11, 300], [13, 380], [21, 480], [60, 830], [83, 815]],
        events=[{'at': frame(13), 'sfx': 'smash'}, {'at': frame(22), 'sfx': 'quake'}],
        cost={'charge': 'limit', 'uses': 1})}],
    'phases': [phase('charge', 13, charge={'frame': frame(7), 'max': charge_frames, 'holdAfterMax': 600,
                                           'button': 'special'},
                     anim=[{'at': 0, 'pose': 'guard'}, {'at': 5, 'pose': 'hold', 'lerp': True},
                           {'at': 8, 'pose': 'hold'}],
                     attach=attach([[0, 150], [6, 95], [12, 95]]),
                     events=[{'at': 0, 'sfx': 'charge'},
                             {'at': 12, 'minCharge': charge_frames,
                              'restore': {'charge': 'limit', 'uses': 1}, 'sfx': 'ready'}],
                     transitions=[{'on': 'shieldPress', 'to': 'end'}])]}

# ---------------------------------------------------------------- throws
def throw(slot, script, total, release, name, ticks=()):
    s = scripts['cloud'][script]
    line = next(x for x in s.splitlines() if 'ATTACK_ABS(Kind=FIGHTER_ATTACK_ABSOLUTE_KIND_THROW' in x)
    a = args(line.split('THROW,', 1)[1].rstrip(')'))
    box = hit(a, 0, 0, 0, 0, 60)
    ledger['rawAttacks'][slot] = {'article': 'cloud', 'script': script, 'attacks': [a]}
    return {'release': frame(release), 'total': total, 'hit': box, 'ticks': list(ticks)}


throws = {
    'f': throw('fthrow', 'ThrowF', 35, 9, 'Forward', [{'at': frame(7), 'damage': 4.0}]),
    'b': throw('bthrow', 'ThrowB', 31, 15, 'Back', [{'at': frame(13), 'damage': 3.0}]),
    'u': throw('uthrow', 'ThrowHi', 50, 15, 'Up', [{'at': frame(8), 'damage': 2.0},
                                                  {'at': frame(13), 'damage': 2.5}]),
    'd': throw('dthrow', 'ThrowLw', 43, 18, 'Down'),
}

# ---------------------------------------------------------------- fighter document
# UFD movement (units/frame, units/frame²) × SCALE. Jump speeds come from the ssbwiki Cloud (SSBU) attributes
# (units: full hop 32.5, short hop 17, double jump 32.5); Limit Break's boosts are not modeled. runAccel is fitted
# (1.0 at the old 3.5 px/unit scale).
GRAVITY = round(0.098 * SCALE, 5)
stats = {
    'weight': 1.0, 'pummelDamage': 1.3,
    'walkSpeed': round(1.155 * SCALE, 5), 'runSpeed': round(2.167 * SCALE, 5), 'dashSpeed': round(2.145 * SCALE, 5),
    'runAccel': 1.4286,
    **units.gait(scripts['cloud']),  # Dash / TurnDash / TurnRun / RunBrakeR flag frames
    'groundFriction': 0.8,
    **units.jumps(32.5, 17, 32.5, GRAVITY), 'airJumpDrift': round(1.155 * SCALE, 5), 'airJumps': 1,
    'airSpeed': round(1.155 * SCALE, 5), 'airAccel': round(0.07 * SCALE, 5), 'gravity': GRAVITY,
    'fallSpeed': round(1.68 * SCALE, 5), 'fastFallSpeed': round(2.688 * SCALE, 5),
    'traction': round(0.106 * SCALE, 5), 'airFriction': round(0.009 * SCALE, 5),  # Ultimate ground_brake / air_brake_x (fighter_param dump; ssbwiki Traction, Air friction)
}
doc = {
    '$schema': '../../schema/fighter.schema.json',
    'schemaVersion': 1,
    'extends': 'base',
    'name': 'Stratus',
    'tagline': 'Soldier of the sky',
    'blurb': 'A fast-moving swordsman with a huge disjointed blade and slow, heavy swings. Charge Limit with the '
             'down special, then spend it on a Limit Break special: a piercing Blade Beam, a five-hit Cross Slash, '
             'a towering Climhazzard, or Finishing Touch.',
    'stats': stats,
    'charges': [
        {'id': 'limit', 'uses': 1, 'recharge': 0, 'emptySfx': 'toss', 'refillSfx': 'ready'},
    ],
    'ai': {
        'reach': 82, 'counterChance': 0,
        'specials': [
            {'minDistance': 150, 'height': 40, 'grounded': True, 'noProjectile': False, 'chance': 0.5,
             'neutral': True, 'down': False, 'hold': None},
            {'minDistance': 70, 'maxDistance': 210, 'height': 60, 'grounded': True, 'noProjectile': True,
             'chance': 0.45, 'neutral': False, 'down': False, 'hold': None},
            # Charge Limit while the opponent is far: the hold must reach the source's 334 ticks to fill it.
            {'minDistance': 200, 'height': 60, 'grounded': True, 'noProjectile': False, 'chance': 0.5,
             'neutral': False, 'down': True, 'hold': [334, 350]},
        ],
    },
    'moves': moves,
    'throws': throws,
    'projectiles': {k: v for k, v in projectiles.items()},
    'style': style,
    'poses': poses,
    'anims': anims,
    'attachments': attachments,
}

out = ROOT / 'assets/fighters/stratus/fighter.json'
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(doc, indent=2) + '\n')
(SOURCES / 'ledger.json').write_text(json.dumps(ledger, indent=1) + '\n')
print(f'wrote {out.relative_to(ROOT)} ({out.stat().st_size} bytes)')
