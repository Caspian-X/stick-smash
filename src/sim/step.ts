// One 60Hz simulation tick.

import { BLAST } from '../data/stage';
import { type Input, type World, EMPTY, G, actionable, setState, sfx } from './core';
import { hitPass, releaseGrab } from './combat';
import { stepProjectiles } from './projectiles';
import { ko, updateFighter } from './states';
import { aiInput } from './ai';

const NO_PUSH = ['out', 'dead', 'respawn', 'ledge', 'climb', 'grabbed', 'hold', 'throw', 'win', 'roll', 'spot', 'airdodge'];
const NO_KO = ['out', 'dead', 'respawn', 'grabbed', 'win'];

/** Advance the world one frame. `inputs[i]` is used for non-CPU fighters (null/undefined = no input). */
export function stepWorld(w: World, inputs: (Input | null | undefined)[]) {
  const g = G();
  w.frame++;
  if (w.count > 0) {
    if (w.count % 60 === 0) sfx(w, 'beep');
    if (--w.count === 0) { w.goT = g.match.goFrames; sfx(w, 'go'); }
  }
  if (w.goT > 0) w.goT--;
  const frozen = w.count > 0 || w.endT > 0;
  const ins = w.fighters.map((f, i) => frozen ? EMPTY : f.cpu ? aiInput(w, f) : inputs[i] ?? EMPTY);
  w.fighters.forEach((f, i) => updateFighter(w, f, ins[i]));
  hitPass(w);
  stepProjectiles(w);

  const [a, b] = w.fighters;
  if (a && b && !NO_PUSH.includes(a.state) && !NO_PUSH.includes(b.state)) {
    const dx = b.x - a.x, r = g.movement.pushRadius;
    if (Math.abs(dx) < r && Math.abs(a.y - b.y) < 50) {
      const s = dx === 0 ? 1 : Math.sign(dx), push = (r - Math.abs(dx)) * g.movement.pushStrength;
      a.x -= s * push; b.x += s * push;
    }
  }

  for (const f of w.fighters) {
    if (NO_KO.includes(f.state)) continue;
    if (f.x < BLAST.L || f.x > BLAST.R || f.y > BLAST.B || f.y < BLAST.T) ko(w, f);
  }

  const tr = w.trace;
  if (tr && !tr.done) {
    const at = w.fighters[tr.attacker], df = w.fighters[tr.defender];
    if (tr.aFree < 0 && at && actionable(at)) tr.aFree = w.frame;
    if (tr.dFree < 0 && df && actionable(df)) tr.dFree = w.frame;
    if (tr.aFree >= 0 && tr.dFree >= 0) tr.done = true;
  }

  if (w.endT > 0 && --w.endT === 0 && w.winner) {
    w.over = true;
    const win = w.winner;
    releaseGrab(win); setState(win, 'win'); win.vx = win.vy = 0;
  }
}
