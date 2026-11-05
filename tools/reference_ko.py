#!/usr/bin/env python3
"""Smash Ultimate reference launch calculator, in source units (1 unit = 5 px here).

    python3 tools/reference_ko.py [FIGHTER ...] [--all] [--victim mario|W,G,FALL]
    python3 tools/reference_ko.py --compare FILE      # engine rows from `just calibrate`

For a hitbox hitting a victim at a percent on Battlefield centre, with no DI, rage, staleness or 1v1 multiplier:

- knockback KB = ((p/10 + p·d/20) · 200/(w+100) · 1.4 + 18) · kbg/100 + bkb, p = percent after the hit; a fixed
  (weight-based) hit uses p = 10 and d = FKB (Ultimate's WBKB);
- launch speed KB × 0.03 units/frame along the angle; the launch vector shrinks by 0.051 per frame to zero, while
  the victim's own gravity accumulates on a separate vertical speed capped at its fall speed;
- a tumbling launch at 70°–110° uses the launch gravity 0.042 + 0.000336 × weight and fall speed 1.8 while in
  hitstun (`damage_fly_top_*` params); afterwards a fall faster than the fighter's own slows by its gravity per frame;
- each frame: decay the launch, add gravity, then move (the sim's order, also used for jumps in `units.py`);
- hitstun floor(KB × 0.4) × hitstun multiplier; hitlag floor((d × 0.65 + 6) × hitlag multiplier), capped at 30;
- 361 (`sakurai`): 38° in the air; on the ground 0° up to 60 KB, rising linearly to 38° at 88 KB;
- a grounded victim launched downward bounces up at 0.6 of its vertical speed (the sim's rule; unverified);
- KO when crossing x ±240, y above 192 (relative to the floor) before touching the ground, or below −140 while still
  in hitstun (a spike). Falling out after hitstun, or landing, is not a launch KO.

Sources: ssbwiki Knockback, Hitstun, Hitlag, Sakurai angle and Gravity pages and rubendal's SSBU-Calculator
(github.com/rubendal/SSBU-Calculator, js/formulas.js, js/knockback.js); `damage_fly_top_*` from the fighter_param dump
github.com/j99thoms/smash-charts (data/raw/ultimate_fighter_params.csv). Hitboxes come from the resolved fighter JSON (`extends` merged like the loader: objects key by key, arrays whole,
null deletes). Per move the row uses the hitbox that launches hardest at 100%. `--compare` reads the engine's JSON
lines (one measured hit per move: the hitbox, victim, start position and engine numbers) and prints both side by side.
"""

import argparse
import json
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MARIO = {'weight': 98.0, 'gravity': 0.087, 'fall': 1.5, 'fastFall': 2.4}
TOP = {'min': 70.0, 'max': 110.0, 'gravity': 0.042, 'perWeight': 0.000336, 'fall': 1.8}
BLAST = {'side': 240.0, 'top': 192.0, 'bottom': -140.0}
EDGE = 80.0  # Battlefield's main platform half-width
LAUNCH_SPEED, DECAY, HITSTUN, TUMBLE = 0.03, 0.051, 0.4, 80.0
SAKURAI = {'air': 38.0, 'groundLowKb': 60.0, 'groundHighKb': 88.0, 'groundHigh': 38.0}
GROUND_BOUNCE = 0.6
PERCENTS = (0, 50, 100, 150)
KO_CAP = 300
SLOTS = ('jab', 'ftilt', 'utilt', 'dtilt', 'dashatk', 'fsmash', 'usmash', 'dsmash', 'nair', 'fair', 'bair', 'uair',
         'dair', 'zair', 'grab', 'dashgrab', 'nspec', 'sspec', 'uspec', 'dspec')
THROWS = ('f', 'b', 'u', 'd')


def merge(base, over):
    out = dict(base)
    for k, v in over.items():
        if v is None:
            out.pop(k, None)
        elif isinstance(v, dict) and isinstance(out.get(k), dict):
            out[k] = merge(out[k], v)
        else:
            out[k] = v
    return out


def fighter(fid, root=ROOT / 'assets'):
    doc = json.loads((root / 'fighters' / fid / 'fighter.json').read_text())
    parent = doc.get('extends')
    return merge(fighter(parent, root), doc) if parent else doc


