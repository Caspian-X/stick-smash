#!/usr/bin/env python3
"""Rebuild the Piranha Plant package from pinned SSBU 13.0.1 sources.

Facts come from the retained UFD page rows and Rubendal scripts. The 2D rig, pose fitting,
hitbox joint anchors and fitted motion are authoring/runtime adaptations documented in
docs/imports/plant.md. Run `just import-plant` for the generated package.
"""
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
SOURCES = ROOT / "docs/imports/sources/plant"
raw = json.loads((SOURCES / "scripts.json").read_text())
ufd = json.loads((SOURCES / "ufd.json").read_text())
scripts = raw["scripts"]
page = {row["movename"]: row for row in ufd["moves"]}
SCALE = units.SCALE  # source units to px for geometry, px/tick for speeds and px/tick^2 for accelerations
# UFD Misc Info (https://ultimateframedata.com/piranha_plant, accessed 2026-11-05), per frame.
MOBILITY = {"walk": 0.76, "run": 1.72, "dash": 1.82, "air": 1.0, "airAccel": 0.06,
            "gravity": 0.14, "fall": 1.95, "fastFall": 2.73}
# Jump heights (units): ssbwiki "Jump" (SSBU jump height ranking), accessed 2026-11-05.
JUMP_HEIGHTS = {"full": 37.4, "short": 17.5, "air": 38.52}
FRAME = "sourceFrame - 1"  # engine phase frame convention

# Source script offsets are bone-local. `top` is Plant's root bone: its offsets are already feet-relative
# (`Z` forward, `Y` up), so binding them to an animated head joint would add the head translation twice.
# `mouth`/`potc`/leaves are animated rig joints: their offsets stay in that joint's rotated frame.
# `virtualhit*` anchors are animated virtual bones with no numeric offset; the fitted rig has no equivalent,
# so they follow the head (documented fitted presentation). None of those scripts use the lip bones.
# A bone without a numeric axis falls back to the root with no offset, never silently at a joint.
BONE_JOINT = {
    "top": (None, "z"),
    "mouth": ("mouth", "x"),
    "potc": ("pot", "x"),
    "arml": ("leafL", "x"),
    "armr": ("leafR", "x"),
    "virtualhit1": ("head", "v"),
    "virtualhit2": ("head", "v"),
    "virtualhit3": ("head", "v"),
    "lipu3": ("head", "v"),
    "lipd3": ("head", "v"),
}


def jp(t=None, a=None):
    d = {}
    if t is not None:
        d["translation"] = t
    if a is not None:
        d["angle"] = a
    return d


def pose(joints=None, rot=None, pivot=None, offset_y=None):
    d = {}
    if joints:
        d["joints"] = joints
    if rot is not None:
        d["rot"] = rot
    if pivot is not None:
        d["pivot"] = pivot
    if offset_y is not None:
        d["offsetY"] = offset_y
    return d


def key(at, p, lerp=False, cond=None, min_charge=None):
    d = {"at": at, "pose": p}
    if lerp:
        d["lerp"] = True
    if cond:
        d["if"] = cond
    if min_charge:
        d["minCharge"] = min_charge
    return d


def anim(keys, clock=None, loop=None):
    d = {}
    if clock:
        d["clock"] = clock
    if loop is not None:
        d["loop"] = loop
    d["keys"] = keys
    return d


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
    """Inclusive source-frame windows for each ATTACK/ATTACK_ABS/CATCH block."""
    t = 0
    live = {}
    out = []
    raw_script = scripts[article].get(name, "")

    def clear(i):
        if i in live:
            b = live.pop(i)
            b["end"] = t - 1
            if b["end"] >= b["start"]:
                out.append(b)

    for line in expand(raw_script).splitlines():
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
        b = live.pop(i)
        b["end"] = max(t, b["start"])
        out.append(b)
    return sorted(out, key=lambda b: (b["start"], int(b["source"]["ID"])))


def fixed_bkb(a):
    if num(a, "FKB"):
        return ((1 + num(a, "FKB") / 2) * 1.4 + 18) * num(a, "KBG") / 100 + num(a, "BKB")
    return num(a, "BKB")


def bone_name(a):
    m = re.search(r'hash40\("([^"]+)"\)', a.get("Bone", ""))
    return m.group(1) if m else "top"


def offset(a, axis, suffix=""):
    if axis == "z":
        return [num(a, "Z" + suffix) * SCALE, num(a, "Y" + suffix) * SCALE]
    if axis == "x":
        return [num(a, "X" + suffix) * SCALE, num(a, "Y" + suffix) * SCALE]
    return [0.0, 0.0]


def bound(bone, a, suffix=""):
    """One script point: feet-relative for the `top` root bone, else the named joint's local frame."""
    joint, axis = BONE_JOINT.get(bone, (None, "v"))
    off = offset(a, axis, suffix)
    point = {"offset": [round(off[0], 4), round(off[1], 4)]}
    if joint:
        point["joint"] = joint
    return point


def shaped(bone, a, radius):
    """Circle, or a source capsule when X2/Y2/Z2 carry a second point."""
    second = any(a.get(k) not in (None, "LUA_VOID") for k in ("X2", "Y2", "Z2"))
    if second:
        p, q = bound(bone, a), bound(bone, a, "2")
        if p["offset"] == q["offset"]:
            return {"kind": "circle", "center": p, "radius": radius}
        return {"kind": "capsule", "a": p, "b": q, "radius": radius}
    return {"kind": "circle", "center": bound(bone, a), "radius": radius}


def hit(a, start, end, overrides=None):
    angle = num(a, "Angle")
    kind = None
    if angle == 361:
        kind, angle = "sakurai", 45
    elif angle in (366, 367, 368):
        kind, angle = "autolink", 90
    elif angle == 270:
        kind, angle = "spike", -90
    elif 180 < angle < 360:
        angle -= 360
    bone = bone_name(a)
    r = round(num(a, "Size") * SCALE, 4)
    b = {
        "start": start,
        "end": end,
        "x": 0.0,
        "y": 0.0,
        "r": r,
        "damage": num(a, "Damage"),
        "bkb": round(fixed_bkb(a), 4),
        "kbg": round(num(a, "KBG") / 100, 4),
        "angle": angle,
    }
    if kind:
        b["type"] = kind
    if num(a, "FKB"):
        b["setKb"] = True
    fr = a.get("FacingRestrict", "")
    if fr.endswith("_POS"):
        b["dir"] = "away"
    elif fr.endswith("_B"):
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
    if overrides:
        b.update(overrides)
    # Source offsets are converted in the bone's own frame (`top` = fighter root).
    if a.get("Bone"):
        b["geometry"] = shaped(bone, a, r)
    return b


def hits_from(script, entry=1, overrides=None, keep=None):
    out = []
    for b in attacks("packun", script):
        if keep and not keep(b):
            continue
        start = max(b["start"] - entry, 0)
        end = b["end"] - entry
        if end < start:
            continue
        h = hit(b["source"], start, end)
        if overrides:
            h.update(overrides(b) or {})
        out.append(h)
    return out


def phase(name, frames=None, **kw):
    names = {
        "charge_frames": "chargeFrames",
        "strong": "strong",
        "armor_threshold": "armorThreshold",
        "hurt_regions": "hurtRegions",
        "ground_speed": "groundSpeed",
        "travel_speed": "travelSpeed",
        "air_accel_mult": "airAccelMult",
        "fall_cap": "fallCap",
        "ledge_grab": "ledgeGrab",
        "pass_platforms": "passPlatforms",
    }
    p = {"name": name}
    if frames is not None:
        p["frames"] = frames
    for k, v in kw.items():
        if v in (None, [], {}):
            continue
        p[names.get(k, k)] = v
    return p


# ---------------------------------------------------------------------------
# Rig and appearance
# ---------------------------------------------------------------------------
def leaf(points, mid, veins, fill="#6ddd58"):
    shapes = [{"shape": "poly", "points": points, "fill": fill, "stroke": "#348f3d", "width": 1.5},
              {"shape": "line", "from": mid[0], "to": mid[1], "width": 1.2, "color": "#246d31"}]
    shapes.extend({"shape": "line", "from": a, "to": b, "width": 0.8, "color": "#2d7937"} for a, b in veins)
    return shapes


