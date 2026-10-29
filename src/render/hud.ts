// In-match HUD: damage cards, name tags, off-screen bubbles, minimap, big center text.

import type Phaser from 'phaser';
import { vec } from './vec';
import { DATA } from '../data';
import { COLORS } from '../data/colors';
import { BLAST, PLATS, STAGE, UNDER, WORLD_H, WORLD_W } from '../data/stage';
import type { World } from '../sim/core';
import { pctColor } from './draw';
import { type View } from './fx';
import { hex, txt } from '../scenes/common';

type Text = Phaser.GameObjects.Text;
const rnd = (a: number, b: number) => a + Math.random() * (b - a);
const NO_TAG = ['ledge', 'climb', 'grabbed', 'knockdown', 'out', 'dead'];

export class Hud {
  g: Phaser.GameObjects.Graphics;
  private cards: { name: Text; ch: Text; pct: Text }[];
  private tags: Text[];
  center: Text;
  sub: Text;
  hint: Text;
  private mmA = 0;

  constructor(scene: Phaser.Scene, w: World, worldLayer: Phaser.GameObjects.Container) {
    this.g = scene.add.graphics();
    this.cards = w.fighters.map(f => ({
      name: txt(scene, 0, 0, f.name, 18, COLORS[f.i].css, 'left'),
      ch: txt(scene, 0, 0, DATA.fighters[f.ch].name.toUpperCase(), 10, '#99a', 'right', 800),
      pct: txt(scene, 0, 0, '0%', 42, '#fff', 'right'),
    }));
    this.tags = w.fighters.map(f => { const t = txt(scene, 0, 0, f.name, 13, COLORS[f.i].css, 'center', 800); worldLayer.add(t); return t; });
    this.center = txt(scene, WORLD_W / 2, WORLD_H / 2 - 60, '', 140);
    this.sub = txt(scene, WORLD_W / 2, WORLD_H / 2 + 20, '', 22, '#fff', 'center', 700);
    this.hint = txt(scene, 0, 0, 'Esc: pause', 13, '#ffffff88', 'right', 600);
  }

  update(w: World, v: View) {
    const g = this.g;
    g.clear();
    const cy = v.y1 - 58;
    w.fighters.forEach((f, k) => {
      const cx = k === 0 ? 440 : 840, c = COLORS[f.i], ui = this.cards[k];
      g.fillStyle(0x000000, 0.66).fillRoundedRect(cx - 115, cy - 42, 230, 78, 12);
      g.lineStyle(2, c.c, 1).strokeRoundedRect(cx - 115, cy - 42, 230, 78, 12);
      ui.name.setPosition(cx - 100, cy - 20).setText(f.name);
      ui.ch.setPosition(cx + 100, cy - 30).setText(DATA.fighters[f.ch].name.toUpperCase());
      const out = f.state === 'out', sx = f.hudShake ? rnd(-3, 3) : 0, sy = f.hudShake ? rnd(-3, 3) : 0;
      ui.pct.setPosition(cx + 100 + sx, cy + 2 + sy).setText(out ? '—' : Math.floor(f.dmg) + '%').setColor(out ? '#666' : hex(pctColor(f.dmg)));
      g.fillStyle(c.c, 1);
      if (w.cfg.training) g.fillRect(cx - 100, cy + 14, 40, 3);
      else for (let s = 0; s < f.stocks; s++) g.fillCircle(cx - 94 + s * 18, cy + 16, 6);
    });

    w.fighters.forEach((f, k) => {
      const t = this.tags[k];
      t.setVisible(!NO_TAG.includes(f.state)).setText(f.name);
      t.setPosition(f.x, f.y - 104);
    });
    for (const f of w.fighters) {
      if (f.state === 'out' || f.state === 'dead') continue;
      if (f.x > v.x0 && f.x < v.x1 && f.y - 40 > v.y0 && f.y - 40 < v.y1) continue;
      const bx = Math.max(v.x0 + 34, Math.min(v.x1 - 34, f.x)), by = Math.max(v.y0 + 34, Math.min(v.y1 - 34, f.y - 40)), c = COLORS[f.i].c;
      g.fillStyle(0x000000, 0.8).fillCircle(bx, by, 24);
      g.lineStyle(3, c, 1).strokeCircle(bx, by, 24).strokeCircle(bx, by - 4, 7).lineBetween(bx, by + 3, bx, by + 14);
    }
    this.hint.setPosition(v.x1 - 14, v.y0 + 18);
    this.minimap(w, v);
  }

  private minimap(w: World, v: View) {
    const live = w.fighters.filter(f => f.state !== 'out' && f.state !== 'dead');
    const off = live.some(f => f.x < v.x0 || f.x > v.x1 || f.y - 40 < v.y0 || f.y - 40 > v.y1);
    this.mmA += ((off ? 1 : 0) - this.mmA) * 0.15;
    if (this.mmA < 0.02) return;
    const g = this.g, a = this.mmA, s = 200 / (BLAST.R - BLAST.L), mw = 200, mh = (BLAST.B - BLAST.T) * s;
    const ox = v.x0 + 14, oy = v.y0 + 14;
    const mx = (x: number) => ox + (x - BLAST.L) * s, my = (y: number) => oy + (y - BLAST.T) * s;
    g.fillStyle(0x000000, 0.73 * a).fillRoundedRect(ox, oy, mw, mh, 6);
    g.lineStyle(1.5, 0xff6a6a, 0.53 * a).strokeRoundedRect(ox, oy, mw, mh, 6);
    const vx0 = Math.max(v.x0, BLAST.L), vx1 = Math.min(v.x1, BLAST.R), vy0 = Math.max(v.y0, BLAST.T), vy1 = Math.min(v.y1, BLAST.B);
    g.lineStyle(1, 0xffffff, 0.27 * a).strokeRect(mx(vx0), my(vy0), (vx1 - vx0) * s, (vy1 - vy0) * s);
    const st = STAGE;
    g.fillStyle(0x8f9ac8, a).fillPoints(vec([
      { x: mx(st.L), y: my(st.top) }, { x: mx(st.R), y: my(st.top) }, { x: mx(st.R), y: my(st.bot) },
      { x: mx(st.R - UNDER.inset), y: my(st.bot + UNDER.depth) }, { x: mx(st.L + UNDER.inset), y: my(st.bot + UNDER.depth) }, { x: mx(st.L), y: my(st.bot) },
    ]), true);
    g.fillStyle(0xc9d4ff, a);
    for (const p of PLATS) g.fillRect(mx(p.x1), my(p.y) - 1, (p.x2 - p.x1) * s, 2);
    g.fillStyle(0xffffff, a);
    for (const p of w.projs) g.fillRect(mx(p.x) - 1, my(p.y) - 1, 2, 2);
    for (const f of live) {
      const x = mx(Math.max(BLAST.L, Math.min(BLAST.R, f.x))), y = my(Math.max(BLAST.T, Math.min(BLAST.B, f.y - 40)));
      g.fillStyle(COLORS[f.i].c, a).fillCircle(x, y, 5);
      g.lineStyle(1.5, 0xffffff, a).strokeCircle(x, y, 5);
    }
  }
}
