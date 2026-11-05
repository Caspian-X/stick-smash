#!/usr/bin/env python3
"""Rescale content numbers by unit, driven by the `x-unit` / `x-axis` tags in `assets/schema/` (see
`crates/sim/src/data/unit.rs`).

    python3 tools/rescale.py CONFIG [PATH ...] [--dry-run] [--schemas DIR]

PATHs are content files or folders (default `assets/`, skipping `schema/`). Only values present in each file are
touched, so a fighter with `extends` keeps inheriting what it doesn't override. Files are written back in the
canonical compact format (`sim::data::json::to_compact_string`, as `just bless` writes them).

CONFIG (JSON):
    {
      "factors": {"px/tick:x": "5/3.5", "px/tick2:y": "5/6", "*:xy": 1, "kb": 1},
      "scale": {"fighters/*/fighter.json#/stats/gravity": 1.1},
      "set": {"global.json#/knockback/ultimate/launchSpeed": 0.15},
      "skip": ["bkb", "Hitbox.kbg", "/render", "stages/*/stage.json#/art"],
      "digits": 6
    }

- `factors`: per tag. A value tagged (unit, axis) uses the first of `unit:axis`, `unit`, `*:axis`; unmatched values
  stay as they are. Factors are numbers or "a/b" strings.
- `scale` / `set`: a factor, or an absolute value, for the numbers at matching locations (overrides `factors`).
- `skip`: locations left alone (wins over everything).
- A location is a property name (`bkb`), `Def.property` (schema definition, `Hitbox.bkb`), or a JSON pointer
  (`/moves/*/phases`; `*` matches one segment; a pointer also covers everything below it), each optionally
  prefixed by `file-glob#` (relative to the path argument it was found under, e.g. `stages/*/stage.json#/blast`).
- `digits`: decimal places results are rounded to (default 6, `null` for none). Integral results print as integers.

Tuple fields tag each element (`[frame, x, y, r]` hitbox tracks); `[min, max]` ranges share their field's tag. Prints
per-file counts of changed numbers by `unit:axis` (`--dry-run` writes nothing).
"""

import argparse
import fnmatch
import json
import math
import sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
WIDTH, INDENT = 120, '  '


# --- canonical formatting (port of crates/sim/src/data/json.rs) -------------------------------------------------

def fmt_float(v):
    """serde_json's (ryu) shortest round-trip float text."""
    if v == 0:
        return '-0.0' if math.copysign(1, v) < 0 else '0.0'
    r = repr(v)
    sign = '-' if r.startswith('-') else ''
    mant, _, exp = r.lstrip('-').partition('e')
    ip, _, fp = mant.partition('.')
    k = (int(exp) if exp else 0) - len(fp)
    digits = (ip + fp).lstrip('0')
    stripped = digits.rstrip('0')
    k += len(digits) - len(stripped)
    digits, n = stripped, len(stripped)
    kk = n + k
    if 0 <= k and kk <= 16:
        s = digits + '0' * k + '.0'
    elif 0 < kk <= 16:
        s = digits[:kk] + '.' + digits[kk:]
    elif -5 < kk <= 0:
        s = '0.' + '0' * -kk + digits
    elif n == 1:
        s = f'{digits}e{kk - 1}'
    else:
        s = f'{digits[0]}.{digits[1:]}e{kk - 1}'
    return sign + s


def inline(v):
    if isinstance(v, dict):
        if not v:
            return '{}'
        return '{ ' + ', '.join(f'{inline(k)}: {inline(x)}' for k, x in v.items()) + ' }'
    if isinstance(v, list):
        return '[' + ', '.join(inline(x) for x in v) + ']'
    if isinstance(v, bool) or v is None or isinstance(v, str):
        return json.dumps(v, ensure_ascii=False)
    if isinstance(v, int):
        return str(v)
    return fmt_float(v)


def has_nested_object(v):
    kids = v.values() if isinstance(v, dict) else v if isinstance(v, list) else []
    return any(isinstance(k, dict) or has_nested_object(k) for k in kids)


def fits(v, line, depth, prefix):
    n = depth * len(INDENT) + prefix + len(line) + 1
    return n <= 40 or (n <= WIDTH and not has_nested_object(v))


def write_value(out, v, depth, prefix):
    if isinstance(v, (dict, list)) and v:
        line = inline(v)
        if fits(v, line, depth, prefix):
            out.append(line)
            return
        items = list(v.items()) if isinstance(v, dict) else list(enumerate(v))
        out.append('{' if isinstance(v, dict) else '[')
        for i, (k, x) in enumerate(items):
            out.append('\n' + INDENT * (depth + 1))
            key = ''
            if isinstance(v, dict):
                key = json.dumps(k, ensure_ascii=False)
                out.append(key + ': ')
            write_value(out, x, depth + 1, len(key) + 2 if key else 0)
            if i + 1 < len(items):
                out.append(',')
        out.append('\n' + INDENT * depth + ('}' if isinstance(v, dict) else ']'))
    else:
        out.append(inline(v))


