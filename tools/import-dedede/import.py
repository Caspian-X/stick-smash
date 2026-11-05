#!/usr/bin/env python3
"""Rebuild the King package as a King Dedede (SSBU 13.0.1) adaptation.

The package id stays `king` so existing selections, settings and replays keep resolving it.
Facts come from the pinned Rubendal scripts and the UFD page; fitted 2D geometry/animation is
documented in docs/imports/king.md. Run `just import-king` for the generated package.
"""
import copy
import hashlib
import importlib
import json
import math
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
units = importlib.import_module("units")
SOURCES = ROOT / "docs/imports/sources/king"
source = json.loads((SOURCES / "scripts.json").read_text())
ufd = json.loads((SOURCES / "ufd.json").read_text())
scripts = {article: {s["Name"]: s["Data"] for s in entries} for article, entries in source["scripts"].items()}
page = {row["movename"]: row for row in ufd["moves"]}
SCALE = units.SCALE  # source units to px for geometry, px/tick for speeds and px/tick^2 for accelerations
# Source jump heights (units): ssbwiki "Jump" (SSBU jump height ranking), accessed 2026-11-05. The air jump
# is the first of the four midair jumps; the sim uses one airJumpSpeed for all four.
JUMP_HEIGHTS = {"full": 32.85, "short": 16.02, "air": 28.50284665}
SHOULDER = [0.0, 64.0]
HOLD_HAND = 30.0
HEAD_DISTANCE = 50.0  # hammer head centre from the grip
HAMMER_REACH = HOLD_HAND + HEAD_DISTANCE
FRAME = "sourceFrame - 1"  # engine phase frame convention


def expand(s):
    while m := re.search(r"for\((\d+) Iterations\)\{", s):
        pos = m.end()
        depth = 1
        while depth:
            depth += (s[pos] == "{") - (s[pos] == "}")
            pos += 1
        s = s[: m.start()] + expand(s[m.end() : pos - 1]) * int(m[1]) + s[pos:]
    return s


def args(s):
    return {k: v.strip() for k, v in re.findall(r"(\w+)=(.*?)(?=, \w+=|$)", s)}


def num(a, k, default=0):
    try:
        return float(a.get(k, default))
    except ValueError:
        return default


def attacks(article, name):
    """Inclusive source-frame windows for each ATTACK/ATTACK_ABS block."""
    t = 0
    live = {}
    out = []
    raw = scripts[article].get(name, "")

    def clear(i):
        if i in live:
            b = live.pop(i)
            b["end"] = t - 1
            if b["end"] >= b["start"]:
                out.append(b)

    for line in expand(raw).splitlines():
        if m := re.search(r"frame\(Frame=(\d+)\)", line):
            t = int(m[1])
        if m := re.search(r"wait\(Frames=(\d+)\)", line):
            t += int(m[1])
        if "AttackModule::clear_all()" in line:
            for i in list(live):
                clear(i)
        elif m := re.search(r"AttackModule::clear\(ID=(\d+)\)", line):
            clear(int(m[1]))
        if m := re.search(r"\b(ATTACK|ATTACK_ABS|CATCH)\((.*)\)", line):
            a = args(m[2])
            if "ID" not in a:
                continue
            i = int(a["ID"])
            clear(i)
            live[i] = {"start": t, "end": t, "source": a, "kind": m[1]}
        if m := re.search(r"set_add_reaction_frame\(ID=(\d+), Frames=(\d+)", line):
            if int(m[1]) in live:
                live[int(m[1])]["reaction"] = int(m[2])
        if m := re.search(r"ATK_SET_SHIELD_SETOFF_MUL\(ID=(\d+), ShieldstunMul=([\d.]+)", line):
            if int(m[1]) in live:
                live[int(m[1])]["shieldstun"] = float(m[2])
    for i in list(live):
        clear(i)
    return sorted(out, key=lambda b: (b["start"], int(b["source"]["ID"])))


def fixed_bkb(a):
    """Source BKB; a fixed (FKB) hit keeps it next to `fkb` (Ultimate WBKB)."""
    return num(a, "BKB")


def hit(a, start, end, x, y, r=None, frames=None, overrides=None):
    angle = num(a, "Angle")
    kind = "normal"
    if angle == 361:
        kind, angle = "sakurai", 45
    elif angle in (366, 367, 368):
        kind, angle = "autolink", 90
    elif angle == 270:
        kind, angle = "spike", -90
    elif 180 < angle < 360:
        angle -= 360
    b = {
        "start": start,
        "end": end,
        "x": round(x, 4),
        "y": round(y, 4),
        "r": round(r if r is not None else num(a, "Size") * SCALE, 4),
        "damage": num(a, "Damage"),
        "bkb": round(fixed_bkb(a), 4),
        "kbg": round(num(a, "KBG") / 100, 4),
        "angle": angle,
    }
    if kind != "normal":
        b["type"] = kind
    if num(a, "FKB"):
        b["setKb"] = True
        b["fkb"] = num(a, "FKB")
    if a.get("FacingRestrict", "").endswith("_POS"):
        b["dir"] = "away"
    elif a.get("FacingRestrict", "").endswith("_B"):
        b["dir"] = "back"
    mask = a.get("Ground_or_Air", "")
    if mask.endswith("_G"):
        b["situation"] = "ground"
    elif mask.endswith("_A"):
        b["situation"] = "air"
    if num(a, "Rehit"):
        b["rehit"] = int(num(a, "Rehit"))
    if num(a, "Hitlag", 1) != 1:
        b["hitlagMult"] = round(num(a, "Hitlag"), 4)
    if num(a, "ShieldDamage") < 0 and b["damage"] > 0:
        b["shieldDmgMult"] = round(max(0, (b["damage"] + num(a, "ShieldDamage")) / b["damage"]), 4)
    if overrides:
        b.update(overrides)
    if frames:
        b["frames"] = [[round(t, 4), round(fx, 4), round(fy, 4), round(fr, 4)] for t, fx, fy, fr in frames]
    return b


def keyed(keys, t):
    if not keys:
        return 0.0
    if t <= keys[0][0]:
        return keys[0][1]
    for (t0, v0), (t1, v1) in zip(keys, keys[1:]):
        if t <= t1:
            return v0 + (v1 - v0) * (t - t0) / max(1e-9, t1 - t0)
    return keys[-1][1]


def dir_point(angle, distance, shoulder_y=SHOULDER[1]):
    return [distance * math.cos(math.radians(angle)), shoulder_y + distance * math.sin(math.radians(angle))]


def hammer_point(angle, distance=HAMMER_REACH):
    return dir_point(angle, distance)


def merge_windows(windows):
    out = []
    for lo, hi in sorted(windows):
        if out and lo <= out[-1][1] + 1:
            out[-1][1] = max(out[-1][1], hi)
        else:
            out.append([lo, hi])
    return out


def hammer_boxes(raw, keys, slot, skip_zero=True, head_from=None):
    """Collapse source hammer-bone spheres onto the fitted hammer-head track.

    Source hits on hammer1/hammer2/top are kept as separate hits when damage or launch differs.
    Bone-local offsets and capsules are not resolved: the animation is fitted, so each window is
    placed at the visible head path instead of pretending to transform the original skeleton.
    """
    groups = {}
    for b in raw:
        a = b["source"]
        if skip_zero and num(a, "Damage") <= 0:
            continue
        key = (
            num(a, "Damage"),
            num(a, "BKB"),
            num(a, "KBG"),
            num(a, "FKB"),
            num(a, "Angle"),
            a.get("Effect", ""),
            a.get("Ground_or_Air", ""),
            a.get("FacingRestrict", ""),
            int(num(a, "Rehit")),
        )
        groups.setdefault(key, []).append(b)
    out = []
    for _, entries in sorted(
        groups.items(),
        key=lambda kv: (min(e["start"] for e in kv[1]), min(int(e["source"]["ID"]) for e in kv[1])),
    ):
        entries.sort(key=lambda e: (e["start"], int(e["source"]["ID"])))
        a = entries[0]["source"]
        windows = merge_windows((e["start"] - 1, e["end"] - 1) for e in entries)
        for lo, hi in windows:
            if head_from:
                x, y = head_from(lo)
                frames = [(t, *head_from(t)) for t in range(lo, hi + 1)] if hi > lo else None
            else:
                x, y = hammer_point(keyed(keys, lo))
                frames = (
                    [(t, *hammer_point(keyed(keys, t)), max(num(e["source"], "Size") for e in entries) * SCALE)
                     for t in range(lo, hi + 1)]
                    if hi > lo
                    else None
                )
            radius = max(num(e["source"], "Size") for e in entries) * SCALE
            extra = {}
            if any(e.get("reaction") for e in entries):
                extra["hitstunAdd"] = max(e.get("reaction", 0) for e in entries)
            if any(e.get("shieldstun") for e in entries):
                extra["shieldStunMult"] = max(e.get("shieldstun", 1) for e in entries)
            out.append(hit(a, lo, hi, x, y, radius, frames=frames, overrides=extra))
    return out


def cut(boxes, lo, hi):
    out = []
    for b in boxes:
        if b["start"] > hi or b["end"] < lo:
            continue
        c = copy.deepcopy(b)
        c["start"] = max(lo, b["start"]) - lo
        c["end"] = min(hi, b["end"]) - lo
        if c.get("frames"):
            c["frames"] = [[t - lo, x, y, r] for t, x, y, r in c["frames"] if lo <= t <= hi]
            if len(c["frames"]) == 1:
                c["x"], c["y"], c["r"] = c["frames"][0][1:]
                c.pop("frames")
        out.append(c)
    return out


def phase(name, frames, hits=None, **kw):
    p = {"name": name}
    if frames is not None:
        p["frames"] = frames
    if hits:
        p["hitboxes"] = hits
    p.update({k: v for k, v in kw.items() if v is not None})
    return p


