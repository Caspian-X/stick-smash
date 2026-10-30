import { afterEach, describe, expect, it } from 'vitest';
import { DATA } from '../src/data';
import type { StandardMove } from '../src/data/types';
import { entries, patchFighterSource, patchMove } from '../src/editor/source';
import { EditorModel } from '../src/editor/model';
import brawlerSrc from '../src/data/fighters/brawler.ts?raw';
import swordSrc from '../src/data/fighters/swordfighter.ts?raw';
import kingSrc from '../src/data/fighters/king.ts?raw';

const SOURCES: Record<string, string> = { brawler: brawlerSrc, swordfighter: swordSrc, king: kingSrc };
const movesOf = (src: string) => entries(src, src.indexOf('{', src.search(/\bmoves\s*:/)));
const clone = <T>(v: T): T => JSON.parse(JSON.stringify(v));

describe('hitbox editor source patching', () => {
  it('re-saving unchanged moves leaves every fighter file byte-identical', () => {
    for (const f of DATA.fighters) {
      const src = SOURCES[f.id];
      const own = movesOf(src).map(e => e.key).filter(k => k && f.moves[k as keyof typeof f.moves]?.kind === undefined) as string[];
      expect(own.length).toBeGreaterThan(5);
      const moves = Object.fromEntries(own.map(k => [k, f.moves[k as keyof typeof f.moves] as StandardMove]));
      expect(patchFighterSource(src, moves)).toBe(src);
    }
  });

  it('writes edited hitboxes and end lag into the right move only', () => {
    const jab = clone(DATA.fighters[0].moves.jab) as StandardMove;
    jab.endLag = 99;
    jab.hitboxes[0].r = 31;
    jab.hitboxes.push({ ...jab.hitboxes[0], x: -20, dir: 'back' });
    const out = patchMove(brawlerSrc, 'jab', jab);
    expect(out).toContain("name: 'Jab', endLag: 99,");
    expect(out).toContain("{ start: 3, end: 6, x: -20, y: 52, r: 31, damage: 4, bkb: 4, kbg: 0.05, angle: 30, dir: 'back' },");
    expect(out.replace(/ {4}jab: \{[\s\S]*?\n {4}\},\n/, '')).toBe(brawlerSrc.replace(/ {4}jab: \{[\s\S]*?\n {4}\},\n/, ''));
  });

  it('adds an inherited move (common grab) as its own entry', () => {
    const grab = clone(DATA.fighters[0].moves.grab) as StandardMove;
    grab.hitboxes[0].r = 25;
    const out = patchMove(brawlerSrc, 'grab', grab);
    const keys = movesOf(out).map(e => e.key);
    expect(keys.slice(0, 3)).toEqual([null, 'grab', 'jab']);
    expect(out).toContain("{ start: 6, end: 8, x: 34, y: 46, r: 25, damage: 0, bkb: 0, kbg: 0, angle: 0, type: 'grab' }");
    expect(patchMove(out, 'grab', grab)).toBe(out);
  });

  it('writes type, multi-hit and per-frame data in canonical order', () => {
    const jab = clone(DATA.fighters[0].moves.jab) as StandardMove;
    Object.assign(jab.hitboxes[0], { frames: [[3, 36, 52, 18], [4, 40, 54, 20]], rehit: 3, type: 'autolink' });
    const out = patchMove(brawlerSrc, 'jab', jab);
    expect(out).toContain("angle: 30, type: 'autolink', rehit: 3, frames: [[3, 36, 52, 18], [4, 40, 54, 20]] }");
  });
});

describe('editor model', () => {
  const fresh = () => { const m = new EditorModel(); m.fi = 1; m.selectMove('ftilt'); return m; };
  afterEach(() => { DATA.fighters[1].moves.ftilt = structuredClone(sword.ftilt); });
  const sword = clone(DATA.fighters[1].moves);

  it('per-frame mode keys every active frame, then edits only the current one', () => {
    const m = fresh(), b = m.box()!;
    m.setFrame(b.start);
    m.setPerFrame(true);
    expect(b.frames!.map(f => f[0])).toEqual(Array.from({ length: b.end - b.start + 1 }, (_, i) => b.start + i));
    m.setFrame(b.start + 2);
    m.begin(); m.setGeom({ x: 99, r: 7 });
    expect(m.geom(b, b.start + 2)).toMatchObject({ x: 99, r: 7 });
    expect(m.geom(b, b.start + 1).x).toBe(b.x);
    m.applyToAll();
    for (let t = b.start; t <= b.end; t++) expect(m.geom(b, t)).toMatchObject({ x: 99, r: 7 });
    expect(m.isDirty(1, 'ftilt')).toBe(true);
  });

  it('copy → next frame, retiming, undo and redo', () => {
    const m = fresh(), b = m.box()!, s = b.start, e = b.end;
    m.setFrame(s); m.setPerFrame(true);
    m.begin(); m.setGeom({ y: 80 });
    m.copyToNext();
    expect(m.frame).toBe(s + 1);
    expect(m.geom(b, s + 1).y).toBe(80);
    m.begin(); m.setWindow(0, s, e + 3);
    expect(b.frames!.length).toBe(e + 3 - s + 1);
    expect(m.geom(m.box()!, e + 3)).toEqual(m.geom(m.box()!, e));
    m.undo();
    expect(m.box()!.end).toBe(e);
    m.redo();
    expect(m.box()!.end).toBe(e + 3);
    m.undo(); m.undo(); m.undo(); m.undo();
    expect(m.isDirty(1, 'ftilt')).toBe(false);
  });

  it('switching type applies sensible defaults', () => {
    const m = fresh();
    m.setType('spike');
    expect(m.box()!.angle).toBeLessThan(0);
    m.setType('grab');
    expect(m.box()).toMatchObject({ type: 'grab', damage: 0 });
    m.setType('normal');
    expect(m.box()!.type).toBeUndefined();
    expect(m.box()!.damage).toBeGreaterThan(0);
  });
});