def to_compact_string(v):
    out = []
    write_value(out, v, 0, 0)
    return ''.join(out) + '\n'


# --- schema-guided walk -----------------------------------------------------------------------------------------

def load_schemas(folder):
    return {k: json.loads((folder / f'{k}.schema.json').read_text()) for k in ('global', 'stage', 'fighter')}


def schema_for(rel, doc, schemas):
    """`(root schema, node)` for a content file, by its `$schema` or its path."""
    name = Path(doc.get('$schema', '')).name.removesuffix('.schema.json') if isinstance(doc, dict) else ''
    parts = rel.parts
    if not name:
        if parts[-1] == 'global.json':
            name = 'global'
        elif parts[-1] in ('stage.json', 'fighter.json'):
            name = parts[-1].removesuffix('.json')
        elif 'rigs' in parts:
            return schemas['fighter'], schemas['fighter']['$defs']['Rig']
    if name in schemas:
        return schemas[name], schemas[name]
    return None, None


def types(node):
    t = node.get('type', [])
    return set(t if isinstance(t, list) else [t])


def branch_fits(node, value, root):
    node = resolve(node, root)[0]
    t = types(node)
    if isinstance(value, bool):
        return not t or 'boolean' in t
    if isinstance(value, (int, float)):
        return not t or t & {'number', 'integer'}
    if isinstance(value, str):
        return not t or 'string' in t
    if isinstance(value, list):
        return not t or 'array' in t
    if value is None:
        return not t or 'null' in t
    if t and 'object' not in t:
        return False
    if 'oneOf' in node or 'anyOf' in node:
        return any(branch_fits(b, value, root) for b in node.get('oneOf', []) + node.get('anyOf', []))
    props = node.get('properties', {})
    for k, v in value.items():
        if k not in props and node.get('additionalProperties') is False:
            return False
        p = props.get(k, {})
        if 'const' in p and p['const'] != v or 'enum' in p and v not in p['enum']:
            return False
    return True


def resolve(node, root, tag=None, here=None):
    """Follow `$ref`s, picking up the outermost unit tag and the definition name."""
    while True:
        if tag is None and 'x-unit' in node:
            tag = (node['x-unit'], node.get('x-axis'))
        ref = node.get('$ref')
        if not ref:
            return node, tag, here
        here = ref.rsplit('/', 1)[-1].removesuffix('Partial')
        node = root['$defs'][ref.rsplit('/', 1)[-1]]


def walk(node, value, root, ptr, tag, here, owner, prop, index, visit):
    """Call `visit(ptr, value, tag, owner, prop, index)` for every number in `value`. `here` is the definition being
    walked; `owner` the one holding `prop`."""
    node, tag, here = resolve(node, root, tag, here)
    for key in ('oneOf', 'anyOf'):
        if key in node:
            for b in node[key]:
                if branch_fits(b, value, root):
                    walk(b, value, root, ptr, tag, here, owner, prop, index, visit)
                    return
            return
    if isinstance(value, bool) or value is None or isinstance(value, str):
        return
    if isinstance(value, (int, float)):
        visit(ptr, value, tag, owner, prop, index)
    elif isinstance(value, list):
        prefix = node.get('prefixItems', [])
        for i, x in enumerate(value):
            item = prefix[i] if i < len(prefix) else node.get('items', {})
            walk(item, x, root, f'{ptr}/{i}', tag, here, owner, prop, i, visit)
    elif isinstance(value, dict):
        props = node.get('properties', {})
        extra = node.get('additionalProperties')
        for k, x in value.items():
            if k in props:
                walk(props[k], x, root, f'{ptr}/{esc(k)}', tag, here, here, k, None, visit)
            elif isinstance(extra, dict):
                walk(extra, x, root, f'{ptr}/{esc(k)}', tag, here, owner, prop, None, visit)


def esc(k):
    return k.replace('~', '~0').replace('/', '~1')


def pick(v, index):
    if isinstance(v, list):
        return v[index] if index is not None and index < len(v) else None
    return v


# --- config -----------------------------------------------------------------------------------------------------

def number(v):
    if isinstance(v, str):
        a, _, b = v.partition('/')
        return float(a) / float(b) if b else float(a)
    return float(v)


class Location:
    """`prop`, `Def.prop` or `/json/pointer`, optionally `file-glob#...`."""

    def __init__(self, text):
        self.text = text
        self.file, _, spec = text.rpartition('#')
        if spec.startswith('/'):
            self.ptr = [s for s in spec.split('/')[1:]]
            self.owner = self.prop = None
        else:
            self.ptr = None
            self.owner, _, self.prop = spec.rpartition('.')

    def matches(self, rel, ptr, owner, prop):
        if self.file and not fnmatch.fnmatch(rel, self.file):
            return False
        if self.ptr is not None:
            segs = ptr.split('/')[1:]
            return len(segs) >= len(self.ptr) and all(p in ('*', s) for p, s in zip(self.ptr, segs))
        return prop == self.prop and (not self.owner or owner == self.owner)


