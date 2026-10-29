// Hitbox editor (dev tool): see and edit every hitbox of a move on a posed fighter, then save it back into the fighter's data file.
// Edits go straight into the live DATA, so Training uses them immediately. Saving posts to the Vite dev server
// (vite.config.ts → /__editor/save), which rewrites src/data/fighters/<id>.ts; HMR then reloads it.

import Phaser from 'phaser';
import { DATA, moveTotal } from '../data';
import type { Hitbox, MoveId, StandardMove } from '../data/types';
import { STAGE } from '../data/stage';
import { consumePressed, hh, pp } from '../game/input';
import { type Fighter, type MoveState, type World, createWorld } from '../sim/core';
import { hurt } from '../sim/combat';
import { drawFighter } from '../render/draw';
import { drawBackground, drawStage } from '../render/stage';
import { RES, txt } from '../scenes/common';
import { EditorPanel } from './panel';

const FX = 640, FY = STAGE.top, PANEL_W = 360;
const ORDER: MoveId[] = ['jab', 'ftilt', 'utilt', 'dtilt', 'dashatk', 'fsmash', 'usmash', 'dsmash', 'nair', 'fair', 'bair', 'uair', 'dair', 'grab', 'dashgrab', 'nspec', 'uspec', 'dspec'];
const SPEEDS = [0.25, 0.5, 1];
const NEW_BOX: Hitbox = { start: 1, end: 4, x: 40, y: 45, r: 20, damage: 6, bkb: 5, kbg: 0.08, angle: 40 };

type Drag = { kind: 'move' | 'size' | 'angle'; dx: number; dy: number };
export interface EditorInit { fi?: number; move?: MoveId; from?: string }

export class EditorScene extends Phaser.Scene {
  private w!: World;
  private f!: Fighter;
  private g!: Phaser.GameObjects.Graphics;
  private gb!: Phaser.GameObjects.Graphics;
  private labels: Phaser.GameObjects.Text[] = [];
  private panel!: EditorPanel;
  private fi = 0;
  private id: MoveId = 'jab';
  private frame = 1;
  private sel = 0;
  private playing = false;
  private speedI = 1;
  private playAcc = 0;
  private zoomMul = 1;
  private from = 'menu';
  private drag: Drag | null = null;
  private version = 0;
  private status = '';
  private onKey = (e: KeyboardEvent) => this.metaKey(e);
  // Kept across visits on purpose: unsaved edits live in DATA until saved or the page reloads.
  private dirty = new Set<string>();
  private orig = new Map<string, string>();
  private undos: { fi: number; id: MoveId; json: string }[] = [];
  private owned = new WeakSet<object>();

  constructor() { super('editor'); }

  init(data: EditorInit) {
    if (data.fi !== undefined && DATA.fighters[data.fi]) this.fi = data.fi;
    if (data.move) this.id = data.move;
    this.from = data.from ?? 'menu';
  }

