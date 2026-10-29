// Hitboxes, hits, shields, grabs, counters.

import type { PhaseHitbox } from '../data/types';
import {
  type Fighter, type World, G, FD, MV, burst, col, countering, hittable, ring, setState, sfx, shake,
} from './core';
import { batGordos } from './projectiles';

export type ActiveBox = PhaseHitbox & { wx: number; wy: number };

export function hurt(f: Fighter) {
  if (f.state === 'knockdown') return { x1: f.x - 30, x2: f.x + 30, y1: f.y - 16, y2: f.y };
  return { x1: f.x - 16, x2: f.x + 16, y1: f.y - 78, y2: f.y };
}
export type Rect = ReturnType<typeof hurt>;

export function circRect(cx: number, cy: number, r: number, R: Rect) {
  const nx = Math.max(R.x1, Math.min(cx, R.x2)), ny = Math.max(R.y1, Math.min(cy, R.y2));
  return (cx - nx) ** 2 + (cy - ny) ** 2 <= r * r;
}

const inWin = (t: number, [a, b]: [number, number]) => t >= a && t <= b;

function rawBoxes(f: Fighter): PhaseHitbox[] {
  const g = G();
  if (f.state === 'attack' && f.move) {
    const m = f.move, d = MV(f);
    switch (d.kind) {
      case undefined: return d.hitboxes.filter(b => m.t >= b.start && m.t <= b.end);
      case 'groundPound': return m.ph === 1 ? [d.fall] : m.ph === 2 && m.t2 < d.waveFrames ? d.wave : [];
      case 'counter': return m.ph === 1 && m.cd && inWin(m.t2, d.strike) ? [{ ...d.hit, damage: m.cd }] : [];
      case 'superJump':
        return m.ph === 1 && m.t < d.riseHitUntil ? [d.rise] : m.ph === 2 ? [d.fall] : m.ph === 3 && m.t2 < d.waveFrames ? d.wave : [];
      default: return [];
    }
  }
  if (f.state === 'getup' && f.sd.kind === 'atk' && inWin(f.t, g.knockdown.attackActive)) return g.knockdown.attack;
  if (f.state === 'climb' && f.sd.kind === 'attack' && inWin(f.t, g.ledge.attackActive)) return g.ledge.attackHit;
  return [];
}

export function activeBoxes(f: Fighter): ActiveBox[] {
  const k = G().knockback;
  const mult = f.state === 'attack' && f.move ? 1 + f.move.charge / k.chargeDamageDivisor : 1;
  return rawBoxes(f).map(b => ({
    ...b, damage: b.damage * mult, bkb: b.bkb * (1 + (mult - 1) * k.chargeKbRatio),
    wx: f.x + b.x * f.face, wy: f.y - b.y,
  }));
}

export function attackLabel(f: Fighter): string {
  if (f.state === 'attack' && f.move) return MV(f).name ?? f.move.id;
  if (f.state === 'throw') return `Throw (${f.sd.kind})`;
  if (f.state === 'getup') return 'Getup Attack';
  if (f.state === 'climb') return 'Ledge Attack';
  return f.state;
}

function traceHit(w: World, a: Fighter, v: Fighter, label: string, onShield: boolean, dmg: number) {
  const p = w.trace;
  const chained = p && !onShield && !p.onShield && p.defender === v.i && p.dFree < 0;
  w.trace = {
    attacker: a.i, defender: v.i, move: label, onShield, hitFrame: w.frame, aFree: -1, dFree: -1, done: false,
    combo: chained ? p.combo + 1 : 1, comboDmg: chained ? p.comboDmg + dmg : dmg,
  };
}

export function releaseGrab(f: Fighter) {
  if ((f.state === 'hold' || f.state === 'throw') && f.sd.v && f.sd.v.state === 'grabbed') {
    const v = f.sd.v; setState(v, 'air'); v.vy = -5; v.vx = f.face * 3; v.ground = false; v.surf = null;
  }
  if (f.state === 'grabbed') {
    const g = f.sd.by; if (g && (g.state === 'hold' || g.state === 'throw')) setState(g, 'idle');
  }
}

export function applyHit(w: World, att: Fighter | null, vic: Fighter, b: PhaseHitbox, sgn: number, px: number, py: number, isProj: boolean, label?: string) {
  const g = G();
  releaseGrab(vic);
  vic.dmg = Math.min(999, vic.dmg + b.damage);
  const kb = (b.bkb + vic.dmg * b.kbg * g.knockback.kbPercentScale) * FD(vic).stats.weight, a = b.angle * Math.PI / 180;
  vic.vx = Math.cos(a) * kb * sgn; vic.vy = -Math.sin(a) * kb;
  if (vic.ground && vic.vy > 0) vic.vy = -vic.vy * g.knockback.groundBounce;
  vic.ground = false; vic.surf = null;
  setState(vic, 'hitstun');
  vic.hitstun = Math.floor((Math.floor(kb * g.hitstun.perKnockback) + g.hitstun.base) * (b.hitstunMult ?? 1) + (b.hitstunAdd ?? 0));
  vic.tumble = kb > g.knockback.tumbleThreshold;
  vic.ff = false; vic.upB = false; vic.adUsed = false;
  const hl = Math.floor(Math.min(g.hitlag.max, Math.floor(g.hitlag.base + b.damage * g.hitlag.perDamage + kb * g.hitlag.perKnockback)) * (b.hitlagMult ?? 1));
  vic.lag = hl; if (!isProj && att) att.lag = hl;
  vic.flash = hl + 2; vic.hudShake = 12;
  if (att) traceHit(w, att, vic, label ?? attackLabel(att), false, b.damage);
  const hx = (px + vic.x) / 2, hy = (py + vic.y - 40) / 2;
  burst(w, hx, hy, 0xffffff, 8 + kb * 0.6, 4 + kb * 0.25, 18, 4, 0);
  burst(w, hx, hy, att ? col(att) : 0xffffff, 6 + kb * 0.4, 3 + kb * 0.2, 24, 3, 0);
  ring(w, hx, hy, 6, 3 + kb * 0.15, 14);
  shake(w, kb * 0.45);
  sfx(w, 'hit', kb);
}