rig = {
    "joints": [
        {
            "id": "pot", "translation": [0, 0],
            "shapes": [
                {"shape": "poly", "points": [[-15, 0], [15, 0], [20, 26], [-20, 26]], "fill": "#b45e40", "stroke": "#633528", "width": 2},
                {"shape": "line", "from": [-12, 4], "to": [-15, 21], "width": 1.5, "color": "#d88965", "alpha": 0.8},
                {"shape": "poly", "points": [[-20, 21], [20, 21], [21, 29], [-21, 29]], "fill": "#c77350", "stroke": "#633528", "width": 2},
                {"shape": "poly", "points": [[-20, 28], [-16, 30], [-8, 31], [8, 31], [16, 30], [20, 28], [16, 26], [8, 25], [-8, 25], [-16, 26]], "fill": "#493429", "stroke": "#633528", "width": 1.5},
                {"shape": "line", "from": [-17, 23], "to": [17, 23], "width": 1, "color": "#e39870", "alpha": 0.7},
            ],
        },
        {"id": "stem0", "parent": "pot", "translation": [0, 24]},
        {"id": "stem1", "parent": "stem0", "translation": [0, 24]},
        {"id": "stem2", "parent": "stem1", "translation": [0, 20]},
        {"id": "neck", "parent": "stem2", "translation": [0, 14]},
        {
            "id": "head", "parent": "neck", "translation": [0, 18],
            "shapes": leaf([[0, -6], [-8, 0], [-10, 8], [-8, 15], [-4, 20], [0, 22], [5, 19], [9, 13], [9, 6], [6, 0]],
                [[0, -5], [0, 20]], [[[0, 2], [-7, 7]], [[0, 6], [7, 11]], [[0, 11], [-6, 15]], [[0, 15], [4, 18]]], "#79eb62")
                + leaf([[0, -5], [-8, -6], [-16, -3], [-22, 3], [-24, 8], [-17, 9], [-10, 7], [-4, 2]],
                [[-1, -4], [-22, 7]], [[[-7, -1], [-12, -2]], [[-11, 1], [-15, 5]], [[-15, 3], [-19, 4]]], "#5bd65b"),
        },
        {
            "id": "jaw", "parent": "head", "translation": [18, -8], "angle": -6,
            "shapes": leaf([[-18, 4], [-12, 2], [-4, 2], [3, 5], [7, 10], [1, 13], [-6, 13], [-13, 10]],
                [[-17, 5], [5, 10]], [[[-11, 6], [-7, 3]], [[-6, 7], [-3, 11]], [[-2, 8], [1, 6]]], "#91f270"),
        },
        {"id": "mouth", "parent": "head", "translation": [22, 0]},
        {"id": "top", "parent": "head", "translation": [0, 16]},
        {"id": "leafBase", "parent": "stem2", "translation": [0, 4]},
        {
            "id": "leafL", "parent": "leafBase", "translation": [-2, 2], "angle": 150,
            "shapes": leaf([[0, 0], [5, 5], [13, 7], [21, 5], [27, 0], [21, -4], [13, -6], [5, -4]],
                [[0, 0], [25, 0]], [[[8, 0], [12, 4]], [[13, 0], [17, -3]], [[18, 0], [21, 3]]]),
        },
        {
            "id": "leafR", "parent": "leafBase", "translation": [2, 2], "angle": 30,
            "shapes": leaf([[0, 0], [5, 5], [13, 7], [21, 5], [27, 0], [21, -4], [13, -6], [5, -4]],
                [[0, 0], [25, 0]], [[[8, 0], [12, 4]], [[13, 0], [17, -3]], [[18, 0], [21, 3]]]),
        },
    ],
    "bones": [
        {"from": "stem0", "to": "stem1", "width": 6, "color": "#48ac48"},
        {"from": "stem1", "to": "stem2", "width": 6, "color": "#48ac48"},
        {"from": "stem2", "to": "neck", "width": 5, "color": "#48ac48"},
        {"from": "neck", "to": "head", "width": 5, "color": "#48ac48"},
        {"from": "leafBase", "to": "leafL", "width": 3, "color": "#48ac48"},
        {"from": "leafBase", "to": "leafR", "width": 3, "color": "#48ac48"},
    ],
}

# The jaw joint carries the forward leaf; its existing motion fans the crown open.
poses = {
    "stand": pose(),
    "crouch": pose(
        {
            "stem0": jp([0, 18]),
            "stem1": jp([0, 18]),
            "stem2": jp([0, 14]),
            "neck": jp([0, 10]),
            "head": jp([2, 14], -8),
            "leafL": jp(a=128),
            "leafR": jp(a=52),
            "jaw": jp(a=-3),
        }
    ),
    "coil": pose(
        {
            "stem0": jp(a=16),
            "stem1": jp(a=14),
            "stem2": jp(a=10),
            "neck": jp(a=8),
            "head": jp([-6, 16], 14),
            "jaw": jp(a=-34),
            "leafL": jp(a=168),
            "leafR": jp(a=12),
        }
    ),
    "strike": pose(
        {
            "stem0": jp([0, 25], -22),
            "stem1": jp([0, 25], -20),
            "stem2": jp([0, 22], -15),
            "neck": jp([0, 16], -8),
            "head": jp([10, 18], -12),
            "jaw": jp(a=-38),
            "leafL": jp(a=112),
            "leafR": jp(a=68),
        }
    ),
    "bite": pose(
        {
            "stem0": jp([0, 25], -22),
            "stem1": jp([0, 25], -20),
            "stem2": jp([0, 22], -15),
            "neck": jp([0, 16], -8),
            "head": jp([10, 18], -10),
            "jaw": jp(a=-2),
            "leafL": jp(a=112),
            "leafR": jp(a=68),
        }
    ),
    # Low bites: the source jab/tilt boxes sit near the pot and ground (`Y` about 4-8 units), so the fitted
    # stem dips until the jaw meets them instead of keeping the head at its tall stand height.
    "lowStrike": pose(
        {
            "stem0": jp([0, 22], -26),
            "stem1": jp([0, 21], -22),
            "stem2": jp([0, 17], -16),
            "neck": jp([0, 11], -6),
            "head": jp([8, 14], -8),
            "jaw": jp(a=-30),
            "leafL": jp(a=124),
            "leafR": jp(a=56),
        }
    ),
    "lowSweep": pose(
        {
            "stem0": jp([0, 22], -74),
            "stem1": jp([0, 20], -8),
            "stem2": jp([0, 16], 8),
            "neck": jp([0, 10], 8),
            "head": jp([10, 14], 4),
            "jaw": jp(a=-30),
            "leafL": jp(a=104),
            "leafR": jp(a=76),
        }
    ),
    "upStrike": pose(
        {
            "stem0": jp([0, 26], -6),
            "stem1": jp([0, 26], -4),
            "stem2": jp([0, 22], -2),
            "neck": jp([0, 16]),
            "head": jp([4, 22], -18),
            "jaw": jp(a=-34),
            "leafL": jp(a=170),
            "leafR": jp(a=10),
        }
    ),
    "lowSweepBack": pose(
        {
            "stem0": jp([0, 22], 74),
            "stem1": jp([0, 20], 8),
            "stem2": jp([0, 16], -8),
            "neck": jp([0, 10], -8),
            "head": jp([-10, 14], -62),
            "jaw": jp(a=-30),
            "leafL": jp(a=76),
            "leafR": jp(a=104),
        }
    ),
    "airCoil": pose(
        {
            "stem0": jp(a=10),
            "stem1": jp(a=8),
            "stem2": jp(a=6),
            "head": jp([-4, 16], 10),
            "jaw": jp(a=-30),
            "leafL": jp(a=174),
            "leafR": jp(a=6),
        }
    ),
    "airStrike": pose(
        {
            "stem0": jp([0, 24], -16),
            "stem1": jp([0, 24], -14),
            "stem2": jp([0, 20], -10),
            "head": jp([8, 18], -8),
            "jaw": jp(a=-36),
            "leafL": jp(a=112),
            "leafR": jp(a=68),
        }
    ),
    "charge": pose(
        {
            "stem0": jp([0, 20], 10),
            "stem1": jp([0, 18], 8),
            "stem2": jp([0, 14], 6),
            "neck": jp([0, 10], 2),
            "head": jp([-10, 12], 12),
            "jaw": jp(a=-2),
            "leafL": jp(a=176),
            "leafR": jp(a=4),
        }
    ),
    "grabOpen": pose(
        {
            "stem0": jp(a=-25),
            "stem1": jp(a=-18),
            "stem2": jp(a=-10),
            "neck": jp(a=-5),
            "head": jp([10, 16], -14),
            "jaw": jp(a=-38),
            "leafL": jp(a=120),
            "leafR": jp(a=60),
        }
    ),
    "grabHold": pose(
        {
            "stem0": jp(a=-20),
            "stem1": jp(a=-14),
            "stem2": jp(a=-8),
            "neck": jp(a=-4),
            "head": jp([10, 16], -10),
            "jaw": jp(a=-3),
            "leafL": jp(a=126),
            "leafR": jp(a=54),
        }
    ),
    "throwWind": pose({"head": jp([-8, 18], 12), "jaw": jp(a=-30), "leafL": jp(a=160), "leafR": jp(a=20)}),
    "throwHeave": pose({"head": jp([16, 18], -16), "jaw": jp(a=-34), "leafL": jp(a=110), "leafR": jp(a=70)}),
    "ledgeHang": pose(
        {
            "stem0": jp([0, 22], -4),
            "stem1": jp([0, 22], -4),
            "stem2": jp([0, 18], -4),
            "neck": jp([0, 12], -2),
            "head": jp([2, 16], 4),
            "jaw": jp(a=-6),
            "leafL": jp(a=144),
            "leafR": jp(a=36),
        }
    ),
    "guard": pose(
        {
            "stem0": jp([0, 20], 18),
            "stem1": jp([0, 18], 16),
            "stem2": jp([0, 14], 12),
            "neck": jp([0, 10], 8),
            "head": jp([-8, 14], 14),
            "jaw": jp(a=-2),
            "leafL": jp(a=196),
            "leafR": jp(a=-16),
        }
    ),
    "win": pose({"head": jp([4, 18], -10), "jaw": jp(a=-28), "leafL": jp(a=126), "leafR": jp(a=54), "stem0": jp(a=-4)}),
}

