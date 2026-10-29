// Drawing fighters, weapons and projectiles with Phaser Graphics.

import type Phaser from 'phaser';
import { vec } from './vec';
import { COLORS } from '../data/colors';
import { DATA } from '../data';
import { type Fighter, type Projectile, type World, FD, MV, clamp, countering } from '../sim/core';
import { activeBoxes } from '../sim/combat';
import { type Pose, type Pt, kfAt, pose, swingOf } from './pose';

type G = Phaser.GameObjects.Graphics;
const rnd = (a: number, b: number) => a + Math.random() * (b - a);
const DODGES = ['roll', 'spot', 'airdodge', 'tech'];
export const isDodging = (f: Fighter) => f.inv > 0 && DODGES.includes(f.state);

/** Polyline with round joints/caps. Caps are skipped when translucent to avoid overlap blotches. */
function poly(g: G, pts: Pt[], col: number, w: number, a = 1) {
  g.lineStyle(w, col, a);
  g.beginPath(); g.moveTo(pts[0][0], pts[0][1]);
  for (let k = 1; k < pts.length; k++) g.lineTo(pts[k][0], pts[k][1]);
  g.strokePath();
  if (a >= 0.99) { g.fillStyle(col, a); for (const p of pts) g.fillCircle(p[0], p[1], w / 2); }
}

function star(g: G, x: number, y: number, r: number, col: number, a = 1) {
  g.fillStyle(col, a);
  g.fillTriangle(x - r, y, x + r, y, x, y - r * 0.3); g.fillTriangle(x - r, y, x + r, y, x, y + r * 0.3);
  g.fillTriangle(x, y - r, x, y + r, x - r * 0.3, y); g.fillTriangle(x, y - r, x, y + r, x + r * 0.3, y);
}

function ghost(g: G, f: Fighter, P: Pose, x: number, y: number, a: number) {
  g.save(); g.translateCanvas(x, y + P.oy); g.scaleCanvas(f.face, 1);
  if (P.rot) { g.translateCanvas(0, P.pv); g.rotateCanvas(P.rot); g.translateCanvas(0, -P.pv); }
  const c = COLORS[f.i].l;
  for (const [o, l] of [[P.hp, P.lB], [P.sh, P.aB], [P.hp, P.lF], [P.sh, P.aF]] as const) poly(g, [o, l[0], l[1]], c, 5, a);
  poly(g, [P.sh, P.hp], c, 5, a);
  g.lineStyle(5, c, a); g.strokeCircle(P.hd[0], P.hd[1], 10);
  g.restore();
}

function drawSword(g: G, f: Fighter, P: Pose, glow: boolean, T: number) {
  const [ex, ey] = P.aF[0], [hx, hy] = P.aF[1];
  const a = P.sw !== undefined ? P.sw : Math.atan2(hy - ey, hx - ex) - 0.8, c = Math.cos(a), s = Math.sin(a);
  const seg = (a0: number, a1: number, col: number, w: number, al = 1) => {
    g.lineStyle(w, col, al); g.lineBetween(hx + c * a0, hy + s * a0, hx + c * a1, hy + s * a1);
  };
  seg(3, 54, glow ? 0xbbffff : COLORS[f.i].c, glow ? 12 : 8, 0.25);
  seg(3, 54, glow ? 0xffffff : 0xe6ecff, 4);
  seg(-7, 3, 0x7a5530, 4);
  g.lineStyle(3.5, 0xe0b84a, 1);
  g.lineBetween(hx + c * 3 - s * 8, hy + s * 3 + c * 8, hx + c * 3 + s * 8, hy + s * 3 - c * 8);
  if (glow && (T >> 2) % 2) star(g, hx + c * 54, hy + s * 54, 7, 0xffffff);
}

function drawHammer(g: G, f: Fighter, P: Pose, hot: boolean) {
  const [ex, ey] = P.aF[0], [hx, hy] = P.aF[1];
  const a = P.sw !== undefined ? P.sw : Math.atan2(hy - ey, hx - ex) - 3.2;
  g.save(); g.translateCanvas(hx, hy); g.rotateCanvas(a);
  g.lineStyle(4.5, 0x8a5a2b, 1); g.lineBetween(-6, 0, 40, 0);
  g.fillStyle(hot ? 0xff9a3c : COLORS[f.i].c, hot ? 0.35 : 0.15); g.fillRoundedRect(32, -19, 26, 38, 6);
  g.fillStyle(hot ? 0xffcf7a : 0xb98545, 1); g.fillRoundedRect(36, -15, 18, 30, 4);
  g.lineStyle(2, 0x4a2f14, 1); g.strokeRoundedRect(36, -15, 18, 30, 4);
  g.fillStyle(0xe0b84a, 1); g.fillRect(38, -15, 3, 30); g.fillRect(49, -15, 3, 30);
  g.restore();
}