  create() {
    this.drag = null; this.playing = false; this.zoomMul = 1; this.version = DATA.version; this.labels = [];
    this.status = import.meta.env.DEV ? '' : 'Read-only build: saving needs `npm run dev`.';
    drawBackground(this.add.graphics());
    drawStage(this.add.graphics());
    this.g = this.add.graphics();
    this.gb = this.add.graphics();
    this.w = createWorld({ fighters: [{ ch: this.fi, cpu: false, name: 'P1' }], stocks: 1, cpuLevel: 0, training: true });
    this.f = this.w.fighters[0];
    if (!this.ids().includes(this.id)) this.id = this.ids()[0];
    this.frame = Math.min(this.frame, moveTotal(this.mv()));
    this.panel = new EditorPanel({
      fighter: fi => { this.fi = fi; this.f.ch = fi; if (!this.ids().includes(this.id)) this.id = this.ids()[0]; this.sel = 0; this.setFrame(this.frame); },
      move: id => { this.id = id as MoveId; this.sel = 0; this.setFrame(this.firstActive()); },
      frame: t => this.setFrame(t),
      select: i => { this.sel = i; const b = this.mv().hitboxes[i]; if (b && (this.frame < b.start || this.frame > b.end)) this.frame = b.start; this.refresh(); },
      begin: () => this.begin(),
      edit: fn => { const mv = this.mv(); fn(mv, mv.hitboxes[this.sel]); this.changed(); },
      add: () => this.addBox(false),
      dup: () => this.addBox(true),
      del: () => this.delBox(),
      prio: d => this.prio(d),
      play: () => { this.playing = !this.playing; this.refresh(); },
      save: () => void this.save(),
      undo: () => this.undo(),
      revert: () => this.revert(),
      exit: () => this.exit(),
    });
    this.fit();
    this.scale.on('resize', this.fit, this);
    this.input.on('pointerdown', this.down, this);
    this.input.on('pointermove', this.move, this);
    this.input.on('pointerup', this.up, this);
    this.input.on('wheel', this.wheel, this);
    addEventListener('keydown', this.onKey);
    this.events.once('shutdown', () => {
      this.panel.destroy();
      removeEventListener('keydown', this.onKey);
      this.scale.off('resize', this.fit, this);
      this.input.off('pointerdown', this.down, this);
      this.input.off('pointermove', this.move, this);
      this.input.off('pointerup', this.up, this);
      this.input.off('wheel', this.wheel, this);
    });
    this.refresh();
  }

  // ---------- data ----------
  private ids(): MoveId[] {
    const mv = DATA.fighters[this.fi].moves;
    return ORDER.filter(id => mv[id]?.kind === undefined);
  }

  /** Current move. Cloned once so edits never leak into shared objects (e.g. COMMON_GRABS). */
  private mv(): StandardMove {
    const fd = DATA.fighters[this.fi];
    let m = fd.moves[this.id];
    if (!this.owned.has(m)) { m = structuredClone(m); fd.moves[this.id] = m; this.owned.add(m); }
    return m as StandardMove;
  }

  private key(fi = this.fi, id = this.id) { return `${fi}:${id}`; }

  private begin() {
    const json = JSON.stringify(this.mv()), k = this.key();
    if (!this.orig.has(k)) this.orig.set(k, json);
    const top = this.undos[this.undos.length - 1];
    if (top && top.fi === this.fi && top.id === this.id && top.json === json) return;
    this.undos.push({ fi: this.fi, id: this.id, json });
    if (this.undos.length > 300) this.undos.shift();
  }

  private changed() {
    const k = this.key();
    if (this.orig.get(k) === JSON.stringify(this.mv())) this.dirty.delete(k); else this.dirty.add(k);
    this.status = '';
    this.refresh();
  }

  private restore(fi: number, id: MoveId, json: string) {
    const m = JSON.parse(json) as StandardMove;
    DATA.fighters[fi].moves[id] = m; this.owned.add(m);
    this.fi = fi; this.f.ch = fi; this.id = id;
    this.sel = Math.min(this.sel, m.hitboxes.length - 1);
  }

  private undo() {
    let u = this.undos.pop();
    while (u && u.json === JSON.stringify(DATA.fighters[u.fi].moves[u.id])) u = this.undos.pop();
    if (!u) { this.status = 'Nothing to undo.'; this.refresh(); return; }
    this.restore(u.fi, u.id, u.json);
    this.changed();
  }

  private revert() {
    const o = this.orig.get(this.key());
    if (!o) { this.status = 'No unsaved changes on this move.'; this.refresh(); return; }
    this.begin();
    this.restore(this.fi, this.id, o);
    this.changed();
  }

  private addBox(copy: boolean) {
    this.begin();
    const mv = this.mv(), src = copy ? mv.hitboxes[this.sel] : undefined;
    const b: Hitbox = src ? { ...src, x: src.x + 10 } : { ...NEW_BOX, start: this.frame, end: this.frame + 3, ...(this.id.includes('grab') ? { damage: 0, bkb: 0, kbg: 0, angle: 0, grab: true } : {}) };
    mv.hitboxes.push(b);
    this.sel = mv.hitboxes.length - 1;
    this.changed();
  }

