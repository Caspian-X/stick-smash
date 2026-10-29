// Per-state fighter update (the state machine).

import { GROUND, LEDGE, RESPAWN, STAGE } from '../data/stage';
import {
  type Fighter, type Input, type StateData, type World, FD, G, ST, burst, clamp, col, dust, easeOut, lerp, maxJ, other,
  part, ring, rnd, setState, sfx, shake,
} from './core';
import { applyHit, grabRelease, releaseGrab, shieldBreak } from './combat';
import { airPhys, fall, integrate, tryLedge } from './physics';
import { groundActions, special, stAttack, startMove } from './moves';

export function roll(w: World, f: Fighter, dir: number, extra: Partial<StateData> = {}) {
  setState(f, 'roll', { dir, ...extra }); f.face = dir; f.inv = G().dodge.rollInvuln; sfx(w, 'roll');
}

function airdodge(w: World, f: Fighter, inp: Input, dir: number) {
  const g = G().dodge, dy = (inp.down ? 1 : 0) - (inp.up ? 1 : 0);
  f.adUsed = true; f.inv = dir || dy ? g.dirAirDodgeInvuln : g.airDodgeInvuln; f.ff = false; sfx(w, 'roll');
  if (dir || dy) {
    const l = Math.hypot(dir, dy);
    setState(f, 'airdodge', { dirl: true, dx: dir / l, dy: dy / l });
    f.vx = f.sd.dx! * g.dirAirDodgeSpeed; f.vy = f.sd.dy! * g.dirAirDodgeSpeed;
  } else setState(f, 'airdodge', { dirl: false });
}

