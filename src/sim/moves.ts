// Starting and stepping attacks, including special-move behaviours.

import { moveTotal } from '../data';
import type { MoveId } from '../data/types';
import {
  type Fighter, type Input, type MoveState, type World, FD, G, MV, ST, col, dust, part, ring, rnd, setState, sfx, shake,
} from './core';
import { fall, fastFallCheck, airDrift, integrate, tryLedge } from './physics';
import { spawnGordo, spawnProj } from './projectiles';

export function startMove(w: World, f: Fighter, id: MoveId) {
  setState(f, 'attack', { id, t: 0, ph: 0, t2: 0, charge: 0 });
  f.move = f.sd as MoveState;
  const d = MV(f, id);
  if (d.kind === undefined && d.hitboxes.length && id !== 'uspec' && !FD(f).weapon) sfx(w, 'swing');
}

export function quake(w: World, f: Fighter, ph = 2) {
  const m = f.move!; m.ph = ph; m.t2 = 0; m.hit = new Set();
  ring(w, f.x, f.y - 6, 10, 7, 16, col(f));
  dust(w, f.x - 30, f.y, 8); dust(w, f.x + 30, f.y, 8);
  shake(w, 8); sfx(w, 'quake');
}

const done = (f: Fighter) => setState(f, f.ground ? 'idle' : 'air');

function stepMove(w: World, f: Fighter, inp: Input) {
  const m = f.move!, d = MV(f), c = d.charge;
  if (c && m.t === c.frame && (c.button === 'special' ? inp.spHeld : inp.atkHeld) && m.charge < (c.max ?? 60)) {
    m.charge++;
    if (m.charge % 10 === 0) sfx(w, 'charge');
    if (m.charge % 3 === 0) part(w, f.x + rnd(w, -20, 20), f.y - rnd(w, 10, 80), 0, -1, 14, 0xffe680, 3);
    if (c.jet) {
      part(w, f.x - f.face * rnd(w, 56, 66), f.y - rnd(w, 56, 70), -f.face * rnd(w, 2, 4), rnd(w, -0.5, 0.5), 12, m.charge % 2 ? 0xff9a3c : 0xffe680, rnd(w, 3, 6));
      if (m.charge % 6 === 0) sfx(w, 'jet');
    }
    return;
  }
  m.t++;
  const weapon = FD(f).weapon;
  if (d.kind === undefined) {
    const first = d.hitboxes[0];
    if (c && first && m.t === first.start) sfx(w, 'smash');
    if (weapon && d.hitboxes.some(b => b.start === m.t)) sfx(w, weapon === 'sword' ? 'slash' : 'whack');
    if (d.forwardStep && first && m.t === first.start) f.vx = f.face * d.forwardStep;
  }
  if (d.quakeFrame === m.t && f.ground) {
    const x = f.x + f.face * 76;
    dust(w, x, f.y, 10); ring(w, x, f.y - 4, 8, 5, 12);
    shake(w, 6); sfx(w, 'quake');
  }

  switch (d.kind) {
    case 'projectile':
      if (!f.ground) f.vy = Math.min(f.vy, d.airFallCap);
      if (m.t === d.spawnFrame) spawnProj(w, f, d);
      break;
    case 'gordo':
      if (!f.ground) f.vy = Math.min(f.vy, d.airFallCap);
      if (m.t === d.spawnFrame) spawnGordo(w, f, d);
      break;
    case 'superJump':
      if (m.ph === 0) {
        f.vx *= 0.6;
        if (m.t === d.launchFrame) {
          m.ph = 1; f.vy = -d.launchVy; f.vx = f.face * d.launchVx; f.ground = false; f.surf = null;
          dust(w, f.x, f.y, 8); sfx(w, 'djump');
        }
      } else if (m.ph === 1) {
        if (w.frame % 2 === 0) part(w, f.x, f.y - 30, 0, 1, 16, col(f), 6);
        if (f.vy >= 0) { m.ph = 2; m.hit = new Set(); f.vx *= 0.4; sfx(w, 'smash'); }
      } else if (m.ph === 2) {
        f.vy = d.plungeSpeed;
        if (w.frame % 2 === 0) part(w, f.x + rnd(w, -8, 8), f.y - 50, 0, -2, 14, 0xffe680, 5);
        if (inp.shieldP) setState(f, 'helpless');
      } else if (++m.t2 >= d.recovery) done(f);
      return;
    case 'counter':
      if (m.ph === 1) {
        if (++m.t2 === d.strike[0] && m.cd) sfx(w, 'slash');
        if (m.t2 >= d.recovery) done(f);
        return;
      }
      break;
    case 'groundPound':
      if (m.ph === 0) {
        if (!f.ground) f.vx *= 0.8;
        if (m.t >= d.startup) { if (f.ground) quake(w, f); else m.ph = 1; }
      } else if (m.ph === 1) {
        f.vy = d.fallSpeed; f.vx *= 0.9;
        if (w.frame % 2 === 0) part(w, f.x + rnd(w, -6, 6), f.y - 30, 0, -2, 14, col(f), 5);
        if (f.ground) quake(w, f); else if (m.t > d.timeout) setState(f, 'air');
      } else if (++m.t2 >= d.recovery) done(f);
      return;
    case undefined:
      if (d.dash && m.t === d.dash.frame) { f.vx = f.face * (f.ground ? d.dash.groundSpeed : d.dash.airSpeed); dust(w, f.x, f.y, 4); }
      if (d.rise) {
        const r = d.rise;
        if (m.t < r.frame) f.vx *= 0.5;
        if (m.t === r.frame) { f.vy = -r.vy; f.vx = f.face * r.vx; f.ground = false; f.surf = null; sfx(w, 'djump'); }
        if (m.t > r.frame && m.t < r.frame + 14 && w.frame % 2 === 0) part(w, f.x, f.y - 40, 0, 1, 16, col(f), 6);
      }
      break;
  }
  if (m.t >= moveTotal(d)) setState(f, f.ground ? 'idle' : d.helpless ? 'helpless' : 'air');
}