  private delBox() {
    const mv = this.mv();
    if (!mv.hitboxes[this.sel]) return;
    this.begin();
    mv.hitboxes.splice(this.sel, 1);
    this.sel = Math.min(this.sel, mv.hitboxes.length - 1);
    this.changed();
  }

  private prio(d: number) {
    const bx = this.mv().hitboxes, j = this.sel + d;
    if (!bx[this.sel] || j < 0 || j >= bx.length) return;
    this.begin();
    [bx[this.sel], bx[j]] = [bx[j], bx[this.sel]];
    this.sel = j;
    this.changed();
  }

  private async save() {
    if (!import.meta.env.DEV) { this.status = 'Saving needs the dev server (npm run dev).'; this.refresh(); return; }
    if (!this.dirty.size) { this.status = 'Nothing to save.'; this.refresh(); return; }
    const byFighter = new Map<number, Record<string, StandardMove>>();
    for (const k of this.dirty) {
      const [fi, id] = k.split(':'), n = +fi;
      if (!byFighter.has(n)) byFighter.set(n, {});
      byFighter.get(n)![id] = DATA.fighters[n].moves[id as MoveId] as StandardMove;
    }
    const files: string[] = [];
    try {
      for (const [fi, moves] of byFighter) {
        const res = await fetch('/__editor/save', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ fighter: DATA.fighters[fi].id, moves }) });
        const j = await res.json() as { ok: boolean; file?: string; error?: string };
        if (!j.ok) throw new Error(j.error);
        files.push(j.file!);
        for (const id of Object.keys(moves)) { this.dirty.delete(this.key(fi, id as MoveId)); this.orig.delete(this.key(fi, id as MoveId)); }
      }
      this.status = `Saved ✓ ${files.join(', ')}`;
    } catch (e) {
      this.status = `Save failed: ${e instanceof Error ? e.message : e}`;
    }
    this.refresh();
  }

  private exit() { consumePressed(); this.scene.start(this.from); }

  private metaKey(e: KeyboardEvent) {
    if (!(e.metaKey || e.ctrlKey)) return;
    if (e.code === 'KeyS') { e.preventDefault(); void this.save(); }
    else if (e.code === 'KeyZ') { e.preventDefault(); (document.activeElement as HTMLElement | null)?.blur?.(); this.undo(); }
  }

  // ---------- frame / panel ----------
  private firstActive() { const bx = this.mv().hitboxes; return bx.length ? Math.min(...bx.map(b => b.start)) : 1; }

  private setFrame(t: number) {
    const total = moveTotal(this.mv());
    this.frame = t < 1 ? total : t > total ? 1 : t;
    this.refresh();
  }

  private refresh() {
    const mv = this.mv();
    this.frame = Math.max(1, Math.min(this.frame, moveTotal(mv)));
    const names = [...this.dirty].map(k => { const [fi, id] = k.split(':'); return `${DATA.fighters[+fi].name} ${id}`; });
    this.panel.update({
      fi: this.fi, ids: this.ids(), id: this.id, mv, frame: this.frame, sel: this.sel, playing: this.playing, speed: SPEEDS[this.speedI],
      dirty: [...this.dirty], canSave: import.meta.env.DEV,
      status: this.status || (names.length ? `Unsaved: ${names.join(', ')}` : 'No unsaved changes.'),
    });
  }

  // ---------- camera / pointer ----------
  private fit() {
    const cam = this.cameras.main, s = this.scale, rect = this.game.canvas.getBoundingClientRect();
    cam.setSize(s.width, s.height);
    const k = s.width / Math.max(1, rect.width), z = (s.height / 340) * this.zoomMul;
    cam.setZoom(z);
    cam.centerOn(FX + (PANEL_W * k) / z / 2, FY - 45);
  }

  /** Screen pixels → world units, so handles stay the same on-screen size at any zoom. */
  private px(n: number) { return n * RES / this.cameras.main.zoom; }

  private wp(p: Phaser.Input.Pointer): [number, number] {
    const v = this.cameras.main.getWorldPoint(p.x, p.y);
    return [v.x, v.y];
  }

  private sgn(b: Hitbox) { return b.dir === 'back' ? -1 : b.dir === 'away' ? Math.sign(b.x) || 1 : 1; }

  private tip(b: Hitbox): [number, number] {
    const a = b.angle * Math.PI / 180, len = b.r + this.px(26);
    return [FX + b.x + Math.cos(a) * len * this.sgn(b), FY - b.y - Math.sin(a) * len];
  }

  private down(p: Phaser.Input.Pointer) {
    (document.activeElement as HTMLElement | null)?.blur?.();
    const [x, y] = this.wp(p), mv = this.mv(), hr = this.px(9), b = mv.hitboxes[this.sel];
    if (b) {
      const [tx, ty] = this.tip(b);
      if (!b.grab && Math.hypot(x - tx, y - ty) <= hr) { this.begin(); this.drag = { kind: 'angle', dx: 0, dy: 0 }; return; }
      if (Math.abs(Math.hypot(x - FX - b.x, y - FY + b.y) - b.r) <= hr) { this.begin(); this.drag = { kind: 'size', dx: 0, dy: 0 }; return; }
    }
    let best = -1, score = Infinity;
    mv.hitboxes.forEach((h, i) => {
      const d = Math.hypot(x - FX - h.x, y - FY + h.y);
      if (d > h.r) return;
      const s = d / h.r + (this.frame >= h.start && this.frame <= h.end ? 0 : 1);
      if (s < score) { score = s; best = i; }
    });
    this.sel = best;
    if (best >= 0) {
      const h = mv.hitboxes[best];
      this.begin();
      this.drag = { kind: 'move', dx: x - (FX + h.x), dy: y - (FY - h.y) };
      if (this.frame < h.start || this.frame > h.end) this.frame = h.start;
    }
    this.refresh();
  }

  private move(p: Phaser.Input.Pointer) {
    const d = this.drag, b = this.mv().hitboxes[this.sel];
    if (!d || !b || !p.isDown) return;
    const [x, y] = this.wp(p);
    if (d.kind === 'move') { b.x = Math.round(x - d.dx - FX); b.y = Math.round(FY - (y - d.dy)); }
    else if (d.kind === 'size') b.r = Math.max(2, Math.round(Math.hypot(x - FX - b.x, y - FY + b.y)));
    else {
      let a = Math.atan2(FY - b.y - y, (x - FX - b.x) * this.sgn(b)) * 180 / Math.PI;
      a = (p.event as MouseEvent).shiftKey ? Math.round(a / 15) * 15 : Math.round(a);
      b.angle = a === -180 ? 180 : a;
    }
    this.changed();
  }

  private up() { this.drag = null; }

  private wheel(_p: Phaser.Input.Pointer, _o: unknown, _dx: number, dy: number) {
    this.zoomMul = Phaser.Math.Clamp(this.zoomMul * (dy > 0 ? 0.9 : 1.1), 0.4, 4);
    this.fit();
  }

  // ---------- loop ----------
  update(_time: number, delta: number) {
    if (DATA.version !== this.version) { this.version = DATA.version; this.refresh(); }
    const mv = this.mv(), n = mv.hitboxes.length;
    if (pp('Escape')) { this.exit(); return; }
    if (pp('ArrowLeft', 'Comma')) this.setFrame(this.frame - 1);
    if (pp('ArrowRight', 'Period')) this.setFrame(this.frame + 1);
    if (pp('ArrowUp', 'ArrowDown')) {
      const ids = this.ids(), i = ids.indexOf(this.id);
      this.id = ids[(i + (pp('ArrowUp') ? -1 : 1) + ids.length) % ids.length]; this.sel = 0; this.setFrame(this.firstActive());
    }
    if (pp('KeyF')) { this.fi = (this.fi + 1) % DATA.fighters.length; this.f.ch = this.fi; if (!this.ids().includes(this.id)) this.id = this.ids()[0]; this.sel = 0; this.refresh(); }
    if (pp('Tab') && n) { this.sel = (this.sel + (hh('ShiftLeft', 'ShiftRight') ? n - 1 : 1)) % n; this.refresh(); }
    if (pp('KeyN')) this.addBox(false);
    if (pp('Delete', 'Backspace')) this.delBox();
    if (pp('Space')) { this.playing = !this.playing; this.refresh(); }
    if (pp('KeyM')) { this.speedI = (this.speedI + 1) % SPEEDS.length; this.refresh(); }
    consumePressed();
    if (this.playing) {
      this.playAcc += delta * SPEEDS[this.speedI];
      let moved = false;
      while (this.playAcc >= 1000 / 60) { this.playAcc -= 1000 / 60; this.frame = this.frame % moveTotal(this.mv()) + 1; moved = true; }
      if (moved) this.refresh();
    }
    this.draw();
  }

  private draw() {
    const f = this.f, mv = this.mv(), g = this.g, gb = this.gb, t = this.frame;
    Object.assign(f, { ch: this.fi, x: FX, y: FY, vx: 0, vy: 0, face: 1, ground: true, state: 'attack', lag: 0, flash: 0, inv: 0, blink: 0 });
    const m: MoveState = { hit: new Set(), id: this.id, t, ph: 0, t2: 0, charge: 0 };
    f.sd = m; f.move = m;
    g.clear();
    drawFighter(g, this.w, f, false);
    gb.clear();
    const h = hurt(f);
    gb.lineStyle(this.px(1), 0xffee55, 0.6).strokeRect(h.x1, h.y1, h.x2 - h.x1, h.y2 - h.y1);
    gb.fillStyle(0xffffff, 1).fillCircle(FX, FY, this.px(3));
    mv.hitboxes.forEach((b, i) => {
      const wx = FX + b.x, wy = FY - b.y, on = t >= b.start && t <= b.end, sel = i === this.sel, c = b.grab ? 0xc070ff : 0xff3355;
      if (on) gb.fillStyle(c, sel ? 0.42 : 0.3).fillCircle(wx, wy, b.r);
      gb.lineStyle(this.px(sel ? 3 : 1.5), sel ? 0xffd35a : c, on || sel ? 1 : 0.35).strokeCircle(wx, wy, b.r);
      if (!b.grab) {
        const [ex, ey] = this.tip(b), a = on || sel ? 0.95 : 0.3;
        gb.lineStyle(this.px(sel ? 2.5 : 1.5), 0xffffff, a).lineBetween(wx, wy, ex, ey);
        gb.fillStyle(sel ? 0xffd35a : 0xffffff, a).fillCircle(ex, ey, this.px(sel ? 6 : 3.5));
        if (b.dir === 'away') gb.lineStyle(this.px(1.5), 0xffffff, a).strokeCircle(wx, wy, this.px(5));
      }
      if (sel) {
        gb.fillStyle(0x0b0e24, 1).fillCircle(wx + b.r, wy, this.px(6));
        gb.lineStyle(this.px(2), 0xffd35a, 1).strokeCircle(wx + b.r, wy, this.px(6));
      }
      const lbl = this.labels[i] ?? (this.labels[i] = txt(this, 0, 0, '', 11, '#fff', 'center', 800).setResolution(RES * 4));
      lbl.setText(String(i + 1)).setPosition(wx, wy).setScale(RES / this.cameras.main.zoom * 1.4).setAlpha(on || sel ? 1 : 0.45).setVisible(true);
    });
    for (let i = mv.hitboxes.length; i < this.labels.length; i++) this.labels[i].setVisible(false);
  }
}
