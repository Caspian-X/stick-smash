#!/usr/bin/env python3
"""Rebuild Throbbin (SSBU Robin) from pinned 13.0.1 scripts and documented authoring rules.

Numbers come from the pinned attack scripts (damage, launch fields, masks, hitlag, rehit) and the pinned
Ultimate Frame Data rows (startup, active windows, totals, landing lag, autocancel, charge frames). All
weapon-bone geometry is *fitted* to the stick-figure poses authored below, not projected from Nintendo's
skeleton. Run `just import-robin`.
"""
import copy, hashlib, json, math, re
from pathlib import Path

# Phase drift (`airAccelMult`) is written as a multiple of the old shared air accels (aerial 0.45, attack 0.25,
# upSpecial 0.3). Under `movement.ultimate` it multiplies airAccel × the kind's multiplier instead, so drift()
# stores the value that keeps the same effective acceleration.
AIR_ACCEL = 0.28
def drift(legacy, kind):
    old = {'aerial': 0.45, 'attack': 0.25, 'upSpecial': 0.3}[kind]
    mult = {'aerial': 1.0, 'attack': 0.55, 'upSpecial': 0.65}[kind]
    return round(legacy * old / (AIR_ACCEL * mult), 4)

ROOT = Path(__file__).resolve().parents[2]
SOURCES = ROOT / 'docs/imports/sources/throbbin'
source = json.loads((SOURCES / 'scripts.json').read_text())
ufd = json.loads((SOURCES / 'ufd.json').read_text())
scripts = {art: {s['Name']: s['Data'] for s in ss} for art, ss in source['scripts'].items()}
ledger = {'sourceCommit': source['commit'], 'sourceSha256': source['sha256'], 'geometryScale': 5, 'rawAttacks': {}}


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
    """Attack records with the sword-variant block they sit in: 'levin', 'bronze' or 'both'."""
    src = expand(scripts[article][name])
    t, live, out, stack, pending = 0, {}, [], [], None

    def tag():
        for c in reversed(stack):
            if c['tag']:
                return c['tag']
        return 'both'

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
        if 'FLAG_THUNDER_SWORD_ON' in line and 'is_flag' in line:
            stack.append({'kind': 'sword', 'tag': None})
            pending = 'levin'
            continue
        if pending == 'levin' and 'operator==' in line:
            stack[-1]['tag'] = 'levin'
            pending = None
            continue
        if line.startswith('else{') and stack and stack[-1]['kind'] == 'sword':
            stack[-1]['tag'] = 'bronze'
            continue
        if line == '}' and stack:
            stack.pop()
            continue
        if 'AttackModule::clear_all()' in line:
            for i in list(live):
                clear(i)
        elif m := re.search(r'AttackModule::clear\(ID=(\d+)\)', line):
            clear(int(m[1]))
        if m := re.search(r'\bATTACK\((.*)\)', line):
            a = args(m[1]); i = int(a['ID']); clear(i)
            live[i] = {'start': t, 'end': t + 1, 'tag': tag(), 'source': a}
    for i in list(live):
        clear(i)
    return sorted(out, key=lambda b: (b['start'], int(b['source']['ID'])))


def raw(slot, article, name):
    rows = attacks(article, name)
    ledger['rawAttacks'][slot] = {'article': article, 'script': name, 'attacks': rows}
    return rows


def take(rows, damage, nth=0, tag=None):
    found = [b for b in rows if abs(float(b['source']['Damage']) - damage) < 1e-6 and (tag is None or b['tag'] == tag)]
    return found[nth]


def take_bone(rows, bone, damage, tag=None):
    """The first attack row with this bone (and damage), so an arm box can be picked independently."""
    for b in rows:
        if bone in b['source'].get('Bone', '') and abs(float(b['source']['Damage']) - damage) < 1e-6 \
                and (tag is None or b['tag'] == tag):
            return b
    raise LookupError(f'no {bone} attack at {damage}')


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
    # The engine has one hit type per hitbox. A 361/autolink source angle keeps its angle behavior (Sakurai,
    # autolink); electricity is then presentation only. Other electric hits use the electric hitlag/spark type.
    if 'elec' in a.get('Effect', '') and kind == 'normal':
        b['type'] = 'electric'
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
    """Fitted per-frame track: `pts` is [(phase frame, x, y)] over the active window."""
    x, y, _ = pts[0][1], pts[0][2], 0
    b = hit(a, pts[0][0], pts[-1][0], x, y, r, **kw)
    b['frames'] = [[t, round(px, 2), round(py, 2), round(r, 2)] for t, px, py in pts]
    return b


def phase(name, frames, boxes=(), **kw):
    p = {'name': name, 'frames': frames}
    if boxes:
        p['hitboxes'] = list(boxes)
    p.update(kw)
    return p


def land(lag):
    return {'on': 'land', 'to': 'landing', 'lag': lag}


def wait(to):
    return {'on': 'timeout', 'to': to}


def swing(pose, start, end, total, air=False, **kw):
    return [{'at': 0, 'pose': 'air' if air else 'stand'}] + \
        [{'at': max(0, start - 3), 'pose': pose, 'lerp': True}, {'at': end, 'pose': pose}] + \
        [{'at': min(max(1, total - 1), end + 6), 'pose': 'air' if air else 'stand', 'lerp': True}]


def tilted(base, deg, pivot=50, **over):
    p = {'from': base, 'rot': deg, 'pivot': pivot}
    p.update(over)
    return p


def tilt_attach(attach, deg):
    """Keep the blade's world direction under a body lean: the body rotation takes `deg` off every pose angle,
    so add it back to the keyed track."""
    a = copy.deepcopy(attach)
    if a.get('sword', {}).get('angle'):
        a['sword']['angle'] = [[t, d + deg] for t, d in a['sword']['angle']]
    return a


BRONZE = 0.62


def swordk(keys, scale=None):
    """`attach` map for the sword from `[frame, degrees]` keys (0 = forward, 90 = up). `scale` shortens the
    blade: the Bronze Sword is about 62% of the Levin Sword's length."""
    a = {'angle': [[float(f), float(d)] for f, d in keys]}
    if scale is not None:
        a['scale'] = [[0, scale]]
    return {'sword': a}


def sword(end_angle, start_angle=None, end_frame=None, scale=None):
    if start_angle is None:
        return swordk([[0, end_angle]], scale)
    return swordk([[0, start_angle], [max(1, end_frame), end_angle]], scale)


def stowed():
    """Hide the hand sword and show it sheathed on his back for this phase (casting, grabbing, wind loops)."""
    return {'sword': {'hide': [0, 4294967295]}, 'swordBack': {'show': [0, 4294967295]}}


# ---------------------------------------------------------------- poses and bodies
stand = {'head': [2, 64], 'shoulder': [0, 50], 'hip': [0, 30], 'elbowF': [11, 43], 'handF': [20, 39],
         'elbowB': [-11, 43], 'handB': [-19, 37], 'kneeF': [4, 16], 'footF': [6, 1], 'kneeB': [-5, 16],
         'footB': [-8, 1]}
