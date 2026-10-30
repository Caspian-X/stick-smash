import { afterEach, describe, expect, it } from 'vitest';
import { DATA, boxAt } from '../src/data';
import type { Hitbox, PhaseHitbox, StandardMove } from '../src/data/types';
import { type Fighter, type World, blankInput, createWorld } from '../src/sim/core';
import { activeBoxes, applyHit, launchAngle } from '../src/sim/combat';
import { stepWorld } from '../src/sim/step';

const world = () => {
  const w = createWorld({ fighters: [{ ch: 0, cpu: false, name: 'P1' }, { ch: 0, cpu: false, name: 'P2' }], stocks: 3, cpuLevel: 0, training: true });
  return { w, a: w.fighters[0], v: w.fighters[1] };
};
const HB: PhaseHitbox = { x: 0, y: 40, r: 20, damage: 8, bkb: 6, kbg: 0.1, angle: 40 };
const hit = (w: World, a: Fighter, v: Fighter, b: Partial<PhaseHitbox>) => applyHit(w, a, v, { ...HB, ...b }, 1, v.x, v.y - 40, false);

const jab0 = structuredClone(DATA.fighters[0].moves.jab);
afterEach(() => { DATA.fighters[0].moves.jab = structuredClone(jab0); });
/** Replace brawler's jab with custom hitboxes for a test. */
const setJab = (boxes: Hitbox[], endLag = 20) => { DATA.fighters[0].moves.jab = { name: 'Jab', endLag, hitboxes: boxes } as StandardMove; };
function run(w: World, frames: number, first?: Partial<ReturnType<typeof blankInput>>) {
  for (let i = 0; i < frames; i++) { stepWorld(w, [i === 0 && first ? { ...blankInput(), ...first } : blankInput(), blankInput()]); w.fx.length = 0; }
}

describe('hitbox types', () => {
  it('spike always launches downward and bounces grounded victims up', () => {
    const { w, a, v } = world();
    v.ground = false; v.y -= 100;
    hit(w, a, v, { type: 'spike', angle: 70 });
    expect(v.vy).toBeGreaterThan(0);
    const g = world();
    hit(g.w, g.a, g.v, { type: 'spike', angle: -80 });
    expect(g.v.vy).toBeLessThan(0);
  });

  it('trip knocks a grounded victim prone but hits an airborne one normally', () => {
    const { w, a, v } = world();
    hit(w, a, v, { type: 'trip', bkb: 3, kbg: 0.02 });
    expect(v.state).toBe('knockdown');
    expect(v.dmg).toBe(8);
    const air = world();
    air.v.ground = false; air.v.y -= 120;
    hit(air.w, air.a, air.v, { type: 'trip' });
    expect(air.v.state).toBe('hitstun');
  });

  it('sakurai angle: flat on weak grounded hits, 45° in the air', () => {
    const s = DATA.g.hitTypes.sakurai;
    expect(launchAngle({ ...HB, type: 'sakurai' }, 5, true)).toBe(s.groundLow);
    expect(launchAngle({ ...HB, type: 'sakurai' }, 40, true)).toBe(s.groundHigh);
    expect(launchAngle({ ...HB, type: 'sakurai' }, 5, false)).toBe(s.air);
  });

  it('autolink pulls the victim toward the hitbox center without tumble', () => {
    const { w, a, v } = world();
    v.x = a.x + 60; v.ground = false; v.y -= 50;
    applyHit(w, a, v, { ...HB, type: 'autolink', bkb: 30 }, 1, a.x + 20, v.y - 40, false);
    expect(v.vx).toBeLessThan(0);
    expect(v.tumble).toBe(false);
  });

  it('fixed knockback ignores percent', () => {
    const lo = world(), hi = world();
    hi.v.dmg = 150;
    hit(lo.w, lo.a, lo.v, { setKb: true, kbg: 0.3, angle: 90 });
    hit(hi.w, hi.a, hi.v, { setKb: true, kbg: 0.3, angle: 90 });
    expect(hi.v.vy).toBeCloseTo(lo.v.vy, 6);
  });

  it('electric hits keep the victim in hitlag longer than the attacker', () => {
    const { w, a, v } = world();
    hit(w, a, v, { type: 'electric' });
    expect(v.lag).toBeGreaterThan(a.lag);
  });

  it('windbox pushes without damage or hitstun, every frame', () => {
    setJab([{ start: 2, end: 12, x: 40, y: 40, r: 40, damage: 0, bkb: 3, kbg: 0, angle: 0, type: 'windbox' }]);
    const { w, a, v } = world();
    v.x = a.x + 50; a.face = 1;
    const x0 = v.x;
    run(w, 1); run(w, 12, { atk: true });
    expect(v.dmg).toBe(0);
    expect(v.state).not.toBe('hitstun');
    expect(v.x).toBeGreaterThan(x0 + 10);
  });

  it('multi-hit boxes hit again every `rehit` frames; normal boxes hit once', () => {
    setJab([{ start: 2, end: 13, x: 30, y: 40, r: 30, damage: 1, bkb: 2, kbg: 0, angle: 0, setKb: true, rehit: 4, type: 'autolink' }]);
    const { w, a, v } = world();
    v.x = a.x + 30; a.face = 1;
    run(w, 1); run(w, 40, { atk: true });
    expect(v.dmg).toBeGreaterThanOrEqual(3);
    DATA.fighters[0].moves.jab = structuredClone(jab0);
    const once = world();
    once.v.x = once.a.x + 40;
    run(once.w, 1); run(once.w, 20, { atk: true });
    expect(once.v.dmg).toBe((jab0 as StandardMove).hitboxes[0].damage);
  });
});

describe('per-frame hitboxes', () => {
  const b: Hitbox = { start: 3, end: 6, x: 10, y: 20, r: 5, damage: 1, bkb: 1, kbg: 0, angle: 0, frames: [[3, 10, 20, 5], [4, 30, 22, 8], [6, 50, 25, 9]] };
  it('resolves each frame from the closest entry at or before it', () => {
    expect(boxAt(b, 3)).toMatchObject({ x: 10, y: 20, r: 5 });
    expect(boxAt(b, 4)).toMatchObject({ x: 30, y: 22, r: 8 });
    expect(boxAt(b, 5)).toMatchObject({ x: 30, y: 22, r: 8 });
    expect(boxAt(b, 6)).toMatchObject({ x: 50, y: 25, r: 9 });
  });
  it('the sim uses the per-frame position', () => {
    setJab([b]);
    const { w, a } = world();
    w.fighters[1].x = 3000;
    run(w, 1); run(w, 5, { atk: true });
    expect(a.move?.t).toBe(4);
    expect(activeBoxes(a)[0]).toMatchObject({ wx: a.x + 30 * a.face, r: 8 });
  });
});