export function shieldHit(w: World, v: Fighter, b: PhaseHitbox, sgn: number, a: Fighter | null, isProj: boolean, label?: string) {
  const s = G().shield;
  v.sh -= b.damage * s.damageMult * (b.shieldDmgMult ?? 1);
  v.sd.stun = Math.floor((b.damage * s.stunPerDamage + s.stunBase) * (b.shieldStunMult ?? 1));
  v.vx = sgn * (b.damage * s.pushPerDamage + s.pushBase);
  const hl = Math.floor(s.hitlagBase + b.damage * s.hitlagPerDamage);
  v.lag = hl;
  if (a && !isProj) { a.lag = hl; if (a.ground) a.vx = -a.face * s.attackerPushback; }
  if (a) traceHit(w, a, v, label ?? attackLabel(a), true, b.damage);
  ring(w, v.x, v.y - 38, 20, 2.5, 10);
  sfx(w, 'shieldHit');
  if (v.sh <= 0) shieldBreak(w, v);
}

export function shieldBreak(w: World, f: Fighter) {
  f.sh = 0; setState(f, 'shieldbreak');
  f.vy = -G().shield.breakLaunch; f.vx = 0; f.ground = false; f.surf = null;
  burst(w, f.x, f.y - 40, 0xccddff, 30, 8, 30, 4, 0.1);
  ring(w, f.x, f.y - 38, 30, 6, 18, col(f));
  shake(w, 10); sfx(w, 'shieldBreak');
}

export function startHold(w: World, a: Fighter, v: Fighter) {
  const g = G().grab;
  releaseGrab(v);
  setState(a, 'hold', { v, timer: g.holdBase + v.dmg * g.holdPerPercent, pt: 0 }); a.vx = 0;
  setState(v, 'grabbed', { by: a });
  Object.assign(v, { vx: 0, vy: 0, face: -a.face, hitstun: 0, tumble: false, y: a.y, x: a.x + a.face * 30, ground: a.ground });
  sfx(w, 'grab');
}

export function grabRelease(f: Fighter) {
  const v = f.sd.v;
  setState(f, 'landing', { n: G().grab.releaseLag }); f.vx = -f.face * 4;
  if (v && v.state === 'grabbed') { setState(v, 'air'); v.vx = f.face * 5; v.vy = -6; v.ground = false; v.surf = null; }
}

/** Counter caught a hit: the attacker freezes and eats a strike (a = null for a reflected projectile). */
export function counter(w: World, v: Fighter, a: Fighter | null, dmg: number) {
  const m = v.move!, d = MV(v);
  if (d.kind !== 'counter') return;
  m.ph = 1; m.t2 = 0; m.cd = a ? Math.max(d.minDamage, dmg * d.damageMult) : 0; m.hit = new Set();
  if (a) { v.face = Math.sign(a.x - v.x) || v.face; a.lag = d.attackerFreeze; a.flash = 4; }
  v.inv = Math.max(v.inv, d.invuln); v.lag = 0;
  ring(w, v.x, v.y - 40, 14, 5, 14);
  burst(w, v.x, v.y - 40, 0xbbffff, 16, 6, 20, 3, 0);
  shake(w, 4); sfx(w, 'counter');
}

export function hitPass(w: World) {
  const g = G().grab;
  for (const a of w.fighters) {
    if (a.lag > 0 || !['attack', 'getup', 'climb'].includes(a.state)) continue;
    const bx = activeBoxes(a); if (!bx.length) continue;
    batGordos(w, a, bx);
    const hs = a.sd.hit;
    for (const v of w.fighters) {
      if (v === a || hs.has(v.i) || !hittable(v)) continue;
      const hb = hurt(v);
      for (const b of bx) {
        if (!circRect(b.wx, b.wy, b.r, hb)) continue;
        hs.add(v.i);
        const sgn = b.dir === 'away' ? (Math.sign(v.x - a.x) || a.face) : b.dir === 'back' ? -a.face : a.face;
        if (b.grab) { if (!['ledge', 'climb', 'knockdown'].includes(v.state) && Math.abs(v.y - a.y) < g.heightTolerance) startHold(w, a, v); }
        else if (countering(v)) counter(w, v, a, b.damage);
        else if (v.state === 'shield') shieldHit(w, v, b, sgn, a, false);
        else applyHit(w, a, v, b, sgn, b.wx, b.wy, false);
        break;
      }
    }
  }
}