class Config:
    def __init__(self, raw):
        self.factors = {k: number(v) for k, v in raw.get('factors', {}).items()}
        self.scale = [(Location(k), number(v)) for k, v in raw.get('scale', {}).items()]
        self.set = [(Location(k), number(v)) for k, v in raw.get('set', {}).items()]
        self.skip = [Location(k) for k in raw.get('skip', [])]
        self.digits = raw.get('digits', 6)
        self.used = set()
        unknown = set(raw) - {'factors', 'scale', 'set', 'skip', 'digits'}
        if unknown:
            raise SystemExit(f'unknown config keys: {sorted(unknown)}')

    def factor(self, unit, axis):
        for key in (f'{unit}:{axis}' if axis else None, unit, f'*:{axis}' if axis else None):
            if key in self.factors:
                return self.factors[key]
        return 1.0

    def new_value(self, rel, ptr, value, tag, owner, prop, index):
        """`(new value or None, label)`."""
        if any(s.matches(rel, ptr, owner, prop) for s in self.skip):
            return None, None
        unit = pick(tag[0], index) if tag else None
        axis = pick(tag[1], index) if tag else None
        label = f'{unit}:{axis}' if axis else unit or 'untagged'
        for loc, v in self.set:
            if loc.matches(rel, ptr, owner, prop):
                self.used.add(loc.text)
                return v, label
        f = None
        for loc, v in self.scale:
            if loc.matches(rel, ptr, owner, prop):
                self.used.add(loc.text)
                f = v
                break
        if f is None:
            f = self.factor(unit, axis) if unit else 1.0
        return (None, None) if f == 1 else (value * f, label)

    def tidy(self, v, original):
        if self.digits is not None:
            v = round(v, self.digits)
        if v == 0:
            return 0 if isinstance(original, int) else 0.0
        return int(v) if v == int(v) and abs(v) < 2 ** 53 else v


# --- files ------------------------------------------------------------------------------------------------------

def content_files(paths):
    """`(file, path relative to its argument folder)` for every JSON file."""
    for p in paths:
        if p.is_file():
            base = next((a for a in p.parents if (a / 'global.json').exists() or a.name == 'assets'), p.parent)
            yield p, p.relative_to(base)
        else:
            for f in sorted(p.rglob('*.json')):
                rel = f.relative_to(p)
                if 'schema' not in rel.parts and not f.is_symlink():
                    yield f, rel


def set_ptr(doc, ptr, v):
    segs = [s.replace('~1', '/').replace('~0', '~') for s in ptr.split('/')[1:]]
    for s in segs[:-1]:
        doc = doc[int(s)] if isinstance(doc, list) else doc[s]
    last = segs[-1]
    if isinstance(doc, list):
        doc[int(last)] = v
    else:
        doc[last] = v


def rescale(doc, root, node, rel, cfg):
    """Rescale `doc` in place; returns per-label counts of changed numbers."""
    counts, changes = Counter(), []

    def visit(ptr, value, tag, owner, prop, index):
        v, label = cfg.new_value(rel, ptr, value, tag, owner, prop, index)
        if v is not None:
            v = cfg.tidy(v, value)
            if v != value or type(v) is not type(value):
                changes.append((ptr, v))
                counts[label] += 1

    walk(node, doc, root, '', None, node.get('title'), None, None, None, visit)
    for ptr, v in changes:
        set_ptr(doc, ptr, v)
    return counts


def main(argv=None):
    ap = argparse.ArgumentParser(description=(__doc__ or '').split('\n\n')[0])
    ap.add_argument('config', type=Path)
    ap.add_argument('paths', nargs='*', type=Path)
    ap.add_argument('--dry-run', action='store_true', help='print counts without writing')
    ap.add_argument('--schemas', type=Path, default=ROOT / 'assets' / 'schema')
    args = ap.parse_args(argv)
    cfg = Config(json.loads(args.config.read_text()))
    schemas = load_schemas(args.schemas)
    total = Counter()
    for path, rel in content_files(args.paths or [ROOT / 'assets']):
        text = path.read_text()
        doc = json.loads(text)
        root, node = schema_for(rel, doc, schemas)
        if root is None:
            print(f'{rel}: no schema, skipped', file=sys.stderr)
            continue
        counts = rescale(doc, root, node, rel.as_posix(), cfg)
        if not counts:
            continue
        total += counts
        print(f'{rel}: ' + ', '.join(f'{k} {n}' for k, n in sorted(counts.items())))
        if not args.dry_run:
            path.write_text(to_compact_string(doc))
    print('total: ' + (', '.join(f'{k} {n}' for k, n in sorted(total.items())) or 'nothing to change'))
    for loc, _ in cfg.set + cfg.scale:
        if loc.text not in cfg.used:
            print(f'warning: {loc.text} matched nothing', file=sys.stderr)


if __name__ == '__main__':
    main()
