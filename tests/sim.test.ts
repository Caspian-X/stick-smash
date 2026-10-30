import { describe, expect, it } from 'vitest';
import { DATA, moveTotal } from '../src/data';
import { type Input, type World, blankInput, createWorld } from '../src/sim/core';
import { stepWorld } from '../src/sim/step';
import { activeBoxes } from '../src/sim/combat';
import { LEDGE, STAGE } from '../src/data/stage';

const cpuMatch = (a: number, b: number, seed: number, level = 2) =>
  createWorld({ fighters: [{ ch: a, cpu: true, name: 'A' }, { ch: b, cpu: true, name: 'B' }], stocks: 3, cpuLevel: level, seed });

function run(w: World, frames: number, inputs: (Input | null)[] = []) {
  for (let i = 0; i < frames && !w.over; i++) { stepWorld(w, inputs); w.fx.length = 0; }
}

describe('sim', () => {
  it('CPU vs CPU runs every matchup without errors and is deterministic', () => {
    const n = DATA.fighters.length;
    for (let a = 0; a < n; a++) for (let b = 0; b < n; b++) {
      const w1 = cpuMatch(a, b, 7 + a * 3 + b), w2 = cpuMatch(a, b, 7 + a * 3 + b);
      run(w1, 6000); run(w2, 6000);
      expect(w1.fighters.map(f => [f.x, f.y, f.dmg, f.stocks])).toEqual(w2.fighters.map(f => [f.x, f.y, f.dmg, f.stocks]));
      const dealt = w1.fighters.some(f => f.dmg > 0 || f.stocks < 3);
      expect(dealt).toBe(true);
    }
  });

  it('jab hits a standing dummy for its listed damage and causes hitstun', () => {
    const w = createWorld({ fighters: [{ ch: 0, cpu: false, name: 'P1' }, { ch: 0, cpu: false, name: 'P2' }], stocks: 3, cpuLevel: 0, training: true });
    const [p1, p2] = w.fighters;
    p2.x = p1.x + 40;
    run(w, 1);
    run(w, 1, [{ ...blankInput(), atk: true }]);
    expect(p1.state).toBe('attack');
    run(w, 10);
    const jab = DATA.fighters[0].moves.jab;
    expect(p2.dmg).toBe(jab.kind === undefined ? jab.hitboxes[0].damage : -1);
    expect(['hitstun', 'idle']).toContain(p2.state);
    expect(w.trace?.move).toBe('Jab');
  });

  it('every standard move has hitboxes inside its total length', () => {
    for (const fd of DATA.fighters) for (const [id, d] of Object.entries(fd.moves)) {
      if (d.kind !== undefined) continue;
      for (const h of d.hitboxes) {
        expect(h.start, `${fd.id}.${id}`).toBeLessThanOrEqual(h.end);
        expect(h.end, `${fd.id}.${id}`).toBeLessThanOrEqual(moveTotal(d));
      }
    }
  });

  it('activeBoxes scales damage with smash charge', () => {
    const w = createWorld({ fighters: [{ ch: 0, cpu: false, name: 'P1' }, { ch: 0, cpu: false, name: 'P2' }], stocks: 3, cpuLevel: 0, training: true });
    const p1 = w.fighters[0];
    w.fighters[1].x = 2000;
    run(w, 1);
    run(w, 1, [{ ...blankInput(), atk: true, atkHeld: true, tapR: true, right: true }]);
    expect(p1.move?.id).toBe('fsmash');
    run(w, 40, [{ ...blankInput(), atkHeld: true }]);
    expect(p1.move!.charge).toBeGreaterThan(25);
    const fs = DATA.fighters[0].moves.fsmash;
    if (fs.kind !== undefined) throw new Error();
    let maxDmg = 0;
    for (let i = 0; i < 20; i++) { run(w, 1, [blankInput()]); for (const b of activeBoxes(p1)) maxDmg = Math.max(maxDmg, b.damage); }
    expect(maxDmg).toBeGreaterThan(fs.hitboxes[0].damage);
  });

  it('jumps are either a short hop or a full hop, decided at takeoff', () => {
    const apex = (heldFrames: number) => {
      const w = createWorld({ fighters: [{ ch: 0, cpu: false, name: 'P1' }, { ch: 0, cpu: false, name: 'P2' }], stocks: 3, cpuLevel: 0, training: true });
      const p1 = w.fighters[0], y0 = p1.y;
      w.fighters[1].x = 2000;
      run(w, 1);
      let top = y0;
      for (let i = 0; i < 90; i++) {
        run(w, 1, [{ ...blankInput(), jump: i === 0, jumpHeld: i < heldFrames }]);
        top = Math.min(top, p1.y);
      }
      return y0 - top;
    };
    const short = apex(1), full = apex(90);
    expect(full).toBeGreaterThan(short * 1.5);
    // Releasing jump after takeoff no longer cuts the hop: any hold past jumpsquat = the same full hop.
    expect(apex(DATA.g.movement.jumpsquat + 2)).toBeCloseTo(full, 5);
    expect(apex(DATA.g.movement.jumpsquat - 1)).toBeCloseTo(short, 5);
  });

  it('grabs the ledge when the hand point enters the half-circle zone', () => {
    const w = createWorld({ fighters: [{ ch: 0, cpu: false, name: 'P1' }, { ch: 0, cpu: false, name: 'P2' }], stocks: 3, cpuLevel: 0, training: true });
    const p1 = w.fighters[0];
    w.fighters[1].x = 2000;
    Object.assign(p1, { x: STAGE.R + 30, y: STAGE.top - 20, vx: 0, vy: 2, ground: false, surf: null, state: 'air' });
    for (let i = 0; i < 40 && p1.state !== 'ledge'; i++) run(w, 1, [blankInput()]);
    expect(p1.state).toBe('ledge');
    // Far outside the arc: keeps falling.
    const w2 = createWorld({ fighters: [{ ch: 0, cpu: false, name: 'P1' }, { ch: 0, cpu: false, name: 'P2' }], stocks: 3, cpuLevel: 0, training: true });
    const q = w2.fighters[0];
    w2.fighters[1].x = 2000;
    Object.assign(q, { x: STAGE.R + LEDGE.r + 10, y: STAGE.top - 20, vx: 0, vy: 2, ground: false, surf: null, state: 'air' });
    for (let i = 0; i < 40; i++) run(w2, 1, [blankInput()]);
    expect(q.state).not.toBe('ledge');
  });
});