export function updateFighter(w: World, f: Fighter, inp: Input) {
  if (f.state === 'out' || f.state === 'win') return;
  if (f.flash > 0) f.flash--;
  if (f.hudShake > 0) f.hudShake--;
  if (f.state === 'dead') { if (--f.sd.n! <= 0) respawn(f); return; }
  if (f.lag > 0) { f.lag--; return; }
  const g = G(), s = ST(f);
  if (inp.shieldP) f.lastSP = w.frame;
  if (f.inv > 0) f.inv--;
  if (f.blink > 0) f.blink--;
  if (f.ledgeCD > 0) f.ledgeCD--;
  if (f.drop > 0) f.drop--;
  if (f.flipT > 0) f.flipT--;
  if (f.state !== 'shield') f.sh = Math.min(g.shield.max, f.sh + g.shield.regen);
  f.t++;
  const dir = (inp.right ? 1 : 0) - (inp.left ? 1 : 0);
  f.inDir = dir;
  const sd = f.sd;
  switch (f.state) {
    case 'respawn': {
      const r = g.respawn;
      if (f.t <= r.descendFrames) { f.y = RESPAWN.startY + (RESPAWN.endY - RESPAWN.startY) * easeOut(f.t / r.descendFrames); break; }
      if (f.t > r.maxWait || inp.left || inp.right || inp.downP || inp.jump || inp.atk || inp.sp || inp.shieldP || inp.grab) {
        setState(f, 'air'); f.blink = r.invuln;
      }
      break;
    }
    case 'idle':
      if (groundActions(w, f, inp, dir)) break;
      if (dir) {
        if (dir !== f.face && Math.abs(f.vx) > 3) dust(w, f.x, f.y, 3);
        f.face = dir; f.vx = clamp(f.vx + dir * s.runAccel, -s.runSpeed, s.runSpeed); f.runPh += Math.abs(f.vx) * 0.07;
      } else f.vx *= s.groundFriction;
      fall(w, f);
      if (!f.ground) setState(f, 'air');
      break;
    case 'jumpsquat':
      if (inp.atk) { startMove(w, f, 'usmash'); break; }
      if (inp.sp && inp.up && !f.upB) { f.upB = true; startMove(w, f, 'uspec'); break; }
      if (inp.grab) { startMove(w, f, 'grab'); break; }
      f.vx *= 0.9;
      if (f.t >= g.movement.jumpsquat) {
        f.vy = inp.jumpHeld ? -s.jumpSpeed : -s.shortHopSpeed;
        f.ground = false; f.surf = null; f.ff = false;
        setState(f, 'air'); dust(w, f.x, f.y, 4); sfx(w, 'jump'); integrate(w, f, false);
      } else fall(w, f, true);
      break;
    case 'air':
      if (inp.shieldP && !f.adUsed) { airdodge(w, f, inp, dir); integrate(w, f, false); break; }
      if (inp.sp) special(w, f, inp, dir);
      else if (inp.atk) {
        if (inp.up) startMove(w, f, 'uair');
        else if (inp.down) startMove(w, f, 'dair');
        else if (dir && dir === -f.face) startMove(w, f, 'bair');
        else if (dir) startMove(w, f, 'fair');
        else startMove(w, f, 'nair');
      } else if (inp.jump && f.jumps > 0) {
        f.jumps--; f.vy = -s.airJumpSpeed; if (dir) f.vx = dir * s.airJumpDrift; f.ff = false; f.flipT = 20; sfx(w, 'djump');
        if (FD(f).look === 'king') for (let k = 0; k < 6; k++) part(w, f.x + rnd(w, -14, 14), f.y - 4, rnd(w, -1.5, 1.5), rnd(w, 0.5, 2), 18, 0xddddee, 5);
      }
      airPhys(w, f, inp, dir, s.airAccel);
      if (f.state === 'air') tryLedge(w, f, inp);
      break;
    case 'attack': stAttack(w, f, inp, dir); break;
    case 'helpless':
      airPhys(w, f, inp, dir, g.movement.helplessAccel);
      if (f.state === 'helpless') tryLedge(w, f, inp);
      break;
    case 'landing':
      f.vx *= 0.8; fall(w, f, true);
      if (!f.ground) setState(f, 'air'); else if (f.t >= sd.n!) setState(f, 'idle');
      break;
    case 'shield':
      f.sh -= g.shield.decay; f.vx *= 0.85; fall(w, f, true);
      if (!f.ground) { setState(f, 'air'); break; }
      if (f.sh <= 0) { shieldBreak(w, f); break; }
      if (sd.stun! > 0) { sd.stun!--; break; }
      if (inp.jump) { setState(f, 'jumpsquat'); break; }
      if (inp.grab || inp.atk) { startMove(w, f, 'grab'); break; }
      if (inp.sp && inp.up && !f.upB) { f.upB = true; startMove(w, f, 'uspec'); break; }
      if (inp.tapR || inp.tapL) { roll(w, f, inp.tapR ? 1 : -1); break; }
      if (inp.tapD) { setState(f, 'spot'); f.inv = g.dodge.spotInvuln; sfx(w, 'roll'); break; }
      if (!inp.shield) setState(f, 'landing', { n: g.shield.dropLag, kind: 'shield' });
      break;
    case 'roll': {
      const d = g.dodge;
      f.vx = f.t >= d.rollMoveStart && f.t <= d.rollMoveEnd ? sd.dir! * d.rollSpeed : f.vx * 0.7; fall(w, f, true);
      if (!f.ground) setState(f, 'air');
      else if (f.t >= d.rollFrames) { f.face = -sd.dir!; setState(f, 'idle'); }
      break;
    }
    case 'spot':
      f.vx *= 0.7; fall(w, f, true);
      if (f.t >= g.dodge.spotFrames) setState(f, 'idle');
      break;
    case 'airdodge': {
      const d = g.dodge;
      if (sd.dirl && f.t < d.dirAirDodgeDecayFrames) { f.vx *= d.dirAirDodgeDecay; f.vy *= d.dirAirDodgeDecay; integrate(w, f, false); }
      else airPhys(w, f, inp, dir, g.movement.helplessAccel);
      if (f.state === 'airdodge') {
        tryLedge(w, f, inp);
        if (f.state === 'airdodge' && f.t >= (sd.dirl ? d.dirAirDodgeFrames : d.airDodgeFrames)) setState(f, 'air');
      }
      break;
    }
    case 'hitstun': {
      const h = g.hitstun;
      f.hitstun--;
      f.vx *= f.ground ? h.groundDrag : h.airDrag; f.vx += dir * h.driftAccel;
      if (f.tumble) f.spin += 0.3 * (f.vx >= 0 ? 1 : -1);
      if (Math.hypot(f.vx, f.vy) > 10 && w.frame % 2 === 0) part(w, f.x, f.y - 40, 0, 0, 26, 0xbbbbcc, 7, -0.05);
      f.vy = Math.min(f.vy + h.gravity, h.maxFall); integrate(w, f, false);
      if (f.state === 'hitstun' && f.hitstun <= 0) { f.tumble = false; setState(f, f.ground ? 'idle' : 'air'); }
      break;
    }
    case 'knockdown': {
      const k = g.knockdown;
      f.vx *= 0.8; fall(w, f, true);
      if (!f.ground) { setState(f, 'air'); break; }
      if (f.t > k.minFrames) {
        if (inp.atk) { setState(f, 'getup', { kind: 'atk' }); f.inv = k.attackInvuln; }
        else if (dir) roll(w, f, dir, { getup: true });
        else if (inp.jump || inp.up || inp.shieldP || inp.sp || inp.grab) { setState(f, 'getup', { kind: 'stand' }); f.inv = k.getupInvuln; }
      }
      if (f.state === 'knockdown' && f.t >= k.autoGetup) { setState(f, 'getup', { kind: 'stand' }); f.inv = k.getupInvuln; }
      break;
    }
    case 'getup': {
      const k = g.knockdown;
      f.vx *= 0.8; fall(w, f, true);
      if (sd.kind === 'atk' && f.t === k.attackActive[0]) sfx(w, 'swing');
      if (f.t >= (sd.kind === 'atk' ? k.attackFrames : k.getupFrames)) setState(f, 'idle');
      break;
    }
    case 'tech':
      f.vx *= 0.7; fall(w, f, true);
      if (f.t >= g.tech.frames) setState(f, 'idle');
      break;
    case 'ledge': stLedge(f, inp, dir); break;
    case 'climb': stClimb(w, f); break;
    case 'hold': stHold(w, f, inp, dir); break;
    case 'throw': stThrow(w, f); break;
    case 'grabbed': {
      const by = sd.by;
      if (!by || (by.state !== 'hold' && by.state !== 'throw')) { setState(f, 'air'); break; }
      if (inp.anyP && by.state === 'hold') by.sd.timer! -= g.grab.mashPerPress;
      break;
    }
    case 'shieldbreak':
      f.vx *= 0.95; f.vy = Math.min(f.vy + g.shield.breakGravity, g.shield.breakMaxFall); integrate(w, f, false);
      break;
    case 'dizzy':
      f.vx *= 0.8; fall(w, f, true);
      if (inp.anyP) sd.n! -= g.shield.dizzyMash;
      if (f.t >= sd.n!) { f.sh = g.shield.dizzyRecoverHP; setState(f, 'idle'); }
      break;
  }
}