def timeout(to):
    return {"on": "timeout", "to": to}


def land(lag, to="landing"):
    return {"on": "land", "to": to, "lag": lag} if to == "landing" else {"on": "land", "to": to}


def anim(pose, start, end, total, air=False, hold_until=None):
    keys = [{"at": 0, "pose": "air" if air else "stand"}]
    keys.append({"at": max(0, start - 2), "pose": pose, "lerp": True})
    keys.append({"at": end, "pose": pose if hold_until is None else hold_until})
    if total > end + 1:
        keys.append({"at": min(total - 1, end + 8), "pose": "air" if air else "stand", "lerp": True})
    return keys


def source_anim(pose, start, end, total, air=False):
    return anim(pose, start, end, total, air)


# -------------------------------------------------------------------------------------------------
# Facts recorded independently of the emitted package.
ledger = {
    "source": {
        "page": ufd["url"],
        "pageSha256": ufd["sha256"],
        "scripts": "docs/imports/sources/king/scripts.json",
        "repository": "https://github.com/rubendal/ssbu",
        "commit": "2e8f0d245481cb0c936524b6ffe1cdea867c4f68",
        "patch": "13.0.1",
        "dataSha256": "a081a4dbc20a4432c670229ce737fca482b217e2417c305bd342e0a6d93fc24a",
        "scriptSha256": hashlib.sha256((SOURCES / "scripts.json").read_bytes()).hexdigest(),
    },
    "units": {
        "frame": FRAME,
        "scale": SCALE,
        "hammerReach": HAMMER_REACH,
    },
    "ufd": page,
    "rawAttacks": {},
    "moves": {},
}

# -------------------------------------------------------------------------------------------------
# Fitted animation / appearance. Coordinates are fighter-local: x forward, y up, feet at 0.
poses = {
    "stand": {
        "head": [0, 84], "shoulder": [0, 64], "hip": [0, 42],
        "elbowF": [18, 59], "handF": [30, 54], "elbowB": [-16, 58], "handB": [-26, 50],
        "kneeF": [8, 20], "footF": [16, 0], "kneeB": [-8, 20], "footB": [-16, 0],
        "pivot": 42,
    },
    "crouch": {
        "from": "stand", "head": [2, 70], "shoulder": [0, 52], "hip": [0, 30],
        "elbowF": [16, 46], "handF": [26, 40], "elbowB": [-14, 46], "handB": [-24, 38],
        "kneeF": [14, 14], "footF": [24, 0], "kneeB": [-12, 14], "footB": [-22, 0],
        "pivot": 32,
    },
    # Two alternating foot contacts (runA/runB, same height) with one pass pose between them, so a
    # heavy waddle never limps on one side.
    "runA": {
        "from": "stand", "head": [6, 81], "shoulder": [4, 61], "hip": [0, 39],
        "elbowF": [20, 57], "handF": [28, 51], "elbowB": [-14, 57], "handB": [-24, 51],
        "kneeF": [16, 16], "footF": [30, 2], "kneeB": [-12, 15], "footB": [-20, 0],
    },
    "runB": {
        "from": "stand", "head": [6, 81], "shoulder": [4, 61], "hip": [0, 39],
        "elbowF": [12, 57], "handF": [20, 55], "elbowB": [-18, 57], "handB": [-28, 55],
        "kneeF": [-8, 18], "footF": [-18, 0], "kneeB": [12, 17], "footB": [22, 4],
    },
    "runPass": {
        "from": "stand", "head": [6, 85], "shoulder": [4, 65], "hip": [0, 43],
        "elbowF": [16, 60], "handF": [26, 56], "elbowB": [-16, 60], "handB": [-26, 56],
        "kneeF": [18, 22], "footF": [12, 16], "kneeB": [-16, 18], "footB": [-22, 4],
    },
    "air": {
        "from": "stand", "head": [2, 82], "shoulder": [0, 62], "hip": [0, 40],
        "elbowF": [20, 62], "handF": [30, 66], "elbowB": [-20, 60], "handB": [-30, 64],
        "kneeF": [14, 28], "footF": [20, 18], "kneeB": [-10, 26], "footB": [-16, 14],
    },
    "airFall": {
        "from": "air", "kneeF": [10, 20], "footF": [14, 6], "kneeB": [-8, 18], "footB": [-12, 4],
    },
    "lean": {
        "from": "stand", "head": [8, 82], "shoulder": [4, 62], "hip": [-2, 40], "pivot": 40,
    },
    "swing": {
        "from": "stand", "head": [-4, 82], "shoulder": [-2, 62], "hip": [0, 40],
        "elbowF": [-14, 58], "handF": [-24, 54], "elbowB": [-20, 56], "handB": [-30, 50],
    },
    "airSwing": {
        "from": "air", "head": [-4, 80], "shoulder": [-2, 60], "hip": [0, 38],
    },
    "hold": {
        "from": "stand", "head": [2, 82], "elbowF": [22, 60], "handF": [38, 58],
        "elbowB": [18, 58], "handB": [34, 56],
    },
    # Fitted jump/landing/charge weights: the takeoff squashes deeper than a crouch, the launch stretches,
    # and the landing absorbs further than any attack stance. Presentation only: no hurt region or rig
    # follows these, and no move phase references them.
    "jumpHalf": {
        "from": "stand", "head": [3, 74], "shoulder": [1, 58], "hip": [0, 34],
        "elbowF": [14, 52], "handF": [24, 46], "elbowB": [-16, 52], "handB": [-24, 48],
        "kneeF": [14, 16], "footF": [22, 0], "kneeB": [-12, 16], "footB": [-22, 0], "pivot": 34,
    },
    "jumpLoad": {
        "from": "crouch", "head": [6, 64], "shoulder": [2, 48], "hip": [0, 26],
        "elbowF": [4, 40], "handF": [0, 28], "elbowB": [-20, 38], "handB": [-30, 28],
        "kneeF": [20, 12], "footF": [30, 0], "kneeB": [-18, 12], "footB": [-30, 0], "pivot": 26,
    },
    "launchReach": {
        "from": "stand", "head": [0, 90], "shoulder": [0, 70], "hip": [0, 46],
        "elbowF": [12, 84], "handF": [16, 102], "elbowB": [-12, 82], "handB": [-18, 100],
        "kneeF": [4, 24], "footF": [0, 6], "kneeB": [-6, 22], "footB": [-10, 4], "pivot": 46,
    },
    "riseTuck": {
        "from": "air", "head": [2, 84], "elbowF": [18, 66], "handF": [26, 76],
        "elbowB": [-18, 64], "handB": [-26, 72], "kneeF": [18, 32], "footF": [26, 22],
        "kneeB": [-14, 28], "footB": [-22, 20],
    },
    "fallBrace": {
        "from": "air", "head": [0, 82], "kneeF": [16, 24], "footF": [28, 10],
        "kneeB": [-14, 22], "footB": [-28, 8], "elbowF": [26, 58], "handF": [40, 52],
        "elbowB": [-28, 56], "handB": [-42, 50],
    },
    "fallSway": {
        "from": "fallBrace", "head": [-2, 81], "handF": [38, 54], "handB": [-40, 52],
        "kneeB": [-16, 24], "footB": [-26, 10],
    },
    "helplessLoose": {
        "from": "airFall", "head": [-4, 80], "elbowF": [8, 54], "handF": [2, 42],
        "elbowB": [-8, 52], "handB": [0, 42], "kneeF": [8, 20], "footF": [4, 6],
        "kneeB": [-8, 18], "footB": [-6, 4],
    },
    "helplessFlail": {
        "from": "helplessLoose", "head": [4, 80], "handF": [10, 44], "handB": [-8, 42],
        "footF": [10, 8], "footB": [-12, 4],
    },
    "landAbsorb": {
        "from": "crouch", "head": [8, 54], "shoulder": [2, 40], "hip": [0, 16],
        "elbowF": [18, 30], "handF": [30, 20], "elbowB": [-16, 30], "handB": [-28, 18],
        "kneeF": [24, 8], "footF": [32, 0], "kneeB": [-20, 8], "footB": [-30, 0], "pivot": 20,
    },
    "landSettle": {
        "from": "crouch", "head": [2, 70], "shoulder": [0, 52], "hip": [0, 30],
        "elbowF": [16, 46], "handF": [26, 40], "elbowB": [-14, 46], "handB": [-24, 38],
        "kneeF": [14, 14], "footF": [24, 0], "kneeB": [-12, 14], "footB": [-22, 0], "pivot": 32,
    },
    # Super Dedede Jump wind-up: the retained GIF's settle frame plants the hammer and leans back before
    # the leap (no hitboxes exist during these phases).
    "uspecWind": {
        "from": "crouch", "head": [-6, 68], "shoulder": [-4, 50], "hip": [0, 30],
        "elbowF": [-10, 44], "handF": [-28, 36], "elbowB": [-18, 42], "handB": [-34, 34],
        "kneeF": [16, 15], "footF": [26, 0], "kneeB": [-14, 15], "footB": [-26, 0], "pivot": 32,
    },
    "uspecWindHold": {"from": "uspecWind", "head": [-8, 67], "hip": [0, 29], "pivot": 31},
    # Jet Hammer carry: a braced stance that keeps the crouch shoulder height, so the keyed hammer angle
    # (and the drawn grip) stays where the release swing expects it.
    "jetBrace": {
        "from": "crouch", "head": [6, 66], "hip": [0, 28],
        "elbowF": [18, 48], "handF": [30, 42], "elbowB": [-12, 46], "handB": [-22, 38],
        "kneeF": [18, 14], "footF": [30, 0], "kneeB": [-16, 14], "footB": [-28, 0], "pivot": 30,
    },
    "jetStrain": {"from": "jetBrace", "head": [8, 64], "hip": [0, 27]},
    "hurt": {
        "from": "stand", "head": [-6, 80], "shoulder": [-4, 60], "hip": [-2, 40],
        "elbowF": [8, 70], "handF": [16, 80], "elbowB": [-16, 68], "handB": [-24, 78],
        "kneeF": [8, 18], "footF": [14, 2], "kneeB": [-8, 18], "footB": [-14, 2],
    },
    "ledge": {
        "from": "stand", "head": [4, 78], "shoulder": [2, 60], "hip": [0, 38],
        "elbowF": [10, 76], "handF": [12, 94], "elbowB": [6, 74], "handB": [8, 92],
        "kneeF": [8, 18], "footF": [6, 2], "kneeB": [-6, 18], "footB": [-10, 2],
    },
    "win": {
        "from": "stand", "head": [0, 86], "shoulder": [0, 66],
        "elbowF": [12, 80], "handF": [18, 100], "elbowB": [-12, 78], "handB": [-18, 98],
    },
    "throwF": {
        "from": "stand", "head": [8, 80], "shoulder": [4, 60], "hip": [0, 40],
        "elbowF": [28, 62], "handF": [48, 58], "elbowB": [22, 60], "handB": [42, 56],
    },
    "throwB": {
        "from": "stand", "head": [-8, 80], "shoulder": [-4, 60], "hip": [0, 40],
        "elbowF": [-8, 62], "handF": [-26, 60], "elbowB": [-14, 60], "handB": [-30, 56],
    },
    "throwU": {
        "from": "stand", "head": [0, 86], "shoulder": [0, 66],
        "elbowF": [18, 78], "handF": [10, 100], "elbowB": [-18, 76], "handB": [-10, 98],
    },
    "throwD": {
        "from": "crouch", "head": [8, 64], "shoulder": [4, 48],
        "elbowF": [22, 36], "handF": [38, 22], "elbowB": [16, 34], "handB": [32, 20],
    },
}

