// Visual effects layer: particles, rings, KO beams, screen shake. Consumes sim Fx events.

import type Phaser from 'phaser';
import type { Fx } from '../sim/core';
import { playSfx } from '../game/audio';
import { WORLD_H, WORLD_W } from '../data/stage';

interface Particle { x: number; y: number; vx: number; vy: number; life: number; max: number; col: number; sz: number; g: number }
interface Ring { x: number; y: number; r: number; vr: number; life: number; max: number; col: number }
interface Beam { x: number; y: number; ang: number; life: number; max: number; col: number }
export interface View { x0: number; y0: number; x1: number; y1: number }

const rnd = (a: number, b: number) => a + Math.random() * (b - a);

export class FxLayer {
  parts: Particle[] = [];
  rings: Ring[] = [];
  beams: Beam[] = [];
  shake = 0;

  burst(x: number, y: number, col: number, n: number, spd: number, life: number, sz: number, g = 0.15) {
    for (let k = 0; k < n; k++) {
      const a = Math.random() * Math.PI * 2, s = Math.random() * spd;
      this.parts.push({ x, y, vx: Math.cos(a) * s, vy: Math.sin(a) * s, life, max: life, col, sz: sz * (0.5 + Math.random()), g });
    }
  }

  consume(fx: Fx[], view: View, sound = true) {
    for (const e of fx) {
      switch (e.t) {
        case 'sfx': if (sound) playSfx(e.name, e.arg); break;
        case 'burst': this.burst(e.x, e.y, e.col, e.n, e.spd, e.life, e.sz, e.g); break;
        case 'dust':
          for (let k = 0; k < e.n; k++) this.parts.push({ x: e.x + rnd(-10, 10), y: e.y, vx: rnd(-2.5, 2.5), vy: rnd(-1.6, -0.3), life: 22, max: 22, col: 0xaaaabb, sz: rnd(3, 6), g: 0 });
          break;
        case 'ring': this.rings.push({ x: e.x, y: e.y, r: e.r, vr: e.vr, life: e.life, max: e.life, col: e.col }); break;
        case 'part': this.parts.push({ x: e.x, y: e.y, vx: e.vx, vy: e.vy, life: e.life, max: e.life, col: e.col, sz: e.sz, g: e.g }); break;
        case 'shake': this.shake = Math.max(this.shake, e.k); break;
        case 'beam': {
          const x = Math.max(view.x0 + 10, Math.min(view.x1 - 10, e.x)), y = Math.max(view.y0 + 10, Math.min(view.y1 - 10, e.y));
          this.burst(x, y, e.col, 50, 14, 40, 6, 0); this.burst(x, y, 0xffffff, 30, 10, 30, 4, 0);
          this.beams.push({ x, y, ang: Math.atan2(WORLD_H / 2 - y, WORLD_W / 2 - x), life: 45, max: 45, col: e.col });
          this.rings.push({ x, y, r: 20, vr: 10, life: 20, max: 20, col: 0xffffff });
          break;
        }
      }
    }
    fx.length = 0;
  }

  tick() {
    for (const p of this.parts) { p.x += p.vx; p.y += p.vy; p.vy += p.g; p.vx *= 0.95; p.life--; }
    this.parts = this.parts.filter(p => p.life > 0);
    for (const r of this.rings) { r.r += r.vr; r.life--; }
    this.rings = this.rings.filter(r => r.life > 0);
    for (const b of this.beams) b.life--;
    this.beams = this.beams.filter(b => b.life > 0);
    this.shake *= 0.85; if (this.shake < 0.3) this.shake = 0;
  }

  draw(g: Phaser.GameObjects.Graphics) {
    for (const b of this.beams) {
      const a = b.life / b.max, w = 30 + (1 - a) * 90;
      g.save(); g.translateCanvas(b.x, b.y); g.rotateCanvas(b.ang);
      const steps = 6;
      for (let k = 0; k < steps; k++) {
        const x0 = 900 * k / steps, x1 = 900 * (k + 1) / steps, fade = 1 - k / steps;
        g.fillStyle(k === 0 ? 0xffffff : b.col, a * fade * 0.9);
        g.beginPath(); g.moveTo(x0, -w * x0 / 900); g.lineTo(x1, -w * x1 / 900); g.lineTo(x1, w * x1 / 900); g.lineTo(x0, w * x0 / 900); g.closePath(); g.fillPath();
      }
      g.restore();
    }
    for (const p of this.parts) {
      g.fillStyle(p.col, p.life / p.max);
      g.fillCircle(p.x, p.y, p.sz * (0.4 + 0.6 * p.life / p.max));
    }
    for (const r of this.rings) {
      g.lineStyle(3, r.col, r.life / r.max);
      g.strokeCircle(r.x, r.y, r.r);
    }
  }
}