poses = {
    'stand': stand,
    'thrust': {'head': [4, 62], 'shoulder': [2, 49], 'hip': [-2, 29], 'elbowF': [18, 47], 'handF': [34, 47],
               'elbowB': [-10, 44], 'handB': [-16, 41], 'kneeF': [12, 15], 'footF': [20, 1], 'kneeB': [-6, 15],
               'footB': [-12, 1]},
    # Fitted body shapes: a deep lunge for the smash/dash cut, a raised-blade windup, and airborne leans.
    'lunge': {'from': 'stand', 'head': [6, 60], 'shoulder': [3, 48], 'hip': [2, 28], 'elbowF': [20, 46],
              'handF': [38, 46], 'elbowB': [-12, 42], 'handB': [-20, 38], 'kneeF': [17, 14], 'footF': [30, 1],
              'kneeB': [-10, 14], 'footB': [-24, 2]},
    'windup': {'from': 'stand', 'head': [0, 62], 'shoulder': [-2, 49], 'hip': [3, 28], 'elbowF': [6, 54],
               'handF': [0, 69], 'elbowB': [-13, 44], 'handB': [-21, 40], 'kneeF': [9, 15], 'footF': [13, 1],
               'kneeB': [-8, 14], 'footB': [-17, 2]},
    # Both hands out at chest height: the Thunder tiers are thrown from the forward hand.
    'castForward': {'from': 'stand', 'head': [2, 63], 'shoulder': [1, 50], 'hip': [0, 30], 'elbowF': [18, 52],
                    'handF': [36, 55], 'elbowB': [4, 50], 'handB': [20, 53], 'kneeF': [5, 16], 'footF': [8, 1],
                    'kneeB': [-6, 16], 'footB': [-10, 1]},

    'sweepLow': {'head': [6, 50], 'shoulder': [2, 38], 'hip': [-2, 20], 'elbowF': [20, 34], 'handF': [40, 26],
                 'elbowB': [-12, 32], 'handB': [-18, 24], 'kneeF': [18, 11], 'footF': [30, 1], 'kneeB': [-8, 10],
                 'footB': [-14, 1]},
    'sweepUp': {'head': [-2, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [8, 62], 'handF': [12, 78],
                'elbowB': [-10, 46], 'handB': [-16, 40], 'kneeF': [3, 17], 'footF': [4, 1], 'kneeB': [-6, 17],
                'footB': [-8, 1]},
    'overhead': {'head': [-6, 64], 'shoulder': [-4, 51], 'hip': [2, 31], 'elbowF': [4, 62], 'handF': [2, 80],
                 'elbowB': [-14, 46], 'handB': [-20, 40], 'kneeF': [8, 16], 'footF': [12, 1], 'kneeB': [-8, 16],
                 'footB': [-14, 1]},
    'cast': {'head': [3, 64], 'shoulder': [1, 50], 'hip': [-1, 30], 'elbowF': [16, 50], 'handF': [36, 56],
             'elbowB': [-12, 44], 'handB': [-20, 42], 'kneeF': [5, 16], 'footF': [8, 1], 'kneeB': [-6, 16],
             'footB': [-10, 1]},
    'castHigh': {'head': [0, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [14, 60], 'handF': [22, 78],
                 'elbowB': [-14, 48], 'handB': [-24, 52], 'kneeF': [4, 17], 'footF': [6, 1], 'kneeB': [-6, 17],
                 'footB': [-9, 1]},
    'drain': {'head': [8, 60], 'shoulder': [4, 48], 'hip': [0, 28], 'elbowF': [24, 46], 'handF': [46, 48],
              'elbowB': [-6, 42], 'handB': [-12, 38], 'kneeF': [14, 14], 'footF': [22, 1], 'kneeB': [-8, 14],
              'footB': [-16, 1]},
    'air': {'head': [2, 62], 'shoulder': [0, 49], 'hip': [0, 30], 'elbowF': [12, 44], 'handF': [24, 42],
            'elbowB': [-12, 42], 'handB': [-22, 38], 'kneeF': [10, 18], 'footF': [14, 5], 'kneeB': [-8, 16],
            'footB': [-14, 4]},
    'airHigh': {'head': [0, 62], 'shoulder': [0, 49], 'hip': [-2, 30], 'elbowF': [10, 58], 'handF': [14, 74],
                'elbowB': [-10, 44], 'handB': [-18, 40], 'kneeF': [12, 17], 'footF': [18, 4], 'kneeB': [-6, 15],
                'footB': [-10, 6]},
    'airLow': {'head': [10, 56], 'shoulder': [4, 44], 'hip': [-2, 26], 'elbowF': [20, 30], 'handF': [34, 16],
               'elbowB': [-10, 34], 'handB': [-18, 24], 'kneeF': [20, 12], 'footF': [34, 6], 'kneeB': [-6, 12],
               'footB': [-12, 4]},
    'hold': {'head': [-2, 62], 'shoulder': [-2, 49], 'hip': [-2, 30], 'elbowF': [10, 44], 'handF': [18, 46],
             'elbowB': [-12, 42], 'handB': [-18, 44], 'kneeF': [4, 17], 'footF': [6, 1], 'kneeB': [-8, 17],
             'footB': [-12, 1]},
    'win': {'head': [0, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [16, 46], 'handF': [26, 62],
            'elbowB': [-16, 50], 'handB': [-28, 58], 'kneeF': [5, 17], 'footF': [9, 1], 'kneeB': [-6, 17],
            'footB': [-10, 1]},
}

shapes = lambda *xs: list(xs)


def line(a, b, w, c, **kw):
    return {'shape': 'line', 'from': list(a), 'to': list(b), 'width': w, 'color': c, **kw}


def poly(points, c, **kw):
    return {'shape': 'poly', 'points': [list(p) for p in points], 'fill': c, **kw}


def circle(at, r, fill=None, **kw):
    s = {'shape': 'circle', 'at': list(at), 'r': r, **kw}
    if fill:
        s['fill'] = fill
    return s


poses['dive'] = tilted('air', 50)
poses['arch'] = tilted('air', 20)
poses['tuck'] = tilted('air', 40)
poses['guard'] = tilted('air', 15)

blade = 58  # Levin Sword blade length in px (fitted)
sword_shapes = [
    line((-11, 0), (2, 0), 5, '#4a3b2d'),
    line((2, -7), (2, 7), 5, '#c9a24a'),
    line((4, 0), (blade, 0), 3.2, '#8d93a8'),
    line((4, 2), (blade - 3, 2), 1.2, '#dfe4f2'),
    line((4, 0), (blade, 0), 8, '#ffdd6a', alpha=0.5, when='glow'),
    line((4, 0), (blade, 0), 2.4, '#fff7cf', when='glow'),
]
attachments = [
    {'id': 'sword', 'joint': 'handF', 'angle': 95, 'trail': {'inner': 20, 'outer': 74},
     'glow': 'active', 'hold': {'elbow': 14, 'hand': 26, 'bend': 18}, 'shapes': sword_shapes},
    # The source sheathes the Levin Sword to cast or grab: it rides diagonally on his back.
    {'id': 'swordBack', 'joint': 'shoulder', 'angle': 238, 'behind': True, 'hidden': True,
     'shapes': sword_shapes},
    {'id': 'tome', 'joint': 'handB', 'angle': 20, 'behind': True,
     'shapes': [
         poly(((-7, -9), (0, -6), (7, -9), (7, 8), (0, 11), (-7, 8)), '#3b2a52'),
         poly(((-5, -7), (0, -4), (0, 9), (-5, 6)), '#e6e2f2'),
         poly(((0, -4), (5, -7), (5, 6), (0, 9)), '#cfc9e6'),
         circle((0, 1), 2.4, '#c05ce0', alpha=0.9),
     ]},
]
style = {'headRadius': 11, 'torsoWidth': 7, 'frontLimbWidth': 6, 'backLimbWidth': 5.5, 'glowWidth': 2.4,
         'torsoGlowWidth': 3, 'headGlowRadius': 2.6, 'headRing': 2, 'headFill': '#2b2340',
         'eye': [2, 0, 4, 2]}

anims = {
    'idle': {'clock': 'world', 'loop': 40.0, 'keys': [
        {'at': 0, 'pose': 'stand'},
        {'at': 20, 'pose': {'from': 'stand', 'head': [2, 66], 'handB': [-19, 39]}, 'lerp': True}]},
    'walk': {'clock': 'world', 'loop': 26.0, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'kneeF': [6, 16], 'footF': [12, 1], 'kneeB': [-6, 16], 'footB': [-12, 1]}},
        {'at': 13, 'pose': {'from': 'stand', 'kneeF': [-2, 16], 'footF': [-6, 1], 'kneeB': [4, 16], 'footB': [8, 1]},
         'lerp': True}]},
    'run': {'clock': 'run', 'loop': 360.0, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'head': [4, 62], 'hip': [1, 30], 'kneeF': [16, 17], 'footF': [24, 6],
                           'kneeB': [-14, 15], 'footB': [-22, 2], 'elbowF': [16, 48], 'handF': [26, 50],
                           'elbowB': [-16, 44], 'handB': [-26, 40]}},
        {'at': 180, 'pose': {'from': 'stand', 'head': [4, 62], 'hip': [1, 30], 'kneeF': [-14, 15], 'footF': [-22, 2],
                             'kneeB': [16, 17], 'footB': [24, 6], 'elbowF': [16, 48], 'handF': [26, 50],
                             'elbowB': [-16, 44], 'handB': [-26, 40]}, 'lerp': True}]},
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
    'crouch': {'clock': 'state', 'keys': [{'at': 0, 'pose': {'from': 'sweepLow'}}]},
    'jumpsquat': {'clock': 'state', 'keys': [{'at': 0, 'pose': {'from': 'sweepLow', 'head': [2, 56]}}]},
    'shield': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'hold', 'elbowF': [14, 44], 'handF': [24, 44]}}]},
    'hitstun': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'head': [-8, 60], 'handF': [10, 34], 'handB': [-24, 30]}}]},
    'tumble': {'clock': 'spin', 'keys': [{'at': 0, 'pose': {'from': 'air', 'head': [-6, 60]}}]},
    'win': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'win'}, {'at': 20, 'pose': {'from': 'win', 'handF': [30, 72]}, 'lerp': True}]},
    'respawn': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'air'}]},
}


