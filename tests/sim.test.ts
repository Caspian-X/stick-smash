import { describe, expect, it } from 'vitest';
import { DATA, moveTotal } from '../src/data';
import { type Input, type World, blankInput, createWorld } from '../src/sim/core';
import { stepWorld } from '../src/sim/step';
import { activeBoxes } from '../src/sim/combat';

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
});
