// Background and stage art (static Graphics) plus twinkling stars.

import type Phaser from 'phaser';
import { vec } from './vec';
import { PLATS, STAGE, UNDER } from '../data/stage';

const X0 = -1400, X1 = 2700, Y0 = -900, Y1 = 1500;

function ridge(g: Phaser.GameObjects.Graphics, base: number, amp: number, seed: number, col: number) {
  const pts: { x: number; y: number }[] = [{ x: X0, y: Y1 }];
  for (let x = X0; x <= X1; x += 20) pts.push({ x, y: base - amp * (0.5 + 0.3 * Math.sin(x * 0.006 + seed) + 0.2 * Math.sin(x * 0.017 + seed * 3)) });
  pts.push({ x: X1, y: Y1 });
  g.fillStyle(col, 1); g.fillPoints(vec(pts), true);
}

export function drawBackground(g: Phaser.GameObjects.Graphics) {
  g.fillStyle(0x0b0d2a, 1); g.fillRect(X0, Y0, X1 - X0, 0 - Y0);
  g.fillGradientStyle(0x0b0d2a, 0x0b0d2a, 0x2a2150, 0x2a2150, 1); g.fillRect(X0, 0, X1 - X0, 432);
  g.fillGradientStyle(0x2a2150, 0x2a2150, 0x4a2a4a, 0x4a2a4a, 1); g.fillRect(X0, 432, X1 - X0, 300);
  g.fillStyle(0x4a2a4a, 1); g.fillRect(X0, 732, X1 - X0, Y1 - 732);
  for (let k = 6; k > 0; k--) { g.fillStyle(0xffd38a, 0.035); g.fillCircle(1030, 140, 56 + k * 7); }
  g.fillStyle(0xffe9c4, 1); g.fillCircle(1030, 140, 56);
  ridge(g, 560, 260, 1, 0x221a3e); ridge(g, 640, 200, 4, 0x1a1530); ridge(g, 720, 120, 9, 0x120f22);
}

export function drawStage(g: Phaser.GameObjects.Graphics) {
  const s = STAGE;
  g.fillGradientStyle(0x5b6386, 0x5b6386, 0x3b4060, 0x3b4060, 1);
  g.fillRect(s.L, s.top, s.R - s.L, s.bot - s.top);
  g.fillStyle(0x2c3048, 1);
  g.fillPoints(vec([{ x: s.L, y: s.bot }, { x: s.R, y: s.bot }, { x: s.R - UNDER.inset, y: s.bot + UNDER.depth }, { x: s.L + UNDER.inset, y: s.bot + UNDER.depth }]), true);
  g.fillStyle(0xb8c4ff, 1); g.fillRect(s.L, s.top, s.R - s.L, 6);
  g.fillStyle(0xffffff, 0.13); for (let x = s.L + 40; x < s.R; x += 80) g.fillRect(x, s.top + 16, 40, 3);
  for (const p of PLATS) {
    g.fillStyle(0x8fa2ff, 0.2); g.fillRect(p.x1, p.y + 6, p.x2 - p.x1, 10);
    g.fillStyle(0xc9d4ff, 1); g.fillRoundedRect(p.x1, p.y, p.x2 - p.x1, 7, 3);
  }
}

const STARS = Array.from({ length: 300 }, () => ({ u: Math.random(), v: Math.random(), r: Math.random() * 1.5 + 0.3, p: Math.random() * 6 }));
export function drawStars(g: Phaser.GameObjects.Graphics, t: number) {
  g.clear();
  for (const s of STARS) {
    g.fillStyle(0xffffff, 0.5 + 0.5 * Math.sin(t * 0.03 + s.p));
    g.fillRect(-700 + s.u * 2680, -500 + s.v * 970, s.r, s.r);
  }
}
