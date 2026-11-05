#!/usr/bin/env python3
"""Rebuild Duchess (SSBU Peach, patch 13.0.1) from pinned scripts and documented authoring rules.

Numbers come from the pinned Peach attack scripts (damage, launch fields, masks, hitlag, rehit, windows) and
the pinned Ultimate Frame Data rows (startup, active windows, totals, landing lag, autocancel, charge frames,
movement stats). Limb-bone geometry is fitted to the stick-figure poses below; source `top`/root offsets,
speeds and accelerations are converted with the shared 5 px/unit scale. Special mechanics are documented in `docs/imports/duchess.md`.

    just import-duchess
"""
import copy
import importlib
import json
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
units = importlib.import_module('units')
SOURCES = ROOT / 'docs/imports/sources/duchess'
source = json.loads((SOURCES / 'scripts.json').read_text())
ufd = json.loads((SOURCES / 'ufd.json').read_text())
items = json.loads((SOURCES / 'items.json').read_text())
scripts = {art: {s['Name']: s['Data'] for s in ss} for art, ss in source['scripts'].items()}
rows = {r['movename']: r for r in ufd['moves']}
ledger = {'sourceCommit': source['commit'], 'sourceSha256': source['sha256'],
          'ufdSha256': ufd['pageSha256'], 'itemsSourceSha256': items['sha256'],
          'scale': units.SCALE, 'rawAttacks': {}}

SCALE = units.SCALE  # px per source unit: geometry, speeds (px/tick) and accelerations (px/tick²)
GRAVITY = round(0.070 * SCALE, 4)
# Source jump heights (units; ssbwiki "Jump", SSBU jump height ranking): full, short, air.
JUMP_HEIGHTS = (30.03, 14.5, 30.03)

# Phase drift (`airAccelMult`) is written as a multiple of the old shared air accels (aerial 0.45, attack 0.25,
# upSpecial 0.3, old ×3.5 scale, ×5/3.5 here). Under `movement.ultimate` it multiplies airAccel × the kind's
# multiplier instead, so drift() stores the value that keeps the same effective acceleration.
AIR_ACCEL = round(0.12 * SCALE, 4)
def drift(legacy, kind):
    old = {'aerial': 0.45, 'attack': 0.25, 'upSpecial': 0.3}[kind] * 5 / 3.5
    mult = {'aerial': 1.0, 'attack': 0.55, 'upSpecial': 0.65}[kind]
    return round(legacy * old / (AIR_ACCEL * mult), 4)


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


def attacks(name):
    """Every ATTACK row of one `peach` script with its source-frame window (`end` is the clear frame)."""
    src = expand(scripts['peach'][name])
    t, live, out = 0, {}, []

    def close(i):
        b = live.pop(i)
        b['end'] = t
        out.append(b)

    for raw_line in src.split('\n'):
        line = raw_line.strip()
        if m := re.search(r'frame\(Frame=(\d+)\)', line):
            t = int(m[1])
        elif m := re.search(r'frame\((\d+), (\d+)\)', line):
            t = int(m[1])
        if m := re.search(r'wait\(Frames=(\d+)\)', line):
            t += int(m[1])
        if 'AttackModule::clear_all()' in line:
            for i in list(live):
                close(i)
        elif m := re.search(r'AttackModule::clear\(ID=(\d+)\)', line):
            i = int(m[1])
            if i in live:
                close(i)
        elif m := re.search(r'\bATTACK\((.*)\)', line):
            a = args(m[1])
            i = int(a['ID'])
            if i in live:
                close(i)
            live[i] = {'start': t, 'end': t + 1, 'source': a}
    for i in list(live):
        close(i)
    return sorted(out, key=lambda b: (b['start'], int(b['source']['ID'])))


def raw(slot, name):
    found = attacks(name)
    ledger['rawAttacks'][slot] = {'script': name, 'attacks': found}
    return found


def take(rows_, damage, nth=0):
    found = [b for b in rows_ if abs(float(b['source']['Damage']) - damage) < 1e-6]
    return found[nth]


def num(a, k, default=0):
    try:
        return float(a.get(k, default))
    except ValueError:
        return default


def frame(source_frame):
    """Source frame -> phase frame. A move's first tick is source frame 1 = phase frame 0."""
    return max(0, int(source_frame) - 1)


# ---------------------------------------------------------------- conversion
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
    if 'elec' in a.get('Effect', '') and kind == 'normal':
        b['type'] = 'electric'
    b.update(kw)
    return b


def root(a, start, end, **kw):
    """Root-bone row: source Z is forward, Y is up, 5 px per unit. A numeric Z2/Y2 pair becomes a chain of
    overlapping circles no farther apart than their radius (the source capsule)."""
    x, y, r = num(a, 'Z') * SCALE, num(a, 'Y') * SCALE, num(a, 'Size') * SCALE
    if a.get('Z2', 'LUA_VOID') == 'LUA_VOID' and a.get('Y2', 'LUA_VOID') == 'LUA_VOID':
        return [hit(a, start, end, x, y, r, **kw)]
    x2 = x if a.get('Z2', 'LUA_VOID') == 'LUA_VOID' else num(a, 'Z2') * SCALE
    y2 = y if a.get('Y2', 'LUA_VOID') == 'LUA_VOID' else num(a, 'Y2') * SCALE
    steps = max(1, math.ceil(math.hypot(x2 - x, y2 - y) / max(r, 1)))
    return [hit(a, start, end, x + (x2 - x) * i / steps, y + (y2 - y) * i / steps, r, **kw)
            for i in range(steps + 1)]


# ---------------------------------------------------------------- poses and bodies
stand = {'head': [2, 64], 'shoulder': [0, 50], 'hip': [0, 30], 'elbowF': [10, 43], 'handF': [18, 40],
         'elbowB': [-10, 43], 'handB': [-17, 38], 'kneeF': [3, 15], 'footF': [4, 1], 'kneeB': [-4, 15],
         'footB': [-6, 1]}