# ---------------------------------------------------------------- moves
def aerial(slot, name, total, landing, ac_start, ac_end, boxes, pose, attach, air_pose='air', split=None):
    """Split an aerial into start (initial autocancel), main (landing lag) and late (ending autocancel)."""
    start, late = frame(ac_start), frame(ac_end)
    parts = []
    spans = [(0, start, 'startup'), (start, late, 'main'), (late, total, 'late')]
    if split is not None:
        spans = [(0, start, 'startup'), (start, split, 'swing1'), (split, late, 'swing2'), (late, total, 'late')]
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
        ph = phase(tag, hi - lo, cut, anim=swing(pose, max(0, min(b['start'] for b in cut) if cut else 0),
                                                0, hi - lo, air=True))
        ph['transitions'] = [land(landing) if tag in ('main', 'swing1', 'swing2') else {'on': 'land', 'to': 'end'}]
        if cut:
            ph['anim'] = [{'at': 0, 'pose': air_pose}] + \
                [{'at': max(0, cut[0]['start'] - 2), 'pose': pose, 'lerp': True},
                 {'at': max(1, cut[-1].get('end', 0)), 'pose': pose},
                 {'at': min(hi - lo - 1, cut[-1].get('end', 0) + 3), 'pose': air_pose, 'lerp': True}]
        if attach:
            ph['attach'] = copy.deepcopy(attach)
        parts.append(ph)
    for i in range(len(parts) - 1):
        parts[i]['transitions'].append(wait(parts[i + 1]['name']))
    return {'name': name, 'air': 'aerial', 'phases': parts}


moves = {}

# --- neutral attack: two sword pokes, a Fire-tome burst, then the Wind-tome loop and finisher
j1 = raw('jab', 'reflet', 'Attack11')
jab1 = [b for b in root(take(j1, 2.0, 0)['source'], frame(4), frame(5))]
jab1.append(hit(take(j1, 2.0, 1)['source'], frame(4), frame(5), 58, 42, 15))
moves['jab'] = {'name': 'Bronze Jab', 'phases': [
    phase('jab1', 31, jab1, anim=swing('thrust', 4, 9, 31),
          attach=swordk([[0, 95], [2, 60], [4, 0], [30, 95]], BRONZE), transitions=[
              {'on': 'attackHeld', 'from': frame(13), 'to': 'jab2'}, wait('end')]),
    phase('jab2', 34, [], anim=swing('thrust', 4, 9, 34), attach=swordk([[0, 95], [2, 60], [4, 0], [30, 95]], BRONZE), transitions=[
        {'on': 'attackHeld', 'from': frame(9), 'needs': {'charge': 'fire', 'uses': 1}, 'to': 'jab3'},
        # No Fire tome: skip the burst and its Fire-only follow-up (the source produces no hitboxes).
        {'on': 'attackHeld', 'from': frame(9), 'to': 'jabEmpty'}, wait('end')]),
    phase('jabEmpty', 35, [], anim=swing('thrust', 4, 9, 35),
          attach=swordk([[0, 95], [2, 60], [4, 0], [30, 95]], BRONZE))]}
j2 = raw('jab2', 'reflet', 'Attack12')
jab2 = [b for b in root(take(j2, 1.5, 0)['source'], frame(5), frame(6))]
moves['jab']['phases'][1]['hitboxes'] = jab2
j3 = raw('jab3', 'reflet', 'Attack13')
jab3 = root(take(j3, 5.0)['source'], frame(6), frame(7))
moves['jab']['phases'].append(phase('jab3', 35, jab3, cost={'charge': 'fire', 'uses': 1},
                                   anim=swing('cast', frame(6), frame(7), 35), attach=stowed(),
                                   events=[{'at': frame(6), 'sfx': 'slash'}],
                                   transitions=[
                                       {'on': 'attackHeld', 'from': frame(9), 'needs': {'charge': 'wind', 'uses': 1},
                                        'to': 'rapidStart'}, wait('end')]))
rr = raw('rapid', 'reflet', 'Attack100Sub')
rapid = [hit(rr[0]['source'], 0, 0, 60, 42, 40, rehit=2)]
rapid[0].pop('end')  # active for the whole loop: the source hits every other frame while attack is held
moves['jab']['phases'] += [
    phase('rapidStart', 4, [], cost={'charge': 'wind', 'uses': 1}, anim=[{'at': 0, 'pose': 'cast'}],
          attach=stowed(), transitions=[wait('rapid')]),
    # One Wind use per 60-tick loop (the source drains about one per second); the loop exits when it empties.
    phase('rapid', 60, rapid, cost={'charge': 'wind', 'uses': 1}, anim=[{'at': 0, 'pose': 'cast'}],
          attach=stowed(), events=[{'at': 0, 'sfx': 'swing'}],
          transitions=[{'on': 'release', 'to': 'rapidEnd'},
                       {'on': 'timeout', 'needs': {'charge': 'wind', 'uses': 1}, 'to': 'rapid'},
                       wait('rapidEnd')]),
]
re_ = raw('rapidEnd', 'reflet', 'Attack100End')
rapid_end = [hit(re_[0]['source'], frame(5), frame(6), 45, 65, 25),
             hit(re_[1]['source'], frame(5), frame(6), 85, 65, 25)]
moves['jab']['phases'].append(phase('rapidEnd', 47, rapid_end, anim=swing('castHigh', frame(5), frame(6), 47),
                                   attach=swordk([[0, 95], [4, 30], [6, 120], [40, 95]], BRONZE), events=[{'at': frame(5), 'sfx': 'smash'}]))

# --- tilts
ft = raw('ftilt', 'reflet', 'AttackS3')
moves['ftilt'] = {'name': 'Cinder Slash', 'phases': [phase(
    'main', 32, root(take(ft, 7.5)['source'], frame(9), frame(10)),
    anim=swing('lunge', 8, 12, 32), attach=swordk([[0, 150], [6, 60], [9, 8], [32, 95]], BRONZE),
    events=[{'at': frame(9), 'sfx': 'slash'}])]}
ut = raw('utilt', 'reflet', 'AttackHi3')
utilt = [swept(take(ut, 6.0, 0)['source'], [(frame(6), 40, 58), (frame(9), 44, 82), (frame(12), 30, 104),
                                            (frame(14), 12, 112)], 18),
         swept(take(ut, 6.0, 1)['source'], [(frame(6), 30, 52), (frame(9), 36, 74), (frame(12), 26, 94),
                                            (frame(14), 10, 100)], 18),
         hit(take(ut, 6.0, 2)['source'], frame(6), frame(14), 24, 56, 15)]
moves['utilt'] = {'name': 'Rising Arc', 'phases': [phase(
    'main', 32, utilt, anim=swing('sweepUp', 6, 16, 32),
    attach=swordk([[0, 150], [3, -20], [6, 30], [9, 80], [14, 120], [32, 95]], BRONZE), events=[{'at': frame(6), 'sfx': 'slash'}])]}
