// Movement integration, stage collision, landing and ledge grabs.

import { GROUND, LEDGE, PLATS, STAGE, UNDER, stageBottom } from '../data/stage';
import { type Fighter, type Input, type World, G, MV, ST, clamp, dust, maxJ, ring, setState, sfx, shake } from './core';
import { quake } from './moves';
import { roll } from './states';

const HALF_W = 14, HEIGHT = 74;

export function integrate(w: World, f: Fighter, edgeStop: boolean) {
  const wasG = f.ground, vy = f.vy, py = f.y, surf = f.surf;
  f.x += f.vx;
  if (wasG && surf && f.vy >= 0) {
    if (edgeStop) {
      if (f.x < surf.x1) { f.x = surf.x1; f.vx = 0; }
      if (f.x > surf.x2) { f.x = surf.x2; f.vx = 0; }
    }
    const pad = surf.main ? 14 : 4;
    if (f.x >= surf.x1 - pad && f.x <= surf.x2 + pad && !(f.drop > 0 && !surf.main)) { f.y = surf.y; f.vy = 0; return; }
  }
  f.y += f.vy; f.ground = false; f.surf = null;
  if (vy >= 0 && f.drop <= 0) {
    for (const p of PLATS) {
      if (py <= p.y + 0.5 && f.y >= p.y && f.x >= p.x1 - 4 && f.x <= p.x2 + 4) { f.y = p.y; f.vy = 0; f.ground = true; f.surf = p; break; }
    }
  }
  if (!f.ground) {
    const s = STAGE, hx1 = f.x - HALF_W, hx2 = f.x + HALF_W, hy1 = f.y - HEIGHT, hy2 = f.y;
    const xs = clamp(f.x < (s.L + s.R) / 2 ? hx2 : hx1, s.L, s.R), sb = stageBottom(xs);
    if (hx2 > s.L && hx1 < s.R && hy2 > s.top && hy1 < sb) {
      const over = Math.max(0, hy1 - s.bot) * UNDER.inset / UNDER.depth;
      const up = hy2 - s.top, dn = sb - hy1, lf = hx2 - s.L - over, rt = s.R - hx1 - over, mn = Math.min(up, dn, lf, rt);
      if (mn === up) { f.y = s.top; f.vy = 0; f.ground = true; f.surf = GROUND; }
      else if (mn === dn) { f.y = sb + HEIGHT; if (f.vy < 0) f.vy = 0; }
      else if (mn === lf) { f.x -= lf; if (f.vx > 0) f.vx = 0; }
      else { f.x += rt; if (f.vx < 0) f.vx = 0; }
    }
  }
  if (f.ground && !wasG) onLand(w, f, vy);
}

export function fall(w: World, f: Fighter, edgeStop = false, cap = ST(f).fallSpeed) {
  f.vy = Math.min(f.vy + ST(f).gravity, cap);
  integrate(w, f, edgeStop);
}

export function fastFallCheck(f: Fighter, inp: Input) {
  const m = G().movement;
  if (inp.downP && f.vy > m.fastFallMinVy) { f.ff = true; f.vy = Math.max(f.vy, m.fastFallKick); }
}

/** Accelerate toward `dir` up to air speed, or drift with drag. */
export function airDrift(f: Fighter, dir: number, acc: number, drag: number) {
  const max = ST(f).airSpeed;
  if (dir && acc && f.vx * dir < max) f.vx = Math.min(max, f.vx * dir + acc) * dir;
  else f.vx *= drag;
}

export function airPhys(w: World, f: Fighter, inp: Input, dir: number, acc: number) {
  const m = G().movement;
  if (dir && acc && f.vx * dir < ST(f).airSpeed) airDrift(f, dir, acc, 1);
  else f.vx *= dir ? m.airDragHeld : m.airDrag;
  fastFallCheck(f, inp);
  fall(w, f, false, f.ff ? ST(f).fastFallSpeed : ST(f).fallSpeed);
}

export function onLand(w: World, f: Fighter, vy: number) {
  const g = G();
  f.jumps = maxJ(f); f.upB = false; f.adUsed = false; f.ff = false; f.flipT = 0;
  const s = f.state;
  if (s === 'air') { if (vy > g.landing.softLandSpeed) setState(f, 'landing', { n: g.landing.hard }); else setState(f, 'idle'); }
  else if (s === 'helpless') setState(f, 'landing', { n: g.landing.helpless });
  else if (s === 'airdodge') setState(f, 'landing', { n: f.sd.dirl ? g.dodge.dirAirDodgeLandingLag : g.dodge.airDodgeLandingLag });
  else if (s === 'attack') {
    const m = f.move!, d = MV(f);
    if (d.kind === 'superJump') { if (m.ph === 1 || m.ph === 2) quake(w, f, 3); }
    else if (d.aerial) setState(f, 'landing', { n: d.landingLag ?? 0 });
    else if (m.id === 'uspec' && d.kind === undefined && m.t > (d.rise?.frame ?? 4) + 2) setState(f, 'landing', { n: g.landing.upSpecial });
  } else if (s === 'hitstun' && f.tumble && vy > 2) {
    f.hitstun = 0; f.tumble = false;
    if (w.frame - f.lastSP <= g.tech.window) {
      if (f.inDir) roll(w, f, f.inDir, { tech: true });
      else { setState(f, 'tech'); f.inv = g.tech.invuln; }
      ring(w, f.x, f.y - 20, 10, 4, 12);
      sfx(w, 'tech');
    } else {
      setState(f, 'knockdown'); f.vx *= 0.3;
      dust(w, f.x, f.y, 10); shake(w, 3);
    }
  } else if (s === 'shieldbreak') {
    const sh = g.shield;
    setState(f, 'dizzy', { n: Math.max(sh.dizzyMin, sh.dizzyBase - f.dmg * sh.dizzyPerPercent) });
  }
  if (vy > 3) dust(w, f.x, f.y, 5);
  if (vy > 8) sfx(w, 'land');
}

export function tryLedge(w: World, f: Fighter, inp: Input, rising = false) {
  if (f.ground || f.ledgeCD > 0 || (f.vy < 0 && !rising) || inp.down) return;
  if (f.y < STAGE.top + LEDGE.minBelowTop || f.y > STAGE.top + LEDGE.maxBelowTop) return;
  for (const side of [-1, 1]) {
    const ex = side < 0 ? STAGE.L : STAGE.R;
    const ok = side < 0 ? f.x > ex - LEDGE.reachOut && f.x < ex + LEDGE.reachIn : f.x < ex + LEDGE.reachOut && f.x > ex - LEDGE.reachIn;
    if (!ok || w.fighters.some(g => g !== f && (g.state === 'ledge' || g.state === 'climb') && g.sd.side === side)) continue;
    setState(f, 'ledge', { side });
    Object.assign(f, { jumps: maxJ(f), upB: false, adUsed: false, ff: false, vx: 0, vy: 0, x: ex + side * LEDGE.hangX, y: STAGE.top + LEDGE.hangY, face: -side });
    f.inv = Math.max(f.inv, G().ledge.invuln);
    sfx(w, 'land');
    return;
  }
}