describe('specials', () => {
  const SWORD = DATA.fighters.findIndex(f => f.id === 'swordfighter'), BRAWLER = DATA.fighters.findIndex(f => f.id === 'brawler');
  const solo = (ch: number) => {
    const w = createWorld({ fighters: [{ ch, cpu: false, name: 'P1' }, { ch, cpu: false, name: 'P2' }], stocks: 3, cpuLevel: 0, training: true });
    w.fighters[1].x = 3000;
    run(w, 1);
    return { w, f: w.fighters[0] };
  };
  const press = (w: World, i: Partial<Input>) => run(w, 1, [{ ...blankInput(), ...i }]);

  it('side + special does the side special; without one it falls back to neutral', () => {
    const s = solo(SWORD);
    press(s.w, { sp: true, right: true });
    expect(s.f.move?.id).toBe('sspec');
    const n = solo(SWORD);
    press(n.w, { sp: true });
    expect(n.f.move?.id).toBe('nspec');
    const b = solo(BRAWLER);
    press(b.w, { sp: true, left: true });
    expect([b.f.move?.id, b.f.face]).toEqual(['nspec', -1]);
  });

  it('B-reverse: up/side special toward the back turns around, and flips air momentum', () => {
    const s = solo(SWORD);
    s.f.face = 1; s.f.ground = false; s.f.y -= 200; s.f.vx = 3; s.f.state = 'air';
    press(s.w, { sp: true, left: true });
    expect([s.f.move?.id, s.f.face]).toEqual(['sspec', -1]);
    expect(s.f.vx).toBeLessThan(0);
    const u = solo(SWORD);
    u.f.face = 1;
    press(u.w, { sp: true, up: true, left: true });
    expect([u.f.move?.id, u.f.face]).toEqual(['uspec', -1]);
  });

  it('neutral special turns around if the other direction is pressed right after, but not later', () => {
    const s = solo(SWORD);
    s.f.face = 1;
    press(s.w, { sp: true });
    press(s.w, { left: true });
    expect([s.f.move?.id, s.f.face]).toEqual(['nspec', -1]);
    const late = solo(SWORD);
    late.f.face = 1;
    press(late.w, { sp: true });
    run(late.w, DATA.g.movement.bReverseWindow + 1);
    press(late.w, { left: true });
    expect(late.f.face).toBe(1);
  });
});
