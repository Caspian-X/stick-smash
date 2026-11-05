"""The content scale shared by the import generators: one source (SSBU) unit is `SCALE` px on both axes, for
geometry, speeds (units/frame → px/tick) and accelerations (units/frame² → px/tick²) alike.

Jump speeds are derived from source jump heights so that the sim's discrete integration reaches exactly
`height × SCALE` px. On the jump tick the sim sets `vy = -v` and moves without gravity (a ground jump leaves
jumpsquat that way), then each later tick adds gravity `g` before moving, so after `n` airborne ticks the rise is
`n·v − g·n(n−1)/2`, peaking at `n = ceil(v/g)`:

    height(v) = n·v − g·n(n−1)/2,  n = ceil(v/g)      ⇒  v = h/n + g(n−1)/2 for the n with (n−1)g < v ≤ n·g

An air jump sets `vy` and applies gravity in the same tick, so it needs exactly one `g` more for the same rise.
`tools/units_test.py` checks both against a step-by-step integration.
"""

SCALE = 5.0  # px per source unit (geometry, speed, gravity)


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


def apex(v, gravity, air=False):
    """The rise `jump_speed` inverts, by stepping the sim's integration."""
    y = top = 0.0
    vy = v
    if air:
        vy -= gravity
    while vy > 0:
        y += vy
        top = max(top, y)
        vy -= gravity
    return top


def jumps(full, short, air, gravity):
    """`stats` jump speeds from source heights in units (gravity in px/tick²), rounded to 4 decimals."""
    h = lambda u: u * SCALE  # noqa: E731
    return {'jumpSpeed': round(jump_speed(h(full), gravity), 4),
            'shortHopSpeed': round(jump_speed(h(short), gravity), 4),
            'airJumpSpeed': round(jump_speed(h(air), gravity, air=True), 4)}


if __name__ == '__main__':
    import sys
    for arg in sys.argv[1:]:  # `full,short,air,gravity`, e.g. `29.9,16,31,0.46`
        print(arg, jumps(*map(float, arg.split(','))))
