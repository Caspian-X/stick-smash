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


if __name__ == '__main__':
    import sys
    for arg in sys.argv[1:]:  # `full,short,air,gravity`, e.g. `29.9,16,31,0.46`
        print(arg, jumps(*map(float, arg.split(','))))