anims = {
    "idle": {
        "clock": "world", "loop": 90,
        "keys": [
            {"at": 0, "pose": "stand"},
            {"at": 22.5, "pose": {"from": "stand", "head": [0, 85.8], "shoulder": [0, 65.8], "hip": [0, 43],
                                  "elbowF": [19, 60], "handF": [32, 55], "elbowB": [-17, 59],
                                  "handB": [-27, 51]}, "lerp": True},
            {"at": 45, "pose": "stand", "lerp": True},
            {"at": 67.5, "pose": {"from": "stand", "head": [-1, 82.4], "shoulder": [-1, 62.4], "hip": [0, 41],
                                    "elbowF": [17, 58], "handF": [28, 53], "elbowB": [-15, 57]}, "lerp": True},
            {"at": 90, "pose": "stand", "lerp": True},
        ],
    },
    "crouch": {
        "clock": "world", "loop": 60,
        "keys": [{"at": 0, "pose": "crouch"}, {"at": 30, "pose": {"from": "crouch", "head": [2, 71]}, "lerp": True},
                 {"at": 60, "pose": "crouch", "lerp": True}],
    },
    "run": {
        "clock": "run", "loop": 360,
        "keys": [{"at": 0, "pose": "runA"}, {"at": 45, "pose": "runPass", "lerp": True},
                 {"at": 90, "pose": "runB", "lerp": True}, {"at": 135, "pose": "runPass", "lerp": True},
                 {"at": 180, "pose": "runA", "lerp": True}, {"at": 225, "pose": "runPass", "lerp": True},
                 {"at": 270, "pose": "runB", "lerp": True}, {"at": 315, "pose": "runPass", "lerp": True},
                 {"at": 360, "pose": "runA", "lerp": True}],
    },
    "jumpsquat": {"keys": [{"at": 0, "pose": "jumpHalf"}, {"at": 2, "pose": "jumpLoad", "lerp": True}]},
    "landing": {"keys": [{"at": 0, "pose": "landAbsorb"}, {"at": 1, "pose": "landSettle", "lerp": True},
                         {"at": 2, "pose": {"from": "stand", "head": [0, 82], "shoulder": [0, 63], "hip": [0, 41]}, "lerp": True},
                         {"at": 10, "pose": "stand", "lerp": True}]},
    "helpless": {"clock": "world", "loop": 40,
                 "keys": [{"at": 0, "pose": "helplessLoose"},
                          {"at": 20, "pose": "helplessFlail", "lerp": True},
                          {"at": 40, "pose": "helplessLoose", "lerp": True}]},
    "shield": {"keys": [{"at": 0, "pose": {"from": "crouch", "elbowF": [18, 56], "handF": [10, 70],
                                         "elbowB": [14, 54], "handB": [6, 68]}}]},
    "spot": {"keys": [{"at": 0, "pose": "crouch"}]},
    "airdodge": {"keys": [{"at": 0, "pose": "air"}]},
    "hold": {"keys": [{"at": 0, "pose": "hold"}]},
    "knockdown": {"keys": [{"at": 0, "pose": {"from": "hurt", "rot": -90, "offsetY": -34}}]},
    "respawn": {"keys": [{"at": 0, "pose": "stand"}]},
    "shieldDrop": {"keys": [{"at": 0, "pose": "crouch"}]},
    "air.rise": {"keys": [{"at": 0, "pose": "launchReach"}, {"at": 4, "pose": "riseTuck", "lerp": True},
                          {"at": 16, "pose": "air", "lerp": True}]},
    "air.fall": {"clock": "world", "loop": 48,
                 "keys": [{"at": 0, "pose": "fallBrace"}, {"at": 24, "pose": "fallSway", "lerp": True},
                          {"at": 48, "pose": "fallBrace", "lerp": True}]},
    "air.flip": {"loop": 20, "keys": [{"at": 0, "pose": {"from": "airFall", "rot": 0}},
                                     {"at": 20, "pose": {"from": "airFall", "rot": 360}}]},
    "roll": {"keys": [{"at": 0, "pose": {"from": "crouch", "rot": -20}}, {"at": 8, "pose": "crouch"}]},
    "pummel": {"keys": [{"at": 0, "pose": "hold"}, {"at": 4, "pose": {"from": "hold", "head": [6, 82]}},
                        {"at": 8, "pose": "hold"}]},
    "throw.f": {"keys": [{"at": 0, "pose": "hold"}, {"at": 8, "pose": "throwF", "lerp": True},
                         {"at": 20, "pose": "stand", "lerp": True}]},
    "throw.b": {"keys": [{"at": 0, "pose": "hold"}, {"at": 9, "pose": "throwB", "lerp": True},
                         {"at": 22, "pose": "stand", "lerp": True}]},
    "throw.u": {"keys": [{"at": 0, "pose": "hold"}, {"at": 10, "pose": "throwU", "lerp": True},
                         {"at": 22, "pose": "stand", "lerp": True}]},
    "throw.d": {"keys": [{"at": 0, "pose": "hold"}, {"at": 14, "pose": "throwD", "lerp": True},
                         {"at": 26, "pose": "stand", "lerp": True}]},
    "grabbed": {"clock": "world", "loop": 12,
                "keys": [{"at": 0, "pose": "hurt"}, {"at": 4, "pose": {"from": "hurt", "head": [-8, 82]}, "lerp": True},
                         {"at": 8, "pose": "hurt", "lerp": True}]},
    "hitstun": {"clock": "world", "loop": 12,
                "keys": [{"at": 0, "pose": {"from": "hurt", "rot": -12}},
                         {"at": 6, "pose": {"from": "hurt", "rot": -18}, "lerp": True},
                         {"at": 12, "pose": {"from": "hurt", "rot": -12}, "lerp": True}]},
    "tumble": {"clock": "spin", "loop": 360,
               "keys": [{"at": 0, "pose": {"from": "airFall", "rot": 0}},
                        {"at": 360, "pose": {"from": "airFall", "rot": 360}}]},
    "shieldbreak": {"clock": "world", "loop": 12,
                    "keys": [{"at": 0, "pose": {"from": "hurt", "rot": 8}},
                             {"at": 6, "pose": {"from": "hurt", "rot": -8}, "lerp": True},
                             {"at": 12, "pose": {"from": "hurt", "rot": 8}, "lerp": True}]},
    "getup": {"keys": [{"at": 0, "pose": {"from": "crouch", "rot": -80, "offsetY": -28}},
                       {"at": 10, "pose": "crouch"}]},
    "getupAttack": {"keys": [{"at": 0, "pose": {"from": "crouch", "rot": -80, "offsetY": -28}},
                             {"at": 12, "pose": "crouch"}]},
    "tech": {"keys": [{"at": 0, "pose": {"from": "crouch", "rot": -50}}, {"at": 8, "pose": "crouch"}]},
    "ledge": {"clock": "world", "loop": 60,
              "keys": [{"at": 0, "pose": "ledge"}, {"at": 30, "pose": {"from": "ledge", "head": [4, 76]}, "lerp": True},
                       {"at": 60, "pose": "ledge", "lerp": True}]},
    "climb": {"keys": [{"at": 0, "pose": "ledge"}, {"at": 10, "pose": "crouch"}]},
    "ledgeAttack": {"keys": [{"at": 0, "pose": "ledge"}, {"at": 10, "pose": "crouch"}]},
    "ledgeRoll": {"keys": [{"at": 0, "pose": "ledge"}, {"at": 10, "pose": "crouch"}]},
    "ledgeJump": {"keys": [{"at": 0, "pose": "ledge"}, {"at": 6, "pose": "air"}]},
    "dizzy": {"clock": "world", "loop": 60,
              "keys": [{"at": 0, "pose": {"from": "stand", "head": [-4, 82]}},
                       {"at": 15, "pose": {"from": "stand", "head": [4, 82]}, "lerp": True},
                       {"at": 30, "pose": {"from": "stand", "head": [4, 84]}, "lerp": True},
                       {"at": 45, "pose": {"from": "stand", "head": [-4, 84]}, "lerp": True},
                       {"at": 60, "pose": {"from": "stand", "head": [-4, 82]}, "lerp": True}]},
    "win": {"clock": "world", "loop": 40,
            "keys": [{"at": 0, "pose": "win"}, {"at": 20, "pose": {"from": "win", "head": [0, 88]}, "lerp": True},
                     {"at": 40, "pose": "win", "lerp": True}]},
    "buried": {"keys": [{"at": 0, "pose": {"from": "crouch", "offsetY": 24}}]},
}