# Long-Stem Strike reach tiers. Stem lengths/angles are fitted to the reference's extended silhouette:
# the head reaches roughly three body heights at full charge and returns near pot-top height. Each tier has
# its own named pose so the mouth-anchored hitbox and the stem/head hurt regions follow the same tick.
LSS_ANGLES = (-78, -10, 0, 4)
LSS_TIERS = [(0, (70, 65, 45), 8.4), (15, (90, 85, 60), 9.3), (30, (110, 105, 75), 10.2), (45, (145, 140, 100), 12.0)]
for _tier, (_l1, _l2, _l3), _dmg in LSS_TIERS:
    _stem = {
        "stem0": jp([0, 24], LSS_ANGLES[0]),
        "stem1": jp([0, _l1], LSS_ANGLES[1]),
        "stem2": jp([0, _l2], LSS_ANGLES[2]),
        "neck": jp([0, _l3], LSS_ANGLES[3]),
        "leafL": jp(a=100),
        "leafR": jp(a=80),
    }
    poses[f"stretch{_tier}"] = pose({**_stem, "head": jp([18, 18], 0), "jaw": jp(a=-38)})
    poses[f"stretchedBite{_tier}"] = pose({**_stem, "head": jp([18, 18], -8), "jaw": jp(a=-2)})


