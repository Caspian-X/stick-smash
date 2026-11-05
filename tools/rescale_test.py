"""Tests for tools/rescale.py against the generated schemas: `python3 -m unittest tools/rescale_test.py`."""

import contextlib
import importlib
import io
import json
import sys
import tempfile
import unittest
from pathlib import Path
from typing import Any

sys.path.insert(0, str(Path(__file__).resolve().parent))
rescale = importlib.import_module('rescale')

ROOT = rescale.ROOT
SCHEMAS = rescale.load_schemas(ROOT / 'assets' / 'schema')

FIGHTER: dict[str, Any] = {
    '$schema': '../../schema/fighter.schema.json',
    'schemaVersion': 1,
    'extends': 'base',
    'stats': {'weight': 1, 'runSpeed': 3.5, 'gravity': 0.6, 'jumpSpeed': 12},
    'moves': {'jab': {'phases': [{
        'name': 'hit', 'frames': 5, 'travelSpeed': 4,
        'hitboxes': [{'x': 10, 'y': 20, 'r': 5, 'damage': 3, 'bkb': 20, 'kbg': 1, 'angle': 45, 'frames': [[2, 10, 20, 5]]}],
        'events': [{'at': 1, 'setVel': {'x': 3.5, 'y': 6}, 'mulVel': {'x': 0.5},
                    'fx': [{'type': 'part', 'x': [0, 7], 'vx': [1, 2], 'life': 10, 'size': 3, 'color': '#ffffff'}]}],
    }]}},
    'projectiles': {'ball': {'vel': [3.5, -6], 'spawn': [10, 20], 'r': 6, 'gravity': 0.5}},
    'poses': {'stand': {'head': [0, 60], 'rot': 10}},
}
STAGE = {
    '$schema': '../../schema/stage.schema.json',
    'blocks': [{'left': 100, 'right': 300, 'top': 500, 'bottom': 560}],
    'platforms': [{'x1': 150, 'x2': 250, 'y': 400, 'motion': {'mode': 'loop', 'phase': 3, 'waypoints': [{'x': 0, 'y': 10, 'travelTicks': 30}]}}],
    'spawns': [{'x': 200, 'face': 1}],
    'art': {'background': {'sky': [[100, '#000000']], 'parallax': 0.5}},
}
GLOBAL = {'$schema': './schema/global.schema.json', 'knockback': {'ultimate': {'launchSpeed': 0.14, 'base': 18}}}


def run(doc, config, rel='fighters/x/fighter.json'):
    doc = json.loads(json.dumps(doc))
    root, node = rescale.schema_for(Path(rel), doc, SCHEMAS)
    counts = rescale.rescale(doc, root, node, rel, rescale.Config(config))
    return doc, counts


class Format(unittest.TestCase):
    def test_floats_match_serde_json(self):
        for v, s in [(1e-7, '1e-7'), (1.5e-7, '1.5e-7'), (1e-6, '1e-6'), (1e-5, '0.00001'), (0.0001, '0.0001'),
                     (1e15, '1000000000000000.0'), (1e16, '1e16'), (1.25e17, '1.25e17'), (123.0, '123.0'),
                     (0.1, '0.1'), (-2.5, '-2.5'), (0.0, '0.0'), (-0.0, '-0.0'), (34.2857142857, '34.2857142857'),
                     (1.7976931348623157e308, '1.7976931348623157e308')]:
            self.assertEqual(rescale.fmt_float(v), s)

    def test_every_asset_is_reproduced_byte_for_byte(self):
        files = [p for p in (ROOT / 'assets').rglob('*.json')]
        self.assertGreater(len(files), 25)
        for p in files:
            text = p.read_text()
            self.assertEqual(rescale.to_compact_string(json.loads(text)), text, p)