export function stAttack(w: World, f: Fighter, inp: Input, dir: number) {
  const m = f.move!, d = MV(f), mv = G().movement;
  if (m.id === 'nspec' && !m.rev && m.t < mv.bReverseWindow && dir === -f.face) { m.rev = true; bReverse(f, dir); }
  stepMove(w, f, inp);
  if (f.ground) {
    f.vx *= d.slide && m.t < d.slide ? mv.attackSlideFriction : mv.attackFriction;
    fall(w, f, true);
    return;
  }
  const pound = d.kind === 'groundPound', sj = d.kind === 'superJump', plunge = sj && m.ph === 2;
  const acc = d.aerial ? mv.aerialAccel : m.id === 'uspec' ? (plunge ? mv.upSpecialAccel / 2 : mv.upSpecialAccel) : pound ? 0 : mv.attackAirAccel;
  airDrift(f, dir, acc, mv.attackAirDrag);
  if (d.aerial) fastFallCheck(f, inp);
  const s = ST(f);
  const cap = pound && m.ph === 1 ? d.fallSpeed : plunge ? d.plungeSpeed : f.ff ? s.fastFallSpeed : s.fallSpeed;
  f.vy = Math.min(f.vy + s.gravity, cap);
  const riseFrame = d.kind === undefined && d.rise ? d.rise.frame : 4;
  if ((pound && m.ph === 0) || (m.id === 'uspec' && (sj ? m.ph === 0 : m.t < riseFrame))) f.vy = 0;
  integrate(w, f, false);
  if (f.state === 'attack' && m.id === 'uspec' && m.t > 6) tryLedge(w, f, inp, true);
}

/** Turn to face `dir` for a special. In the air, horizontal momentum flips too (B-reverse). */
function bReverse(f: Fighter, dir: number) {
  if (!dir || dir === f.face) return;
  f.face = dir;
  if (!f.ground) f.vx = -f.vx;
}

export function upSpecial(w: World, f: Fighter, dir: number) {
  f.upB = true; bReverse(f, dir); startMove(w, f, 'uspec');
}

export function special(w: World, f: Fighter, inp: Input, dir: number) {
  if (inp.up && !f.upB) { upSpecial(w, f, dir); return; }
  bReverse(f, dir);
  startMove(w, f, inp.down ? 'dspec' : dir && FD(f).moves.sspec ? 'sspec' : 'nspec');
}

export function groundActions(w: World, f: Fighter, inp: Input, dir: number): boolean {
  const mv = G().movement;
  if (inp.grab) { if (dir) f.face = dir; startMove(w, f, Math.abs(f.vx) > mv.dashGrabSpeed ? 'dashgrab' : 'grab'); return true; }
  if (inp.shield) { setState(f, 'shield'); return true; }
  if (inp.sp) { special(w, f, inp, dir); return true; }
  if (inp.atk) {
    const tap = inp.tapR ? 1 : inp.tapL ? -1 : 0;
    if (tap) { f.face = tap; startMove(w, f, 'fsmash'); }
    else if (inp.tapU) startMove(w, f, 'usmash');
    else if (inp.tapD) startMove(w, f, 'dsmash');
    else if (inp.up) startMove(w, f, 'utilt');
    else if (inp.down) startMove(w, f, 'dtilt');
    else if (dir && dir === f.face && Math.abs(f.vx) > mv.dashAttackSpeed) startMove(w, f, 'dashatk');
    else if (dir) { f.face = dir; startMove(w, f, 'ftilt'); }
    else startMove(w, f, 'jab');
    return true;
  }
  if (inp.jump) { setState(f, 'jumpsquat'); return true; }
  if (inp.downP && f.surf && !f.surf.main) {
    f.drop = mv.platformDrop; f.ground = false; f.surf = null; f.y += 1; setState(f, 'air');
    return true;
  }
  return false;
}