type ClimbKind = 'climb' | 'attack' | 'roll' | 'jump';

function stLedge(f: Fighter, inp: Input, dir: number) {
  const g = G().ledge, side = f.sd.side!, ex = side < 0 ? STAGE.L : STAGE.R;
  f.vx = f.vy = 0; f.x = ex + side * LEDGE.hangX; f.y = STAGE.top + LEDGE.hangY;
  if (f.t < g.minHang) return;
  const toward = dir === -side, away = dir === side;
  const go = (kind: ClimbKind) => { setState(f, 'climb', { side, kind }); f.inv = Math.max(f.inv, g[kind].inv); };
  if (inp.jump) go('jump');
  else if (inp.atk || inp.sp) go('attack');
  else if (inp.shieldP) go('roll');
  else if (toward || inp.up) go('climb');
  else if (inp.down || away || f.t > g.maxHang) { setState(f, 'air'); f.ledgeCD = g.regrabCooldown; f.vy = 1; f.x += side * 4; }
}

function stClimb(w: World, f: Fighter) {
  const g = G().ledge, kind = f.sd.kind as ClimbKind, c = g[kind], side = f.sd.side!, ex = side < 0 ? STAGE.L : STAGE.R, fx = -side;
  f.vx = f.vy = 0;
  if (f.t <= c.a) {
    const k = easeOut(f.t / c.a);
    f.x = lerp(ex + side * LEDGE.hangX, ex + fx * 4, k); f.y = lerp(STAGE.top + LEDGE.hangY, STAGE.top, k);
    f.ground = false; f.surf = null;
    return;
  }
  if (kind === 'jump') {
    setState(f, 'air'); f.y = STAGE.top - 1; f.vy = -g.jumpSpeed; f.vx = fx * 2; f.flipT = 20; f.ledgeCD = 20; sfx(w, 'jump');
    return;
  }
  f.ground = true; f.surf = GROUND; f.y = STAGE.top;
  if (f.t <= c.b) f.x = ex + fx * 4 + fx * (c.dist - 4) * easeOut((f.t - c.a) / (c.b - c.a));
  if (kind === 'attack' && f.t === g.attackActive[0]) sfx(w, 'swing');
  if (kind === 'roll' && f.t === c.a + 1) sfx(w, 'roll');
  if (f.t >= c.end) { f.face = fx; setState(f, 'idle'); }
}