poses = {
    'stand': stand,
    'crouch': {'head': [8, 46], 'shoulder': [4, 34], 'hip': [0, 20], 'elbowF': [14, 30], 'handF': [22, 22],
               'elbowB': [-8, 30], 'handB': [-14, 20], 'kneeF': [10, 10], 'footF': [12, 1], 'kneeB': [-8, 10],
               'footB': [-12, 1]},
    'pluck': {'head': [14, 42], 'shoulder': [8, 31], 'hip': [2, 18], 'elbowF': [18, 24], 'handF': [26, 4],
              'elbowB': [-6, 24], 'handB': [-12, 8], 'kneeF': [14, 9], 'footF': [14, 1], 'kneeB': [-10, 9],
              'footB': [-14, 1]},
    'holdTurnip': {'head': [4, 62], 'shoulder': [2, 49], 'hip': [0, 29], 'elbowF': [12, 54], 'handF': [16, 64],
                   'elbowB': [-10, 44], 'handB': [-18, 42], 'kneeF': [4, 15], 'footF': [5, 1], 'kneeB': [-5, 15],
                   'footB': [-8, 1]},
    'tossOver': {'head': [8, 60], 'shoulder': [4, 47], 'hip': [0, 28], 'elbowF': [18, 50], 'handF': [34, 56],
                 'elbowB': [-8, 42], 'handB': [-14, 38], 'kneeF': [8, 14], 'footF': [12, 1], 'kneeB': [-8, 14],
                 'footB': [-12, 1]},
    'slapIn': {'head': [4, 63], 'shoulder': [2, 49], 'hip': [-2, 29], 'elbowF': [16, 50], 'handF': [30, 54],
               'elbowB': [-12, 44], 'handB': [-18, 40], 'kneeF': [5, 15], 'footF': [6, 1], 'kneeB': [-6, 15],
               'footB': [-9, 1]},
    'slapOut': {'head': [2, 64], 'shoulder': [0, 50], 'hip': [0, 30], 'elbowF': [18, 48], 'handF': [36, 46],
                'elbowB': [-10, 42], 'handB': [-14, 40], 'kneeF': [4, 15], 'footF': [5, 1], 'kneeB': [-5, 15],
                'footB': [-8, 1]},
    'kickLow': {'head': [6, 56], 'shoulder': [2, 42], 'hip': [-4, 22], 'elbowF': [12, 38], 'handF': [18, 32],
                'elbowB': [-14, 36], 'handB': [-20, 30], 'kneeF': [20, 16], 'footF': [42, 12], 'kneeB': [-8, 14],
                'footB': [-12, 1]},
    'sweepUp': {'head': [-2, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [10, 64], 'handF': [16, 84],
                'elbowB': [-10, 60], 'handB': [-16, 78], 'kneeF': [4, 16], 'footF': [6, 1], 'kneeB': [-6, 16],
                'footB': [-8, 1]},
    'stompLow': {'head': [10, 48], 'shoulder': [4, 36], 'hip': [0, 20], 'elbowF': [12, 28], 'handF': [16, 14],
                 'elbowB': [-12, 28], 'handB': [-18, 14], 'kneeF': [18, 10], 'footF': [34, 2], 'kneeB': [-6, 10],
                 'footB': [-12, 1]},
    'dashPush': {'head': [8, 60], 'shoulder': [4, 47], 'hip': [-2, 27], 'elbowF': [22, 48], 'handF': [44, 48],
                 'elbowB': [10, 46], 'handB': [40, 44], 'kneeF': [16, 14], 'footF': [32, 2], 'kneeB': [-10, 14],
                 'footB': [-18, 1]},
    'clubSwing': {'head': [-2, 61], 'shoulder': [0, 48], 'hip': [2, 29], 'elbowF': [14, 56], 'handF': [26, 66],
                  'elbowB': [-12, 42], 'handB': [-18, 38], 'kneeF': [8, 15], 'footF': [12, 1], 'kneeB': [-8, 15],
                  'footB': [-14, 1]},
    'riseMagic': {'head': [-4, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [8, 64], 'handF': [10, 86],
                  'elbowB': [-8, 64], 'handB': [-12, 84], 'kneeF': [4, 16], 'footF': [6, 1], 'kneeB': [-6, 16],
                  'footB': [-8, 1]},
    'spinLow': {'head': [4, 48], 'shoulder': [0, 36], 'hip': [-2, 18], 'elbowF': [20, 34], 'handF': [44, 30],
                'elbowB': [-18, 32], 'handB': [-38, 28], 'kneeF': [12, 9], 'footF': [20, 1], 'kneeB': [-10, 9],
                'footB': [-18, 1]},
    'air': {'head': [2, 62], 'shoulder': [0, 49], 'hip': [0, 30], 'elbowF': [12, 44], 'handF': [22, 42],
            'elbowB': [-12, 42], 'handB': [-22, 38], 'kneeF': [8, 18], 'footF': [10, 6], 'kneeB': [-8, 16],
            'footB': [-14, 5]},
    'airSpread': {'head': [2, 62], 'shoulder': [0, 52], 'hip': [0, 34], 'elbowF': [16, 52], 'handF': [32, 54],
                  'elbowB': [-16, 52], 'handB': [-32, 52], 'kneeF': [6, 20], 'footF': [12, 8], 'kneeB': [-6, 18],
                  'footB': [-16, 8]},
    'airHigh': {'head': [0, 62], 'shoulder': [0, 49], 'hip': [-2, 30], 'elbowF': [10, 62], 'handF': [14, 80],
                'elbowB': [-10, 62], 'handB': [-18, 78], 'kneeF': [10, 18], 'footF': [14, 6], 'kneeB': [-6, 16],
                'footB': [-10, 6]},
    'airLow': {'head': [8, 56], 'shoulder': [4, 44], 'hip': [-2, 26], 'elbowF': [18, 32], 'handF': [26, 18],
               'elbowB': [-12, 32], 'handB': [-20, 18], 'kneeF': [18, 12], 'footF': [34, 4], 'kneeB': [-8, 12],
               'footB': [-14, 2]},
    'crownStrike': {'head': [2, 62], 'shoulder': [0, 50], 'hip': [0, 30], 'elbowF': [14, 60], 'handF': [22, 78],
                    'elbowB': [-12, 42], 'handB': [-20, 38], 'kneeF': [8, 18], 'footF': [12, 6], 'kneeB': [-8, 16],
                    'footB': [-12, 6]},
    'hipBack': {'head': [8, 58], 'shoulder': [4, 46], 'hip': [-6, 26], 'elbowF': [18, 40], 'handF': [26, 34],
                'elbowB': [-14, 38], 'handB': [-24, 32], 'kneeF': [10, 16], 'footF': [16, 6], 'kneeB': [-12, 14],
                'footB': [-26, 8]},
    'counter': {'head': [-2, 60], 'shoulder': [-2, 48], 'hip': [-2, 28], 'elbowF': [8, 58], 'handF': [4, 66],
                'elbowB': [-10, 56], 'handB': [-4, 66], 'kneeF': [2, 15], 'footF': [2, 1], 'kneeB': [-8, 15],
                'footB': [-12, 1]},
    'bomber': {'head': [10, 54], 'shoulder': [6, 42], 'hip': [-2, 24], 'elbowF': [-6, 38], 'handF': [0, 44],
               'elbowB': [-16, 36], 'handB': [-26, 34], 'kneeF': [14, 12], 'footF': [24, 2], 'kneeB': [-12, 10],
               'footB': [-20, 2]},
    'parasol': {'head': [0, 62], 'shoulder': [0, 49], 'hip': [0, 30], 'elbowF': [10, 54], 'handF': [6, 78],
                'elbowB': [-12, 42], 'handB': [-20, 38], 'kneeF': [4, 17], 'footF': [6, 2], 'kneeB': [-6, 16],
                'footB': [-10, 4]},
    'parasolFall': {'head': [0, 62], 'shoulder': [0, 49], 'hip': [0, 30], 'elbowF': [10, 54], 'handF': [6, 78],
                    'elbowB': [-14, 40], 'handB': [-22, 32], 'kneeF': [2, 18], 'footF': [2, 4], 'kneeB': [-6, 16],
                    'footB': [-10, 6]},
    'win': {'head': [0, 66], 'shoulder': [0, 52], 'hip': [0, 32], 'elbowF': [16, 46], 'handF': [24, 62],
            'elbowB': [-16, 50], 'handB': [-28, 56], 'kneeF': [5, 17], 'footF': [9, 1], 'kneeB': [-6, 17],
            'footB': [-10, 1]},
    'helpless': {'head': [-2, 58], 'shoulder': [0, 47], 'hip': [0, 29], 'elbowF': [8, 40], 'handF': [16, 28],
                 'elbowB': [-12, 40], 'handB': [-22, 30], 'kneeF': [6, 16], 'footF': [8, 4], 'kneeB': [-8, 15],
                 'footB': [-12, 4]},
}

# A source bone rides a fitted pose joint (limb-bone geometry is fitted, root geometry is converted).
JOINT = {
    'top': None, 'hip': 'hip', 'head': 'head', 'handr': 'handF', 'handl': 'handB', 'armr': 'handF',
    'arml': 'handB', 'shoulderr': 'shoulder', 'shoulderl': 'shoulder', 'kneer': 'kneeF', 'kneel': 'kneeB',
    'legr': 'footF', 'legl': 'footB', 'haver': 'handF', 'havel': 'handF',
}


def fitted(a, start, end, pose, bone='top', r=None, **kw):
    """Hitbox for one script row. `top` converts the root offsets; limb bones follow the posed joint."""
    bone = re.sub(r'^hash40\("?|"?\)$', '', bone)
    if JOINT[bone] is None:
        return root(a, start, end, **kw)
    j = poses[pose][JOINT[bone]]
    r = num(a, 'Size') * SCALE if r is None else r
    return hit(a, start, end, j[0] + num(a, 'Z') * SCALE, j[1] + num(a, 'Y') * SCALE, r, **kw)


def flat(*xs):
    """Flatten `fitted` results: root capsules expand to several circles."""
    out = []
    for x in xs:
        out.extend(x if isinstance(x, list) else [x])
    return out


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


def wait(to):
    return {'on': 'timeout', 'to': to}


def swing(pose, start, end, total, air=False, start_pose=None):
    off = 'air' if air else 'stand'
    return [{'at': 0, 'pose': start_pose or off}] + \
        [{'at': max(0, start - 3), 'pose': pose, 'lerp': True}, {'at': end, 'pose': pose}] + \
        [{'at': min(max(1, total - 1), end + 6), 'pose': off, 'lerp': True}]


# ---------------------------------------------------------------- moves
moves = {}


def win(b):
    """Engine window for one parsed script row: the clear frame is exclusive."""
    return frame(b['start']), frame(b['end'] - 1)


def rows_at(rows_, pose, base=0, bone=None):
    """Fitted hitboxes for parsed rows, re-based into a phase that starts at move frame `base`."""
    out = []
    for b in rows_:
        s, e = win(b)
        assert s >= base, (s, base)
        out += flat(fitted(b['source'], s - base, e - base, pose, bone or b['source']['Bone']))
    return out


def hit_anim(pose, boxes_, frames, rest='stand'):
    """Swing a pose out for the phase's hit window and return to `rest` afterwards."""
    if not boxes_:
        return [{'at': 0, 'pose': rest}]
    first = min(b['start'] for b in boxes_)
    last = max(b.get('end', frames - 1) for b in boxes_)
    return [{'at': 0, 'pose': rest}] + \
        [{'at': max(0, first - 2), 'pose': pose, 'lerp': True}, {'at': last, 'pose': pose},
         {'at': min(max(1, frames - 1), last + 3), 'pose': rest, 'lerp': True}]


def chain_ground(parts):
    """Assign identities across the full timeline, retaining them through presentation splits."""
    boxes, offset = [], 0
    for p in parts:
        for b in p.get('hitboxes', []):
            boxes.append({**b, 'start': b['start'] + offset,
                          'end': b['end'] + offset if b.get('end') is not None else None})
        offset += p.get('frames', 0)
    grouped = iter(assign_groups(boxes))
    for p in parts:
        for b in p.get('hitboxes', []):
            group = next(grouped)['_group']
            b.pop('group', None)
            if group:
                b['group'] = group
    for a, b in zip(parts, parts[1:]):
        transition = next((t for t in a.get('transitions', []) if t['on'] == 'timeout' and t['to'] == b['name']), None)
        if transition is None:
            transition = wait(b['name'])
            a.setdefault('transitions', []).append(transition)
        transition['preserveHits'] = True
    return parts


def chain_air(parts, lands):
    """Aerial sequences: `lands[i]` is the landing lag during that phase (None = autocancel/no lag)."""
    for i, p in enumerate(parts):
        p.setdefault('transitions', []).append({'on': 'land', 'to': 'end'} if lands[i] is None else land(lands[i]))
    return chain_ground(parts)


# Neutral attack: forehand slap then backhand slap, continuing while attack is held.
j1 = raw('jab', 'Attack11')
j2 = raw('jab2', 'Attack12')
jab1 = rows_at(j1, 'slapIn')
moves['jab'] = {'name': 'Royal Slap', 'phases': [
    phase('jab1', 27, jab1, anim=hit_anim('slapIn', jab1, 27), events=[{'at': jab1[0]['start'], 'sfx': 'hit'}],
          transitions=[{'on': 'attackHeld', 'from': frame(7), 'to': 'jab2'}, wait('end')]),
    phase('jab2', 29, rows_at(j2, 'slapOut'), anim=hit_anim('slapOut', rows_at(j2, 'slapOut'), 29),
          events=[{'at': frame(2), 'sfx': 'hit'}])]}

# Tilts: coverage changes may split phases while continuous swings share contact history.
ft = raw('ftilt', 'AttackS3')
ft_close = [b for b in ft if b['start'] < 10]
ft_late = [b for b in ft if b['start'] >= 10]
moves['ftilt'] = {'name': 'Crown Kick', 'phases': chain_ground([
    phase('close', 9, rows_at(ft_close, 'kickLow'), anim=hit_anim('kickLow', rows_at(ft_close, 'kickLow'), 9),
          events=[{'at': frame(7), 'sfx': 'hit'}]),
    phase('late', 27, rows_at(ft_late, 'kickLow', 9), anim=hit_anim('kickLow', rows_at(ft_late, 'kickLow', 9), 27),
          events=[{'at': 0, 'sfx': 'hit'}]),
])}

ut = raw('utilt', 'AttackHi3')
utilt = rows_at(ut, 'sweepUp', bone='top')
moves['utilt'] = {'name': 'Royal Wave', 'phases': [phase(
    'main', 37, utilt, anim=hit_anim('sweepUp', utilt, 37), events=[{'at': frame(9), 'sfx': 'swing'}])]}

dt = raw('dtilt', 'AttackLw3')
dtilt = rows_at(dt, 'kickLow', bone='top')
moves['dtilt'] = {'name': 'Underfoot Kick', 'phases': [phase(
    'main', 23, dtilt, anim=hit_anim('kickLow', dtilt, 23), events=[{'at': frame(8), 'sfx': 'hit'}])]}

da = raw('dashatk', 'AttackDash')
dash1 = [b for b in da if b['start'] < 10]
dash2 = [b for b in da if b['start'] >= 10]
moves['dashatk'] = {'name': 'Lady Push', 'phases': chain_ground([
    phase('slide', 16, rows_at(dash1, 'dashPush', bone='hip'), slide=12,
          anim=hit_anim('dashPush', rows_at(dash1, 'dashPush', bone='hip'), 16),
          events=[{'at': 0, 'setVel': {'x': 5.1429}}, {'at': frame(6), 'sfx': 'shoot'}]),
    phase('push', 21, rows_at(dash2, 'dashPush', 16, bone='hip'),
          anim=hit_anim('dashPush', rows_at(dash2, 'dashPush', 16, bone='hip'), 21),
          events=[{'at': 0, 'sfx': 'slash'}]),
])}

# Smashes: Peach's fsmash weapon is picked by the source's stick angle. Only the un-angled golf club is
# imported; the frying pan and tennis racket rows are parsed into the ledger but not reachable.
fs = raw('fsmash', 'AttackS4')
raw('fsmash_pan', 'AttackS4Hi')
raw('fsmash_racket', 'AttackS4Lw')
fsmash = flat(*[hit(b['source'], *win(b), x, y, r)
                for b, (x, y, r) in zip([take(fs, 15.0, i) for i in range(3)],
                                        [(40, 58, 13.5), (46, 68, 13.5), (52, 82, 21.5)])])
moves['fsmash'] = {'name': 'Peach Smash', 'phases': [phase(
    'main', 45, fsmash, charge={'frame': frame(5)},
    anim=hit_anim('clubSwing', fsmash, 45),
    attach={'club': {'show': [0, 44], 'angle': [[0, 150], [12, 200], [15, 20], [17, -30], [45, 60]]}},
    events=[{'at': frame(5), 'sfx': 'smash'}, {'at': frame(15), 'sfx': 'whack'}])]}

us = raw('usmash', 'AttackHi4')
us_early = [b for b in us if b['start'] < 17]
us_late = [b for b in us if b['start'] >= 17]
usmash_early = rows_at(us_early, 'riseMagic', 13)
usmash_late = rows_at(us_late, 'riseMagic', 16)
moves['usmash'] = {'name': 'Crown Rise', 'phases': chain_ground([
    phase('windup', 13, anim=[{'at': 0, 'pose': 'stand'}], charge={'frame': frame(9)},
          events=[{'at': frame(9), 'sfx': 'smash'}]),
    phase('early', 3, usmash_early, anim=hit_anim('riseMagic', usmash_early, 3),
          events=[{'at': 0, 'sfx': 'shoot'}]),
    phase('late', 4, usmash_late, anim=[{'at': 0, 'pose': 'riseMagic'}]),
    phase('cooldown', 24, anim=[{'at': 0, 'pose': 'riseMagic'}, {'at': 10, 'pose': 'stand', 'lerp': True}]),
])}

ds = raw('dsmash', 'AttackLw4')
spins = [b for b in ds if num(b['source'], 'Damage') == 2.0]
final = [b for b in ds if num(b['source'], 'Damage') == 3.0]
spin_windows = sorted({b['start'] for b in spins})
dsmash = [phase('windup', 5, anim=[{'at': 0, 'pose': 'stand'}], charge={'frame': frame(2)},
                events=[{'at': frame(2), 'sfx': 'smash'}])]
for i, start in enumerate(spin_windows):
    lo = start - 1
    group = [b for b in spins if b['start'] == start]
    base = 5 + 4 * i
    assert lo == base, (lo, base)
    dsmash.append(phase(f'spin{i + 1}', 4, rows_at(group, 'spinLow', base, bone='top'),
                        anim=[{'at': 0, 'pose': 'spinLow'}], events=[{'at': 0, 'sfx': 'shoot'}]))
final_start = min(b['start'] for b in final) - 1
dsmash.append(phase('final', 54 - final_start, rows_at(final, 'spinLow', final_start, bone='top'),
                    anim=[{'at': 0, 'pose': 'spinLow'}, {'at': 12, 'pose': 'stand', 'lerp': True}]))
moves['dsmash'] = {'name': 'Ribbon Sweep', 'phases': chain_ground(dsmash)}

# Aerials: the autocancel windows and each independent hit window are separate phases, so landing lag,
# autocancel timing and repeat hits all survive.
an = raw('nair', 'AttackAirN')
nair_clean = [b for b in an if b['start'] < 9]
nair_late = [b for b in an if b['start'] >= 9]
moves['nair'] = {'name': 'Royal Spin', 'air': 'aerial', 'phases': chain_air([
    phase('startup', 4, anim=[{'at': 0, 'pose': 'air'}]),
    phase('hit1', 4, rows_at(nair_clean, 'airSpread', 4),
          anim=hit_anim('airSpread', rows_at(nair_clean, 'airSpread', 4), 4, 'air')),
    phase('hit2', 27, rows_at(nair_late, 'airSpread', 8),
          anim=hit_anim('airSpread', rows_at(nair_late, 'airSpread', 8), 27, 'air')),
    phase('late', 13, anim=[{'at': 0, 'pose': 'air'}]),
], [None, 7, 7, None])}

af = raw('fair', 'AttackAirF')
fair = flat(hit(take(af, 14.0)['source'], frame(16), frame(20), 10, 78, 17,
                frames=[[frame(16), 10, 78, 17], [frame(18), 16, 70, 17], [frame(20), 20, 60, 17]]),
            hit(take(af, 15.0)['source'], frame(16), frame(20), 18, 86, 22,
                frames=[[frame(16), 18, 86, 22], [frame(18), 28, 76, 22], [frame(20), 32, 64, 22]]))
fair_main = [{**b, 'start': b['start'] - 9, 'end': b['end'] - 9,
              'frames': [[t - 9, x, y, r] for t, x, y, r in b['frames']]} for b in fair]
moves['fair'] = {'name': 'Crown Knuckle', 'air': 'aerial', 'phases': chain_air([
    phase('startup', 9, anim=[{'at': 0, 'pose': 'air'}]),
    phase('main', 31, fair_main, anim=hit_anim('crownStrike', fair_main, 31, 'air'), attach={
        'crown': {'scale': [[0, 0]]},
        'crownHand': {'show': [fair_main[0]['start'], fair_main[0]['end']], 'angle': [[0, 0]]},
    }),
    phase('late', 17, anim=[{'at': 0, 'pose': 'air'}]),
], [None, 13, None])}

ab = raw('bair', 'AttackAirB')
bair = rows_at(ab, 'hipBack', 4, bone='top')
moves['bair'] = {'name': 'Iron Hip', 'air': 'aerial', 'phases': chain_air([
    phase('startup', 4, anim=[{'at': 0, 'pose': 'air'}]),
    phase('main', 25, bair, anim=hit_anim('hipBack', bair, 25, 'air')),
    phase('late', 24, anim=[{'at': 0, 'pose': 'air'}]),
], [None, 11, None])}

au = raw('uair', 'AttackAirHi')
uair_hit1 = [b for b in au if b['start'] < 14]
uair_hit2 = [b for b in au if b['start'] >= 14]
moves['uair'] = {'name': 'Rainbow Arc', 'air': 'aerial', 'phases': chain_air([
    phase('startup', 4, anim=[{'at': 0, 'pose': 'air'}]),
    phase('windup', 5, anim=[{'at': 0, 'pose': 'airHigh'}]),
    phase('hit1', 4, rows_at(uair_hit1, 'airHigh', 9, bone='top'),
          anim=hit_anim('airHigh', rows_at(uair_hit1, 'airHigh', 9, bone='top'), 4, 'air')),
    phase('hit2', 27, rows_at(uair_hit2, 'airHigh', 13, bone='top'),
          anim=hit_anim('airHigh', rows_at(uair_hit2, 'airHigh', 13, bone='top'), 27, 'air')),
    phase('late', 5, anim=[{'at': 0, 'pose': 'air'}]),
], [None, 7, 7, 7, None])}

ad = raw('dair', 'AttackAirLw')


def dair_group(damage):
    return [b for b in ad if num(b['source'], 'Damage') == damage]


def dair_stomp(i, base):
    group = dair_group(1.0) + dair_group(2.0)
    group = [b for b in group if b['start'] == 12 + 6 * i]
    return phase(f'stomp{i + 1}', 2, rows_at(group, 'airLow', base, bone='top'),
                 anim=[{'at': 0, 'pose': 'airLow'}], events=[{'at': 0, 'sfx': 'hit'}] if i == 0 else [])


dair_parts = [phase('startup', 11, anim=[{'at': 0, 'pose': 'air'}]),
              dair_stomp(0, 11), phase('gap1', 4, anim=[{'at': 0, 'pose': 'air'}]),
              dair_stomp(1, 17), phase('gap2', 4, anim=[{'at': 0, 'pose': 'air'}]),
              dair_stomp(2, 23), phase('gap3', 4, anim=[{'at': 0, 'pose': 'air'}]),
              phase('finish', 8, rows_at(dair_group(5.0), 'airLow', 29, bone='top'),
                    anim=[{'at': 0, 'pose': 'airLow'}, {'at': 6, 'pose': 'air', 'lerp': True}]),
              phase('late', 1, anim=[{'at': 0, 'pose': 'air'}])]
moves['dair'] = {'name': 'Piston Kick', 'air': 'aerial', 'phases': chain_air(
    dair_parts, [None] + [8] * 7 + [None])}

# Grabs
cg = raw('grab', 'Catch')
grab_box = root({'Damage': '0', 'Angle': '0', 'KBG': '0', 'BKB': '0', 'Size': '3.3', 'Z': '4.0', 'Y': '8.5',
                 'Z2': '8.7', 'Y2': '8.5', 'FacingRestrict': 'F', 'Ground_or_Air': 'GA', 'Bone': 'top'},
                frame(6), frame(7), type='grab')
moves['grab'] = {'name': 'Grab', 'phases': [phase(
    'main', 36, grab_box, anim=swing('slapOut', frame(6), frame(7), 36))]}
dg = raw('dashgrab', 'CatchDash')
dash_grab = root({'Damage': '0', 'Angle': '0', 'KBG': '0', 'BKB': '0', 'Size': '2.6', 'Z': '4.0', 'Y': '8.5',
                  'Z2': '10.4', 'Y2': '8.5', 'FacingRestrict': 'F', 'Ground_or_Air': 'GA', 'Bone': 'top'},
                 frame(9), frame(10), type='grab')
moves['dashgrab'] = {'name': 'Dash Grab', 'phases': [phase(
    'main', 44, dash_grab, slide=frame(18), anim=swing('slapOut', frame(9), frame(10), 44))]}

# --- specials
# Toad: a 9-34 counter window that answers with six reflectable spore projectiles. The source's intangibility
# on frames 8-10 cannot be authored alongside the counter (an intangible victim is skipped before the counter
# check), so the counter window itself is the protection here.
raw('nspec', 'SpecialN')
raw('nspec_hit', 'SpecialNHit')
ledger['rawAttacks']['spore'] = {'article': 'peach_kinopiospore', 'script': 'Shot',
                                 'attacks': scripts['peach_kinopiospore']['Shot']}
moves['nspec'] = {'name': 'Toad', 'phases': [
    phase('counter', 62, anim=[{'at': 0, 'pose': 'counter'}],
          counter={'from': frame(9), 'to': frame(34), 'damageMult': 1.0, 'minDamage': 0.0,
                   'attackerFreeze': 18, 'invuln': 4, 'reflectSpeed': 0.7, 'reflectLife': 24},
          attach={'toad': {'show': [0, 61], 'offset': [[0, 30, -16]]}},
          events=[{'at': 0, 'sfx': 'shieldHit'}],
          transitions=[{'on': 'counter', 'to': 'spores'}]),
    phase('spores', 60, anim=[{'at': 0, 'pose': 'counter'}],
          attach={'toad': {'show': [0, 59], 'offset': [[0, 30, -16]]}},
          events=[{'at': frame(3), 'spawn': 'spore_up', 'sfx': 'shoot'},
                  {'at': frame(6), 'spawn': 'spore_fwd'},
                  {'at': frame(9), 'spawn': 'spore_low'},
                  {'at': frame(12), 'spawn': 'spore_up'},
                  {'at': frame(15), 'spawn': 'spore_fwd'},
                  {'at': frame(18), 'spawn': 'spore_low'}])]}

# Peach Bomber: a hip-first lunge that detonates on contact; a whiff leaves her open. The windup is one tick
# shorter than the source's 12 so the search box is live for source frames 13-36: phase entry does not check
# transitions, so the first check happens one tick into `thrust`.
# The lunge launches at the tilt-input 2.9 (SmashWiki) and holds it until the script's frame-4
# `FIGHTER_KINETIC_TYPE_PEACH_SPECIAL_S_BRAKE`, then brakes linearly. The brake rate is not in any public source:
# it is fitted (low confidence) so a grounded whiff covers ~45 units, clearly short of a Battlefield ledge from centre.
# A per-tick `setVel` schedule rather than `addVel` so the speed profile does not compound with traction (she lands
# a couple of ticks into the lunge; grounded traction still applies after each set, airborne air friction).
BOMBER_SPEED, BOMBER_BRAKE_FROM, BOMBER_BRAKE = 2.9, 3, 0.095
bomber_brake = [{'at': t, 'setVel': {'x': round(max(BOMBER_SPEED - BOMBER_BRAKE * (t - BOMBER_BRAKE_FROM + 1), 0) * SCALE, 4)}}
                for t in range(BOMBER_BRAKE_FROM, frame(37) - frame(13) + 1)]
raw('sspec_jump', 'SpecialSJump')
raw('sspec_hit', 'SpecialSHitEnd')
moves['sspec'] = {'name': 'Peach Bomber', 'phases': [
    phase('windup', frame(13) - 1, anim=[{'at': 0, 'pose': 'stand'}, {'at': 7, 'pose': 'bomber', 'lerp': True}],
          events=[{'at': 0, 'sfx': 'charge'}], transitions=[wait('thrust')]),
    phase('thrust', frame(37) - frame(13) + 1, detection={'x1': 6, 'x2': 62, 'y1': 6, 'y2': 70},
          anim=[{'at': 0, 'pose': 'bomber'}], ledgeGrab=frame(16) - frame(13),
          events=[{'at': 0, 'setVel': {'x': BOMBER_SPEED * SCALE, 'y': 0.4167}, 'leaveGround': True, 'sfx': 'swing'},
                  {'from': 1, 'to': BOMBER_BRAKE_FROM - 1, 'setVel': {'x': BOMBER_SPEED * SCALE}},
                  *bomber_brake,
                  {'from': 3, 'every': 3, 'fx': [{'type': 'burst', 'x': -18, 'y': 30, 'color': '#ff9ecb',
                                                  'n': 2, 'speed': 1.6, 'life': 12, 'size': 3.4}]}],
          # The timeouts are listed first: on the phase's last tick (source 37) they win over the search box,
          # so detection is exactly source frames 13-36.
          transitions=[{'on': 'timeout', 'if': 'grounded', 'to': 'recover'},
                       {'on': 'timeout', 'to': 'fall'},
                       {'on': 'targetNear', 'to': 'burst'}]),
    phase('burst', 25, [hit({'Damage': '12.0', 'Angle': '361', 'KBG': '73', 'BKB': '60', 'Size': '7.7',
                             'FacingRestrict': 'ATTACK_LR_CHECK_F', 'Ground_or_Air': 'COLLISION_SITUATION_MASK_GA',
                             'ShieldDamage': '6', 'Hitlag': '1.0'}, 1, 2, 22, 40, 38.5)],
          anim=[{'at': 0, 'pose': 'bomber'}], ledgeGrab=0,
          events=[{'at': 0, 'setVel': {'x': -0.95 * SCALE, 'y': 1.4 * SCALE}, 'sfx': 'whack'}],
          transitions=[{'on': 'land', 'to': 'landing', 'lag': 35}, {'on': 'timeout', 'to': 'fall'}]),
    phase('recover', 26, anim=[{'at': 0, 'pose': 'crouch'}, {'at': 12, 'pose': 'stand', 'lerp': True}]),
    phase('fall', ledgeGrab=0, anim=[{'at': 0, 'pose': 'air'}],
          transitions=[{'on': 'land', 'to': 'landing', 'lag': 35}]),
]}

# Peach Parasol: the rising hits follow the pinned `SpecialHiStart` rows (including the frame-22 autolink
# replacements), then an open parasol slows the descent until a fresh special press closes it.
raw('uspec', 'SpecialHiStart')
raw('uspec_open', 'SpecialHiOpen')
raw('uspec_fall', 'SpecialHiFall')


def parasol(damage, angle, kbg, fkb, bkb, size, x, y, r, start, end, *, rehit=0, hitlag=1.0):
    a = {'Damage': str(damage), 'Angle': str(angle), 'KBG': str(kbg), 'FKB': str(fkb), 'BKB': str(bkb),
         'Size': str(size), 'FacingRestrict': 'ATTACK_LR_CHECK_F',
         'Ground_or_Air': 'COLLISION_SITUATION_MASK_GA', 'Hitlag': str(hitlag), 'Rehit': str(rehit)}
    return hit(a, start, end, x, y, r)


rise1 = [parasol(3.0, 88, 100, 160, 0, 5.0, 8, 96, 25, 6, 8),
         parasol(3.0, 100, 100, 130, 0, 4.0, 0, 68, 20, 6, 8)]
rise2 = [parasol(1.0, 93, 100, 160, 0, 4.0, 4, 92, 20, 0, 4, rehit=5),
         parasol(1.0, 87, 100, 80, 0, 1.5, 10, 78, 7.5, 0, 5, rehit=5),
         parasol(1.0, 367, 100, 70, 0, 5.0, 2, 84, 25, 0, 5, rehit=5)]
rise3 = [parasol(1.0, 87, 100, 80, 0, 1.5, 10, 78, 7.5, 0, 4, rehit=5),
         parasol(1.0, 367, 100, 70, 0, 5.0, 2, 84, 25, 0, 4, rehit=5)]
rise4 = [parasol(1.0, 368, 100, 80, 0, 1.5, 10, 78, 7.5, 0, 5, rehit=5),
         parasol(1.0, 368, 100, 70, 0, 5.0, 2, 84, 25, 0, 5, rehit=5)]
rise5 = [parasol(4.0, 81, 70, 0, 90, 5.2, 8, 88, 26, 3, 4, hitlag=3.0)]
moves['uspec'] = {'name': 'Peach Parasol', 'air': 'upSpecial', 'oncePerAirtime': True, 'phases': [
    phase('rise1', 10, rise1, anim=[{'at': 0, 'pose': 'parasol'}],
          attach={'parasol': {'show': [0, 38], 'angle': [[0, 0]]}},
          events=[{'at': frame(6), 'leaveGround': True, 'setVel': {'y': 11.4109}, 'sfx': 'swing'}],
          transitions=[land(26), wait('rise2')]),
    phase('rise2', 6, rise2, anim=[{'at': 0, 'pose': 'parasol'}],
          attach={'parasol': {'show': [0, 5], 'angle': [[0, 0]]}}, transitions=[land(26), wait('rise3')]),
    phase('rise3', 6, rise3, anim=[{'at': 0, 'pose': 'parasol'}], ledgeGrab=0,
          attach={'parasol': {'show': [0, 5], 'angle': [[0, 0]]}},
          transitions=[land(26), wait('rise4')]),
    phase('rise4', 6, rise4, anim=[{'at': 0, 'pose': 'parasol'}], ledgeGrab=0,
          attach={'parasol': {'show': [0, 5], 'angle': [[0, 0]]}},
          transitions=[land(26), wait('rise5')]),
    phase('rise5', 11, rise5, anim=[{'at': 0, 'pose': 'parasol'}], ledgeGrab=0,
          attach={'parasol': {'show': [0, 10], 'angle': [[0, 0]]}},
          events=[{'at': 0, 'sfx': 'slash'}], transitions=[land(26), wait('open')]),
    phase('open', 480,
          [hit({'Damage': '2.0', 'Angle': '80', 'KBG': '15', 'BKB': '65', 'Size': '2.0',
                'FacingRestrict': 'ATTACK_LR_CHECK_POS', 'Ground_or_Air': 'COLLISION_SITUATION_MASK_GA',
                'Hitlag': '0.0', 'Rehit': '18'}, frame(8), 479, x, 102, 12) for x in (-13, 0, 13)],
          fallCap=1.6667, airAccelMult=drift(1.2, 'upSpecial'), ledgeGrab=0,
          anim=[{'at': 0, 'pose': 'parasol'}, {'at': 30, 'pose': 'parasolFall', 'lerp': True}],
          attach={'parasol': {'show': [0, 479], 'angle': [[0, 0]]}},
          events=[{'at': 0, 'sfx': 'swing'}],
          transitions=[{'on': 'specialPress', 'to': 'closed'}, land(26), wait('closed')]),
    # Closed parasol = special fall: it runs until she lands (shared helplessness would end in its own lag).
    # Special fall still grabs ledges, as in the source.
    phase('closed', None, anim=[{'at': 0, 'pose': 'helpless'}], ledgeGrab=0,
          attach={'parasol': {'show': [0, 14], 'angle': [[0, 0]]}},
          events=[{'at': 0, 'sfx': 'swing'}], transitions=[land(40)]),
]}

# The rising timeline changes coverage without resetting a continuing loop's cooldown.
chain_ground(moves['uspec']['phases'][:5])

# Vegetable: the 39-frame pluck puts a turnip item in her hand (out of the ground on source frame 30); it is
# thrown like any held item (the shared `toss` state; `items.turnip` below).
raw('dspec', 'SpecialLw')
ledger['rawAttacks']['turnip'] = {'article': 'item', 'script': 'peachdaikon/game_Throw',
                                  'attacks': items['items']['peachdaikon']['Scripts']}
moves['dspec'] = {'name': 'Vegetable', 'phases': [
    phase('pluck', 39, anim=[{'at': 0, 'pose': 'crouch'}, {'at': 10, 'pose': 'pluck', 'lerp': True},
                             {'at': 30, 'pose': 'holdTurnip', 'lerp': True}],
          events=[{'at': frame(20), 'sfx': 'toss'}, {'at': frame(30), 'sfx': 'ready', 'item': 'turnip'}]),
]}

# ---------------------------------------------------------------- throws
def throw(slot, script, total, release, tick=None, tick_damage=None):
    line = next(x for x in scripts['peach'][script].splitlines() if 'ATTACK_ABS(Kind=FIGHTER_ATTACK_ABSOLUTE_KIND_THROW' in x)
    a = args(line.split('THROW,', 1)[1].rstrip(')'))
    b = hit(a, 0, 0, 0, 0, 0)
    ledger['rawAttacks'][slot] = {'script': script, 'attacks': [a]}
    t = {'release': frame(release), 'total': total, 'hit': b, 'ticks': []}
    if tick is not None:
        t['ticks'] = [{'at': frame(tick), 'damage': tick_damage}]
    return t


throws = {
    'f': throw('fthrow', 'ThrowF', 39, 16, 14, 2.0),
    'b': throw('bthrow', 'ThrowB', 49, 22, 20, 2.0),
    'u': throw('uthrow', 'ThrowHi', 49, 26, 19, 2.0),
    'd': throw('dthrow', 'ThrowLw', 64, 43, 34, 1.0),
}

# ---------------------------------------------------------------- projectiles
turnip_hit = hit({'Damage': '8.3', 'Angle': '30', 'KBG': '45', 'BKB': '25', 'Size': '2.1',
                  'FacingRestrict': 'ATTACK_LR_CHECK_SPEED', 'Ground_or_Air': 'COLLISION_SITUATION_MASK_GA',
                  'Hitlag': '0.8', 'ShieldDamage': '-2.5'}, 0, 0, 0, 0, 0)
turnip_strong = dict(turnip_hit, damage=11.4,
                     shieldDmgMult=round((11.4 - 2.5) / 11.4, 4))
spore_hit = hit({'Damage': '3.5', 'Angle': '45', 'KBG': '210', 'BKB': '40', 'Size': '4.5',
                 'FacingRestrict': 'ATTACK_LR_CHECK_SPEED', 'Ground_or_Air': 'COLLISION_SITUATION_MASK_GA',
                 'Hitlag': '1.0', 'ShieldDamage': '2'}, 0, 0, 0, 0, 0)

turnip_shapes = [
    {'shape': 'circle', 'at': [2, 0], 'r': 9, 'fill': '#f3dede', 'stroke': '#c9a9a2', 'width': 1.6},
    {'shape': 'circle', 'at': [9, 2], 'r': 2.2, 'fill': '#6b5f4a'},
    {'shape': 'poly', 'points': [[-3, -6], [1, -14], [4, -6]], 'fill': '#5fae52'},
    {'shape': 'poly', 'points': [[4, -6], [9, -15], [10, -6]], 'fill': '#77c466'},
    {'shape': 'poly', 'points': [[-8, -5], [-4, -13], [-1, -5]], 'fill': '#4f9846'},
]
projectiles = {
    'turnip': {'name': 'Turnip', 'spawn': [24, 56], 'vel': [11.4286, 3.3333], 'r': 9, 'life': 90, 'max': 1,
               'gravity': 0.2667, 'maxFall': 7.5,
               'bounce': {'restitution': 0.42, 'minSpeed': 1.3333, 'friction': 0.7, 'maxBounces': 3,
                          'fizzleLife': 28, 'wallRestitution': 0.4, 'sfx': 'bonk'},
               'hit': turnip_hit, 'spawnSfx': 'swing', 'shapes': turnip_shapes},
    'turnip_strong': {'name': 'Turnip (smash throw)', 'spawn': [24, 56], 'vel': [14.2857, 3.5833], 'r': 9,
                      'life': 90, 'max': 1, 'gravity': 0.2667, 'maxFall': 7.5,
                      'bounce': {'restitution': 0.42, 'minSpeed': 1.3333, 'friction': 0.7, 'maxBounces': 3,
                                 'fizzleLife': 28, 'wallRestitution': 0.4, 'sfx': 'bonk'},
                      'hit': turnip_strong, 'spawnSfx': 'swing', 'shapes': turnip_shapes},
}
# The held turnip (Ultimate item throws): tilt and smash projectiles by strength; Ultimate's thrown-item damage
# `base - 0.4 + speed x 3` (3 per unit/frame = 0.6 per px/tick) is applied relative to each projectile's own
# forward speed, so the forward throws keep 8.3 / 11.4 and other directions scale by their speed. Back, up and down
# use the forward speeds (adapted: the source's per-direction item-throw speeds are not in the pinned data); a drop
# lets go at rest. A connecting turnip bounces back and up (catchable), as in the source.
turnip_item = {
    'name': 'Turnip', 'projectile': 'turnip', 'smashProjectile': 'turnip_strong', 'hold': [24, 56],
    'throws': {
        'forward': {'tilt': [11.4286, 3.3333], 'smash': [14.2857, 3.5833]},
        'back': {'tilt': [11.4286, 3.3333], 'smash': [14.2857, 3.5833]},
        'up': {'tilt': [0, 11.9048], 'smash': [0, 14.7281]},
        'down': {'tilt': [0, -11.9048], 'smash': [0, -14.7281]},
        'drop': [0, 0],
    },
    'speedDamage': 0.6, 'rebound': [2.5, 6], 'life': 600, 'catch': True,
}
spore_shape = [{'shape': 'circle', 'at': [0, 0], 'r': 5.5, 'fill': '#efe6cf', 'stroke': '#b4a98d', 'width': 1.4}]
for key, (vel, spawn) in {'spore_up': ([8.5714, 5.8333], [24, 54]), 'spore_fwd': ([14.2857, 1.25], [26, 46]),
                          'spore_low': ([10.7143, -2.5], [24, 36])}.items():
    projectiles[key] = {'name': 'Spore', 'spawn': spawn, 'vel': vel, 'r': 6, 'life': 16, 'max': 2,
                        'hit': dict(spore_hit), 'spawnSfx': 'toss', 'shapes': spore_shape}

# ---------------------------------------------------------------- presentation
def shapes(*xs):
    return list(xs)


def line(a, b, w, c, **kw):
    return {'shape': 'line', 'from': list(a), 'to': list(b), 'width': w, 'color': c, **kw}


def poly(points, fill=None, stroke=None, width=1.5, **kw):
    s = {'shape': 'poly', 'points': [list(p) for p in points]}
    if fill:
        s['fill'] = fill
    if stroke:
        s['stroke'] = stroke
        s['width'] = width
    s.update(kw)
    return s


def circle(at, r, fill=None, stroke=None, width=1.5, **kw):
    s = {'shape': 'circle', 'at': list(at), 'r': r}
    if fill:
        s['fill'] = fill
    if stroke:
        s['stroke'] = stroke
        s['width'] = width
    s.update(kw)
    return s


attachments = [
    # The dress is drawn behind the legs from the hip, widening to the hem (a fitted silhouette, not a rig).
    {'id': 'dress', 'joint': 'hip', 'angle': -90, 'behind': True, 'shapes': [
        poly(((2, -18), (2, 18), (34, 30), (34, -30)), fill='playerLight', alpha=0.95),
        poly(((30, -20), (30, 20), (36, 30), (36, -30)), fill='playerDark', alpha=0.95),
        poly(((-2, -14), (-2, 14), (10, 18), (10, -18)), fill='playerDark', alpha=0.6),
    ]},
    # Crown: a small gold tiara on the head.
    {'id': 'crown', 'joint': 'head', 'angle': 0, 'shapes': [
        poly(((-8, 10), (-5, 17), (-1, 11), (3, 18), (7, 11), (10, 16), (9, 9), (-8, 9)), fill='#ffd76a',
             stroke='#c79a2e', width=1.2),
        circle((1, 13), 1.6, fill='#ff5f8a'),
    ]},
    # The crown leaves the head during forward air and is struck with the hand.
    {'id': 'crownHand', 'joint': 'handF', 'angle': 0, 'hidden': True, 'shapes': [
        poly(((-9, 6), (-6, 13), (-2, 7), (2, 14), (6, 7), (9, 12), (8, 5), (-9, 5)), fill='#ffd76a',
             stroke='#c79a2e', width=1.2),
    ]},
    # Closed parasol carried on the back; shown opened during Peach Parasol.
    {'id': 'parasol', 'joint': 'handF', 'angle': 0, 'hidden': True, 'shapes': [
        line((0, 0), (0, 34), 3.2, '#d9c8a8'),
        poly(((-24, 34), (-18, 48), (0, 52), (18, 48), (24, 34), (0, 30)), fill='#ff9ecb',
             stroke='#c6558c', width=1.6),
        line((-24, 34), (0, 30), 1.2, '#c6558c'),
        line((24, 34), (0, 30), 1.2, '#c6558c'),
        circle((0, 53), 2.4, fill='#ffd76a'),
        line((24, 34), (30, 26), 2, '#c6558c'),
    ]},
    # Golf club for the imported (un-angled) forward smash.
    {'id': 'club', 'joint': 'handF', 'angle': 0, 'hidden': True, 'shapes': [
        line((-6, 0), (2, 0), 4, '#3a3a44'),
        poly(((2, -7), (8, -5), (8, 5), (2, 7)), fill='#c9ccd6', stroke='#7d8494', width=1.2),
        line((0, 0), (58, 0), 3, '#b9becd'),
        line((58, 0), (68, 0), 4.5, '#9aa1b3'),
    ]},
    # Toad steps in front of her during the counter and the spore burst.
    {'id': 'toad', 'joint': 'hip', 'angle': 0, 'hidden': True, 'shapes': [
        circle((0, 30), 12, fill='#f3e6d0', stroke='#b8a68a', width=1.5),
        poly(((-11, 30), (12, 30), (8, 14), (-6, 14)), fill='#c9543f'),
        circle((-11, 30), 5.5, fill='#e2d0b0'),
        circle((12, 30), 5.5, fill='#e2d0b0'),
        circle((-2, 33), 2.2, fill='#3a2c22'),
        circle((6, 33), 2.2, fill='#3a2c22'),
        circle((2, 24), 2.6, fill='#e07a54'),
    ]},
]

style = {'headRadius': 10.5, 'torsoWidth': 6.5, 'frontLimbWidth': 5.5, 'backLimbWidth': 5, 'glowWidth': 2.2,
         'torsoGlowWidth': 3, 'headGlowRadius': 2.6, 'headRing': 2.4, 'headFill': '#f7dc9a',
         'eye': [2, 0, 4, 2]}

anims = {
    'idle': {'clock': 'world', 'loop': 44.0, 'keys': [
        {'at': 0, 'pose': 'stand'},
        {'at': 22, 'pose': {'from': 'stand', 'head': [2, 66], 'handB': [-17, 40]}, 'lerp': True}]},
    'walk': {'clock': 'world', 'loop': 26.0, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'kneeF': [6, 16], 'footF': [12, 1], 'kneeB': [-6, 16], 'footB': [-12, 1],
                           'handF': [20, 42], 'handB': [-19, 40]}},
        {'at': 13, 'pose': {'from': 'stand', 'kneeF': [-2, 16], 'footF': [-6, 1], 'kneeB': [4, 16], 'footB': [8, 1],
                            'handF': [16, 40], 'handB': [-15, 36]}, 'lerp': True}]},
    'run': {'clock': 'run', 'loop': 360.0, 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'head': [5, 61], 'hip': [1, 30], 'kneeF': [16, 17], 'footF': [26, 6],
                           'kneeB': [-14, 15], 'footB': [-24, 2], 'elbowF': [16, 48], 'handF': [26, 52],
                           'elbowB': [-16, 44], 'handB': [-26, 40]}},
        {'at': 180, 'pose': {'from': 'stand', 'head': [5, 61], 'hip': [1, 30], 'kneeF': [-14, 15], 'footF': [-24, 2],
                             'kneeB': [16, 17], 'footB': [26, 6], 'elbowF': [16, 48], 'handF': [26, 52],
                             'elbowB': [-16, 44], 'handB': [-26, 40]}, 'lerp': True}]},
    'air.rise': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'kneeF': [12, 18], 'footF': [18, 8], 'kneeB': [-8, 16], 'footB': [-12, 10]}}]},
    'air.fall': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'kneeF': [8, 16], 'footF': [10, 2], 'kneeB': [-6, 15], 'footB': [-8, 4]}}]},
    'helpless': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'helpless'}]},
    'landing': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'crouch', 'head': [4, 50]}},
        {'at': 8, 'pose': 'stand', 'lerp': True}]},
    'crouch': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'crouch'}]},
    'jumpsquat': {'clock': 'state', 'keys': [{'at': 0, 'pose': {'from': 'crouch', 'head': [4, 52]}}]},
    'shield': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'stand', 'elbowF': [14, 44], 'handF': [26, 44], 'elbowB': [-14, 44],
                           'handB': [-24, 44]}}]},
    'hitstun': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': {'from': 'air', 'head': [-8, 60], 'handF': [10, 34], 'handB': [-24, 30]}}]},
    'tumble': {'clock': 'spin', 'keys': [{'at': 0, 'pose': {'from': 'air', 'head': [-6, 60]}}]},
    'win': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'win'}, {'at': 20, 'pose': {'from': 'win', 'handF': [28, 70]}, 'lerp': True}]},
    'respawn': {'clock': 'state', 'keys': [{'at': 0, 'pose': 'air'}]},
    # Item throws (the shared `toss` state): the overhand turnip toss for every direction.
    'toss': {'clock': 'state', 'keys': [
        {'at': 0, 'pose': 'holdTurnip'}, {'at': 6, 'pose': 'tossOver', 'lerp': True},
        {'at': 14, 'pose': 'stand', 'lerp': True}]},
}