# Attachments use fitted vectors. `hammer` is always held; the head centre is 50 px from the
# grip so the importer can place fitted collision at the visible head.
hammer_shapes = [
    {"shape": "line", "from": [-12, 0], "to": [34, 0], "width": 8, "color": "#6b4a2a"},
    {"shape": "line", "from": [30, -4], "to": [30, 4], "width": 12, "color": "#8a5a2b"},
    {"shape": "rect", "x": 26, "y": -25, "w": 48, "h": 50, "fill": "player", "radius": 8, "alpha": 0.22,
     "when": "noGlow"},
    {"shape": "rect", "x": 26, "y": -25, "w": 48, "h": 50, "fill": "#ff9a3c", "radius": 8, "alpha": 0.42,
     "when": "glow"},
    {"shape": "rect", "x": 29, "y": -21, "w": 42, "h": 42, "fill": "#b9474f", "radius": 7,
     "stroke": "#5c2a2b", "width": 2.5, "when": "noGlow"},
    {"shape": "rect", "x": 29, "y": -21, "w": 42, "h": 42, "fill": "#ffc46a", "radius": 7,
     "stroke": "#5c2a2b", "width": 2.5, "when": "glow"},
    {"shape": "rect", "x": 42, "y": -21, "w": 6, "h": 42, "fill": "#e6c45a"},
    {"shape": "rect", "x": 42, "y": -21, "w": 42, "h": 4, "fill": "#e6c45a"},
    {"shape": "rect", "x": 42, "y": 17, "w": 42, "h": 4, "fill": "#e6c45a"},
]

attachments = [
    {
        "id": "hammer",
        "joint": "handF",
        "angle": 104,
        "hold": {"elbow": 18, "hand": HOLD_HAND, "bend": -18},
        "trail": {"inner": 55, "outer": 110},
        "glow": "jetCharging",
        "shapes": hammer_shapes,
    },
    {
        "id": "crown",
        "joint": "head",
        "shapes": [
            {"shape": "poly", "fill": "#ffd35a",
             "points": [[-12, 8], [-14, 23], [-5, 15], [0, 26], [5, 15], [14, 23], [12, 8]]},
            {"shape": "circle", "at": [0, 12], "r": 2.4, "fill": "#ff5d8a"},
        ],
    },
    {
        "id": "belly",
        "joint": "hip",
        "behind": False,
        "shapes": [
            {"shape": "circle", "at": [0, 8], "r": 27, "fill": "#f7e4b0", "alpha": 0.92,
             "stroke": "playerDark", "width": 2.5},
            {"shape": "poly", "points": [[-18, 12], [18, 12], [21, -6], [-21, -6]],
             "fill": "playerDark", "alpha": 0.38},
        ],
    },
    {
        "id": "beak",
        "joint": "head",
        "shapes": [
            {"shape": "poly", "points": [[10, 0], [25, -2], [12, -7]], "fill": "#f2a63b"},
            {"shape": "circle", "at": [7, 2], "r": 2.2, "fill": "#22252e"},
        ],
    },
    {
        "id": "coat",
        "joint": "hip",
        "behind": True,
        "shapes": [
            {"shape": "poly", "points": [[-15, 8], [15, 8], [22, -34], [-24, -34]],
             "fill": "playerDark", "alpha": 0.72},
            {"shape": "line", "from": [-14, 2], "to": [14, 2], "width": 3, "color": "#e6c45a"},
        ],
    },
    {
        "id": "footF",
        "joint": "footF",
        "shapes": [{"shape": "rect", "x": -5, "y": -3, "w": 16, "h": 7, "fill": "#f2a63b", "radius": 3}],
    },
    {
        "id": "footB",
        "joint": "footB",
        "shapes": [{"shape": "rect", "x": -5, "y": -3, "w": 16, "h": 7, "fill": "#f2a63b", "radius": 3}],
    },
    {
        "id": "gordoHeld",
        "joint": "handF",
        "hidden": True,
        "shapes": [
            {"shape": "circle", "at": [0, 0], "r": 17, "fill": "#d9dbe8", "stroke": "player", "width": 2},
            {"shape": "poly", "fill": "#f4f4ff",
             "points": [[12, 3], [22, 0], [12, -3], [0, 3], [-12, 0], [0, -3]]},
        ],
    },
    {
        "id": "inhale",
        "joint": "head",
        "hidden": True,
        "shapes": [
            {"shape": "poly", "points": [[8, 2], [100, -34], [100, 38], [8, 8]],
             "fill": "#8adfff", "alpha": 0.16, "stroke": "#c6edff", "width": 1.5},
            {"shape": "circle", "at": [72, 2], "r": 22, "fill": "#8adfff", "alpha": 0.12},
        ],
    },
]

# -------------------------------------------------------------------------------------------------
# Normal attack helpers.
def attach(keys):
    return {"hammer": {"angle": keys}}


def hammer_move(slot, name, script, total, keys, pose, charge=None, events=None, transitions=None, head_from=None):
    raw = attacks("dedede", script)
    ledger["rawAttacks"][slot] = {"script": script, "attacks": raw}
    boxes = hammer_boxes(raw, keys, slot, head_from=head_from)
    ev = list(events or [])
    starts = [b["start"] for b in boxes]
    ends = [b["end"] for b in boxes]
    if starts and not ev:
        ev.append({"at": min(starts) - 1, "sfx": "whack"})
    ph = phase("main", total, boxes, attach=attach(keys), events=ev, transitions=transitions)
    if charge is not None:
        ph["charge"] = charge
    phases = [ph]
    if pose is not None:
        ph["anim"] = source_anim(pose, min(starts) if starts else 0, max(ends) if ends else 0, total)
    return {"name": name, "phases": phases}


def body_boxes(raw, slot, point, overrides=None):
    """Source windows with a fitted body point; groups same damage/launch windows."""
    groups = {}
    for b in raw:
        a = b["source"]
        if num(a, "Damage") <= 0:
            continue
        key = (num(a, "Damage"), num(a, "BKB"), num(a, "KBG"), num(a, "FKB"), num(a, "Angle"),
               a.get("Effect", ""), a.get("Ground_or_Air", ""), a.get("FacingRestrict", ""))
        groups.setdefault(key, []).append(b)
    out = []
    for _, entries in sorted(
        groups.items(),
        key=lambda kv: (min(e["start"] for e in kv[1]), min(int(e["source"]["ID"]) for e in kv[1])),
    ):
        a = entries[0]["source"]
        for lo, hi in merge_windows((e["start"] - 1, e["end"] - 1) for e in entries):
            x, y = point(a, lo)
            frames = None
            if hi > lo:
                frames = [(t, *point(a, t), num(a, "Size") * SCALE) for t in range(lo, hi + 1)]
            extra = dict(overrides or {})
            if any(e.get("reaction") for e in entries):
                extra["hitstunAdd"] = max(e.get("reaction", 0) for e in entries)
            out.append(hit(a, lo, hi, x, y, num(a, "Size") * SCALE, frames=frames, overrides=extra))
    return out


def body_move(slot, name, script, total, point, pose, charge=None, events=None, transitions=None, overrides=None):
    raw = attacks("dedede", script)
    ledger["rawAttacks"][slot] = {"script": script, "attacks": raw}
    boxes = body_boxes(raw, slot, point, overrides)
    ev = list(events or [])
    starts = [b["start"] for b in boxes]
    ends = [b["end"] for b in boxes]
    if starts and not ev:
        ev.append({"at": min(starts) - 1, "sfx": "whack"})
    ph = phase("main", total, boxes, events=ev, transitions=transitions)
    if charge is not None:
        ph["charge"] = charge
    if pose is not None:
        ph["anim"] = source_anim(pose, min(starts) if starts else 0, max(ends) if ends else 0, total)
    return {"name": name, "phases": [ph]}, raw, boxes


def uair_point(a, t):
    return hammer_point(keyed(UAIR_KEYS, t))


moves = {}

# Jab: source transitions frame 13 and 22; rapid loop is three ticks.
JAB1_KEYS = [[0, -40], [7, 30], [9, 10], [10, 0], [16, -20], [31, -30]]
JAB2_KEYS = [[0, -30], [8, 35], [10, 10], [11, 5], [18, -10], [26, -25]]
moves["jab"] = hammer_move(
    "jab", "Hammer Jab", "Attack11", 32, JAB1_KEYS, "lean",
    events=[{"at": 9, "sfx": "whack"}],
    transitions=[{"on": "attackHeld", "from": 12, "to": "jab2"}],
)
jab2 = hammer_move(
    "jab2", "Hammer Jab 2", "Attack12", 27, JAB2_KEYS, "lean",
    events=[{"at": 10, "sfx": "whack"}],
    transitions=[{"on": "attackHeld", "from": 21, "to": "rapidStart"}],
)["phases"][0]
rapid_raw = attacks("dedede", "Attack100Sub")
rapid_boxes = [hit(rapid_raw[0]["source"], 0, 2, 70, 65, 35,
                   overrides={"hitstunAdd": 2, "shieldStunMult": 5})]
# A loop phase re-enters itself, clearing contact history every three ticks (source 5/8/11...).
rapid = phase("rapid", 3, rapid_boxes, attach=attach([[0, 100]]), anim=[{"at": 0, "pose": "lean"}],
              transitions=[{"on": "release", "to": "finish"}, timeout("rapid")])
