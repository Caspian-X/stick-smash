"""Tests for tools/units.py: `python3 -m unittest tools/units_test.py`."""

import importlib
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
units = importlib.import_module('units')


class Jumps(unittest.TestCase):
    def test_speed_reaches_the_height_exactly(self):
        for g in (0.35, 0.46, 0.485, 0.625, 0.84):
            for h in (40, 80.1, 107, 164.25, 222.5):
                for air in (False, True):
                    v = units.jump_speed(h, g, air)
                    self.assertAlmostEqual(units.apex(v, g, air), h, places=9, msg=(h, g, air))

    def test_air_jump_needs_one_more_gravity(self):
        self.assertAlmostEqual(units.jump_speed(150, 0.5, True), units.jump_speed(150, 0.5) + 0.5)

    def test_stats(self):
        s = units.jumps(29.9, 16, 31, 0.46, initial=0)
        self.assertEqual(set(s), {'jumpSpeed', 'shortHopSpeed', 'airJumpSpeed'})
        self.assertAlmostEqual(units.apex(s['jumpSpeed'], 0.46), 149.5, delta=0.01)

    def test_initial_height(self):
        s = units.jumps(30.03, 14.5, 30.03, 0.35)
        self.assertAlmostEqual(s['jumpInitialHeight'], 0.55 * 150.15, places=3)
        self.assertAlmostEqual(units.apex(s['jumpSpeed'], 0.35, initial=s['jumpInitialHeight']), 150.15, delta=0.01)
        self.assertAlmostEqual(units.apex(s['shortHopSpeed'], 0.35), 72.5, delta=0.01)



class Gait(unittest.TestCase):
    def test_frames_from_scripts(self):
        body = {'TurnDash': 'frame(Frame=3)\r\nif(is_excute){\r\n\tWorkModule::on_flag(Flag=FIGHTER_STATUS_DASH_FLAG_TURN_DASH)'
                            '\r\n}\r\nframe(Frame=18)\r\n',
                'TurnRun': 'frame(Frame=11)\r\nif(is_excute){\r\n\tWorkModule::on_flag(Flag=FIGHTER_STATUS_RUN_BRAKE_FLAG_STOP)',
                'RunBrakeR': 'frame(Frame=15)\r\nWorkModule::on_flag(Flag=FIGHTER_STATUS_RUN_BRAKE_FLAG_STOP)'}
        self.assertEqual(units.gait(body), {'dashTurnFrames': 2, 'turnFrames': 11, 'brakeFrames': 15})
        self.assertEqual(units.gait({}), {})

if __name__ == '__main__':
    unittest.main()