function drawCrown(g: G, P: Pose) {
  const [x, y] = P.hd;
  g.fillStyle(0xffd35a, 1);
  g.fillPoints(vec([
    { x: x - 9, y: y - 7 }, { x: x - 10, y: y - 19 }, { x: x - 4, y: y - 12 }, { x, y: y - 21 },
    { x: x + 4, y: y - 12 }, { x: x + 10, y: y - 19 }, { x: x + 9, y: y - 7 },
  ]), true);
  g.fillStyle(0xff5d8a, 1); g.fillCircle(x, y - 10, 2);
}

export function drawGordo(g: G, x: number, y: number, r: number, col: number, rot: number) {
  g.save(); g.translateCanvas(x, y); g.rotateCanvas(rot);
  g.fillStyle(0xe8e8f0, 1);
  for (let k = 0; k < 8; k++) {
    const a = k * Math.PI / 4;
    g.fillTriangle(Math.cos(a - 0.3) * r * 0.8, Math.sin(a - 0.3) * r * 0.8, Math.cos(a) * r * 1.45, Math.sin(a) * r * 1.45, Math.cos(a + 0.3) * r * 0.8, Math.sin(a + 0.3) * r * 0.8);
  }
  g.fillStyle(col, 0.25); g.fillCircle(0, 0, r + 5);
  g.fillStyle(0x3a2f5a, 1); g.fillCircle(0, 0, r);
  g.lineStyle(2.5, col, 1); g.strokeCircle(0, 0, r);
  g.restore();
  g.fillStyle(0xffffff, 1);
  g.fillRect(x - r * 0.35 - 1.5, y - r * 0.35, 3, r * 0.45); g.fillRect(x + r * 0.35 - 1.5, y - r * 0.35, 3, r * 0.45);
}

/** Fading wedge behind the weapon's recent path while a swing is live. */
function drawTrail(g: G, f: Fighter, P: Pose) {
  const sw = f.state === 'attack' && swingOf(f);
  if (!sw) return;
  const [kf, t, , ws] = sw, win = ws.filter(([a, b]) => t >= a && t <= b + 4).pop();
  if (!win) return;
  const [a, b] = win, a1 = kfAt(kf, Math.min(t, b)) * Math.PI / 180, a0 = kfAt(kf, Math.max(a, t - 5)) * Math.PI / 180;
  if (Math.abs(a1 - a0) < 0.05) return;
  const [sx, sy] = P.sh, ccw = a1 < a0, [ro, ri] = FD(f).weapon === 'hammer' ? [96, 50] : [84, 34];
  g.fillStyle(COLORS[f.i].l, 0.45 * (t > b ? 1 - (t - b) / 5 : 1));
  g.beginPath(); g.arc(sx, sy, ro, a0, a1, ccw); g.arc(sx, sy, ri, a1, a0, !ccw); g.closePath(); g.fillPath();
}