def knockback(h, pct, weight):
    """Reference knockback for `pct` before the hit."""
    damage, kbg, bkb = h.get('damage', 0.0), h.get('kbg', 0.0) * 100, h.get('bkb', 0.0)
    if h.get('fkb') is not None:
        p, damage = 10.0, h['fkb']
    elif h.get('setKb'):
        return bkb  # pre-resolved fixed knockback without its source FKB
    else:
        p = pct + damage
    return ((p / 10 + p * damage / 20) * 200 / (weight + 100) * 1.4 + 18) * kbg / 100 + bkb


def angle(h, kb, grounded):
    if grounded and h.get('groundAngle') is not None:
        return h['groundAngle']
    kind = h.get('type', 'normal')
    if kind == 'spike':
        return -abs(h['angle'])
    if kind == 'sakurai':
        if not grounded:
            return SAKURAI['air']
        lo, hi = SAKURAI['groundLowKb'], SAKURAI['groundHighKb']
        return SAKURAI['groundHigh'] * min(max((kb - lo) / (hi - lo), 0.0), 1.0)
    return h.get('angle', 0.0)


def hitlag(h):
    return min(30, math.floor((h.get('damage', 0.0) * 0.65 + 6) * h.get('hitlagMult', 1.0)))


def hitstun(h, kb):
    return math.floor(math.floor(kb * HITSTUN + 1e-9) * h.get('hitstunMult', 1.0) + h.get('hitstunAdd', 0.0))


def launch(h, pct, victim=MARIO, grounded=True, x0=0.0, y0=0.0, direction=1.0):
    """One hit: knockback, hitstun, launch speed, dx at hitstun end, and whether it launch-KOs."""
    kb = knockback(h, pct, victim['weight'])
    deg = angle(h, kb, grounded)
    a = math.radians(deg)
    top = kb > TUMBLE and TOP['min'] <= deg % 360 <= TOP['max']
    lx, ly = math.cos(a) * kb * LAUNCH_SPEED * direction, math.sin(a) * kb * LAUNCH_SPEED
    if grounded and ly < 0:
        ly = -ly * GROUND_BOUNCE
    stun, speed0 = hitstun(h, kb), math.hypot(lx, ly)
    x, y, vy, dx, ko = x0, y0, 0.0, None, False
    for t in range(1, 1200):
        speed = math.hypot(lx, ly)
        lx, ly = (0.0, 0.0) if speed <= DECAY else (lx * (speed - DECAY) / speed, ly * (speed - DECAY) / speed)
        if top and t <= stun:
            vy = max(vy - TOP['gravity'] - TOP['perWeight'] * victim['weight'], -TOP['fall'])
        elif vy < -victim['fall']:
            vy = min(vy + victim['gravity'], -victim['fall'])
        else:
            vy = max(vy - victim['gravity'], -victim['fall'])
        py = y
        x, y = x + lx, y + ly + vy
        if t == stun:
            dx = x - x0
        if abs(x) > BLAST['side'] or y > BLAST['top']:
            ko = True
            break
        if y < BLAST['bottom']:
            ko = t <= stun
            break
        if abs(x) <= EDGE and py >= 0 >= y and ly + vy < 0:
            dx = x - x0 if dx is None else dx
            break
    return {'kb': kb, 'hitstun': stun, 'hitlag': hitlag(h), 'speed': speed0, 'dx': dx,
            'tumble': kb > TUMBLE, 'ko': ko}


def ko_percent(h, victim=MARIO, **kw):
    """Lowest whole percent before the hit (0..KO_CAP) that launch-KOs, or None."""
    if not launch(h, KO_CAP, victim, **kw)['ko']:
        return None
    lo, hi = 0, KO_CAP
    while lo < hi:
        mid = (lo + hi) // 2
        if launch(h, mid, victim, **kw)['ko']:
            hi = mid
        else:
            lo = mid + 1
    return lo


def hitboxes(f):
    """(move, hitbox, grounded victim) for every launching hitbox of the moves and throws."""
    for slot in SLOTS:
        for phase in (f.get('moves', {}).get(slot) or {}).get('phases', []):
            for h in phase.get('hitboxes', []):
                if h.get('type', 'normal') not in ('windbox', 'grab', 'damageOnly', 'autolink'):
                    yield slot, h, h.get('situation') != 'air'
    for k in THROWS:
        t = f.get('throws', {}).get(k)
        if t:
            yield f'{k}throw', t['hit'], False