rapid_start = phase("rapidStart", 4, anim=[{"at": 0, "pose": "lean"}], transitions=[timeout("rapid")])
finish_raw = attacks("dedede", "Attack100End")
finish_boxes = [hit(b["source"], b["start"] - 1, b["end"] - 1, 72, 66, 35) for b in finish_raw]
finish = phase("finish", 49, finish_boxes, attach=attach([[0, 100], [3, 70], [20, 40]]),
               events=[{"at": 3, "sfx": "smash"}], anim=source_anim("lean", 3, 4, 49))
for b in moves["jab"]["phases"][0]["hitboxes"]:
    b["start"], b["end"] = 9, 10
moves["jab"]["phases"][0]["name"] = "jab1"
jab2["name"] = "jab2"
moves["jab"]["phases"].extend([jab2, rapid_start, rapid, finish])
ledger["moves"]["jab"] = {"script": ["Attack11", "Attack12", "Attack100Sub", "Attack100End"],
                          "phases": [(p["name"], p.get("frames")) for p in moves["jab"]["phases"]]}

# Tilt/ground hammer moves.
FTILT_KEYS = [[0, 20], [8, 120], [10, 10], [20, 10], [22, 20]]
moves["ftilt"] = hammer_move("ftilt", "Hammer Sweep", "AttackS3", 48, FTILT_KEYS, "lean",
                             events=[{"at": 11, "sfx": "whack"}, {"at": 22, "sfx": "smash"}])
moves["ftilt"]["phases"][0]["events"].append({"at": 11, "setVel": {"x": 1.7143}})
ledger["moves"]["ftilt"] = {"script": "AttackS3", "phases": [("main", 48)]}

UTILT_KEYS = [[0, 70], [6, 80], [13, 80], [24, 50], [37, 40]]
utilt, _, _ = body_move("utilt", "Head Lift", "AttackHi3", 38,
                        lambda a, t: ([5, 88] if num(a, "Damage") > 10 else [11, 86]),
                        "swing", events=[{"at": 6, "sfx": "whack"}])
utilt["phases"][0]["anim"] = source_anim("swing", 6, 13, 38)
moves["utilt"] = utilt
ledger["moves"]["utilt"] = {"script": "AttackHi3", "headIntangible": [6, 12], "phases": [("main", 38)]}

DTILT_KEYS = [[0, 60], [4, 20], [6, 15], [10, 10], [36, 10]]
dtilt, _, _ = body_move("dtilt", "Low Hammer", "AttackLw3", 37,
                        lambda a, t: ([46, 16] if num(a, "Damage") > 8 else [38, 12]),
                        "crouch", events=[{"at": 5, "sfx": "whack"}])
dtilt["phases"][0]["attach"] = attach(DTILT_KEYS)
moves["dtilt"] = dtilt
ledger["moves"]["dtilt"] = {"script": "AttackLw3", "phases": [("main", 37)]}

DASH_KEYS = [[0, 40], [25, 20], [27, 30], [41, 40], [65, 40]]
dash, _, _ = body_move(
    "dashatk", "Body Check", "AttackDash", 66,
    lambda a, t: ([64 - (min(t, 41) - 26) * 1.9, 38 - (min(t, 41) - 26) * 0.75]
                  if num(a, "Damage") > 14 else [52 - (min(t, 41) - 28) * 0.75, 30 - (min(t, 41) - 28) * 0.4]),
    "lean", events=[{"at": 0, "setVel": {"x": 6.4286}}, {"at": 25, "sfx": "whack"}, {"at": 41, "sfx": "whack"}],
)
moves["dashatk"] = dash
ledger["moves"]["dashatk"] = {"script": "AttackDash", "phases": [("main", 66)]}

FSMASH_KEYS = [[0, 30], [20, 155], [31, 165], [37, 80], [39, -35], [42, -30], [59, -25], [76, 20]]
fsmash = hammer_move("fsmash", "Hammer Drop", "AttackS4", 77, FSMASH_KEYS, "lean",
                     charge={"frame": 32}, events=[
                         {"at": 32, "sfx": "charge"},
                         {"at": 39, "sfx": "smash"},
                         {"at": 39, "sfx": "whack"},
                         {"at": 39, "setVel": {"x": 2.8571}},
                         {"at": 43, "if": "grounded", "sfx": "quake",
                          "fx": [{"type": "dust", "x": 72, "y": 0, "n": 12},
                                 {"type": "ring", "x": 72, "y": 5, "r": 10, "grow": 6, "life": 14},
                                 {"type": "shake", "k": 7}]},
                     ])
# Source frame 40 is the close 16% hit; frames 42-43 replace it with 18.5/25; frame 44 is the 11% shockwave.
raw = ledger["rawAttacks"]["fsmash"]["attacks"]
close = hammer_boxes([b for b in raw if num(b["source"], "Damage") == 16], FSMASH_KEYS, "fsmash")
strong = hammer_boxes([b for b in raw if num(b["source"], "Damage") in (18.5, 25)], FSMASH_KEYS, "fsmash")
strong.sort(key=lambda b: -b["damage"])
shock = hammer_boxes([b for b in raw if num(b["source"], "Damage") == 11], FSMASH_KEYS, "fsmash")
for b in shock:
    b.update(x=68.0, y=14.0, r=42.0)
    b.pop("frames", None)
fsmash["phases"][0]["hitboxes"] = close + strong + shock
moves["fsmash"] = fsmash
ledger["moves"]["fsmash"] = {"script": "AttackS4", "chargeHoldFrame": 33,
                             "phases": [("main", 77)],
                             "variants": {"close": 16.0, "handle": 18.5, "head": 25.0, "shockwave": 11.0}}

USMASH_KEYS = [[0, 25], [8, 210], [15, 200], [16, 75], [23, 70], [40, 55], [66, 30]]
moves["usmash"] = hammer_move("usmash", "Overhead Hammer", "AttackHi4", 67, USMASH_KEYS, "crouch",
                              charge={"frame": 5}, events=[{"at": 16, "sfx": "smash"}, {"at": 16, "sfx": "whack"}])
ledger["moves"]["usmash"] = {"script": "AttackHi4", "chargeHoldFrame": 6, "phases": [("main", 67)]}

DSMASH_KEYS = [[0, 60], [8, 110], [12, -30], [16, -25], [17, 160], [21, 200], [38, 200], [51, 40]]
dsmash = hammer_move("dsmash", "Low Sweep", "AttackLw4", 52, DSMASH_KEYS, "crouch",
                     charge={"frame": 2}, events=[{"at": 13, "sfx": "smash"}, {"at": 13, "sfx": "whack"},
                                                  {"at": 13, "if": "grounded", "sfx": "quake",
                                                   "fx": [{"type": "dust", "x": 55, "y": 0, "n": 8},
                                                          {"type": "dust", "x": -55, "y": 0, "n": 8},
                                                          {"type": "ring", "x": 0, "y": 4, "r": 8, "grow": 5, "life": 12},
                                                          {"type": "shake", "k": 6}]}])
# The source exposes front (Z=2) and rear (X=-7) hammer spheres for the whole window.
dsmash["phases"][0]["hitboxes"] = []
dsmash_raw = [b for b in ledger["rawAttacks"]["dsmash"]["attacks"] if num(b["source"], "Damage") == 13]
for a_index, (x, y, r) in enumerate([(70.0, 28.0, 35.0), (-72.0, 34.0, 28.0)]):
    b = dsmash_raw[min(a_index, len(dsmash_raw) - 1)]
    dsmash["phases"][0]["hitboxes"].append(
        hit(b["source"], b["start"] - 1, b["end"] - 1, x, y, r)
    )
moves["dsmash"] = dsmash
ledger["moves"]["dsmash"] = {"script": "AttackLw4", "chargeHoldFrame": 3, "phases": [("main", 52)]}

# Aerials with source autocancel boundaries.
def shift_keys(keys, lo, hi):
    out = [[0.0, keyed(keys, lo)]]
    out.extend([[t - lo, v] for t, v in keys if lo < t <= hi])
    return out


def aerial(slot, name, script, total, lag, ac_before, ac_after, keys, pose, air_pose="air"):
    raw = attacks("dedede", script)
    ledger["rawAttacks"][slot] = {"script": script, "attacks": raw}
    boxes = hammer_boxes(raw, keys, slot)
    boundaries = [0, ac_before, ac_after - 1, total]
    parts = []
    for i, (lo, hi) in enumerate(zip(boundaries, boundaries[1:])):
        if hi <= lo:
            continue
        name_ = ["initial", "active", "late"][i]
        hits = cut(boxes, lo, hi - 1) if name_ == "active" else []
        tr = [land(lag) if name_ == "active" else land(0, "end")]
        if name_ == "active":
            a = [{"at": 0, "pose": air_pose}]
            end = max((b["end"] for b in hits), default=0)
            a.append({"at": min(max(1, end + 4), hi - lo - 1), "pose": pose, "lerp": True})
            if hi - lo - 1 > end + 4:
                a.append({"at": min(hi - lo - 1, end + 10), "pose": air_pose, "lerp": True})
        else:
            a = [{"at": 0, "pose": air_pose}]
        parts.append(phase(name_, hi - lo, hits, attach=attach(shift_keys(keys, lo, hi - 1)), anim=a, transitions=tr))
    for p, nxt in zip(parts, parts[1:]):
        p["transitions"].append(timeout(nxt["name"]))
    move = {"name": name, "air": "aerial", "phases": parts}
    return move, raw, boxes, boundaries


NAIR_KEYS = [[0, 100], [5, 100], [30, 100]]
nair, _, _, _ = aerial("nair", "Belly Spin", "AttackAirN", 39, 9, 2, 35, NAIR_KEYS, "air")
# Body-centred source spheres: 12% early, 7% late.
nair_boxes = [
    hit(ledger["rawAttacks"]["nair"]["attacks"][0]["source"], 6, 7, 0, 52, 50,
        overrides={"type": "normal", "dir": "away"}),
    hit(ledger["rawAttacks"]["nair"]["attacks"][1]["source"], 8, 28, 0, 48, 30,
        overrides={"type": "normal", "dir": "away"}),
]
for p in nair["phases"]:
    p.pop("hitboxes", None)
    p["attach"] = attach(NAIR_KEYS) if p["name"] == "active" else attach(NAIR_KEYS)
