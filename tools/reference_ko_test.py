"""Tests for tools/reference_ko.py: `python3 -m unittest tools/reference_ko_test.py`."""

import importlib
import math
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
ref = importlib.import_module('reference_ko')


def box(**kw):
    return {'damage': 10.0, 'bkb': 40.0, 'kbg': 1.0, 'angle': 45.0, **kw}


class Formula(unittest.TestCase):
    def test_knockback_matches_the_ultimate_equation(self):
        # p = 60 after a 10% hit on Mario: ((6 + 30) × 200/198 × 1.4 + 18) × 1 + 40.
        expected = (36 * 200 / 198 * 1.4 + 18) + 40
        self.assertAlmostEqual(ref.knockback(box(), 50, 98), expected)

    def test_weight_based_knockback_ignores_percent_and_damage(self):
        h = box(fkb=30.0, bkb=20.0)
        expected = (16 * 1.4 * 200 / 198 + 18) + 20
        for pct, dmg in ((0, 1), (150, 20)):
            self.assertAlmostEqual(ref.knockback({**h, 'damage': dmg}, pct, 98), expected)

    def test_hitstun_hitlag_and_launch_speed(self):
        r = ref.launch(box(), 100)
        self.assertEqual(r['hitstun'], math.floor(r['kb'] * 0.4))
        self.assertAlmostEqual(r['speed'], r['kb'] * 0.03)
        self.assertEqual(r['hitlag'], math.floor(10 * 0.65 + 6))
        self.assertEqual(ref.hitlag(box(hitlagMult=1.45)), math.floor(12.5 * 1.45))
        self.assertEqual(ref.hitlag(box(damage=40.0, hitlagMult=2.0)), 30)

    def test_sakurai_ramps_on_the_ground(self):
        h = box(type='sakurai')
        for kb, a in ((50, 0), (60, 0), (74, 19), (88, 38), (200, 38)):
            self.assertAlmostEqual(ref.angle(h, kb, True), a)
            self.assertEqual(ref.angle(h, kb, False), 38)

    def test_upward_tumble_uses_the_launch_gravity_and_fall_speed(self):
        h = box(angle=90.0, bkb=60.0)
        # Launch gravity 0.0749 (Mario) but a 1.8 fall cap: over a long launch the higher cap wins, so the vertical
        # KO comes later than with Mario's own 0.087 / 1.5.
        with_top = ref.ko_percent(h)
        saved = dict(ref.TOP)
        try:
            ref.TOP.update(min=999.0)
            plain = ref.ko_percent(h)
        finally:
            ref.TOP.update(saved)
        self.assertIsNotNone(with_top)
        self.assertGreater(with_top, plain)

    def test_ko_percent_is_the_lowest_launch_ko(self):
        h = box(damage=15.0, bkb=40.0, kbg=1.0, angle=40.0)
        p = ref.ko_percent(h)
        self.assertTrue(ref.launch(h, p)['ko'])
        self.assertFalse(ref.launch(h, p - 1)['ko'])


class Content(unittest.TestCase):
    def test_extends_merges_objects_and_replaces_arrays(self):
        base = {'a': {'x': 1, 'y': 2}, 'l': [1, 2], 'k': 1}
        self.assertEqual(ref.merge(base, {'a': {'y': 3}, 'l': [9], 'k': None}), {'a': {'x': 1, 'y': 3}, 'l': [9]})

    def test_resolved_fighters_have_throws_and_moves(self):
        f = ref.fighter('duchess')
        self.assertEqual(f['extends'], 'base')
        moves = {m for m, _, _ in ref.hitboxes(f)}
        self.assertTrue({'fsmash', 'nair', 'fthrow', 'uthrow'} <= moves)


if __name__ == '__main__':
    unittest.main()