# ---------------------------------------------------------------------------
# State animations
# ---------------------------------------------------------------------------
idle = anim(
    [key(0, "stand"), key(24, {"joints": {"head": jp([1, 18], -2), "stem1": jp([0, 25])}}, True), key(48, "stand", True), key(72, {"joints": {"head": jp([-1, 19], 2), "leafL": jp(a=156), "leafR": jp(a=24)}}, True), key(96, "stand", True)],
    "world",
    96,
)
walk = anim(
    [
        key(0, "stand"),
        key(8, {"joints": {"stem0": jp(a=-5), "stem1": jp(a=-3), "head": jp([3, 18], -2), "leafL": jp(a=146), "leafR": jp(a=34)}}, True),
        key(16, "stand", True),
        key(24, {"joints": {"stem0": jp(a=5), "stem1": jp(a=3), "head": jp([-2, 18], 2), "leafL": jp(a=154), "leafR": jp(a=26)}}, True),
        key(32, "stand", True),
    ],
    "state",
    32,
)
run = anim(
    [
        key(0, {"joints": {"stem0": jp(a=-12), "stem1": jp(a=-8), "stem2": jp(a=-4), "head": jp([5, 17], -4), "leafL": jp(a=136), "leafR": jp(a=44)}}),
        key(6, {"joints": {"stem0": jp(a=-14), "stem1": jp(a=-10), "stem2": jp(a=-5), "head": jp([6, 16], -5), "leafL": jp(a=132), "leafR": jp(a=48), "jaw": jp(a=-12)}}, True),
        key(12, {"joints": {"stem0": jp(a=-10), "stem1": jp(a=-6), "stem2": jp(a=-3), "head": jp([3, 18], -2), "leafL": jp(a=144), "leafR": jp(a=36)}}, True),
        key(18, {"joints": {"stem0": jp(a=-13), "stem1": jp(a=-9), "stem2": jp(a=-5), "head": jp([5, 16], -5), "leafL": jp(a=134), "leafR": jp(a=46)}}, True),
        key(24, {"joints": {"stem0": jp(a=-12), "stem1": jp(a=-8), "stem2": jp(a=-4), "head": jp([5, 17], -4), "leafL": jp(a=136), "leafR": jp(a=44)}}, True),
    ],
    "state",
    24,
)
crawl = anim(
    [
        key(0, {"joints": {"stem0": jp([0, 18], -14), "stem1": jp([0, 17], -10), "stem2": jp([0, 13], -6), "neck": jp([0, 9], -3), "head": jp([7, 13], -10), "leafL": jp(a=120), "leafR": jp(a=60)}}),
        key(8, {"joints": {"stem0": jp([0, 18], -18), "stem1": jp([0, 17], -12), "stem2": jp([0, 13], -7), "neck": jp([0, 9], -4), "head": jp([9, 12], -12), "leafL": jp(a=116), "leafR": jp(a=64)}}, True),
        key(16, {"joints": {"stem0": jp([0, 18], -14), "stem1": jp([0, 17], -10), "stem2": jp([0, 13], -6), "neck": jp([0, 9], -3), "head": jp([7, 13], -10), "leafL": jp(a=120), "leafR": jp(a=60)}}, True),
    ],
    "state",
    16,
)
turn = anim([key(0, "stand"), key(4, {"joints": {"stem0": jp(a=10), "stem1": jp(a=8), "head": jp([-4, 18], 6), "leafL": jp(a=164), "leafR": jp(a=16)}}, True), key(10, "stand", True)], "state", 10)
brake = anim([key(0, {"joints": {"stem0": jp(a=-12), "stem1": jp(a=-8), "head": jp([4, 17], -4), "leafL": jp(a=136), "leafR": jp(a=44)}}), key(5, {"joints": {"stem0": jp(a=-4), "stem1": jp(a=-2), "head": jp([2, 18], -2)}}, True), key(10, "stand", True)], "state", 10)
jumpsquat = anim(
    [
        key(0, {"joints": {"stem0": jp([0, 20]), "stem1": jp([0, 19]), "stem2": jp([0, 15]), "neck": jp([0, 11]), "head": jp([2, 15], -4), "leafL": jp(a=136), "leafR": jp(a=44)}}),
        key(1, {"joints": {"stem0": jp([0, 16]), "stem1": jp([0, 15]), "stem2": jp([0, 12]), "neck": jp([0, 8]), "head": jp([3, 11], -6), "leafL": jp(a=126), "leafR": jp(a=54)}}),
        key(2, {"joints": {"stem0": jp([0, 27], -4), "stem1": jp([0, 27], -2), "stem2": jp([0, 22]), "neck": jp([0, 16]), "head": jp([0, 20], 0), "leafL": jp(a=186), "leafR": jp(a=-6)}}),
    ]
)
landing = anim(
    [
        key(0, {"joints": {"stem0": jp([0, 14]), "stem1": jp([0, 13]), "stem2": jp([0, 10]), "neck": jp([0, 7]), "head": jp([4, 9], -8), "leafL": jp(a=116), "leafR": jp(a=64)}}),
        key(1, {"joints": {"stem0": jp([0, 16]), "stem1": jp([0, 15]), "stem2": jp([0, 12]), "neck": jp([0, 9]), "head": jp([3, 12], -6), "leafL": jp(a=122), "leafR": jp(a=58)}}, True),
        key(2, {"joints": {"stem0": jp([0, 21]), "stem1": jp([0, 21]), "stem2": jp([0, 17]), "neck": jp([0, 12]), "head": jp([1, 15], -2), "leafL": jp(a=144), "leafR": jp(a=36)}}, True),
    ]
)
air_rise = anim(
    [
        key(0, {"joints": {"stem0": jp([0, 28], -4), "stem1": jp([0, 28], -2), "stem2": jp([0, 23]), "neck": jp([0, 17]), "head": jp([0, 21]), "leafL": jp(a=188), "leafR": jp(a=-8), "jaw": jp(a=-20)}}),
        key(4, {"joints": {"stem0": jp([0, 25], -2), "stem1": jp([0, 25]), "stem2": jp([0, 21]), "neck": jp([0, 15]), "head": jp([-1, 19], 2), "leafL": jp(a=172), "leafR": jp(a=8)}}, True),
    ]
)
air_fall = anim(
    [
        key(0, {"joints": {"stem0": jp([0, 24], 4), "stem1": jp([0, 24], 2), "stem2": jp([0, 20]), "head": jp([-2, 18], 2), "leafL": jp(a=164), "leafR": jp(a=16)}}),
        key(10, {"joints": {"stem0": jp([0, 24], 7), "stem1": jp([0, 24], 4), "stem2": jp([0, 20]), "head": jp([-3, 18], 4), "leafL": jp(a=158), "leafR": jp(a=22)}}, True),
        key(20, {"joints": {"stem0": jp([0, 24], 3), "stem1": jp([0, 24], 1), "stem2": jp([0, 20]), "head": jp([-1, 18], 1), "leafL": jp(a=166), "leafR": jp(a=14)}}, True),
        key(30, {"joints": {"stem0": jp([0, 24], 4), "stem1": jp([0, 24], 2), "stem2": jp([0, 20]), "head": jp([-2, 18], 2), "leafL": jp(a=164), "leafR": jp(a=16)}}, True),
    ],
    "world",
    30,
)
air_flip = anim(
    [
        key(0, {"joints": {"stem0": jp([0, 18], -10), "stem1": jp([0, 18], -8), "stem2": jp([0, 14], -4), "head": jp([4, 12], -8), "leafL": jp(a=110), "leafR": jp(a=70)}}),
        key(9, {"joints": {"stem0": jp([0, 20], -16), "stem1": jp([0, 20], -12), "stem2": jp([0, 16], -6), "head": jp([6, 13], -12), "leafL": jp(a=100), "leafR": jp(a=80)}}, True),
        key(19, {"joints": {"stem0": jp([0, 22], -4), "stem1": jp([0, 22], -2), "stem2": jp([0, 18]), "head": jp([1, 16], -2), "leafL": jp(a=152), "leafR": jp(a=28)}}, True),
    ]
)
helpless = anim(
    [
        key(0, {"joints": {"stem0": jp(a=4), "stem1": jp(a=2), "head": jp([-2, 18], 4), "jaw": jp(a=-18), "leafL": jp(a=162), "leafR": jp(a=18)}}),
        key(20, {"joints": {"stem0": jp(a=8), "stem1": jp(a=4), "head": jp([-4, 18], 7), "jaw": jp(a=-10), "leafL": jp(a=152), "leafR": jp(a=28)}}, True),
        key(40, {"joints": {"stem0": jp(a=1), "stem1": jp(a=0), "head": jp([0, 18], 1), "jaw": jp(a=-22), "leafL": jp(a=166), "leafR": jp(a=14)}}, True),
        key(60, {"joints": {"stem0": jp(a=4), "stem1": jp(a=2), "head": jp([-2, 18], 4), "jaw": jp(a=-18), "leafL": jp(a=162), "leafR": jp(a=18)}}, True),
    ],
    "world",
    60,
)
hitstun = anim(
    [
        key(0, {"joints": {"stem0": jp(a=22), "stem1": jp(a=18), "stem2": jp(a=13), "neck": jp(a=9), "head": jp([-9, 16], 20), "jaw": jp(a=-30), "leafL": jp(a=180), "leafR": jp(a=0)}}),
        key(2, {"joints": {"stem0": jp(a=20), "stem1": jp(a=16), "stem2": jp(a=12), "neck": jp(a=8), "head": jp([-8, 16], 18), "jaw": jp(a=-28), "leafL": jp(a=176), "leafR": jp(a=4)}}, True),
        key(5, {"joints": {"stem0": jp(a=14), "stem1": jp(a=11), "stem2": jp(a=8), "neck": jp(a=6), "head": jp([-5, 17], 12), "jaw": jp(a=-20), "leafL": jp(a=166), "leafR": jp(a=14)}}, True),
        key(12, {"joints": {"stem0": jp(a=6), "stem1": jp(a=4), "head": jp([-2, 18], 5), "jaw": jp(a=-10), "leafL": jp(a=154), "leafR": jp(a=26)}}, True),
        key(24, "stand", True),
    ]
)
tumble = anim(
    [
        key(0, {"joints": {"stem0": jp([0, 18]), "stem1": jp([0, 18]), "stem2": jp([0, 14]), "head": jp([4, 12], -8), "leafL": jp(a=110), "leafR": jp(a=70)}}),
        key(90, {"joints": {"stem0": jp([0, 18]), "stem1": jp([0, 18]), "stem2": jp([0, 14]), "head": jp([4, 12], -8), "leafL": jp(a=110), "leafR": jp(a=70)}, "rot": 90}, True),
        key(180, {"joints": {"stem0": jp([0, 18]), "stem1": jp([0, 18]), "stem2": jp([0, 14]), "head": jp([4, 12], -8), "leafL": jp(a=110), "leafR": jp(a=70)}, "rot": 180}, True),
        key(270, {"joints": {"stem0": jp([0, 18]), "stem1": jp([0, 18]), "stem2": jp([0, 14]), "head": jp([4, 12], -8), "leafL": jp(a=110), "leafR": jp(a=70)}, "rot": 270}, True),
        key(360, {"joints": {"stem0": jp([0, 18]), "stem1": jp([0, 18]), "stem2": jp([0, 14]), "head": jp([4, 12], -8), "leafL": jp(a=110), "leafR": jp(a=70)}, "rot": 360}, True),
    ],
    "spin",
    360,
)
shield = anim([key(0, "guard"), key(12, {"joints": {"stem0": jp([0, 19], 20), "head": jp([-9, 13], 16), "jaw": jp(a=-4), "leafL": jp(a=200), "leafR": jp(a=-20)}}), key(24, "guard", True)], "state", 24)
shield_drop = anim([key(0, "guard"), key(1, {"joints": {"stem0": jp([0, 20], 10), "stem1": jp([0, 19], 8), "head": jp([-4, 15], 8), "leafL": jp(a=176), "leafR": jp(a=4)}}, True), key(2, "stand", True)])
roll = anim([key(0, "crouch"), key(6, {"joints": {"stem0": jp([0, 16], -14), "head": jp([4, 12], -6), "leafL": jp(a=120), "leafR": jp(a=60)}, "rot": 90, "pivot": 30}, True), key(12, {"joints": {"stem0": jp([0, 16], -14), "head": jp([4, 12], -6), "leafL": jp(a=120), "leafR": jp(a=60)}, "rot": 180, "pivot": 30}, True), key(20, {"joints": {"stem0": jp([0, 16], -14), "head": jp([4, 12], -6), "leafL": jp(a=120), "leafR": jp(a=60)}, "rot": 270, "pivot": 30}, True), key(28, "stand", True)])
spot = anim([key(0, {"joints": {"stem0": jp([0, 14], 18), "stem1": jp([0, 13], 12), "head": jp([-4, 10], 12), "leafL": jp(a=190), "leafR": jp(a=-10)}}), key(12, {"joints": {"stem0": jp([0, 16], 14), "stem1": jp([0, 15], 10), "head": jp([-2, 12], 10), "leafL": jp(a=184), "leafR": jp(a=-4)}}, True), key(24, "stand", True)])
airdodge = anim([key(0, {"joints": {"stem0": jp([0, 16]), "stem1": jp([0, 16]), "stem2": jp([0, 13]), "head": jp([3, 10], -6), "leafL": jp(a=116), "leafR": jp(a=64)}}), key(3, {"joints": {"stem0": jp([0, 14]), "stem1": jp([0, 14]), "stem2": jp([0, 11]), "head": jp([5, 8], -8), "leafL": jp(a=106), "leafR": jp(a=74)}}, True), key(24, {"joints": {"stem0": jp([0, 14]), "stem1": jp([0, 14]), "stem2": jp([0, 11]), "head": jp([5, 8], -8), "leafL": jp(a=106), "leafR": jp(a=74)}}), key(29, "stand", True)])
knockdown = anim([key(0, {"joints": {"stem0": jp([0, 10], 60), "stem1": jp([0, 10], 50), "head": jp([10, 8], 12), "jaw": jp(a=-14), "leafL": jp(a=100), "leafR": jp(a=80)}, "pivot": 10})])
getup = anim([key(0, {"joints": {"stem0": jp([0, 10], 60), "stem1": jp([0, 10], 50), "head": jp([10, 8], 12)}, "pivot": 10}), key(10, {"joints": {"stem0": jp([0, 20], 20), "stem1": jp([0, 18], 14), "head": jp([4, 14], 8)}}, True), key(20, "stand", True)])
getup_attack = anim([key(0, {"joints": {"stem0": jp([0, 10], 60), "head": jp([10, 8], 12)}}), key(8, {"joints": {"stem0": jp([0, 24], -20), "stem1": jp([0, 22], -14), "head": jp([10, 16], -10), "jaw": jp(a=-34)}}, True), key(20, "stand", True)])
tech = anim([key(0, {"joints": {"stem0": jp([0, 14]), "stem1": jp([0, 14]), "head": jp([4, 10], -6), "leafL": jp(a=116), "leafR": jp(a=64)}}), key(4, "stand", True)])
ledge = anim([key(0, "ledgeHang"), key(20, {"joints": {"stem0": jp([0, 22], -2), "head": jp([2, 16], 2), "leafL": jp(a=148), "leafR": jp(a=32)}}, True), key(40, "ledgeHang", True)], "world", 40)
climb = anim([key(0, "ledgeHang"), key(10, {"joints": {"stem0": jp([0, 20], -10), "stem1": jp([0, 18], -8), "head": jp([6, 14], -4)}}, True), key(20, "stand", True)])
ledge_attack = anim([key(0, "ledgeHang"), key(8, {"joints": {"stem0": jp([0, 24], -24), "stem1": jp([0, 22], -18), "head": jp([12, 16], -12), "jaw": jp(a=-36)}}, True), key(16, "strike", True), key(24, "stand", True)])
ledge_roll = anim([key(0, "ledgeHang"), key(8, {"joints": {"stem0": jp([0, 18], -12), "head": jp([4, 12], -6), "leafL": jp(a=120), "leafR": jp(a=60)}}, True), key(20, "stand", True)])
ledge_jump = anim([key(0, "ledgeHang"), key(4, {"joints": {"stem0": jp([0, 26], -4), "head": jp([0, 20]), "leafL": jp(a=184), "leafR": jp(a=-4)}}, True)])
dizzy = anim([key(0, {"joints": {"stem0": jp(a=6), "stem1": jp(a=4), "head": jp([-4, 18], 8), "jaw": jp(a=-14), "leafL": jp(a=160), "leafR": jp(a=20)}}), key(20, {"joints": {"stem0": jp(a=-4), "stem1": jp(a=-2), "head": jp([4, 18], -6), "jaw": jp(a=-18), "leafL": jp(a=148), "leafR": jp(a=32)}}, True), key(40, {"joints": {"stem0": jp(a=6), "stem1": jp(a=4), "head": jp([-4, 18], 8), "jaw": jp(a=-14), "leafL": jp(a=160), "leafR": jp(a=20)}}, True)], "world", 40)
win = anim([key(0, "win"), key(20, {"joints": {"head": jp([5, 19], -12), "jaw": jp(a=-36), "leafL": jp(a=120), "leafR": jp(a=60)}}, True), key(40, "win", True)], "world", 40)
respawn = anim([key(0, {"joints": {"stem0": jp([0, 18]), "head": jp([0, 14])}}), key(8, "stand", True)])
hold = anim([key(0, "grabHold")])
pummel = anim([key(0, "grabHold"), key(3, {"joints": {"head": jp([10, 18], 2), "jaw": jp(a=-6)}}), key(5, "grabOpen"), key(8, "grabOpen"), key(11, "grabHold")])
throw_f = anim([key(0, "grabHold"), key(6, "throwWind"), key(10, "throwHeave"), key(16, "strike", True)])
throw_b = anim([key(0, "grabHold"), key(8, "throwWind"), key(14, {"joints": {"head": jp([-10, 18], 14), "jaw": jp(a=-32), "leafL": jp(a=176), "leafR": jp(a=4)}}), key(22, "strike", True)])
throw_u = anim([key(0, "grabHold"), key(8, "charge"), key(14, {"joints": {"stem0": jp([0, 26], -2), "head": jp([0, 22], -4), "jaw": jp(a=-36), "leafL": jp(a=184), "leafR": jp(a=-4)}}, True), key(24, "strike", True)])
throw_d = anim([key(0, "grabHold"), key(8, "charge"), key(16, {"joints": {"stem0": jp([0, 18], 10), "head": jp([-6, 12], 10), "jaw": jp(a=-20)}}), key(26, {"joints": {"stem0": jp([0, 16], -14), "stem1": jp([0, 15], -10), "head": jp([8, 10], -10), "jaw": jp(a=-4)}}, True)])
grabbed = anim([key(0, {"joints": {"stem0": jp(a=-6), "stem1": jp(a=-4), "head": jp([-4, 16], 8), "jaw": jp(a=-12), "leafL": jp(a=166), "leafR": jp(a=14)}}), key(8, {"joints": {"stem0": jp(a=-10), "stem1": jp(a=-6), "head": jp([-6, 16], 12), "jaw": jp(a=-6), "leafL": jp(a=172), "leafR": jp(a=8)}}, True), key(16, {"joints": {"stem0": jp(a=-6), "stem1": jp(a=-4), "head": jp([-4, 16], 8), "jaw": jp(a=-12), "leafL": jp(a=166), "leafR": jp(a=14)}}, True)], "world", 16)
buried = anim([key(0, {"joints": {"stem0": jp([0, 8]), "stem1": jp([0, 8]), "stem2": jp([0, 6]), "head": jp([0, 6], 0), "leafL": jp(a=170), "leafR": jp(a=10)}})])
shieldbreak = anim([key(0, {"joints": {"stem0": jp(a=28), "stem1": jp(a=24), "stem2": jp(a=18), "head": jp([-8, 14], 24), "jaw": jp(a=-40), "leafL": jp(a=200), "leafR": jp(a=-20)}}), key(20, {"joints": {"stem0": jp(a=24), "stem1": jp(a=20), "head": jp([-6, 14], 20), "jaw": jp(a=-30), "leafL": jp(a=190), "leafR": jp(a=-10)}}, True)], "world", 60)

