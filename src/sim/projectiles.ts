// Projectiles: Brawler's energy ball and King's gordo.

import type { GordoMove, ProjectileMove } from '../data/types';
import { BLAST, STAGE, SURFS } from '../data/stage';
import { type Fighter, type World, MV, burst, col, countering, dust, hittable, part, sfx } from './core';
import { type ActiveBox, applyHit, circRect, counter, hurt, shieldHit } from './combat';

export function spawnProj(w: World, f: Fighter, d: ProjectileMove) {
  const p = d.projectile;
  if (w.projs.filter(q => q.o === f && !q.gordo).length >= p.max) return;
  w.projs.push({ o: f, gordo: false, x: f.x + f.face * p.x, y: f.y - p.y, vx: f.face * p.speed, vy: 0, life: p.life, r: p.r, hit: p.hit, bn: 0, hcd: 0 });
  sfx(w, 'shoot');
}

/** Gordo: a spiked ball thrown in an arc. It bounces a few times, and any attack can knock it back. */
export function spawnGordo(w: World, f: Fighter, d: GordoMove) {
  if (w.projs.some(p => p.o === f && p.gordo)) { sfx(w, 'bonk'); return; }
  const gp = d.gordo;
  w.projs.push({ o: f, gordo: true, gp, x: f.x + f.face * gp.x, y: f.y - gp.y, vx: f.face * gp.vx, vy: gp.vy, life: gp.life, r: gp.r, hit: gp.hit, bn: 0, hcd: 0 });
  sfx(w, 'toss');
}

function gordoPhys(w: World, p: World['projs'][number]) {
  const gp = p.gp!;
  if (p.hcd > 0) p.hcd--;
  const py = p.y;
  p.vy = Math.min(p.vy + gp.gravity, gp.maxFall); p.y += p.vy;
  if (p.vy > 0) for (const s of SURFS) {
    if (py + p.r > s.y + 0.5 || p.y + p.r < s.y || p.x < s.x1 - 6 || p.x > s.x2 + 6) continue;
    p.y = s.y - p.r; p.vy = -Math.max(gp.minBounce, p.vy * gp.bounce); p.vx *= gp.friction; p.bn++;
    dust(w, p.x, s.y, 3); sfx(w, 'bonk');
    if (p.bn > gp.maxBounces) p.life = Math.min(p.life, 20);
    break;
  }
  if (p.y + p.r > STAGE.top + 2 && p.y - p.r < STAGE.bot && p.x > STAGE.L - p.r && p.x < STAGE.R + p.r) {
    p.vx = -p.vx * 0.6; p.x = p.x < (STAGE.L + STAGE.R) / 2 ? STAGE.L - p.r : STAGE.R + p.r;
  }
}

/** Attacks knock gordos back toward the other player. */
export function batGordos(w: World, a: Fighter, bx: ActiveBox[]) {
  for (const p of w.projs) {
    if (!p.gordo || p.hcd > 0 || p.life <= 0) continue;
    const b = bx.find(b => !b.grab && Math.hypot(b.wx - p.x, b.wy - p.y) < b.r + p.r);
    if (!b) continue;
    const gp = p.gp!;
    const sgn = b.dir === 'away' ? (Math.sign(p.x - a.x) || a.face) : b.dir === 'back' ? -a.face : a.face;
    Object.assign(p, { o: a, vx: sgn * gp.batSpeed, vy: gp.batVy, life: gp.batLife, bn: 0, hcd: gp.batCooldown });
    a.lag = Math.max(a.lag, gp.batHitlag);
    burst(w, p.x, p.y, 0xffffff, 10, 4, 14, 3, 0); sfx(w, 'bonk'); sfx(w, 'whack');
  }
}

export function stepProjectiles(w: World) {
  for (const p of w.projs) {
    p.x += p.vx; p.life--;
    if (p.gordo) gordoPhys(w, p);
    else if (w.frame % 2 === 0) part(w, p.x, p.y, -p.vx * 0.05, 0, 14, col(p.o), 6);
    for (const v of w.fighters) {
      if (v === p.o || !hittable(v) || p.life <= 0) continue;
      if (!circRect(p.x, p.y, p.r, hurt(v))) continue;
      if (countering(v)) {
        const d = MV(v), speed = d.kind === 'counter' ? d.reflectSpeed : 1.25;
        p.o = v; p.vx = -p.vx * speed; p.life = 75; if (p.gordo) p.vy = -4; counter(w, v, null, 0);
        continue;
      }
      const label = p.gordo ? 'Gordo' : 'Projectile';
      if (v.state === 'shield') shieldHit(w, v, p.hit, Math.sign(p.vx), p.o, true, label);
      else applyHit(w, p.o, v, p.hit, Math.sign(p.vx), p.x, p.y, true, label);
      p.life = 0;
    }
    for (const q of w.projs) {
      if (q !== p && q.o !== p.o && q.life > 0 && p.life > 0 && Math.hypot(q.x - p.x, q.y - p.y) < p.r + q.r) {
        burst(w, (p.x + q.x) / 2, p.y, 0xffffff, 14, 5, 18, 3, 0); p.life = q.life = 0;
      }
    }
  }
  w.projs = w.projs.filter(p => p.life > 0 && p.x > BLAST.L && p.x < BLAST.R && p.y < BLAST.B);
}