function stHold(w: World, f: Fighter, inp: Input, dir: number) {
  const g = G().grab, sd = f.sd, v = sd.v;
  f.vx *= 0.8; fall(w, f, true);
  if (!v || v.state !== 'grabbed') { setState(f, 'idle'); return; }
  v.x = f.x + f.face * 30; v.y = f.y; v.face = -f.face; v.ground = f.ground;
  sd.timer!--;
  if (sd.pt! > 0) {
    if (--sd.pt! === g.pummelHitFrame) {
      v.dmg += g.pummelDamage; v.flash = 5; v.hudShake = 6; f.lag = v.lag = g.pummelHitlag;
      burst(w, v.x, v.y - 44, 0xffffff, 6, 3, 12, 3, 0); sfx(w, 'pummel');
    }
    return;
  }
  if (sd.timer! <= 0) { grabRelease(f); return; }
  if (f.t < g.minHold) return;
  if (inp.atk) { sd.pt = g.pummelFrames; return; }
  const k = inp.up ? 'u' : inp.down ? 'd' : dir === f.face ? 'f' : dir === -f.face ? 'b' : null;
  if (k) { setState(f, 'throw', { v, kind: k }); sfx(w, 'throw'); }
}

function stThrow(w: World, f: Fighter) {
  const v = f.sd.v!, k = f.sd.kind as 'f' | 'b' | 'u' | 'd', T = FD(f).throws[k];
  f.vx *= 0.8; fall(w, f, true);
  if (v.state === 'grabbed') {
    const p = clamp(f.t / T.release, 0, 1);
    if (k === 'f') { v.x = f.x + f.face * (30 + p * 14); v.y = f.y; }
    else if (k === 'b') { const a = p * Math.PI; v.x = f.x + f.face * 30 * Math.cos(a); v.y = f.y - 46 * Math.sin(a); }
    else if (k === 'u') { v.x = f.x + f.face * 30 * (1 - p); v.y = f.y - 80 * p; }
    else { v.x = f.x + f.face * 24; v.y = f.y; }
    if (f.t === T.release) {
      setState(v, 'air'); v.ground = false; v.surf = null;
      if (k === 'd') { dust(w, v.x, f.y, 12); shake(w, 5); }
      applyHit(w, f, v, T.hit, k === 'b' ? -f.face : f.face, v.x, v.y - 40, false);
    }
  }
  if (f.t >= T.total) setState(f, 'idle');
}

export function respawn(f: Fighter) {
  Object.assign(f, {
    x: RESPAWN.x, y: RESPAWN.startY, vx: 0, vy: 0, dmg: 0, ground: false, surf: null, hitstun: 0, upB: false, adUsed: false,
    jumps: maxJ(f), tumble: false, ff: false, face: 1, sh: G().shield.max, inv: 0, blink: 0,
  });
  setState(f, 'respawn');
}

export function ko(w: World, f: Fighter) {
  releaseGrab(f);
  f.stocks--;
  const c = col(f);
  w.fx.push({ t: 'beam', x: f.x, y: f.y - 40, col: c });
  shake(w, 20); sfx(w, 'ko');
  Object.assign(f, { vx: 0, vy: 0, hitstun: 0, lag: 0, tumble: false });
  if (w.cfg.training) { f.stocks = Math.max(f.stocks, 1); setState(f, 'dead', { n: G().respawn.deadFrames }); return; }
  if (f.stocks > 0) setState(f, 'dead', { n: G().respawn.deadFrames });
  else { setState(f, 'out'); w.winner = other(w, f); w.endT = G().match.endFrames; }
}