dt = raw('dtilt', 'reflet', 'AttackLw3')
moves['dtilt'] = {'name': 'Low Sweep', 'phases': [phase(
    'main', 21, root(take(dt, 6.0)['source'], frame(7), frame(8)),
    anim=swing('sweepLow', 5, 12, 21), attach=swordk([[0, 150], [4, 300], [8, 330], [21, 455]], BRONZE),
    events=[{'at': frame(7), 'sfx': 'slash'}])]}
da = raw('dashatk', 'reflet', 'AttackDash')
moves['dashatk'] = {'name': 'Rush Cut', 'phases': [phase(
    'main', 41, [hit(take(da, 10.0)['source'], frame(8), frame(10), 76, 46, 20),
                        hit(take(da, 6.0)['source'], frame(11), frame(18), 66, 40, 18)],
    slide=frame(20), anim=swing('lunge', 7, 20, 41),
    attach={'sword': {'angle': [[0, 130], [frame(8), 10], [frame(18), 20], [frame(30), 95], [frame(41), 95]],
                      'scale': [[0, BRONZE]]}},
    events=[{'at': frame(8), 'sfx': 'slash'}, {'at': 0, 'setVel': {'x': 4.2}}])]}

# --- smashes: Levin when the sword has a use left, otherwise the Bronze fallback
def smash(slot, name, total, charge_frame, levin, bronze, pose, angle_keys, anim=None):
    def attach(scale):
        a = copy.deepcopy(angle_keys)
        a['sword']['scale'] = [[0, scale]]
        return a
    anim = anim or swing(pose, frame(charge_frame) + 2, frame(total) - 8, frame(total))
    return {'name': name, 'variants': [{'needs': {'charge': 'levin', 'uses': 1}, 'phases': [phase(
        'levin', total, levin, cost={'charge': 'levin', 'uses': 1}, charge={'frame': frame(charge_frame)},
        anim=copy.deepcopy(anim), attach=attach(1.0),
        events=[{'at': frame(charge_frame), 'sfx': 'smash'}])]}],
        'phases': [phase('bronze', total, bronze, charge={'frame': frame(charge_frame)},
                         anim=copy.deepcopy(anim), attach=attach(BRONZE),
                         events=[{'at': frame(charge_frame), 'sfx': 'smash'}])]}


fsmash_anim = [
    {'at': 0, 'pose': 'stand'},
    {'at': 2, 'pose': 'windup', 'lerp': True},
    {'at': 12, 'pose': 'windup'},
    {'at': 15, 'pose': 'lunge', 'lerp': True},
    {'at': 30, 'pose': 'lunge'},
    {'at': 48, 'pose': 'stand', 'lerp': True},
]
usmash_anim = [
    {'at': 0, 'pose': 'stand'},
    {'at': 3, 'pose': 'sweepLow', 'lerp': True},
    {'at': 8, 'pose': 'sweepLow'},
    {'at': 12, 'pose': 'overhead', 'lerp': True},
    {'at': 28, 'pose': 'overhead'},
    {'at': 46, 'pose': 'stand', 'lerp': True},
]
dsmash_anim = [
    {'at': 0, 'pose': 'stand'},
    {'at': 4, 'pose': 'windup', 'lerp': True},
    {'at': 12, 'pose': 'sweepLow', 'lerp': True},
    {'at': 32, 'pose': 'sweepLow'},
    {'at': 48, 'pose': 'stand', 'lerp': True},
]

fs = raw('fsmash', 'reflet', 'AttackS4')
levin_fsmash = root(take(fs, 16.0, tag='levin')['source'], frame(16), frame(18)) + \
    [hit(take(fs, 10.0)['source'], frame(19), frame(27), 96, 50, 10)]
bronze_fsmash = root(take(fs, 9.6)['source'], frame(16), frame(18))
moves['fsmash'] = smash('fsmash', 'Levin Smash', 57, 6, levin_fsmash, bronze_fsmash, 'lunge',
                        {'sword': {'angle': [[0, 150], [frame(6), 190], [frame(15), 60], [frame(16), 10], [frame(27), -10],
                                             [frame(57), 95]]}}, anim=fsmash_anim)
us = raw('usmash', 'reflet', 'AttackHi4')
levin_usmash = [swept(take(us, 15.0, tag='levin')['source'], [(frame(12), 30, 96), (frame(13), 26, 108),
                                                             (frame(16), 16, 118)], 24),
                swept(take(us, 10.0, 0)['source'], [(frame(12), 44, 52), (frame(13), 40, 66), (frame(16), 30, 78)],
                      18),
                hit(take(us, 10.0, 1)['source'], frame(17), frame(32), 20, 84, 13)]
bronze_usmash = [swept(take(us, 9.0)['source'], [(frame(12), 30, 92), (frame(13), 26, 104), (frame(16), 16, 114)],
                       21)]
moves['usmash'] = smash('usmash', 'Levin Rising', 52, 5, levin_usmash, bronze_usmash, 'overhead',
                        {'sword': {'angle': [[0, 170], [frame(5), 175], [frame(11), 120], [frame(13), 80], [frame(16), 30],
                                             [frame(52), 95]]}}, anim=usmash_anim)
ds = raw('dsmash', 'reflet', 'AttackLw4')
levin_dsmash = [swept(take(ds, 15.0, 0)['source'], [(frame(16), 26, 62), (frame(17), 46, 40)], 23),
                swept(take(ds, 15.0, 1)['source'], [(frame(16), 34, 64), (frame(17), 56, 42)], 23),
                swept(take(ds, 12.0, 0)['source'], [(frame(19), 52, 18), (frame(22), 62, 18), (frame(28), 66, 18)],
                      23),
                swept(take(ds, 12.0, 1)['source'], [(frame(19), -12, 18), (frame(22), -22, 18), (frame(28), -26, 18)],
                      23),
                swept(take(ds, 10.0, 0)['source'], [(frame(23), 80, 18), (frame(28), 90, 18)], 23),
                swept(take(ds, 10.0, 1)['source'], [(frame(23), -40, 18), (frame(28), -50, 18)], 23)]
bronze_dsmash = [swept(take(ds, 8.0, 0)['source'], [(frame(16), 24, 60), (frame(17), 42, 38)], 15),
                 swept(take(ds, 8.0, 1)['source'], [(frame(16), 32, 62), (frame(17), 52, 40)], 15)]
moves['dsmash'] = smash('dsmash', 'Levin Sweep', 56, 3, levin_dsmash, bronze_dsmash, 'sweepLow',
                        {'sword': {'angle': [[0, 150], [frame(3), 190], [frame(15), 320], [frame(19), 320], [frame(22), 200],
                                             [frame(28), 210], [frame(56), 95]]}}, anim=dsmash_anim)

# --- aerials: Levin variants when the sword has uses, Bronze otherwise
an = raw('nair', 'reflet', 'AttackAirN')
# Swing 1 covers the space in front of Robin; the second swing (and the late Levin hit) sweeps behind, which is
# what the pinned UFD frames show (the blade crosses the body). Both swings are GA-masked in the script.
levin_nair = [hit(take(an, 11.5, 0, tag='levin')['source'], frame(7), frame(10), 44, 36, 27),
              hit(take(an, 11.5, 3)['source'], frame(20), frame(23), -40, 38, 27),
              hit(take_bone(an, 'armr', 11.5, 'levin')['source'], frame(7), frame(10), 24, 32, 20),
              hit(take(an, 5.0)['source'], frame(24), frame(28), -40, 52, 16)]
bronze_nair = [hit(take(an, 6.9, 0)['source'], frame(7), frame(10), 42, 28, 16),
               hit(take(an, 6.9, 3)['source'], frame(20), frame(23), -38, 26, 16),
               hit(take_bone(an, 'armr', 6.9)['source'], frame(7), frame(10), 22, 28, 14)]
moves['nair'] = {'name': 'Levin Sweep', 'air': 'aerial', 'variants': [
    {'needs': {'charge': 'levin', 'uses': 1}, 'phases': aerial(
        'nair', 'Levin Sweep', frame(48), 11, 4, 40, levin_nair, 'tuck',
        tilt_attach(swordk([[0, 300], [5, 340], [8, 380], [16, 480], [20, 560], [24, 575], [48, 455]], 1.0), 40),
        split=frame(20)).get('phases')}],
    'phases': aerial('nair', 'Levin Sweep', frame(48), 11, 4, 32, bronze_nair, 'tuck',
                     tilt_attach(swordk([[0, 300], [5, 340], [8, 380], [16, 480], [20, 560], [24, 575], [48, 455]], BRONZE), 40),
                     split=frame(20)).get('phases')}