anims = {
    "idle": idle,
    "walk": walk,
    "run": run,
    "crawl": crawl,
    "turn": turn,
    "brake": brake,
    "crouch": anim([key(0, "crouch"), key(30, {"joints": {"head": jp([4, 14], -10), "leafL": jp(a=122), "leafR": jp(a=58)}}, True), key(60, "crouch", True)], "world", 60),
    "jumpsquat": jumpsquat,
    "landing": landing,
    "air.rise": air_rise,
    "air.fall": air_fall,
    "air.flip": air_flip,
    "helpless": helpless,
    "hitstun": hitstun,
    "tumble": tumble,
    "shield": shield,
    "shieldDrop": shield_drop,
    "roll": roll,
    "spot": spot,
    "airdodge": airdodge,
    "knockdown": knockdown,
    "getup": getup,
    "getupAttack": getup_attack,
    "tech": tech,
    "ledge": ledge,
    "climb": climb,
    "ledgeAttack": ledge_attack,
    "ledgeRoll": ledge_roll,
    "ledgeJump": ledge_jump,
    "dizzy": dizzy,
    "win": win,
    "respawn": respawn,
    "hold": hold,
    "pummel": pummel,
    "throw.f": throw_f,
    "throw.b": throw_b,
    "throw.u": throw_u,
    "throw.d": throw_d,
    "grabbed": grabbed,
    "buried": buried,
    "shieldbreak": shieldbreak,
}


def move_anim(frames, contact_start, contact_end, entry="coil", contact="strike", follow="strike"):
    out = [key(0, entry)]
    if contact_start > 2:
        out.append(key(max(contact_start - 2, 1), "coil", True))
    out.append(key(contact_start, contact, True))
    if contact_end > contact_start:
        out.append(key(contact_end, contact))
    after = contact_end + 2
    if after < frames:
        out.append(key(after, follow))
    if frames - 4 > after:
        out.append(key(frames - 4, "stand", True))
    if frames - 1 > max(after, frames - 4):
        out.append(key(frames - 1, "stand"))
    return out


def move_anim_air(frames, contact_start, contact_end, entry="airCoil", contact="airStrike", follow="airStrike"):
    return move_anim(frames, contact_start, contact_end, entry, contact, follow)


# ---------------------------------------------------------------------------
# Moves
# ---------------------------------------------------------------------------
moves = {}

# Jab: source Attack11/12/13 then Attack100Start/Sub/End.
# Jab: source Attack11/12/13 then Attack100Start/Sub/End. UFD totals are 19/20/25; the continuation
# windows are source frame 5 into jab 2 and source frame 7 into jab 3 (jab 2's earliest held continuation
# per UFD), then the loop starts after jab 3's active window. Releasing early keeps the full recovery.
moves["jab"] = {
    "name": "Jab",
    "phases": [
        phase("jab1", 19, hitboxes=hits_from("Attack11"), anim=move_anim(19, 1, 2, "coil", "lowStrike"), transitions=[{"on": "attackHeld", "from": 4, "to": "jab2"}]),
        phase("jab2", 20, hitboxes=hits_from("Attack12"), anim=move_anim(20, 1, 2, "coil", "lowStrike"), transitions=[{"on": "attackHeld", "from": 6, "to": "jab3"}]),
        phase("jab3", 25, hitboxes=hits_from("Attack13"), anim=move_anim(25, 6, 7, "coil", "lowStrike"), transitions=[{"on": "attackHeld", "from": 8, "to": "rapid"}]),
        phase("rapid", 4, hitboxes=hits_from("Attack100Sub", entry=0), anim=[key(0, "lowStrike"), key(3, "coil")], transitions=[{"on": "timeout", "to": "rapid"}, {"on": "release", "to": "rapidEnd"}]),
        phase("rapidEnd", 40, hitboxes=hits_from("Attack100End"), anim=move_anim(40, 5, 6, "coil", "lowStrike", "bite")),
    ],
}

# Forward tilt: two source variants. A fresh attack press chains into the second bite.
moves["ftilt"] = {
    "name": "Forward Tilt",
    "phases": [
        phase("first", 27, hitboxes=hits_from("AttackS3"), anim=move_anim(27, 6, 7, "coil", "strike"), transitions=[{"on": "attackPress", "from": 11, "to": "second"}]),
        phase("second", 31, hitboxes=hits_from("AttackS32"), anim=move_anim(31, 4, 5, "coil", "strike")),
    ],
}

