// Balance/debug view: hurtboxes, every active hitbox with its launch angle, and per-fighter frame info.

import type Phaser from 'phaser';
import { moveTotal } from '../data';
import { LEDGE, STAGE } from '../data/stage';
import { type Fighter, type World, MV } from '../sim/core';
import { activeBoxes, hurt } from '../sim/combat';
import { txt } from '../scenes/common';

export class DebugOverlay {
  g: Phaser.GameObjects.Graphics;
  private labels: Phaser.GameObjects.Text[];

  constructor(scene: Phaser.Scene, w: World, layer: Phaser.GameObjects.Container) {
    this.g = scene.add.graphics();
    layer.add(this.g);
    this.labels = w.fighters.map(() => {
      const t = txt(scene, 0, 0, '', 11, '#e8ffe8', 'center', 700).setOrigin(0.5, 1);
      layer.add(t);
      return t;
    });
  }

  draw(w: World, on: boolean) {
    const g = this.g;
    g.clear();
    this.labels.forEach(l => l.setVisible(on));
    if (!on) return;
    g.lineStyle(1, 0x66ffcc, 0.35);
    for (const side of [-1, 1]) {
      const ex = side < 0 ? STAGE.L : STAGE.R;
      const x1 = side < 0 ? ex - LEDGE.reachOut : ex - LEDGE.reachIn, x2 = side < 0 ? ex + LEDGE.reachIn : ex + LEDGE.reachOut;
      g.strokeRect(x1, STAGE.top + LEDGE.minBelowTop, x2 - x1, LEDGE.maxBelowTop - LEDGE.minBelowTop);
    }
    for (const p of w.projs) g.lineStyle(2, 0xff3355, 1).strokeCircle(p.x, p.y, p.r);
    w.fighters.forEach((f, k) => {
      if (f.state === 'out' || f.state === 'dead') { this.labels[k].setVisible(false); return; }
      const h = hurt(f), inv = f.inv > 0 || f.blink > 0;
      const hc = inv ? 0x66ddff : f.state === 'hitstun' ? 0xff9a3c : 0xffee55;
      g.fillStyle(hc, 0.12).fillRect(h.x1, h.y1, h.x2 - h.x1, h.y2 - h.y1);
      g.lineStyle(1.5, hc, 0.9).strokeRect(h.x1, h.y1, h.x2 - h.x1, h.y2 - h.y1);
      g.fillStyle(0xffffff, 1).fillCircle(f.x, f.y, 2.5);
      for (const b of activeBoxes(f)) {
        const c = b.grab ? 0xc070ff : 0xff3355;
        g.fillStyle(c, 0.3).fillCircle(b.wx, b.wy, b.r);
        g.lineStyle(2, c, 1).strokeCircle(b.wx, b.wy, b.r);
        if (b.grab) continue;
        const sgn = b.dir === 'back' ? -f.face : f.face, a = b.angle * Math.PI / 180, len = 16 + b.bkb * 3;
        if (b.dir === 'away') { g.lineStyle(2, 0xffffff, 0.9).strokeCircle(b.wx, b.wy, 4); continue; }
        const ex = b.wx + Math.cos(a) * len * sgn, ey = b.wy - Math.sin(a) * len;
        g.lineStyle(2.5, 0xffffff, 0.95).lineBetween(b.wx, b.wy, ex, ey);
        g.fillStyle(0xffffff, 1).fillCircle(ex, ey, 3);
      }
      this.labels[k].setText(info(f)).setPosition(f.x, f.y - 120).setVisible(true);
    });
  }
}

function info(f: Fighter): string {
  const lines = [`${f.state} ${f.t}`];
  if (f.state === 'attack' && f.move) {
    const d = MV(f), tot = moveTotal(d);
    lines.push(`${d.name ?? f.move.id} ${f.move.t}${tot < 999 ? '/' + tot : ''}${f.move.charge ? ` +${f.move.charge}ch` : ''}${f.move.ph ? ` ph${f.move.ph}` : ''}`);
  }
  const extra: string[] = [];
  if (f.lag) extra.push(`hitlag ${f.lag}`);
  if (f.state === 'hitstun') extra.push(`stun ${f.hitstun}`);
  if (f.state === 'shield' && f.sd.stun) extra.push(`shieldstun ${f.sd.stun}`);
  if (f.state === 'landing') extra.push(`lag ${f.sd.n! - f.t}`);
  if (f.inv || f.blink) extra.push(`inv ${Math.max(f.inv, f.blink)}`);
  if (extra.length) lines.push(extra.join(' · '));
  return lines.join('\n');
}