for i, ph in enumerate(moves['nair']['variants'][0]['phases']):
    ph['cost'] = {'charge': 'levin', 'uses': 1} if i == 0 else ph.get('cost')
af = raw('fair', 'reflet', 'AttackAirF')
levin_fair = [hit(take(af, 12.5, 0, tag='levin')['source'], frame(12), frame(15), 54, 36, 26),
              hit(take_bone(af, 'armr', 12.5, 'levin')['source'], frame(12), frame(15), 26, 36, 20),
              hit(take(af, 5.0)['source'], frame(16), frame(19), 66, 54, 16)]
bronze_fair = [hit(take(af, 7.5, 0)['source'], frame(12), frame(15), 48, 38, 15),
               hit(take_bone(af, 'armr', 7.5)['source'], frame(12), frame(15), 24, 36, 14)]
moves['fair'] = {'name': 'Levin Cut', 'air': 'aerial', 'variants': [
    {'needs': {'charge': 'levin', 'uses': 1}, 'phases': aerial(
        'fair', 'Levin Cut', frame(34), 11, 4, 28, levin_fair, 'dive',
        tilt_attach(swordk([[0, 270], [8, 120], [12, 30], [34, 95]], 1.0), 50)).get('phases')}],
    'phases': aerial('fair', 'Levin Cut', frame(34), 11, 4, 27, bronze_fair, 'dive',
                     tilt_attach(swordk([[0, 270], [8, 120], [12, 30], [34, 95]], BRONZE), 50)).get('phases')}
for i, ph in enumerate(moves['fair']['variants'][0]['phases']):
    ph['cost'] = {'charge': 'levin', 'uses': 1} if i == 0 else ph.get('cost')
ab = raw('bair', 'reflet', 'AttackAirB')
levin_bair = [hit(take(ab, 15.0, 0, tag='levin')['source'], frame(9), frame(10), -56, 38, 26),
              hit(take_bone(ab, 'armr', 15.0, 'levin')['source'], frame(9), frame(10), -26, 36, 20),
              hit(take(ab, 5.0)['source'], frame(11), frame(16), -64, 32, 16)]
bronze_bair = [hit(take(ab, 9.0, 0)['source'], frame(9), frame(10), -48, 38, 15),
               hit(take_bone(ab, 'armr', 9.0)['source'], frame(9), frame(10), -24, 36, 14)]
moves['bair'] = {'name': 'Reverse Levin', 'air': 'aerial', 'variants': [
    {'needs': {'charge': 'levin', 'uses': 1}, 'phases': aerial(
        'bair', 'Reverse Levin', frame(40), 11, 1, 32, levin_bair, 'guard',
        tilt_attach(swordk([[0, 95], [6, 60], [9, 200], [40, 95]], 1.0), 15)).get('phases')}],
    'phases': aerial('bair', 'Reverse Levin', frame(40), 11, 1, 32, bronze_bair, 'guard',
                     tilt_attach(swordk([[0, 95], [6, 60], [9, 200], [40, 95]], BRONZE), 15)).get('phases')}
for i, ph in enumerate(moves['bair']['variants'][0]['phases']):
    ph['cost'] = {'charge': 'levin', 'uses': 1} if i == 0 else ph.get('cost')
au = raw('uair', 'reflet', 'AttackAirHi')
levin_uair = [hit(take(au, 13.0, 0, tag='levin')['source'], frame(10), frame(13), 15, 102, 25),
              hit(take_bone(au, 'armr', 13.0, 'levin')['source'], frame(10), frame(13), 12, 66, 20),
              hit(take(au, 5.0)['source'], frame(14), frame(23), 8, 112, 15)]
bronze_uair = [hit(take(au, 7.8, 0)['source'], frame(10), frame(13), 13, 96, 18),
               hit(take_bone(au, 'armr', 7.8)['source'], frame(10), frame(13), 11, 62, 15)]
moves['uair'] = {'name': 'Levin Arc', 'air': 'aerial', 'variants': [
    {'needs': {'charge': 'levin', 'uses': 1}, 'phases': aerial(
        'uair', 'Levin Arc', frame(46), 11, 5, 28, levin_uair, 'arch',
        tilt_attach(swordk([[0, 150], [6, 60], [10, 105], [46, 95]], 1.0), 20)).get('phases')}],
    'phases': aerial('uair', 'Levin Arc', frame(46), 11, 5, 27, bronze_uair, 'arch',
                     tilt_attach(swordk([[0, 150], [6, 60], [10, 105], [46, 95]], BRONZE), 20)).get('phases')}
for i, ph in enumerate(moves['uair']['variants'][0]['phases']):
    ph['cost'] = {'charge': 'levin', 'uses': 1} if i == 0 else ph.get('cost')
ad = raw('dair', 'reflet', 'AttackAirLw')
# The UFD frames show a mostly vertical box under Robin's feet for the whole descent.
levin_dair = [
    # Early hit: the source ships the same 11.0 box twice, once for airborne victims (270 -> spike) and once
    # for grounded ones (70 degree launch). Both are kept.
    swept(take(ad, 11.0, 0, tag='levin')['source'], [(frame(13), 16, 34), (frame(15), 10, -6)], 26),
    swept(take(ad, 11.0, 1, tag='levin')['source'], [(frame(13), 16, 34), (frame(15), 10, -6)], 26),
    swept(take(ad, 12.0)['source'], [(frame(16), 10, -16), (frame(17), 10, -24)], 27),
    hit(take(ad, 8.0)['source'], frame(18), frame(24), 8, -38, 22),
]
bronze_dair = [swept(take(ad, 7.2)['source'], [(frame(13), 14, 30), (frame(15), 10, -10)], 18),
               hit(take(ad, 6.0)['source'], frame(16), frame(24), 8, -30, 17)]
moves['dair'] = {'name': 'Levin Drop', 'air': 'aerial', 'variants': [
    {'needs': {'charge': 'levin', 'uses': 1}, 'phases': aerial(
        'dair', 'Levin Drop', frame(60), 16, 2, 48, levin_dair, 'airLow',
        swordk([[0, 60], [8, -110], [13, -60], [24, -40], [60, 95]], 1.0)).get('phases')}],
    'phases': aerial('dair', 'Levin Drop', frame(60), 16, 2, 48, bronze_dair, 'airLow',
                     swordk([[0, 60], [8, -110], [13, -60], [24, -40], [60, 95]], BRONZE)).get('phases')}
for i, ph in enumerate(moves['dair']['variants'][0]['phases']):
    ph['cost'] = {'charge': 'levin', 'uses': 1} if i == 0 else ph.get('cost')

# --- grabs
cg = raw('grab', 'reflet', 'Catch')
grab_box = root({'Damage': '0', 'Angle': '0', 'KBG': '0', 'BKB': '0', 'Size': '3.8', 'Z': '4.0', 'Y': '8.0',
                 'Z2': '9.2', 'Y2': '8.0', 'FacingRestrict': 'F', 'Ground_or_Air': 'GA', 'Bone': 'top'},
                frame(7), frame(8), type='grab')
moves['grab'] = {'name': 'Grab', 'phases': [phase(
    'main', 39, grab_box, anim=swing('thrust', 7, 12, 39), attach=stowed())]}
dg = raw('dashgrab', 'reflet', 'CatchDash')
dash_grab_box = root({'Damage': '0', 'Angle': '0', 'KBG': '0', 'BKB': '0', 'Size': '3.0', 'Z': '4.0', 'Y': '7.0',
                      'Z2': '11.0', 'Y2': '7.0', 'FacingRestrict': 'F', 'Ground_or_Air': 'GA', 'Bone': 'top'},
                     frame(10), frame(11), type='grab')
moves['dashgrab'] = {'name': 'Dash Grab', 'phases': [phase(
    'main', 47, dash_grab_box, slide=frame(18), anim=swing('thrust', 10, 16, 47),
    attach=stowed())]}