export function drawFighter(g: G, w: World, f: Fighter, showBoxes: boolean) {
  if (f.state === 'out' || f.state === 'dead') return;
  const T = w.frame, P = pose(f, T), m = f.move, d = m ? MV(f) : null, dodging = isDodging(f), C = FD(f);
  const charging = !!(d && m && d.charge && m.t === d.charge.frame && m.charge > 0);
  const pal = COLORS[f.i];
  if (dodging) for (const k of [3, 2, 1]) ghost(g, f, P, f.x - f.vx * k * 2.5, f.y - (f.ground ? 0 : f.vy * k * 2.5), 0.12 * (4 - k));

  g.save();
  const jx = (f.lag > 0 && f.flash > 0) || charging ? rnd(-2, 2) : 0;
  g.translateCanvas(f.x + jx, f.y);
  if (f.state === 'respawn') { g.fillStyle(pal.c, 0.5 + 0.2 * Math.sin(T * 0.2)); g.fillEllipse(0, 4, 68, 14); }
  const A = f.blink > 0 && (T >> 2) % 2 ? 0.45 : dodging ? 0.75 : 1;
  g.translateCanvas(0, P.oy);
  g.scaleCanvas(f.face, 1);
  if (P.rot) { g.translateCanvas(0, P.pv); g.rotateCanvas(P.rot); g.translateCanvas(0, -P.pv); }
  let c = f.flash > 0 ? 0xffffff : f.state === 'helpless' ? 0x888888 : dodging ? pal.l : pal.c;
  if (charging && (T >> 2) % 2) c = 0xffe680;
  const back = f.state === 'helpless' ? 0x555555 : dodging ? pal.l : pal.d;
  const glowC = charging ? 0xffe680 : dodging ? 0xffffff : pal.c, glowA = (charging || dodging ? 0.22 : 0.12) * A;

  if (C.weapon) drawTrail(g, f, P);
  const torso: Pt[] = [[P.hd[0] * 0.5 + P.sh[0] * 0.5, P.hd[1] + 10], P.sh, P.hp];
  const limbs: [Pt, Pt[]][] = [[P.hp, P.lB], [P.sh, P.aB], [P.hp, P.lF], [P.sh, P.aF]];
  for (const [o, l] of limbs) poly(g, [o, l[0], l[1]], glowC, 14, glowA);
  poly(g, torso, glowC, 16, glowA);
  g.fillStyle(glowC, glowA); g.fillCircle(P.hd[0], P.hd[1], 17);

  poly(g, [P.hp, ...P.lB], back, 5, A); poly(g, [P.sh, ...P.aB], back, 5, A);
  poly(g, torso, c, 6, A);
  poly(g, [P.hp, ...P.lF], c, 5.5, A); poly(g, [P.sh, ...P.aF], c, 5.5, A);
  g.fillStyle(0x14182a, A); g.fillCircle(P.hd[0], P.hd[1], 10);
  g.lineStyle(4.5, c, A); g.strokeCircle(P.hd[0], P.hd[1], 10);
  if (f.state === 'dizzy' || f.state === 'knockdown') {
    g.lineStyle(1.8, c, A);
    g.lineBetween(P.hd[0] + 2, P.hd[1] - 3, P.hd[0] + 6, P.hd[1] + 1); g.lineBetween(P.hd[0] + 6, P.hd[1] - 3, P.hd[0] + 2, P.hd[1] + 1);
  } else { g.fillStyle(c, A); g.fillRect(P.hd[0] + 3, P.hd[1] - 3, 3, 4); }
  if (C.look === 'sword') {
    g.lineStyle(3, c, A); g.lineBetween(P.hd[0] - 9, P.hd[1] - 4, P.hd[0] - 20, P.hd[1] - 1 + Math.sin(T * 0.2) * 2);
  }
  if (C.weapon === 'sword') drawSword(g, f, P, countering(f), T);
  if (C.look === 'king') drawCrown(g, P);
  if (C.weapon === 'hammer') drawHammer(g, f, P, charging && d?.charge?.button === 'special');
  if (m && d?.kind === 'gordo' && m.t < d.spawnFrame) {
    const k = m.t / d.spawnFrame;
    drawGordo(g, 20 + k * 36, -50 - Math.sin(Math.PI * k) * 34, 12, pal.c, m.t * 0.3);
  }
  if (m && d?.kind === 'projectile' && m.t < d.spawnFrame) {
    const h = P.aF[1], r = 3 + m.t * 0.6;
    g.fillStyle(pal.c, 0.3); g.fillCircle(h[0], h[1], r + 8);
    g.fillStyle(0xffffff, 1); g.fillCircle(h[0], h[1], r);
  }
  g.restore();

  if (f.state === 'shield') {
    const max = DATA.g.shield.max, r = 12 + 36 * Math.max(0, f.sh) / max, stun = f.sd.stun! > 0;
    g.fillStyle(pal.c, stun ? 0.5 : 0.28); g.fillCircle(f.x, f.y - 38, r);
    g.lineStyle(2.5, stun ? 0xffffff : pal.c, 0.85); g.strokeCircle(f.x, f.y - 38, r);
  }
  if (f.state === 'dizzy') {
    for (let k = 0; k < 3; k++) { const a = T * 0.12 + k * 2.09; star(g, f.x + Math.cos(a) * 18, f.y - 84 + Math.sin(a) * 5, 7, 0xffe680); }
  }
  if (showBoxes) {
    for (const bx of activeBoxes(f)) {
      if (bx.grab) continue;
      g.fillStyle(pal.c, 0.22); g.fillCircle(bx.wx, bx.wy, bx.r);
      g.lineStyle(2, 0xffffff, 0.55); g.strokeCircle(bx.wx, bx.wy, bx.r);
    }
  }
}

export function drawProjectiles(g: G, w: World) {
  for (const p of w.projs) drawProjectile(g, p);
}
function drawProjectile(g: G, p: Projectile) {
  const c = COLORS[p.o.i].c;
  if (p.gordo) { drawGordo(g, p.x, p.y, p.r, c, p.x * 0.06); return; }
  g.fillStyle(c, 0.2); g.fillCircle(p.x, p.y, p.r + 10);
  g.fillStyle(c, 0.3); g.fillCircle(p.x, p.y, p.r + 5);
  g.fillStyle(0xffffff, 1); g.fillCircle(p.x, p.y, p.r);
  g.fillStyle(c, 1); g.fillCircle(p.x, p.y, p.r * 0.55);
}

export const pctColor = (d: number) => {
  const t = clamp(d / 160, 0, 1);
  return (255 << 16) | (Math.round(255 - t * 205) << 8) | Math.round(255 - t * 225);
};
