"""The content scale shared by the import generators: one source (SSBU) unit is `SCALE` px on both axes, for
geometry, speeds (units/frame → px/tick) and accelerations (units/frame² → px/tick²) alike.

Jump speeds are derived from source jump heights so that the sim's discrete integration reaches exactly
`height × SCALE` px. On the jump tick the sim sets `vy = -v` and moves without gravity (a ground jump leaves
jumpsquat that way), then each later tick adds gravity `g` before moving, so after `n` airborne ticks the rise is
`n·v − g·n(n−1)/2`, peaking at `n = ceil(v/g)`:

    height(v) = n·v − g·n(n−1)/2,  n = ceil(v/g)      ⇒  v = h/n + g(n−1)/2 for the n with (n−1)g < v ≤ n·g

An air jump sets `vy` and applies gravity in the same tick, so it needs exactly one `g` more for the same rise.

Ultimate's full hop also has a "jump initial height" (ssbwiki Jump, Ultimate ranking: 0.55 × the full hop height for
most fighters, the `jump_initial_y` param): the first four airborne frames rise that far with no gravity, then the
jump continues from the remaining height as above. `jumps` emits it as `jumpInitialHeight` (the sim climbs it in four
equal steps, the fourth tick launching at `jumpSpeed`), which is why full hops land several frames sooner than a
constant-gravity jump of the same height (Peach: 57 frames, not 63). Short hops and air jumps don't have it.
`tools/units_test.py` checks both against a step-by-step integration.
"""

import re

SCALE = 5.0  # px per source unit (geometry, speed, gravity)
INITIAL = 0.55  # Ultimate jump initial height / full hop height (ssbwiki Jump)


def jump_speed(height, gravity, air=False):
    """Launch speed (px/tick) whose apex is `height` px above takeoff under `gravity` (px/tick²)."""
    if height <= 0:
        return 0.0
    n = 1
    while True:
        v = height / n + gravity * (n - 1) / 2
        if v <= n * gravity:
            return v + (gravity if air else 0.0)
        n += 1


def apex(v, gravity, air=False, initial=0.0):
    """The rise `jump_speed` inverts, by stepping the sim's integration (after an `initial` four-tick rise)."""
    y = top = initial
    vy = v
    if air:
        vy -= gravity
    while vy > 0:
        y += vy
        top = max(top, y)
        vy -= gravity
    return top


def jumps(full, short, air, gravity, initial=INITIAL):
    """`stats` jump speeds from source heights in units (gravity in px/tick²), rounded to 4 decimals. `initial` is the
    full hop's initial height as a fraction of it (0 for none)."""
    h = lambda u: u * SCALE  # noqa: E731
    rise = round(h(full) * initial, 4)
    out = {'jumpSpeed': round(jump_speed(h(full) - rise, gravity), 4),
           'shortHopSpeed': round(jump_speed(h(short), gravity), 4),
           'airJumpSpeed': round(jump_speed(h(air), gravity, air=True), 4)}
    if rise > 0:
        out['jumpInitialHeight'] = rise
    return out


def flag_frame(script, flag):
    """The source frame of the `frame(Frame=N)` marker before `flag` first appears in a script, or None."""
    at = None
    for line in script.splitlines():
        m = re.match(r'\s*frame\(Frame=(\d+)\)', line)
        if m:
            at = int(m.group(1))
        elif flag in line:
            return at
    return None


def gait(body):
    """`stats` gait frames from the fighter body's Ultimate scripts (`{name: data}`): `dashFrames` from where `Dash`
    enables the DASH_TO_RUN transition, `turnFrames` and `brakeFrames` from where `TurnRun` and `RunBrakeR` raise
    FIGHTER_STATUS_RUN_BRAKE_FLAG_STOP (the slide stops and the run, or idle, takes over), `dashTurnFrames` from
    where `TurnDash` raises FIGHTER_STATUS_DASH_FLAG_TURN_DASH.

    `frame(Frame=N)` is 1-indexed: its code runs on game frame N, so N − 1 frames come before it. The sim counts a
    gait's frames the same way (a `dashFrames` dash is that many ticks, the run starts on the next), so each value is
    N − 1. Peach's `Dash` marker at 16 gives SmashWiki's 15 dash frames (King Dedede and Cloud: 11 → 10)."""
    out = {}
    for key, script, flag in [('dashFrames', 'Dash', 'FIGHTER_STATUS_TRANSITION_TERM_ID_DASH_TO_RUN'),
                              ('dashTurnFrames', 'TurnDash', 'FIGHTER_STATUS_DASH_FLAG_TURN_DASH'),
                              ('turnFrames', 'TurnRun', 'FIGHTER_STATUS_RUN_BRAKE_FLAG_STOP'),
                              ('brakeFrames', 'RunBrakeR', 'FIGHTER_STATUS_RUN_BRAKE_FLAG_STOP')]:
        at = flag_frame(body.get(script, ''), flag)
        if at is not None:
            out[key] = at - 1
    return out


if __name__ == '__main__':
    import sys
    for arg in sys.argv[1:]:  # `full,short,air,gravity`, e.g. `29.9,16,31,0.46`
        print(arg, jumps(*map(float, arg.split(','))))