# --- specials
moves['nspec'] = {
    'name': 'Thunder', 'bReverse': True, 'variants': [
        {'needs': {'charge': 'thunder', 'uses': 1}, 'phases': [
            phase('charge', None, anim=[{'at': 0, 'pose': 'cast'}], attach=stowed(),
                  events=[{'at': 0, 'sfx': 'charge'},
                          {'from': 4, 'every': 5,
                           'fx': [{'type': 'part', 'x': 46, 'y': 56, 'vx': 0, 'vy': 1.6, 'life': 10,
                                   'size': 2.4, 'color': '#ffe066'},
                                  {'type': 'ring', 'x': 46, 'y': 56, 'r': 8, 'grow': 3, 'life': 6,
                                   'color': '#ffe066'}]}]),
            phase('thunder', 32, anim=[{'at': 0, 'pose': 'castForward'}], attach=stowed(),
                  cost={'charge': 'thunder', 'uses': 1},
                  events=[{'at': frame(8), 'spawn': 'thunder', 'sfx': 'shoot'}]),
            phase('elthunder', 38, anim=[{'at': 0, 'pose': 'castForward'}], attach=stowed(),
                  cost={'charge': 'thunder', 'uses': 3},
                  events=[{'at': frame(8), 'spawn': 'elthunder', 'sfx': 'shoot'}]),
            phase('arcthunder', 38, anim=[{'at': 0, 'pose': 'castForward'}], attach=stowed(),
                  cost={'charge': 'thunder', 'uses': 5},
                  events=[{'at': frame(8), 'spawn': 'arcthunder', 'sfx': 'shoot'}]),
            phase('thoron', 74, anim=[{'at': 0, 'pose': 'castForward'}], attach=stowed(),
                  cost={'charge': 'thunder', 'uses': 8},
                  events=[{'at': frame(23), 'spawn': 'thoron', 'sfx': 'shoot',
                           'fx': [{'type': 'shake', 'k': 5},
                                  {'type': 'burst', 'x': 46, 'y': 58, 'color': '#ffe066', 'n': 12,
                                   'speed': 4.5, 'life': 14, 'size': 3.5}]}]),
        ]}],
    'phases': [phase('empty', 39, anim=[{'at': 0, 'pose': 'castForward'}], attach=stowed(),
                     events=[{'at': 0, 'sfx': 'charge'}])]}
# The tier a charge release picks: charge frame 8, Elthunder 33, Arcthunder 73, Thoron 133 source frames.
charge = moves['nspec']['variants'][0]['phases'][0]
charge['charge'] = {'frame': frame(8), 'max': 126, 'holdAfterMax': 600, 'button': 'special'}
charge['transitions'] = [
    {'on': 'release', 'minCharge': 126, 'to': 'thoron'},
    {'on': 'release', 'minCharge': 66, 'to': 'arcthunder'},
    {'on': 'release', 'minCharge': 26, 'to': 'elthunder'},
    {'on': 'release', 'to': 'thunder'},
    {'on': 'shieldPress', 'to': 'end'},
]

moves['sspec'] = {'name': 'Arcfire', 'bReverse': True, 'variants': [
    {'needs': {'charge': 'fire', 'uses': 1}, 'phases': [phase(
        'main', 63, cost={'charge': 'fire', 'uses': 1}, anim=[{'at': 0, 'pose': 'cast'},
        {'at': 12, 'pose': 'castHigh', 'lerp': True}, {'at': 22, 'pose': 'castHigh'},
        {'at': 30, 'pose': 'cast', 'lerp': True}], attach=stowed(),
        events=[{'at': frame(17), 'spawn': 'arcfire', 'sfx': 'shoot',
                 'fx': [{'type': 'burst', 'x': 58, 'y': 50, 'color': '#ff8a2a', 'n': 8, 'speed': 3.0,
                         'life': 12, 'size': 3.0}]},
                {'at': frame(23), 'spawn': 'arcfireFlame',
                 'fx': [{'type': 'ring', 'x': 96, 'y': 22, 'r': 12, 'grow': 6, 'life': 10, 'color': '#ffb04a'}]},
                {'at': frame(57), 'spawn': 'arcfireBurst',
                 'fx': [{'type': 'shake', 'k': 4},
                        {'type': 'burst', 'x': 96, 'y': 22, 'color': '#ff8a2a', 'n': 14, 'speed': 4.0,
                         'life': 16, 'size': 4.0}]}])]}],
    'phases': [phase('empty', 50, anim=[{'at': 0, 'pose': 'cast'}], attach=stowed(),
                     events=[{'at': frame(17), 'sfx': 'swing'}])]}

def elwind(first_blade: bool, second_blade: bool):
    """Source timing: blade 1 on frame 8, blade 2 on frame 28, 85 frames total. Each blade costs one Wind use,
    so one remaining use still gives the first (single-blade) recovery."""
    startup = [phase('startup', frame(8), airAccelMult=drift(0.2, 'upSpecial'), anim=[{'at': 0, 'pose': 'cast'}], attach=stowed(),
                     events=[{'at': 0, 'setVel': {'x': 0, 'y': 0}, 'sfx': 'charge'}],
                     transitions=[wait('rise1')])]
    # Source: the ledge can be caught from frame 14 on, which is inside the first rise phase.
    rise1 = phase('rise1', frame(28) - frame(8), airAccelMult=drift(0.4, 'upSpecial'), anim=[{'at': 0, 'pose': 'castHigh'}],
                  attach=stowed(), ledgeGrab=frame(14) - frame(8), events=[{'at': 0, 'leaveGround': True}],
                  transitions=[wait('rise2')])
    if first_blade:
        rise1['cost'] = {'charge': 'wind', 'uses': 1}
        rise1['events'] = [{'at': 0, 'spawn': 'elwind1', 'sfx': 'slash', 'leaveGround': True,
                            'fx': [{'type': 'ring', 'x': 26, 'y': 24, 'r': 10, 'grow': 6, 'life': 10,
                                    'color': '#8af0bf'}]},
                           {'at': 0, 'setVel': {'y': 12.5}}]
    rise2 = phase('rise2', frame(85) - frame(28) + 1, airAccelMult=drift(0.4, 'upSpecial'), anim=[{'at': 0, 'pose': 'castHigh'}],
                  attach=stowed(), ledgeGrab=0,
                  transitions=[{'on': 'land', 'to': 'landing', 'lag': 25}, wait('helpless')])
    if second_blade:
        rise2['cost'] = {'charge': 'wind', 'uses': 1}
        rise2['events'] = [{'at': 0, 'spawn': 'elwind2', 'sfx': 'slash',
                            'fx': [{'type': 'ring', 'x': 26, 'y': 24, 'r': 10, 'grow': 6, 'life': 10,
                                    'color': '#8af0bf'}]},
                           {'at': 0, 'setVel': {'y': 10.5}}]
    return startup + [rise1, rise2]


moves['uspec'] = {'name': 'Elwind', 'air': 'upSpecial', 'variants': [
    {'needs': {'charge': 'wind', 'uses': 2}, 'phases': elwind(True, True)},
    {'needs': {'charge': 'wind', 'uses': 1}, 'phases': elwind(True, False)}],
    'phases': [phase('empty', 51, airAccelMult=drift(0.2, 'upSpecial'), anim=[{'at': 0, 'pose': 'cast'}], attach=stowed(),
                     events=[{'at': 0, 'sfx': 'swing'}],
                     transitions=[{'on': 'land', 'to': 'landing', 'lag': 25}, wait('helpless')])]}