for p in nair["phases"]:
    if p["name"] == "active":
        p["hitboxes"] = cut(nair_boxes, 2, 34)
        p["anim"] = [{"at": 0, "pose": "air"}, {"at": 3, "pose": "airSwing", "lerp": True},
                     {"at": 32, "pose": "air", "lerp": True}]
moves["nair"] = nair
ledger["moves"]["nair"] = {"script": "AttackAirN", "landingLag": 9, "autocancel": [[1, 2], [35, 39]],
                           "phases": [(p["name"], p["frames"]) for p in nair["phases"]]}

FAIR_KEYS = [[0, 80], [5, 145], [12, 20], [15, 20], [25, 5], [40, 20]]
fair, _, _, _ = aerial("fair", "Overhead Slam", "AttackAirF", 41, 18, 4, 40, FAIR_KEYS, "airSwing")
moves["fair"] = fair
ledger["moves"]["fair"] = {"script": "AttackAirF", "landingLag": 18, "autocancel": [[1, 4], [40, 41]],
                           "phases": [(p["name"], p["frames"]) for p in fair["phases"]]}

BAIR_KEYS = [[0, 70], [8, 160], [16, 185], [19, 185], [30, 170], [36, 60]]
bair, _, _, _ = aerial("bair", "Rear Hammer", "AttackAirB", 37, 13, 4, 33, BAIR_KEYS, "airSwing")
moves["bair"] = bair
ledger["moves"]["bair"] = {"script": "AttackAirB", "landingLag": 13, "autocancel": [[1, 4], [33, 37]],
                           "phases": [(p["name"], p["frames"]) for p in bair["phases"]]}

UAIR_KEYS = [[0, 100], [8, 220], [9, 320], [24, 1040], [40, 1080]]
uair, _, _, _ = aerial("uair", "Hammer Copter", "AttackAirHi", 44, 13, 4, 42, UAIR_KEYS, "airSwing")
# Source loops seven 1% autolink hits then a 5% finisher.
uair_raw = ledger["rawAttacks"]["uair"]["attacks"]
uair_boxes = [
    hit(uair_raw[0]["source"], 9, 21, *hammer_point(keyed(UAIR_KEYS, 9)), 30,
        frames=[(t, *hammer_point(keyed(UAIR_KEYS, t)), 30) for t in range(9, 22)],
        overrides={"rehit": 2, "type": "autolink", "setKb": True}),
    hit(uair_raw[-1]["source"], 23, 24, *hammer_point(keyed(UAIR_KEYS, 23)), 32,
        frames=[(t, *hammer_point(keyed(UAIR_KEYS, t)), 32) for t in range(23, 25)]),
]
for p in uair["phases"]:
    p.pop("hitboxes", None)
    if p["name"] == "active":
        p["hitboxes"] = cut(uair_boxes, 4, 41)
        p["anim"] = [{"at": 0, "pose": "air"}, {"at": 4, "pose": "airSwing", "lerp": True},
                     {"at": 32, "pose": "air", "lerp": True}]
moves["uair"] = uair
ledger["moves"]["uair"] = {"script": "AttackAirHi", "landingLag": 13, "autocancel": [[1, 4], [42, 44]],
                           "phases": [(p["name"], p["frames"]) for p in uair["phases"]]}

DAIR_KEYS = [[0, 90], [9, 180], [21, -90], [23, -90], [35, -45], [46, 40]]
dair, _, _, _ = aerial("dair", "Meteor Hammer", "AttackAirLw", 47, 18, 6, 44, DAIR_KEYS, "airSwing")
dair_raw = ledger["rawAttacks"]["dair"]["attacks"]
dair_boxes = []
spike = copy.deepcopy(dair_raw[0]["source"])
spike["Angle"] = "270"
spike["Ground_or_Air"] = "COLLISION_SITUATION_MASK_A"
normal = copy.deepcopy(dair_raw[1]["source"])
normal["Ground_or_Air"] = "COLLISION_SITUATION_MASK_G"
late = copy.deepcopy(dair_raw[2]["source"])
dair_boxes.append(hit(spike, 21, 23, *hammer_point(-90), 28,
                      overrides={"type": "spike", "situation": "air"}))
dair_boxes.append(hit(normal, 21, 23, *hammer_point(-90), 30,
                      overrides={"situation": "ground"}))
dair_boxes.append(hit(late, 21, 23, *hammer_point(-90), 42, overrides={"situation": "both"}))
for p in dair["phases"]:
    p.pop("hitboxes", None)
    if p["name"] == "active":
        p["hitboxes"] = cut(dair_boxes, 6, 43)
        p["anim"] = [{"at": 0, "pose": "air"}, {"at": 7, "pose": "airSwing", "lerp": True},
                     {"at": 34, "pose": "air", "lerp": True}]
moves["dair"] = dair
ledger["moves"]["dair"] = {"script": "AttackAirLw", "landingLag": 18, "autocancel": [[1, 6], [44, 47]],
                           "phases": [(p["name"], p["frames"]) for p in dair["phases"]],
                           "active": [22, 24], "sour": "simultaneous coverage; pinned script preferred over UFD Late label"}

# Grabs: fitted capsule endpoints; pivot grab has no destination slot and is omitted.
GRAB_KEYS = [[0, 80], [7, 80], [10, 80]]
grab_raw = attacks("dedede", "Catch")
ledger["rawAttacks"]["grab"] = {"script": "Catch", "attacks": grab_raw}
grab_hits = []
if grab_raw:
    a = grab_raw[0]["source"]
    r = num(a, "Size") * SCALE
    for z in (num(a, "Z"), num(a, "Z2")):
        grab_hits.append(hit(a, 7, 9, z * SCALE, 44, r, overrides={"type": "grab"}))
moves["grab"] = {"name": "Hammer Grab", "phases": [
    phase("main", 39, grab_hits, events=[{"at": 7, "sfx": "grab"}], anim=source_anim("hold", 7, 9, 39))]}
ledger["moves"]["grab"] = {"script": "Catch", "phases": [("main", 39)]}

dashgrab_raw = attacks("dedede", "CatchDash")
ledger["rawAttacks"]["dashgrab"] = {"script": "CatchDash", "attacks": dashgrab_raw}
dash_hits = []
if dashgrab_raw:
    a = dashgrab_raw[0]["source"]
    r = num(a, "Size") * SCALE
    for z in (num(a, "Z"), num(a, "Z2")):
        dash_hits.append(hit(a, 10, 12, z * SCALE, 40, r, overrides={"type": "grab"}))
moves["dashgrab"] = {"name": "Dash Hammer Grab", "phases": [
    phase("main", 47, dash_hits, slide=10, events=[{"at": 0, "setVel": {"x": 7.1429}}, {"at": 10, "sfx": "grab"}],
          anim=source_anim("lean", 10, 12, 47))]}
ledger["moves"]["dashgrab"] = {"script": "CatchDash", "phases": [("main", 47)]}

# Throws: source pre-release hammer ticks plus the release hit.
def throw_from(script, release, total, tick_at=None, tick_damage=0):
    raw = attacks("dedede", script)
    ledger["rawAttacks"]["throw" + script[-1].lower()] = {"script": script, "attacks": raw}
    m = re.search(r"ATTACK_ABS\(Kind=FIGHTER_ATTACK_ABSOLUTE_KIND_THROW,([^)]*)\)", scripts["dedede"][script])
    a = args(m[1]) if m else {"Damage": "0", "BKB": "0", "KBG": "0", "Angle": "0", "FacingRestrict": "F"}
    t = {"release": release, "total": total, "hit": hit(a, 0, 0, 0, 0, 0)}
    if tick_at is not None:
        t["ticks"] = [{"at": tick_at, "damage": tick_damage}]
    return t


throws = {
    "f": throw_from("ThrowF", 14, 37, 12, 4.0),
    "b": throw_from("ThrowB", 19, 41, 16, 4.0),
    "u": throw_from("ThrowHi", 19, 39, 16, 4.0),
    "d": throw_from("ThrowLw", 26, 41),
}
for key, total, release in [("f", 37, 14), ("b", 41, 19), ("u", 39, 19), ("d", 41, 26)]:
    throws[key]["hit"]["x"] = throws[key]["hit"]["y"] = throws[key]["hit"]["r"] = 0
    ledger["moves"]["throw." + key] = {"script": {"f": "ThrowF", "b": "ThrowB", "u": "ThrowHi", "d": "ThrowLw"}[key],
                                       "release": release, "total": total}

# -------------------------------------------------------------------------------------------------
# Inhale: source grab/search volume; the shared hold stands in for swallow/copy.
INHALE_X, INHALE_Y = 44, 60
moves["nspec"] = {
    "name": "Inhale",
    "bReverse": True,
    "phases": [
        phase("start", 16, events=[{"at": 0, "sfx": "swing"}], attach={"inhale": {"show": [0, 15]}},
              anim=[{"at": 0, "pose": "stand"}, {"at": 5, "pose": "lean", "lerp": True}],
              transitions=[timeout("inhale")]),
        phase("inhale", None, [
            hit({"Damage": "0", "BKB": "0", "KBG": "0", "Angle": "0", "Size": "9", "FacingRestrict": "F"},
                0, 999, INHALE_X, INHALE_Y, 45, overrides={"type": "grab", "start": 0, "end": None}),
            hit({"Damage": "0", "BKB": "1.5", "KBG": "0", "Angle": "180", "Size": "10",
                 "Ground_or_Air": "COLLISION_SITUATION_MASK_GA"}, 0, 999, INHALE_X, INHALE_Y, 70,
                overrides={"type": "windbox", "start": 0, "end": None, "bkb": 1.5}),
        ], attach={"inhale": {"show": [0, 999]}}, events=[{"at": 0, "sfx": "swing"}],
            anim=[{"at": 0, "pose": "lean"}],
            transitions=[{"on": "release", "to": "finish"}]),
        phase("finish", 19, anim=[{"at": 0, "pose": "lean"}, {"at": 12, "pose": "stand", "lerp": True}]),
    ],
}
ledger["moves"]["nspec"] = {"script": "SpecialNStart/SpecialNLoop", "startup": 14, "inhaleActive": 17,
                            "totalMinimum": 74, "extendedEndlag": 19,
                            "adaptation": "shared grab hold; swallow, copy and spit not modeled"}