moves["utilt"] = {
    "name": "Up Tilt",
    "phases": [
        # UFD: head intangibility on source frames 6-13 (engine 5-12).
        phase(
            "main",
            26,
            hitboxes=hits_from("AttackHi3"),
            hurt_regions=[{"region": "head", "enabled": False, "from": 5, "to": 12}],
            anim=move_anim(26, 7, 14, "crouch", "upStrike"),
        )
    ],
}
moves["dtilt"] = {
    "name": "Down Tilt",
    "phases": [phase("main", 26, hitboxes=hits_from("AttackLw3"), anim=move_anim(26, 8, 10, "crouch", "lowSweep"))],
}
moves["dashatk"] = {
    "name": "Dash Attack",
    "phases": [
        phase(
            "main",
            47,
            hitboxes=hits_from("AttackDash"),
            events=[{"at": 6, "setVel": {"x": 7.8571, "y": 0}}],
            slide=10,
            anim=move_anim(47, 6, 13, "crouch", "strike"),
        )
    ],
}
moves["fsmash"] = {
    "name": "Forward Smash",
    "phases": [
        phase(
            "main",
            51,
            hitboxes=hits_from("AttackS4"),
            charge={"frame": 5, "max": 60},
            # UFD: head intangibility on source frames 16-17 (engine 15-16).
            hurt_regions=[{"region": "head", "enabled": False, "from": 15, "to": 16}],
            anim=move_anim(51, 15, 16, "coil", "strike"),
        )
    ],
}
moves["usmash"] = {
    "name": "Up Smash",
    "phases": [
        phase(
            "main",
            51,
            hitboxes=hits_from("AttackHi4"),
            charge={"frame": 6, "max": 60},
            # UFD: head intangibility on source frames 12-18 (engine 11-17).
            hurt_regions=[{"region": "head", "enabled": False, "from": 11, "to": 17}],
            anim=move_anim(51, 11, 17, "crouch", "upStrike"),
        )
    ],
}
moves["dsmash"] = {
    "name": "Down Smash",
    "phases": [
        phase(
            "main",
            43,
            hitboxes=hits_from("AttackLw4"),
            charge={"frame": 5, "max": 60},
            # The source boxes sweep low in front (12%) then behind (14%); both ends share the low head arc.
            anim=[
                key(0, "coil"),
                key(9, "lowSweep", True),
                key(13, "lowSweep"),
                key(15, "lowSweepBack", True),
                key(18, "lowSweepBack"),
                key(24, "coil", True),
                key(34, "stand", True),
                key(42, "stand"),
            ],
        )
    ],
}
moves["nair"] = {
    "name": "Neutral Air",
    "air": "aerial",
    "phases": [
        phase("main", 47, hitboxes=hits_from("AttackAirN"), anim=move_anim_air(47, 7, 18), transitions=[{"on": "land", "to": "landing", "lag": 15}]),
    ],
}
moves["fair"] = {
    "name": "Forward Air",
    "air": "aerial",
    "phases": [
        phase("main", 44, hitboxes=hits_from("AttackAirF", entry=4), anim=move_anim_air(44, 6, 8), transitions=[{"on": "land", "to": "landing", "lag": 13}]),
    ],
}
moves["bair"] = {
    "name": "Back Air",
    "air": "aerial",
    "phases": [
        phase("main", 61, hitboxes=hits_from("AttackAirB"), anim=move_anim_air(61, 13, 14, "airCoil", "lowSweepBack"), transitions=[{"on": "land", "to": "landing", "lag": 16}]),
    ],
}
moves["uair"] = {
    "name": "Up Air",
    "air": "aerial",
    "phases": [
        # UFD: head and stem intangibility on source frames 7-9 (engine 6-8).
        phase(
            "main",
            38,
            hitboxes=hits_from("AttackAirHi"),
            hurt_regions=[
                {"region": "head", "enabled": False, "from": 6, "to": 8},
                {"region": "stem", "enabled": False, "from": 6, "to": 8},
            ],
            anim=move_anim_air(38, 6, 8),
            transitions=[{"on": "land", "to": "landing", "lag": 10}],
        ),
    ],
}
moves["dair"] = {
    "name": "Down Air",
    "air": "aerial",
    "phases": [
        phase("main", 44, hitboxes=hits_from("AttackAirLw"), anim=move_anim_air(44, 8, 13), transitions=[{"on": "land", "to": "landing", "lag": 18}]),
    ],
}

# Grabs use the mouth, not a hand.
grab_hit = {
    "start": 5,
    "end": 7,
    "x": 0,
    "y": 0,
    "r": 17,
    "damage": 0,
    "bkb": 0,
    "kbg": 0,
    "angle": 0,
    "type": "grab",
    "geometry": {"kind": "circle", "center": {"joint": "mouth", "offset": [4, 0]}, "radius": 17},
}
dash_grab_hit = {**grab_hit, "start": 8, "end": 10, "r": 15}
moves["grab"] = {
    "name": "Grab",
    "phases": [phase("main", 34, hitboxes=[grab_hit], anim=[key(0, "coil"), key(4, "grabOpen"), key(6, "grabOpen"), key(12, "grabHold"), key(24, "stand", True)])],
}
moves["dashgrab"] = {
    "name": "Dash Grab",
    "phases": [
        phase("main", 42, hitboxes=[dash_grab_hit], slide=8, anim=[key(0, "coil"), key(7, "grabOpen"), key(9, "grabOpen"), key(16, "grabHold"), key(30, "stand", True)])
    ],
}

# Ptooie: the held ball follows the mouth until a stick direction blows it away; letting go swallows it.
moves["nspec"] = {
    "name": "Ptooie",
    "phases": [
        phase(
            "start",
            10,
            events=[{"at": 9, "spawn": "ptooie", "sfx": "swing"}],
            anim=[key(0, "coil"), key(6, "grabOpen"), key(9, "strike", True)],
            transitions=[{"on": "timeout", "to": "hold"}, {"on": "articleEnd", "to": "swallow"}],
        ),
        phase(
            "hold",
            # A safety bound: the ball auto-releases at 180, and the timeout only matters if the article was
            # taken away (e.g. reflected) before it could be launched or swallowed.
            300,
            anim=[key(0, "strike"), key(8, "coil", True), key(16, "strike", True)],
            transitions=[
                {"on": "articleRelease", "to": "shoot"},
                {"on": "articleEnd", "to": "swallow"},
                {"on": "timeout", "to": "swallow"},
            ],
        ),
        phase("shoot", 41, anim=[key(0, "strike"), key(6, "bite", True), key(18, "coil"), key(30, "stand", True)]),
        phase("swallow", 30, anim=[key(0, "strike"), key(6, "grabOpen", True), key(12, "bite"), key(20, "coil", True), key(29, "stand", True)]),
    ],
}

# Poison Breath: charge, then a growing drifting cloud projectile.
moves["sspec"] = {
    "name": "Poison Breath",
    "phases": [
        phase("start", 10, anim=[key(0, "coil"), key(6, "charge")], transitions=[{"on": "timeout", "to": "charge"}]),
        phase("charge", None, charge={"frame": 2, "max": 95, "button": "special"}, anim=[key(0, "charge"), key(2, "charge")], transitions=[{"on": "release", "to": "shoot"}]),
        phase("shoot", 38, charge_frames=0.105, events=[{"at": 9, "spawn": "poison", "sfx": "swing"}], anim=[key(0, "strike"), key(8, "strike"), key(18, "coil"), key(28, "stand", True)]),
    ],
}

# Piranhacopter: rise, spinning leaf multi-hits, then helpless.
moves["uspec"] = {
    "name": "Piranhacopter",
    "air": "upSpecial",
    "oncePerAirtime": True,
    "phases": [
        phase(
            "spin",
            58,
            hitboxes=hits_from("SpecialHi", entry=13),
            # Sustained lift instead of one ballistic pop: the rotor holds a steady climb while the
            # shared up-special drift steers left/right.
            events=[{"at": 0, "setVel": {"x": 0, "y": 7.86}, "leaveGround": True}, {"from": 1, "to": 56, "setVel": {"y": 6.36}}],
            ledge_grab=0,
            anim=[
                key(0, "airCoil"),
                key(4, {"joints": {"stem0": jp([0, 27], -4), "head": jp([0, 20]), "leafL": jp(a=180), "leafR": jp(a=0), "jaw": jp(a=-20)}}, True),
                key(20, {"joints": {"stem0": jp([0, 25]), "head": jp([0, 18]), "leafL": jp(a=270), "leafR": jp(a=90)}}),
                key(40, {"joints": {"stem0": jp([0, 25]), "head": jp([0, 18]), "leafL": jp(a=450), "leafR": jp(a=270)}}),
                key(57, {"joints": {"stem0": jp([0, 25]), "head": jp([0, 18]), "leafL": jp(a=630), "leafR": jp(a=450)}}, True),
            ],
            transitions=[{"on": "land", "to": "landing", "lag": 20}],
        ),
        phase("fall", 20, anim=[key(0, {"joints": {"stem0": jp([0, 24], 4), "stem1": jp([0, 24], 2), "head": jp([-2, 18], 2), "leafL": jp(a=164), "leafR": jp(a=16)}}), key(10, "stand")], transitions=[{"on": "timeout", "to": "helpless"}, {"on": "land", "to": "landing", "lag": 20}]),
    ],
}