moves['dspec'] = {'name': 'Nosferatu', 'variants': [
    {'needs': {'charge': 'dark', 'uses': 1}, 'phases': [
        # Source: intangibility and grabboxes 15-18 (engine 14-17), interruptible 50 (engine 49).
        phase('startup', frame(15), cost={'charge': 'dark', 'uses': 1}, anim=[{'at': 0, 'pose': 'drain'}],
              attach=stowed(), events=[{'at': 0, 'sfx': 'charge'}],
              transitions=[wait('capture')]),
        phase('capture', 4, airAccelMult=drift(0.2, 'attack'), anim=[{'at': 0, 'pose': 'drain'}], attach=stowed(),
              hitboxes=[{'start': 0, 'end': 3, 'x': 58, 'y': 46, 'r': 34, 'damage': 1.0, 'bkb': 30.0, 'kbg': 1.0,
                         'angle': 60, 'type': 'paralyze', 'setKb': True, 'bkb': 22.0, 'unblockable': True,
                         'hitlagMult': 0.2}],
              events=[{'at': 0, 'sfx': 'grab', 'invuln': 4,
                       'fx': [{'type': 'ring', 'x': 58, 'y': 46, 'r': 16, 'grow': 5, 'life': 8,
                               'color': '#b05ce0'},
                              {'type': 'burst', 'x': 58, 'y': 46, 'color': '#c05ce0', 'n': 8, 'speed': 2.5,
                               'life': 10, 'size': 3.0}]}],
              transitions=[{'on': 'hit', 'to': 'drain'}, wait('whiff')]),
        phase('drain', 35, airAccelMult=drift(0.1, 'attack'), anim=[{'at': 0, 'pose': 'drain'},
                                                   {'at': 20, 'pose': {'from': 'drain', 'handF': [52, 56]}}],
              attach=stowed(),
              hitboxes=[{'start': 0, 'end': 34, 'x': 56, 'y': 44, 'r': 30, 'damage': 2.0, 'bkb': 0.0, 'kbg': 1.0,
                         'angle': 60, 'type': 'electric', 'setKb': True, 'bkb': 18.0, 'rehit': 8, 'heal': 1.5,
                         'unblockable': True, 'hitlagMult': 0.0}],
              events=[{'at': 0, 'sfx': 'slash'},
                      {'from': 2, 'every': 6,
                       'fx': [{'type': 'part', 'x': 56, 'y': 44, 'vx': 0, 'vy': 2.0, 'life': 12,
                               'size': 2.6, 'color': '#c05ce0'}]}],
              transitions=[wait('end')]),
        # Whiffed capture: the source spends the animation with no further hitboxes.
        phase('whiff', 31, airAccelMult=drift(0.2, 'attack'), anim=[{'at': 0, 'pose': 'drain'}], attach=stowed(),
              transitions=[wait('end')]),
    ]}],
    'phases': [phase('empty', 49, anim=[{'at': 0, 'pose': 'drain'}], attach=stowed(),
                     events=[{'at': 0, 'sfx': 'swing'}])]}

# ---------------------------------------------------------------- throws
def throw(slot, script, total, release, name):
    s = scripts['reflet'][script]
    line = next(x for x in s.splitlines() if 'ATTACK_ABS(Kind=FIGHTER_ATTACK_ABSOLUTE_KIND_THROW' in x)
    a = args(line.split('THROW,', 1)[1].rstrip(')'))
    box = hit(a, 0, 0, 0, 0, 60)
    ledger['rawAttacks'][slot] = {'article': 'reflet', 'script': script, 'attacks': [a]}
    return {'release': frame(release), 'total': total, 'hit': box, 'ticks': []}


throws = {
    'f': throw('fthrow', 'ThrowF', 31, 16, 'Forward'),
    'b': throw('bthrow', 'ThrowB', 44, 25, 'Back'),
    'u': throw('uthrow', 'ThrowHi', 47, 15, 'Up'),
    'd': throw('dthrow', 'ThrowLw', 37, 16, 'Down'),
}

# ---------------------------------------------------------------- projectiles
def projectile(name, spawn, vel, r, life, hitb, **kw):
    p = {'name': name, 'spawn': list(spawn), 'vel': list(vel), 'r': r, 'life': life, 'max': 6, 'hit': hitb}
    p.update(kw)
    return p


def fire_hit(a, damage=None, **kw):
    b = hit(a, 0, 0, 0, 0, 0, **kw)
    if damage is not None:
        b['damage'] = damage
    return b


tshoot = {s['Name']: args(re.search(r'ATTACK\((.*)\)', s['Data'])[1]) for s in source['scripts']['reflet_thunder']}
p_thunder = projectile('Thunder', (46, 56), (13, 0), 7.5, 16, fire_hit(tshoot['Shoot0']))
p_elthunder = projectile('Elthunder', (46, 58), (16, 0), 17.5, 20, fire_hit(tshoot['Shoot1']))
arcthunder_final = {'Damage': '8.0', 'Angle': '45', 'BKB': '85', 'KBG': '85', 'Size': '3.0', 'FKB': '0',
                     'FacingRestrict': 'F', 'Ground_or_Air': 'GA', 'Effect': 'elec', 'Hitlag': '0.6'}
p_arcthunder = projectile('Arcthunder', (46, 58), (9, 0), 16, 26, fire_hit(arcthunder_final),
                          explosion={'armTicks': 8, 'proximity': 46, 'radius': 54, 'sfx': 'quake',
                                     'fx': [{'type': 'burst', 'color': '#7fd8ff', 'n': 14, 'speed': 4.5,
                                             'life': 18, 'size': 4.5}]})
tron = {'name': 'Thoron', 'Damage': '2.6', 'Angle': '45', 'BKB': '75', 'KBG': '191', 'Size': '3.0', 'FKB': '0',
        'FacingRestrict': 'F', 'Ground_or_Air': 'GA', 'Effect': 'elec', 'Hitlag': '1.0'}
p_thoron = projectile('Thoron', (46, 58), (22, 0), 15, 46, fire_hit(tron, rehit=4))
gf = {s['Name']: args(re.search(r'ATTACK\((.*)\)', s['Data'])[1]) for s in source['scripts']['reflet_gigafire']}
arcfire_bolt = projectile('Arcfire', (44, 68), (9.5, 2.2), 9, 30, fire_hit(gf['Shoot0']), gravity=0.16,
                          spawnSfx='shoot')
flame = fire_hit(gf['Burn'])
flame['rehit'] = 8
arcfire_flame = projectile('Arcfire Flame', (96, 22), (2.2, 0), 19, 42, flame)
ard = args(re.search(r'ATTACK\(ID=1,(.*)\)', source['scripts']['reflet_gigafire'][3]['Data'])[1])
arcfire_burst = projectile('Arcfire Burst', (96, 22), (0.6, 0), 26, 12, fire_hit(ard))
arcfire_burst['spawnSfx'] = 'quake'
ew = {s['Name']: s['Data'] for s in source['scripts']['reflet_elwind']}
e1 = args(re.search(r'ATTACK\(ID=1,(.*)\)', ew['Shoot0'])[1])
e2 = args(re.search(r'ATTACK\(ID=0,(.*)\)', ew['Shoot1'])[1])
# Elwind 1's 7.0 hit is the point-blank spike (255 degrees); its paired 5.0/80 degree body box cannot coexist on
# one projectile, so the spike is imported and the body box is dropped (recorded as an adaptation).
elwind_early = {'Damage': '7.0', 'Angle': '255', 'BKB': '10', 'KBG': '100', 'Size': '4.0', 'FKB': '0',
                'FacingRestrict': 'F', 'Ground_or_Air': 'GA', 'Effect': 'cutup', 'Hitlag': '1.0'}
p_elwind1 = projectile('Elwind 1', (26, 30), (2.0, 15.0), 20, 24, fire_hit(elwind_early), spawnSfx='slash')
p_elwind2 = projectile('Elwind 2', (26, 30), (1.4, 14.0), 20, 24, fire_hit(e2, damage=None), spawnSfx='slash')

# ---------------------------------------------------------------- spell visuals
# The imported hit data is invisible on its own: give each article the shape the source draws. Colors and
# proportions follow the spell (yellow Thunder tiers, purple Arcthunder, orange Arcfire, green Elwind).
def glow(c, r, core, alpha=0.35):
    return [circle((0, 0), r, c, alpha=alpha), circle((0, 0), r * 0.66, c, alpha=min(1., alpha + 0.35)),
            circle((0, 0), r * 0.3, core)]


def spark_trail(color, size=2.2, life=9, vx=-2.5, vy=1.2, every=2):
    return [{'every': every, 'fx': [{'type': 'part', 'x': -5, 'y': 0, 'vx': vx, 'vy': vy,
                                     'life': life, 'size': size, 'color': color}]}]