# Gordo Throw: hammer swing then one bouncing/battable Gordo.
GORDO_KEYS = [[0, 20], [20, 145], [28, 70], [30, -20], [45, -20], [57, 20]]
gordo_raw = attacks("dedede", "SpecialSStart")
ledger["rawAttacks"]["sspec"] = {"script": "SpecialSStart", "attacks": gordo_raw}
gordo_hits = []
for b in gordo_raw:
    if num(b["source"], "Damage") > 0:
        gordo_hits.append(hit(b["source"], b["start"] - 1, b["end"] - 1, 55, 52, 37.5))
moves["sspec"] = {
    "name": "Gordo Throw", "bReverse": True,
    "phases": [phase("main", 58, gordo_hits, attach=attach(GORDO_KEYS),
                     events=[{"at": 0, "sfx": "swing"}, {"at": 5, "sfx": "toss"},
                             {"at": 28, "spawn": "gordo", "sfx": "toss"}, {"at": 28, "sfx": "whack"}],
                     anim=[{"at": 0, "pose": "swing"}, {"at": 27, "pose": "lean", "lerp": True},
                           {"at": 40, "pose": "stand", "lerp": True}])],
}
# Show the held Gordo during the wind-up.
moves["sspec"]["phases"][0]["attach"]["gordoHeld"] = {"show": [5, 28], "offset": [[5, 35, 50], [27, 42, 52]]}
moves["sspec"]["phases"][0]["attach"]["hammer"] = {"angle": GORDO_KEYS}
ledger["moves"]["sspec"] = {"script": "SpecialSStart", "phases": [("main", 58)],
                            "gordoThrowFrame": 29, "gordoLifetime": [5, 146]}

# Super Dedede Jump: fitted leap, source armor windows, meteor/landing hits and stars.
USPEC_FALL_HIT = [
    hit({"Damage": "15", "BKB": "60", "KBG": "90", "Angle": "270", "Size": "9.8", "FacingRestrict": "POS",
         "Ground_or_Air": "COLLISION_SITUATION_MASK_G", "Effect": "collision_attr_bury"},
        0, 999, 0, 50, 49, overrides={"type": "bury", "situation": "ground", "start": 0, "end": None}),
    hit({"Damage": "15", "BKB": "60", "KBG": "80", "Angle": "270", "Size": "8.5", "FacingRestrict": "POS",
         "Ground_or_Air": "COLLISION_SITUATION_MASK_A"},
        0, 999, 0, 50, 42, overrides={"type": "spike", "situation": "air", "start": 0, "end": None}),
]
WAVE_HIT = [
    hit({"Damage": "12", "BKB": "105", "KBG": "70", "Angle": "90", "Size": "10", "FacingRestrict": "POS"},
        3, 4, 0, 44, 50, overrides={"damage": 12, "dir": "away"}),
    hit({"Damage": "12", "BKB": "100", "KBG": "82", "Angle": "60", "Size": "11", "FacingRestrict": "POS",
         "Ground_or_Air": "COLLISION_SITUATION_MASK_G"},
        3, 4, 50, 22, 55, overrides={"situation": "ground", "dir": "away"}),
    hit({"Damage": "12", "BKB": "100", "KBG": "82", "Angle": "60", "Size": "11", "FacingRestrict": "POS",
         "Ground_or_Air": "COLLISION_SITUATION_MASK_G"},
        3, 4, -50, 22, 55, overrides={"situation": "ground", "dir": "away"}),
]
moves["uspec"] = {
    "name": "Super Dedede Jump", "air": "upSpecial",
    "phases": [
        phase("squat", 18, events=[{"at": 0, "sfx": "charge"}, {"from": 1, "to": 17, "mulVel": {"x": 0.7}}],
              anim=[{"at": 0, "pose": "crouch"}, {"at": 7, "pose": "uspecWind", "lerp": True},
                    {"at": 13, "pose": "uspecWindHold", "lerp": True}], transitions=[timeout("launch")]),
        phase("launch", 17, armorThreshold=9999,
              events=[{"at": 0, "setVel": {"x": 0, "y": 20.0832}, "leaveGround": True, "invuln": 4, "sfx": "djump",
                       "fx": [{"type": "dust", "x": 0, "y": 0, "n": 10}]}],
              attach=attach([[0, 100]]), anim=[{"at": 0, "pose": "uspecWindHold"}, {"at": 3, "pose": "win", "lerp": True}],
              transitions=[timeout("rise")]),
        phase("rise", None, ledgeGrab=1, attach=attach([[0, 100]]), anim=[{"at": 0, "pose": "win"}],
              events=[{"from": 0, "capFall": 10}], transitions=[{"on": "falling", "to": "plungeArmor"},
                                                                {"on": "land", "to": "wave"}]),
        phase("plungeArmor", 8, USPEC_FALL_HIT, armorThreshold=9999, ledgeGrab=1,
              attach=attach([[0, -90]]), events=[{"at": 0, "sfx": "smash"}],
              anim=[{"at": 0, "pose": "air"}, {"at": 4, "pose": "airSwing", "lerp": True}],
              transitions=[timeout("plunge"), {"on": "shieldPress", "from": 1, "to": "helpless"}, {"on": "land", "to": "wave"}]),
        phase("plunge", None, copy.deepcopy(USPEC_FALL_HIT), ledgeGrab=1, attach=attach([[0, -90]]),
              anim=[{"at": 0, "pose": "airSwing"}],
              transitions=[{"on": "shieldPress", "from": 1, "to": "helpless"}, {"on": "land", "to": "wave"}]),
        phase("wave", 60, WAVE_HIT, attach=attach([[0, 20]]),
              events=[{"at": 0, "sfx": "quake", "fx": [{"type": "ring", "x": 0, "y": 5, "r": 10, "grow": 8, "life": 18},
                                                       {"type": "dust", "x": -35, "y": 0, "n": 10},
                                                       {"type": "dust", "x": 35, "y": 0, "n": 10},
                                                       {"type": "shake", "k": 8}]},
                      {"at": 5, "spawn": "starR", "sfx": "toss"}, {"at": 5, "spawn": "starL", "sfx": "toss"}],
              anim=[{"at": 0, "pose": "crouch"}, {"at": 30, "pose": "stand", "lerp": True}]),
    ],
}
ledger["moves"]["uspec"] = {"script": "SpecialHiStartL/SpecialHiJump/SpecialHiLandingL", "startup": 69,
                            "invuln": [18, 21], "armor": [[22, 34], [69, 76]], "landingLag": [60, 30],
                            "fallingHit": 69, "landingHit": [4, 5], "starHit": [6, 20],
                            "adaptation": "fitted leap speed; cancel enters helpless; 30f cancel lag not separate"}

# Jet Hammer: 17f entry, 136 tick charge, release damage scales 11 -> 29.8, full tier 40.
JET_ENTRY = 17
JET_MAX = 136
JET_CHARGE_DAMAGE = (29.8 / 11.0 - 1.0) / JET_MAX
JET_FULL_RAW = 40.0 / (1.0 + JET_MAX * JET_CHARGE_DAMAGE)
jet_keys = [[0, 20], [8, 150], [10, -30], [12, -30], [13, -30]]
jet_hit = [
    hit({"Damage": str(JET_FULL_RAW), "BKB": "60", "KBG": "46", "Angle": "361", "Size": "9",
         "FacingRestrict": "F"}, 10, 12, *hammer_point(-30), 45,
        overrides={"minCharge": JET_MAX, "type": "sakurai"}),
    hit({"Damage": "11", "BKB": "30", "KBG": "100", "Angle": "361", "Size": "9", "FacingRestrict": "F"},
        10, 12, *hammer_point(-30), 45, overrides={"type": "sakurai"}),
]
# The source hammer1 sphere is 16 units along the handle; a head-centred circle is the documented fit.
moves["dspec"] = {
    "name": "Jet Hammer", "chargeDamage": JET_CHARGE_DAMAGE,
    "phases": [
        phase("start", JET_ENTRY, events=[{"at": 0, "sfx": "jet"}], attach=attach([[0, 100]]),
              anim=[{"at": 0, "pose": "swing"}, {"at": 10, "pose": "crouch", "lerp": True}],
              transitions=[timeout("charge")]),
        phase("charge", None, groundSpeed=2.2857, charge={"frame": 0, "max": JET_MAX, "button": "special", "jet": True},
              # Braced carry: a slow settle into the charge and a strain wobble on the held hammer. The
              # last key returns to the resting 105 degrees the entry animation left it at.
              attach=attach([[0, 105], [12, 102], [24, 108], [36, 102.5], [48, 107], [60, 103.5], [JET_MAX, 105]]),
              anim=[{"at": 0, "pose": "crouch"}, {"at": 10, "pose": "jetBrace", "lerp": True},
                    {"at": 40, "pose": "jetStrain", "lerp": True}],
              transitions=[{"on": "release", "to": "swing"}]),
        phase("swing", 14, jet_hit, armorThreshold=14,
              attach=attach(jet_keys), events=[{"at": 9, "sfx": "smash"}, {"at": 9, "sfx": "whack"}],
              anim=[{"at": 0, "pose": "swing"}, {"at": 10, "pose": "lean", "lerp": True}],
              transitions=[timeout("recovery")]),
        phase("recovery", 45, chargeFrames=10.0 / JET_MAX,
              attach=attach([[0, -30], [16, -30], [44, 20]]),
              anim=[{"at": 0, "pose": "lean"}, {"at": 26, "pose": "stand", "lerp": True}]),
    ],
}
ledger["moves"]["dspec"] = {"script": "SpecialLwStart/SpecialLw/SpecialLwMax", "entryFrames": 17,
                            "chargeTicks": JET_MAX, "partialDamage": [11, 29.8], "fullDamage": 40,
                            "partialActive": [27, 29], "fullActive": [10, 11], "armorPercent": 14,
                            "releaseArmor": [1, 14], "releasePhases": [["swing", 14], ["recovery", 45]],
                            "adaptation": "air 32/22 variants use ground values; armor also applies airborne"}