# Long-Stem Strike: planted pot, charge, extend the head at a charge-scaled reach, bite, retract.
# The head is intangible during travel and the bite; the pot keeps damage-based armor.
dspec_start = phase(
    "start",
    18,
    aim=True,
    anim=[key(0, "coil"), key(10, "charge", True), key(17, "charge")],
    transitions=[{"on": "timeout", "to": "charge"}],
)
dspec_charge = phase(
    "charge",
    None,
    charge={"frame": 3, "max": 60, "button": "special"},
    aim=True,
    armor_threshold=15,
    anim=[key(0, "charge"), key(3, "charge")],
    transitions=[{"on": "release", "to": "lunge"}],
)
dspec_lunge = phase(
    "lunge",
    8,
    armor_threshold=12,
    hurt_regions=[{"region": "head", "enabled": False, "from": 3, "to": 7}, {"region": "jaw", "enabled": False, "from": 3, "to": 7}],
    anim=[key(0, "charge")]
    + [key(4 + i, f"stretch{tier}", True, min_charge=tier) for i, (tier, _l, _d) in enumerate(LSS_TIERS)],
    transitions=[{"on": "timeout", "to": "bite"}],
)
# One hitbox per reach tier; the longest active tier is listed last in the pose keys and first here, so
# the mouth-anchored box always agrees with the visible head. Base damage interpolates close (8.4) to
# far (12.0); chargeDamage then reaches the source 18.2 close / 26.0 far at full charge.
dspec_bite = phase(
    "bite",
    4,
    hitboxes=[
        {**hit(attacks("packun", "SpecialLwBite_Attack")[0]["source"], 0, 0), "damage": dmg, "minCharge": tier}
        for tier, _l, dmg in reversed(LSS_TIERS)
    ],
    hurt_regions=[{"region": "head", "enabled": False, "from": 0, "to": 3}, {"region": "jaw", "enabled": False, "from": 0, "to": 3}],
    anim=[key(0, f"stretchedBite{tier}", min_charge=tier) for tier, _l, _d in LSS_TIERS],
    transitions=[{"on": "hit", "to": "recoverHit"}, {"on": "timeout", "to": "recover"}],
)
dspec_recover = phase(
    "recover",
    37,
    charge_frames=0.166,
    anim=[key(0, f"stretchedBite{tier}", min_charge=tier) for tier, _l, _d in LSS_TIERS]
    + [key(6, "stretch0", True), key(16, "charge", True), key(30, "stand", True)],
)
dspec_recover_hit = phase(
    "recoverHit",
    27,
    charge_frames=0.166,
    anim=[key(0, f"stretchedBite{tier}", min_charge=tier) for tier, _l, _d in LSS_TIERS]
    + [key(5, "strike", True), key(14, "coil", True), key(22, "stand", True)],
)
dspec = {
    "name": "Long-Stem Strike",
    "bReverse": True,
    "chargeDamage": (26.0 / 12.0 - 1.0) / 60.0,
    "phases": [dspec_start, dspec_charge, dspec_lunge, dspec_bite, dspec_recover, dspec_recover_hit],
}
moves["dspec"] = dspec

# Throws: source release frames and damage/launch. Down throw has two stomp ticks.
throws = {
    "f": {
        "release": 13,
        "total": 31,
        "hit": {"x": 0, "y": 0, "r": 0, "damage": 8.0, "bkb": 60, "kbg": 0.65, "angle": 45},
    },
    "b": {
        "release": 18,
        "total": 34,
        "hit": {"x": 0, "y": 0, "r": 0, "damage": 11.0, "bkb": 54, "kbg": 0.6, "angle": 45},
    },
    "u": {
        "release": 19,
        "total": 49,
        "hit": {"x": 0, "y": 0, "r": 0, "damage": 12.0, "bkb": 64, "kbg": 0.67, "angle": 90},
        "ticks": [{"at": 18, "damage": 5.0}],
    },
    "d": {
        "release": 29,
        "total": 47,
        "hit": {"x": 0, "y": 0, "r": 0, "damage": 3.0, "bkb": 80, "kbg": 0.8, "angle": 66},
        "ticks": [{"at": 19, "damage": 2.5}, {"at": 27, "damage": 2.5}],
    },
}

# ---------------------------------------------------------------------------
# Projectiles
# ---------------------------------------------------------------------------
spike_shapes = [
    {"shape": "circle", "at": [0, 0], "r": 22, "fill": "#b52828", "stroke": "#5b1010", "width": 2},
    {"shape": "circle", "at": [0, 0], "r": 15, "fill": "#d24848"},
    {"shape": "poly", "points": [[23, 0], [15, 8], [15, -8]], "fill": "#e8e8e8"},
    {"shape": "poly", "points": [[-23, 0], [-15, 8], [-15, -8]], "fill": "#e8e8e8"},
    {"shape": "poly", "points": [[0, 23], [8, 15], [-8, 15]], "fill": "#e8e8e8"},
    {"shape": "poly", "points": [[0, -23], [8, -15], [-8, -15]], "fill": "#e8e8e8"},
    {"shape": "poly", "points": [[16, 16], [8, 18], [14, 8]], "fill": "#dcdcdc"},
    {"shape": "poly", "points": [[-16, 16], [-8, 18], [-14, 8]], "fill": "#dcdcdc"},
    {"shape": "poly", "points": [[16, -16], [8, -18], [14, -8]], "fill": "#dcdcdc"},
    {"shape": "poly", "points": [[-16, -16], [-8, -18], [-14, -8]], "fill": "#dcdcdc"},
    {"shape": "circle", "at": [-6, 5], "r": 3, "fill": "#f0c0b0", "alpha": 0.9},
]

def ptooie_ball(damage, bkb, radius=25.0):
    return {"start": 0, "x": 0, "y": 0, "r": radius, "damage": damage, "bkb": bkb, "kbg": 0.8, "angle": 55, "hitlagMult": 1.2}


projectiles = {
    "ptooie": {
        "name": "Ptooie",
        "spawn": [18, 126],
        "vel": [0, 0],
        "r": 25,
        "life": 400,
        "max": 1,
        "spawnSfx": "swing",
        "hit": ptooie_ball(14.0, 50),
        "article": {
            "slot": "nspec",
            "minRelease": 13,
            "autoRelease": 180,
            "releaseOnDir": True,
            "reflectCooldown": 8,
            # Held at the mouth and blown up/down by the breath; the ball can be blown front or back.
            "hold": {"offset": [18, 126], "bob": 20, "period": 36, "hitboxes": [ptooie_ball(14.0, 50)]},
            "stages": [
                {
                    "from": 0,
                    "startup": 0,
                    "frames": 180,
                    "vel": [0.0, 0.0],
                    "hitboxes": [],
                    "shapes": spike_shapes,
                },
                {
                    "from": 1,
                    "startup": 0,
                    "frames": 70,
                    "vel": [11.4286, 0.0],
                    # Source article: 18% while freshly blown, 13% once it has slowed (SmashWiki
                    # 13% late; UFD's 15.3% after-bounce variant is not modeled).
                    "hitboxes": [{**ptooie_ball(18.0, 50), "end": 3}, {**ptooie_ball(13.0, 45), "start": 4}],
                    "shapes": spike_shapes,
                    "attackShapes": [{"shape": "circle", "at": [0, 0], "r": 27, "fill": "#ff6a5a", "alpha": 0.28}],
                },
            ],
        },
        "shapes": spike_shapes,
        "spin": 8.0,
    },
    "poison": {
        "name": "Poison Breath",
        "spawn": [48, 86],
        "vel": [1.2857, 0],
        "r": 50,
        "life": 82,
        "max": 1,
        "gravity": 0,
        "spawnSfx": "whack",
        # One merged inner/outer box: 1.2% uncharged rising to 2.8% (source inner loop), no flinch.
        "hit": {
            "x": 0,
            "y": 0,
            "r": 50,
            "damage": 1.2,
            "bkb": 0,
            "kbg": 0,
            "angle": 0,
            "rehit": 4,
            "type": "damageOnly",
            "hitlagMult": 0.0,
            "shieldDmgMult": 0.1,
        },
        # Source: 82 -> 122 ticks, rehit 4 -> 7, 1.2% -> 2.8% inner damage. The cloud's 25-frame growth,
        # 12-25 windbox and stop-at-45/75 drift are approximated by a fixed radius and constant drift.
        "chargeScale": {
            "speed": 0.0,
            "damage": (2.8 / 1.2 - 1.0) / 95.0,
            "radius": 0.0,
            "life": (122.0 / 82.0 - 1.0) / 95.0,
            "rehit": 3.0 / 95.0,
        },
        "shapes": [
            {"shape": "circle", "at": [0, 0], "r": 50, "fill": "#8b2f9e", "alpha": 0.28},
            {"shape": "circle", "at": [-8, 4], "r": 32, "fill": "#b347c4", "alpha": 0.38},
            {"shape": "circle", "at": [10, -4], "r": 26, "fill": "#d06ae0", "alpha": 0.34},
            {"shape": "circle", "at": [0, 0], "r": 13, "fill": "#e9a6f0", "alpha": 0.22},
        ],
    },
}