def flame_poly(points, fill, alpha):
    return poly(points, fill, alpha=alpha)


p_thunder['shapes'] = glow('#ffe066', 8, '#fffbe0', 0.3) + \
    [line((-10, 0), (10, 0), 1.4, '#fff7cf', alpha=0.9)]
p_thunder['trail'] = spark_trail('#ffe066')

p_elthunder['shapes'] = glow('#ffd84a', 16, '#fffbe0', 0.32) + \
    [line((-16, 0), (16, 0), 2.0, '#fff7cf', alpha=0.85),
     line((-6, -10), (2, 6), 1.6, '#fffbe0', alpha=0.8),
     line((6, 10), (12, -4), 1.6, '#fffbe0', alpha=0.8)]
p_elthunder['trail'] = spark_trail('#ffd84a', 2.8, 10, -3.0, 2.0)

p_arcthunder['shapes'] = glow('#7b3fd6', 15, '#efe0ff', 0.4) + \
    [{'shape': 'circle', 'at': [0, 0], 'r': 12, 'stroke': '#d9b6ff', 'width': 1.6},
     line((-13, -7), (-3, 3), 1.6, '#e6c9ff', alpha=0.8),
     line((5, -9), (10, 2), 1.6, '#e6c9ff', alpha=0.8)]
p_arcthunder['trail'] = spark_trail('#b07cf0', 3.0, 12, -2.0, 1.5)

p_thoron['shapes'] = [
    poly(((-46, -10), (36, -12), (48, 0), (36, 12), (-46, 10)), '#ffe066', alpha=0.35),
    poly(((-42, -5), (34, -6), (44, 0), (34, 6), (-42, 5)), '#fff3b0', alpha=0.9),
    poly(((-34, -2), (34, -2), (40, 0), (34, 2), (-34, 2)), '#ffffff'),
    circle((42, 0), 11, '#fffbe0', alpha=0.85),
]
p_thoron['trail'] = spark_trail('#ffe066', 2.6, 8, -6.0, 0.0, 1)

p_arcfire = arcfire_bolt
p_arcfire['shapes'] = [
    circle((0, 0), 9, '#d63b1a', alpha=0.85),
    circle((0, 0), 6, '#ff7a2a', alpha=0.9),
    circle((0, 0), 3, '#ffe08a'),
    flame_poly(((-7, 2), (-11, 8), (-4, 6), (-2, 10), (2, 4)), '#ff9a3a', 0.8),
]
p_arcfire['trail'] = [{'every': 1, 'fx': [{'type': 'part', 'x': -4, 'y': 2, 'vx': -2.0, 'vy': 1.6,
                                           'life': 12, 'size': 3.2, 'color': '#ff8a2a', 'gravity': 0.05}]}]

arcfire_flame['shapes'] = [
    flame_poly(((-14, -8), (-6, -42), (4, -26), (10, -36), (14, -6), (18, 12), (8, 20), (-8, 20), (-17, 4)),
               '#d63b1a', 0.6),
    flame_poly(((-8, -2), (-2, -30), (4, -14), (8, -22), (9, 2), (10, 12), (4, 14), (-4, 14), (-9, 4)),
               '#ff8a2a', 0.85),
    flame_poly(((0, 2), (0, -18), (4, -6), (5, 4), (2, 8), (-2, 6)), '#ffe08a', 0.95),
]
arcfire_flame['trail'] = [{'every': 2, 'fx': [{'type': 'part', 'x': 0, 'y': 12, 'vx': 0.8, 'vy': 2.2,
                                               'life': 14, 'size': 3.4, 'color': '#ffb04a'}]}]

arcfire_burst['shapes'] = [
    circle((0, 0), 26, '#ff7a2a', alpha=0.3),
    circle((0, 0), 20, '#ffb04a', alpha=0.45),
    circle((0, 0), 11, '#ffe08a', alpha=0.7),
    {'shape': 'star', 'at': [0, 0], 'r': 18, 'fill': '#fff3c0', 'alpha': 0.8},
]

for el in (p_elwind1, p_elwind2):
    el['shapes'] = [
        flame_poly(((-7, -28), (2, -18), (8, 0), (2, 18), (-7, 28), (0, 0)), '#4fc98f', 0.55),
        flame_poly(((-2, -22), (4, -12), (8, 0), (4, 12), (-2, 22), (3, 0)), '#b9ffe0', 0.9),
        line((-9, -20), (-3, -8), 1.6, '#e6fff2', alpha=0.8),
        line((8, 8), (3, 20), 1.6, '#e6fff2', alpha=0.8),
    ]
    el['trail'] = [{'every': 1, 'fx': [{'type': 'part', 'x': 0, 'y': -18, 'vx': 0.4, 'vy': -2.0,
                                        'life': 10, 'size': 2.6, 'color': '#8af0bf'}]}]

projectiles = {}
for key, p in [('thunder', p_thunder), ('elthunder', p_elthunder), ('arcthunder', p_arcthunder),
               ('thoron', p_thoron), ('arcfire', p_arcfire), ('arcfireFlame', arcfire_flame),
               ('arcfireBurst', arcfire_burst),
               ('elwind1', p_elwind1), ('elwind2', p_elwind2)]:
    projectiles[key] = {k: v for k, v in p.items() if v is not None}

# ---------------------------------------------------------------- fighter document
props = lambda *xs: list(xs)
doc = {
    '$schema': '../../schema/fighter.schema.json',
    'schemaVersion': 1,
    'extends': 'base',
    'name': 'Throbbin',
    'tagline': 'Tactician of tomes',
    'blurb': 'Trades Levin-Sword power and five spell tomes for downtime. Charge Thunder into Thoron, '
             'wall opponents out with Arcfire, recover with Elwind and drain health with Nosferatu.',
    'stats': {
        'weight': round(100 / 95, 8), 'pummelDamage': 1.5,
        'walkSpeed': 3.276, 'runSpeed': 4.4275, 'dashSpeed': 6.3525, 'dashFrames': 15, 'runAccel': 0.62,
        'groundFriction': 0.85,
        'jumpSpeed': 13.2, 'shortHopSpeed': 9.4, 'airJumpSpeed': 13.0, 'airJumpDrift': 3.675, 'airJumps': 1,
        'airSpeed': 3.675, 'airAccel': AIR_ACCEL, 'gravity': 0.534, 'fallSpeed': 9.0, 'fastFallSpeed': 14.4,
    },
    'charges': [
        {'id': 'levin', 'uses': 8, 'recharge': 600, 'emptySfx': 'toss', 'refillSfx': 'ready'},
        {'id': 'thunder', 'uses': 20, 'recharge': 720, 'emptySfx': 'toss', 'refillSfx': 'ready'},
        {'id': 'fire', 'uses': 6, 'recharge': 720, 'emptySfx': 'toss', 'refillSfx': 'ready'},
        {'id': 'wind', 'uses': 10, 'recharge': 360, 'emptySfx': 'toss', 'refillSfx': 'ready'},
        {'id': 'dark', 'uses': 3, 'recharge': 1800, 'emptySfx': 'toss', 'refillSfx': 'ready'},
    ],
    'ai': {
        'reach': 78, 'counterChance': 0,
        'specials': [
            {'minDistance': 150, 'height': 40, 'grounded': True, 'noProjectile': False, 'chance': 0.5,
             'neutral': True, 'down': False, 'hold': [10, 40]},
            {'minDistance': 70, 'maxDistance': 210, 'height': 60, 'grounded': True, 'noProjectile': True,
             'chance': 0.45, 'neutral': False, 'down': False, 'hold': None},
            {'minDistance': 0, 'height': 50, 'grounded': True, 'noProjectile': False, 'chance': 0.3,
             'neutral': False, 'down': True, 'hold': None},
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
out = ROOT / 'assets/fighters/throbbin/fighter.json'
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(doc, indent=2) + '\n')
(SOURCES / 'ledger.json').write_text(json.dumps(ledger, indent=1) + '\n')
print(f'wrote {out.relative_to(ROOT)} ({out.stat().st_size} bytes)')
