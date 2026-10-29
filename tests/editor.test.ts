import { describe, expect, it } from 'vitest';
import { DATA } from '../src/data';
import type { StandardMove } from '../src/data/types';
import { entries, patchFighterSource, patchMove } from '../src/editor/source';
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
      const own = movesOf(src).map(e => e.key).filter(k => k && f.moves[k as keyof typeof f.moves].kind === undefined) as string[];
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
    expect(out).toContain('{ start: 6, end: 8, x: 34, y: 46, r: 25, damage: 0, bkb: 0, kbg: 0, angle: 0, grab: true }');
    expect(patchMove(out, 'grab', grab)).toBe(out);
  });
});