# ---------------------------------------------------------------- fighter document
doc = {
    '$schema': '../../schema/fighter.schema.json',
    'schemaVersion': 1,
    'extends': 'base',
    'name': 'Duchess',
    'tagline': 'Floating royalty',
    'blurb': 'Hovers on held jumps, drifts wide, and answers pressure with Toad. Plucks turnips, '
             'bombers forward and parachutes down.',
    'stats': {
        'weight': round(100 / 89, 8), 'pummelDamage': 1.3,
        'walkSpeed': round(0.924 * SCALE, 4), 'runSpeed': round(1.595 * SCALE, 4),
        'dashSpeed': round(1.826 * SCALE, 4), 'dashFrames': 16, 'runAccel': round(0.6 * 5 / 3.5, 4),
        'groundFriction': 0.74,
        **units.jumps(*JUMP_HEIGHTS, GRAVITY), 'airJumpDrift': round(1.029 * SCALE, 4), 'airJumps': 1,
        'airSpeed': round(1.029 * SCALE, 4), 'airAccel': AIR_ACCEL,
        'gravity': GRAVITY, 'fallSpeed': round(1.19 * SCALE, 4),
        'fastFallSpeed': round(1.904 * SCALE, 4),
        'traction': round(0.111 * SCALE, 5), 'airFriction': round(0.01 * SCALE, 5),  # Ultimate ground_brake / air_brake_x (fighter_param dump; ssbwiki Traction, Air friction)
        # Peach's float: no vertical movement for up to 2.5 s (param uniq_float_float_frame 150), once per airtime
        # (ssbwiki Floating). Jump with down held floats "just above the ground" (ssbwiki Floating; fitted 2 units).
        'float': {'speed': 0, 'frames': 150, 'groundHeight': 2 * SCALE},
    },
    'ai': {
        'reach': 74, 'counterChance': 0.25,
        'specials': [
            {'minDistance': 140, 'height': 40, 'grounded': True, 'noProjectile': False, 'chance': 0.55,
             'neutral': False, 'down': True, 'hold': None},
            {'minDistance': 70, 'maxDistance': 230, 'height': 55, 'grounded': True, 'noProjectile': True,
             'chance': 0.45, 'neutral': False, 'down': False, 'hold': None},
        ],
    },
    'moves': moves,
    'throws': throws,
    'projectiles': projectiles,
    'items': {'turnip': turnip_item},
    'style': style,
    'poses': poses,
    'anims': anims,
    'attachments': attachments,
}

# ---------------------------------------------------------------- write
out = ROOT / 'assets/fighters/duchess/fighter.json'
out.parent.mkdir(parents=True, exist_ok=True)
out.write_text(json.dumps(doc, indent=2) + '\n')
(SOURCES / 'ledger.json').write_text(json.dumps(ledger, indent=1) + '\n')
print(f'wrote {out.relative_to(ROOT)} ({out.stat().st_size} bytes)')