# ---------------------------------------------------------------------------
# Package
# ---------------------------------------------------------------------------
hurt_regions = [
    {"id": "pot", "geometry": {"kind": "capsule", "a": {"joint": "pot", "offset": [0, 8]}, "b": {"joint": "stem0", "offset": [0, 4]}, "radius": 16}},
    {"id": "stem", "geometry": {"kind": "capsule", "a": {"joint": "stem0", "offset": [0, 0]}, "b": {"joint": "neck", "offset": [0, 0]}, "radius": 7}},
    {"id": "head", "geometry": {"kind": "capsule", "a": {"joint": "head", "offset": [0, -10]}, "b": {"joint": "top", "offset": [0, 8]}, "radius": 19}},
    {"id": "jaw", "geometry": {"kind": "capsule", "a": {"joint": "jaw", "offset": [2, 0]}, "b": {"joint": "mouth", "offset": [0, 0]}, "radius": 6}},
]

fighter = {
    "$schema": "../../schema/fighter.schema.json",
    "schemaVersion": 1,
    "extends": "base",
    "name": "Planty",
    "blurb": "A leafy potted fighter with a stretching stem and a floating spiked ball.",
    "tagline": "Small pot, big reach",
    "stats": {
        "weight": 100.0 / 112.0,
        "pummelDamage": 1.4,
        "walkSpeed": round(MOBILITY["walk"] * SCALE, 5),
        "runSpeed": round(MOBILITY["run"] * SCALE, 5),
        "dashSpeed": round(MOBILITY["dash"] * SCALE, 5),
        "dashFrames": 15,
        "runAccel": 1.1429,  # fitted
        "groundFriction": 0.85,
        **units.jumps(JUMP_HEIGHTS["full"], JUMP_HEIGHTS["short"], JUMP_HEIGHTS["air"], MOBILITY["gravity"] * SCALE),
        "airJumpDrift": 5.0,  # fitted
        "airJumps": 1,
        "airSpeed": round(MOBILITY["air"] * SCALE, 5),
        "airAccel": round(MOBILITY["airAccel"] * SCALE, 5),
        "gravity": round(MOBILITY["gravity"] * SCALE, 5),
        "fallSpeed": round(MOBILITY["fall"] * SCALE, 5),
        "fastFallSpeed": round(MOBILITY["fastFall"] * SCALE, 5),
    },
    "body": {"halfWidth": 18, "height": 74},
    "hurtboxes": {"default": {"x1": -24, "x2": 24, "y1": 0, "y2": 122}, "knockdown": {"x1": -40, "x2": 40, "y1": 0, "y2": 26}},
    "hurtRegions": hurt_regions,
    "combatCenter": [0, 58],
    "interactions": {
        "grab": {"catchPoint": [0, 20], "holdPoint": [46, 70], "heldAnchor": [0, 24]},
        "ledge": {"catchPoint": [0, 50]},
    },
    "ai": {
        "reach": 80,
        "counterChance": 0,
        "specials": [
            {"minDistance": 160, "maxDistance": None, "height": 100, "grounded": True, "noProjectile": True, "chance": 0.5, "neutral": True, "down": False, "hold": [12, 26]},
            {"minDistance": 90, "maxDistance": 250, "height": 80, "grounded": False, "noProjectile": True, "chance": 0.4, "down": False, "hold": None},
            {"minDistance": 0, "maxDistance": 150, "height": 120, "grounded": True, "noProjectile": True, "chance": 0.25, "down": True, "hold": [20, 50]},
        ],
    },
    "moves": moves,
    "throws": throws,
    "projectiles": projectiles,
    "style": {
        "headRadius": 24,
        "torsoWidth": 8,
        "frontLimbWidth": 4,
        "backLimbWidth": 4,
        "glowWidth": 16,
        "torsoGlowWidth": 18,
        "headGlowRadius": 27,
        "headRing": 2,
        "headFill": "#79eb62",
        "eye": [4, 4, 3, 3],
    },
    "rig": rig,
    "poses": poses,
    "anims": anims,
}

out = ROOT / "assets/fighters/plant"
out.mkdir(parents=True, exist_ok=True)
(out / "fighter.json").write_text(json.dumps(fighter, indent=1) + "\n")

# Normalized source facts, read back from the pinned inputs rather than from the generated package.
UFD_SLOTS = {
    "jab": ["Jab 1", "Jab 2", "Jab 3", "Rapid Jab", "Rapid Jab Finisher"],
    "ftilt": ["Forward Tilt 1", "Forward Tilt 2"],
    "utilt": ["Up Tilt"],
    "dtilt": ["Down Tilt"],
    "dashatk": ["Dash Attack"],
    "fsmash": ["Forward Smash"],
    "usmash": ["Up Smash"],
    "dsmash": ["Down Smash"],
    "nair": ["Neutral Air"],
    "fair": ["Forward Air"],
    "bair": ["Back Air"],
    "uair": ["Up Air"],
    "dair": ["Down Air"],
    "grab": ["Grab"],
    "dashgrab": ["Dash Grab"],
    "nspec": ["Neutral B (Ptooie)"],
    "sspec": ["Side B (Poison Breath)", "Side B, Full Charge (Poison Breath, Full Charge)"],
    "uspec": ["Up B (Piranhacopter)"],
    "dspec": ["Down B (Long Stem Strike)"],
    "throws": ["Forward Throw", "Backward Throw", "Up Throw", "Down Throw", "Pummel"],
}
SCRIPT_SLOTS = {
    "jab": ["Attack11", "Attack12", "Attack13", "Attack100Sub", "Attack100End"],
    "ftilt": ["AttackS3", "AttackS32"],
    "utilt": ["AttackHi3"],
    "dtilt": ["AttackLw3"],
    "dashatk": ["AttackDash"],
    "fsmash": ["AttackS4"],
    "usmash": ["AttackHi4"],
    "dsmash": ["AttackLw4"],
    "nair": ["AttackAirN"],
    "fair": ["AttackAirF"],
    "bair": ["AttackAirB"],
    "uair": ["AttackAirHi"],
    "dair": ["AttackAirLw"],
    "grab": ["Catch"],
    "dashgrab": ["CatchDash"],
    "nspec": ["SpecialNStart", "SpecialNShootF", "SpecialNShootB"],
    "sspec": ["SpecialSStart", "SpecialSShoot"],
    "uspec": ["SpecialHi"],
    "dspec": ["SpecialLwStart", "SpecialLwFallF", "SpecialLwBite_Attack"],
    "throws": ["CatchAttack", "ThrowF", "ThrowB", "ThrowHi", "ThrowLw"],
}
ARTICLE_SCRIPTS = {
    "packun_spikeball": ["Start", "Loop", "Wait", "Shoot", "Fall"],
    "packun_poisonbreath": ["Start", "Shoot", "StartMax", "ShootMax"],
}
ufd_rows = {row["movename"]: row for row in ufd["moves"]}
source_facts = {
    "ufd": {
        slot: [ufd_rows[name] for name in names if name in ufd_rows]
        for slot, names in UFD_SLOTS.items()
    },
    "scripts": {slot: {name: attacks("packun", name) for name in names} for slot, names in SCRIPT_SLOTS.items()},
    "articles": {
        article: {name: attacks(article, name) for name in names}
        for article, names in ARTICLE_SCRIPTS.items()
    },
}

# Authoring ledger: the generated phase/hit rows are written for review convenience; `sourceFacts` above
# stays independent of them so the two can be compared.
ledger = {
    "source": {"ufd": ufd["sha256"], "scripts": raw["sha256"], "patch": raw["patch"], "commit": raw["commit"]},
    "scale": SCALE,
    "frame": FRAME,
    "sourceFacts": source_facts,
    "moves": {},
    "rigJoints": [j["id"] for j in rig["joints"]],
    "hurtRegions": [r["id"] for r in hurt_regions],
    "projectiles": list(projectiles),
}
for slot, m in moves.items():
    rows = []
    for p in m["phases"]:
        rows.append(
            {
                "name": p["name"],
                "frames": p.get("frames"),
                "charge": p.get("charge"),
                "hits": [
                    {
                        "start": h["start"],
                        "end": h["end"],
                        "damage": h["damage"],
                        "angle": h["angle"],
                        "bkb": h["bkb"],
                        "kbg": h["kbg"],
                        "rehit": h.get("rehit"),
                        "geometry": h.get("geometry"),
                    }
                    for h in p.get("hitboxes", [])
                ],
                "transitions": p.get("transitions", []),
            }
        )
    ledger["moves"][slot] = {"name": m["name"], "phases": rows}
(SOURCES / "ledger.json").write_text(json.dumps(ledger, indent=1) + "\n")
print(f"Wrote {out / 'fighter.json'} and {SOURCES / 'ledger.json'}")