# -------------------------------------------------------------------------------------------------
# Projectiles: Gordo and the two landing stars.
gordo_shapes = [
    {"shape": "poly", "fill": "#e8e8f0",
     "points": [[12.228, 3.783], [23.2, 0], [12.228, -3.783], [11.321, -5.972], [16.405, -16.405],
                [5.972, -11.321], [3.783, -12.228], [0, -23.2], [-3.783, -12.228], [-5.972, -11.321],
                [-16.405, -16.405], [-11.321, -5.972], [-12.228, -3.783], [-23.2, 0], [-12.228, 3.783],
                [-11.321, 5.972], [-16.405, 16.405], [-5.972, 11.321], [-3.783, 12.228], [0, 23.2],
                [3.783, 12.228], [5.972, 11.321], [16.405, 16.405], [11.321, 5.972]]},
    {"shape": "circle", "at": [0, 0], "r": 22, "fill": "player", "alpha": 0.22},
    {"shape": "circle", "at": [0, 0], "r": 18, "fill": "#4b4567", "stroke": "player", "width": 2.5},
    {"shape": "circle", "at": [0, 0], "r": 4, "fill": "#ffffff", "alpha": 0.8},
]
projectiles = {
    "gordo": {
        "name": "Gordo", "spawn": [52, 60], "vel": [7.8571, 5.8333], "r": 19, "life": 140, "max": 1,
        "hit": {"x": 0, "y": 0, "r": 19, "damage": 14, "bkb": 70, "kbg": 0.66, "angle": 60,
                "shieldDmgMult": 0.5, "hitlagMult": 1.5},
        "spawnSfx": "toss", "fullSfx": "bonk", "gravity": 0.3333, "maxFall": 10, "reflectVy": 3.3333,
        "bounce": {"restitution": 0.7, "minSpeed": 3.3333, "friction": 0.85, "maxBounces": 3,
                   "fizzleLife": 20, "wallRestitution": 0.6, "sfx": "bonk"},
        "bat": {"speed": 14.2857, "vy": 4.1667, "life": 120, "cooldown": 12, "hitlag": 4},
        "spin": -3.438, "shapes": gordo_shapes,
    },
    "starR": {
        "name": "Star", "spawn": [36, 6], "vel": [10, -0.8333], "r": 11, "life": 16, "max": 1,
        "hit": {"x": 0, "y": 0, "r": 11, "damage": 5, "bkb": 8, "kbg": 0.65, "angle": 70},
        "spawnSfx": "toss", "gravity": 0.2083, "maxFall": 6.6667,
        "shapes": [{"shape": "star", "at": [0, 0], "r": 11, "fill": "#ffe680"}],
    },
}
projectiles["starL"] = copy.deepcopy(projectiles["starR"])
projectiles["starL"]["spawn"][0] *= -1
projectiles["starL"]["vel"] = [-10, -0.8333]
projectiles["starL"]["shapes"][0]["at"] = [0, 0]

# -------------------------------------------------------------------------------------------------
# Stats and interactions.
fighter = {
    "$schema": "../../schema/fighter.schema.json", "schemaVersion": 1, "extends": "base",
    "name": "Gordo Ramsay",
    "tagline": "The self-made king of Dream Land",
    "blurb": "Gordo Ramsay: a super-heavy penguin with four midair jumps, a huge hammer, a bouncing Gordo, "
             "armored Super Dedede Jump and a chargeable Jet Hammer. Neutral special inhales; side special "
             "throws a Gordo that any attack can bat back.",
    "stats": {
        "weight": round(100.0 / 127.0, 8),
        "pummelDamage": 1.6,
        "walkSpeed": round(1.029 * SCALE, 5),
        "runSpeed": round(1.496 * SCALE, 5),
        "dashSpeed": round(1.815 * SCALE, 5),
        "dashFrames": 15,
        **units.gait(scripts["dedede"]),  # TurnDash / TurnRun / RunBrakeR flag frames
        "runAccel": 1.0286,  # fitted
        "groundFriction": 0.86,
        **units.jumps(JUMP_HEIGHTS["full"], JUMP_HEIGHTS["short"], JUMP_HEIGHTS["air"], 0.097 * SCALE),
        "airJumpDrift": 5.4286,  # fitted
        "airJumps": 4,
        "airSpeed": round(0.735 * SCALE, 5),
        "airAccel": round(0.05 * SCALE, 5),
        "gravity": round(0.097 * SCALE, 5),
        "fallSpeed": round(1.95 * SCALE, 5),
        "fastFallSpeed": round(3.12 * SCALE, 5),
        "traction": round(0.085 * SCALE, 5), "airFriction": round(0.006 * SCALE, 5),  # Ultimate ground_brake / air_brake_x (fighter_param dump; ssbwiki Traction, Air friction)
    },
    # Arena's lowest platform leaves 76 px of body clearance; use the shared 74 px body while
    # the visual figure and hurtboxes keep Dedede's larger proportions.
    "body": {"halfWidth": 18, "height": 74},
    "hurtboxes": {"default": {"x1": -23, "x2": 23, "y1": 0, "y2": 92},
                  "knockdown": {"x1": -36, "x2": 36, "y1": 0, "y2": 20}},
    "interactions": {
        "grab": {"catchPoint": [0, 20], "holdPoint": [58, 40]},
    },
    "style": {"headRadius": 18, "torsoWidth": 9, "frontLimbWidth": 8, "backLimbWidth": 7,
              "glowWidth": 20, "torsoGlowWidth": 22, "headGlowRadius": 25, "headRing": 5,
              "headFill": "#f4e6b8", "eye": [4, -1, 4, 5]},
    "ai": {
        "reach": 100, "counterChance": 0,
        "specials": [
            {"minDistance": 90, "maxDistance": 220, "height": 60, "chance": 0.18, "grounded": True,
             "noProjectile": True, "neutral": True, "down": False, "hold": [10, 40]},
            {"minDistance": 150, "maxDistance": 500, "height": 110, "chance": 0.3, "grounded": True,
             "noProjectile": True, "down": False, "hold": None},
            {"minDistance": 60, "maxDistance": 160, "height": 50, "chance": 0.14, "grounded": True,
             "noProjectile": True, "down": True, "hold": [20, 70]},
        ],
    },
    "poses": poses, "anims": anims, "attachments": attachments,
    "moves": moves, "throws": throws, "projectiles": projectiles,
}

# -------------------------------------------------------------------------------------------------
# Ledger notes: these are the source facts used by tests, not the generated geometry.
ledger["moves"]["inventory"] = {
    "normals": ["jab (3 stages)", "ftilt", "utilt", "dtilt", "dashatk"],
    "smashes": ["fsmash", "usmash", "dsmash"],
    "aerials": ["nair", "fair", "bair", "uair", "dair"],
    "grabs": ["grab", "dashgrab"], "pivotGrab": "omitted (no destination slot)",
    "throws": ["f", "b", "u", "d"], "pummel": "shared timing, pummelDamage 1.6",
    "specials": {"nspec": "Inhale", "sspec": "Gordo Throw", "uspec": "Super Dedede Jump", "dspec": "Jet Hammer"},
}
ledger["gaps"] = [
    "Inhale captures through the shared grab hold; swallow, copy, spit-star and projectile swallowing are not modeled.",
    "Gordo wall stick/get and age-scaled 14/12.5/11 damage are not modeled; one 14% battable/bouncing Gordo remains.",
    "Super Dedede Jump cancel uses the shared helpless state; the separate 30-frame cancel landing lag is not modeled.",
    "Jet Hammer air release variants use the grounded 40/32 and 29.8/22 values; armor applies in the air as well.",
    "Pivot grab and the 0%-aerial-only FSmash boxes are omitted; all fitted 2D poses and hammer collision are approximations.",
    "Pummel timing is the shared game timing; only damage is imported.",
]

ledger["mobility"] = {
    "source": {"weight": 127, "walk": 1.029, "run": 1.496, "dash": 1.815, "airSpeed": 0.735,
               "airAccel": 0.05, "gravity": 0.097, "fall": 1.95, "fastFall": 3.12,
               "jumpHeight": JUMP_HEIGHTS["full"], "shortHopHeight": JUMP_HEIGHTS["short"],
               "airJumpHeight": JUMP_HEIGHTS["air"], "airJumps": 4},
    "conversion": {"scale": SCALE,
                   "jumpSpeed": "units.jumps(source heights x scale, source gravity x scale); supersedes the fitted 13.5/9.4/13"},
    "result": fighter["stats"],
}

# -------------------------------------------------------------------------------------------------
out = ROOT / "assets/fighters/king/fighter.json"
out.write_text(json.dumps(fighter, indent=2) + "\n")
(SOURCES / "ledger.json").write_text(json.dumps(ledger, indent=1) + "\n")
print(f"Rebuilt {out} ({out.stat().st_size} bytes); run `just bless` to format and validate.")