class Rescale(unittest.TestCase):
    def test_speeds_and_accelerations_follow_their_axis(self):
        doc, counts = run(FIGHTER, {'factors': {'px/tick:x': 2, 'px/tick:y': 3, 'px/tick2:y': 10}})
        self.assertEqual(doc['stats'], {'weight': 1, 'runSpeed': 7, 'gravity': 6, 'jumpSpeed': 36})
        phase = doc['moves']['jab']['phases'][0]
        event = phase['events'][0]
        self.assertEqual(event['setVel'], {'x': 7, 'y': 18})
        self.assertEqual(event['mulVel'], {'x': 0.5}, 'mulVel is a ratio even though Vel holds speeds')
        self.assertEqual(event['fx'][0]['vx'], [2, 4], '[min, max] ranges share the field tag')
        self.assertEqual(event['fx'][0]['x'], [0, 7])
        self.assertEqual(phase['travelSpeed'], 4, 'xy has no factor here')
        self.assertEqual(doc['projectiles']['ball']['vel'], [7, -18], 'pairs scale per element')
        self.assertEqual(doc['projectiles']['ball']['gravity'], 5)
        self.assertEqual(phase['hitboxes'][0], FIGHTER['moves']['jab']['phases'][0]['hitboxes'][0])
        self.assertEqual(counts, {'px/tick:x': 5, 'px/tick:y': 3, 'px/tick2:y': 2})

    def test_geometry_tuples_and_untouched_units(self):
        doc, _ = run(FIGHTER, {'factors': {'px:x': 2, 'px:xy': 3, 'tick': 1}})
        hit = doc['moves']['jab']['phases'][0]['hitboxes'][0]
        self.assertEqual(hit, {**hit, 'x': 20, 'y': 20, 'r': 15, 'damage': 3, 'bkb': 20, 'kbg': 1, 'angle': 45})
        self.assertEqual(hit['frames'], [[2, 20, 20, 15]], 'track rows keep their frame')
        self.assertEqual(doc['poses']['stand'], {'head': [0, 60], 'rot': 10})
        self.assertEqual(doc['moves']['jab']['phases'][0]['events'][0]['fx'][0]['x'], [0, 14])
        self.assertEqual(doc['projectiles']['ball']['spawn'], [20, 20])

    def test_axis_fallback_and_rounding(self):
        doc, _ = run(FIGHTER, {'factors': {'*:x': '5/3.5', '*:y': '5/6'}, 'digits': 3})
        self.assertEqual(doc['stats']['runSpeed'], 5)
        self.assertEqual(doc['stats']['jumpSpeed'], 10)
        self.assertEqual(doc['stats']['gravity'], 0.5)
        self.assertEqual(doc['moves']['jab']['phases'][0]['hitboxes'][0]['x'], 14.286)
        self.assertEqual(doc['stats']['weight'], 1)

    def test_skip_set_and_scale_locations(self):
        cfg = {'factors': {'px/tick:x': 2, 'px/tick:y': 2, 'kb': 10},
               'skip': ['bkb', 'Vel.x', 'fighters/*/fighter.json#/projectiles'],
               'scale': {'/stats/runSpeed': 4}, 'set': {'/stats/jumpSpeed': 1.5}}
        doc, _ = run(FIGHTER, cfg)
        self.assertEqual(doc['stats']['runSpeed'], 14)
        self.assertEqual(doc['stats']['jumpSpeed'], 1.5)
        self.assertEqual(doc['moves']['jab']['phases'][0]['hitboxes'][0]['bkb'], 20)
        self.assertEqual(doc['moves']['jab']['phases'][0]['events'][0]['setVel'], {'x': 3.5, 'y': 12})
        self.assertEqual(doc['projectiles']['ball']['vel'], [3.5, -6])
        doc, _ = run(FIGHTER, {'skip': ['/moves/*/phases/*/hitboxes'], 'factors': {'kb': 10}})
        self.assertEqual(doc['moves']['jab']['phases'][0]['hitboxes'][0]['bkb'], 20)
        g, _ = run(GLOBAL, {'set': {'global.json#/knockback/ultimate/launchSpeed': 0.15}, 'factors': {'kb': 2}}, 'global.json')
        self.assertEqual(g['knockback']['ultimate'], {'launchSpeed': 0.15, 'base': 36})

    def test_stage_geometry_motion_and_backdrop(self):
        doc, counts = run(STAGE, {'factors': {'px:y': 2, 'tick': 3}}, 'stages/x/stage.json')
        self.assertEqual(doc['blocks'][0], {'left': 100, 'right': 300, 'top': 1000, 'bottom': 1120})
        self.assertEqual(doc['platforms'][0]['y'], 800)
        self.assertEqual(doc['platforms'][0]['motion'], {'mode': 'loop', 'phase': 9, 'waypoints': [{'x': 0, 'y': 20, 'travelTicks': 90}]})
        self.assertEqual(doc['spawns'], [{'x': 200, 'face': 1}])
        self.assertEqual(doc['art']['background'], {'sky': [[200, '#000000']], 'parallax': 0.5})
        self.assertEqual(counts, {'px:y': 5, 'tick': 2})

    def test_cli_dry_run_then_write(self):
        with tempfile.TemporaryDirectory() as tmp:
            tmp = Path(tmp)
            (tmp / 'fighters' / 'x').mkdir(parents=True)
            (tmp / 'fighters' / 'x' / 'fighter.json').write_text(json.dumps(FIGHTER))
            (tmp / 'schema').mkdir()
            (tmp / 'schema' / 'ignored.json').write_text('{"x": 1}')
            cfg = tmp / 'cfg.json'
            cfg.write_text(json.dumps({'factors': {'px/tick:x': 2}, 'set': {'/nowhere': 1}}))
            out, err = io.StringIO(), io.StringIO()
            with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
                rescale.main([str(cfg), str(tmp), '--dry-run'])
            self.assertIn('fighters/x/fighter.json: px/tick:x 5', out.getvalue())
            self.assertIn('/nowhere matched nothing', err.getvalue())
            self.assertEqual(json.loads((tmp / 'fighters' / 'x' / 'fighter.json').read_text()), FIGHTER)
            with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(io.StringIO()):
                rescale.main([str(cfg), str(tmp)])
            text = (tmp / 'fighters' / 'x' / 'fighter.json').read_text()
            self.assertEqual(json.loads(text)['stats']['runSpeed'], 7)
            self.assertEqual(rescale.to_compact_string(json.loads(text)), text)


if __name__ == '__main__':
    unittest.main()