def row(h, victim, prior=0.0, **kw):
    """Numbers at PERCENTS before the move; `prior` is the move's damage before this hit."""
    hits = [launch(h, p + prior, victim, **kw) for p in PERCENTS]
    ko = ko_percent(h, victim, **kw)
    return {'ko': None if ko is None else max(0, math.ceil(ko - prior)), 'hitstun': [r['hitstun'] for r in hits],
            'speed': [round(r['speed'] * 5, 2) for r in hits], 'dx100': hits[2]['dx'] and round(abs(hits[2]['dx']) * 5, 1)}


def report(fid, victim, every=False):
    best = {}
    for move, h, grounded in hitboxes(fighter(fid)):
        kb = knockback(h, 100, victim['weight'])
        if every:
            print(f'{fid:8} {move:8} {json.dumps(h, separators=(",", ":"))[:80]}')
            print('         ', row(h, victim, grounded=grounded))
        elif move not in best or kb > best[move][0]:
            best[move] = (kb, h, grounded)
    if every:
        return
    print(f'\n{fid}: KO% / hitstun @0/50/100/150 / launch px/tick @0/50/100/150 / dx px @100 (victim {victim})')
    for move, (_, h, grounded) in best.items():
        r = row(h, victim, grounded=grounded)
        print(f'  {move:8} {str(r["ko"]):>4}  {"/".join(map(str, r["hitstun"])):>15}  '
              f'{"/".join(map(str, r["speed"])):>24}  {r["dx100"]}')


def compare(path):
    """Engine rows (JSON lines) against the calculator for the same hitbox, victim and start position."""
    rows = [json.loads(line) for line in Path(path).read_text().splitlines() if line.startswith('{')]
    fmt = lambda v: '–' if v is None else str(v)  # noqa: E731
    worst = {}
    for fid in dict.fromkeys(r['fighter'] for r in rows):
        print(f'\n### {fid}: engine vs reference (KO% ±3, hitstun exact, speed exact)\n')
        print('| Move | KO% eng / ref (Δ) | Hitstun @0/50/100/150 eng | ref | Speed px/t @100 eng / ref | '
              'dx px @100 eng / ref |')
        print('|---|---|---|---|---|---|')
        for r in (r for r in rows if r['fighter'] == fid):
            if r.get('hitbox') is None:
                print(f'| {r["move"]} | {r.get("reason", "no hit")} | | | | |')
                continue
            v, h = r['victim'], r['hitbox']
            kw = {'grounded': r['grounded'], 'x0': r['x0'] / 5, 'y0': r['y0'] / 5, 'direction': r['direction']}
            ref = row(h, v, r.get('prior', 0.0), **kw)
            ref_ko = ref['ko']
            eng_ko = r['ko']
            delta = None if eng_ko is None or ref_ko is None else eng_ko - ref_ko
            stun_d = [a - b for a, b in zip(r['hitstun'], ref['hitstun'])]
            sp_d = round(r['speed'][2] - ref['speed'][2], 2)
            for k, val in (('ko', abs(delta) if delta is not None else None), ('stun', max(map(abs, stun_d))),
                           ('speed', abs(sp_d))):
                if val is not None and val > worst.get(k, (0, ''))[0]:
                    worst[k] = (val, f'{fid} {r["move"]}')
            print(f'| {r["move"]} | {fmt(eng_ko)} / {fmt(ref_ko)} ({fmt(delta)}) | {"/".join(map(str, r["hitstun"]))} | '
                  f'{"/".join(map(str, ref["hitstun"]))} | {r["speed"][2]} / {ref["speed"][2]} | '
                  f'{fmt(r["dx100"] and abs(r["dx100"]))} / {fmt(ref["dx100"])} |')
    print('\nWorst |Δ|:', {k: v for k, v in worst.items()})


def main(argv=None):
    p = argparse.ArgumentParser(description=(__doc__ or '').split('\n\n')[0])
    p.add_argument('fighters', nargs='*')
    p.add_argument('--all', action='store_true', help='every hitbox, not the strongest per move')
    p.add_argument('--victim', default='mario', help='mario, or weight,gravity,fall in source units')
    p.add_argument('--compare', metavar='FILE')
    a = p.parse_args(argv)
    if a.compare:
        return compare(a.compare)
    victim = MARIO if a.victim == 'mario' else dict(zip(('weight', 'gravity', 'fall'), map(float, a.victim.split(','))))
    for fid in a.fighters or ('king', 'duchess', 'stratus'):
        report(fid, victim, a.all)


if __name__ == '__main__':
    sys.exit(main())
