// Hitbox editor viewport (dev tool): a posed fighter at any frame of a move with its hitboxes on top, edited by
// direct manipulation. State lives in EditorModel; the DOM panels are in ui.ts. Saving goes through the Vite dev
// server (vite.config.ts → /__editor/save), which rewrites src/data/fighters/<id>.ts.

import Phaser from 'phaser';
import { DATA } from '../data';
import type { Hitbox, MoveId } from '../data/types';
import { STAGE } from '../data/stage';
import { consumePressed, hh, pp } from '../game/input';
import { type Fighter, type MoveState, type World, createWorld } from '../sim/core';
import { circRect, hurt, launchAngle } from '../sim/combat';
import { drawFighter } from '../render/draw';
import { HIT_TYPES, typeOf } from '../render/hitTypes';
import { drawBackground, drawStage } from '../render/stage';
import { RES, txt } from '../scenes/common';
import { EditorModel, SPEEDS } from './model';
import { EditorUI } from './ui';

const FX = 640, FY = STAGE.top;
/** Kept across visits: unsaved edits, undo history and view toggles survive leaving the editor. */
const model = new EditorModel();

type Target = { kind: 'tip' | 'edge' | 'body' | 'dummy' | 'none'; i: number };
type Drag =
  | { kind: 'move'; dx: number; dy: number }
  | { kind: 'size' | 'angle' }
  | { kind: 'dummy'; dx: number; dy: number }
  | { kind: 'pan'; sx: number; sy: number; px: number; py: number; moved: boolean };
export interface EditorInit { fi?: number; move?: MoveId; from?: string }

export class EditorScene extends Phaser.Scene {
  m = model;
  private ui!: EditorUI;
  private w!: World;
  private f!: Fighter;
  private g!: Phaser.GameObjects.Graphics;
  private gb!: Phaser.GameObjects.Graphics;
  private labels: Phaser.GameObjects.Text[] = [];
  private from = 'menu';
  private zoomMul = 1;
  private pan = { x: 0, y: 0 };
  private drag: Drag | null = null;
  private lastDown = { t: 0, x: 0, y: 0 };
  private playAcc = 0;
  private version = 0;

  constructor() { super('editor'); }

  init(data: EditorInit) {
    const m = this.m;
    if (data.fi !== undefined && DATA.fighters[data.fi]) m.fi = data.fi;
    if (data.move && m.editable(data.move)) { m.id = data.move; m.sel = 0; m.frame = m.firstActive(); }
    this.from = data.from ?? 'menu';
  }

  create() {
    const m = this.m;
    if (!m.editable(m.id)) m.id = m.ids()[0];
    m.sel = Math.min(Math.max(m.sel, m.mv().hitboxes.length ? 0 : -1), m.mv().hitboxes.length - 1);
    m.frame = Math.max(1, Math.min(m.frame, m.total()));
    m.playing = false; m.helpOpen = false; m.hover = -1;
    this.drag = null; this.zoomMul = 1; this.pan = { x: 0, y: 0 }; this.labels = []; this.version = DATA.version;
    drawBackground(this.add.graphics());
    drawStage(this.add.graphics());
    this.g = this.add.graphics();
    this.gb = this.add.graphics();
    this.w = createWorld({ fighters: [{ ch: m.fi, cpu: false, name: 'P1' }], stocks: 1, cpuLevel: 0, training: true });
    this.f = this.w.fighters[0];
    this.ui = new EditorUI(m, { zoom: f => this.zoomBy(f), fit: () => this.fit(), exit: () => this.exit() });
    this.applyCam();
    this.input.on('pointerdown', this.down, this);
    this.input.on('pointermove', this.move, this);
    this.input.on('pointerup', this.up, this);
    this.input.on('wheel', this.wheel, this);
    this.events.once('shutdown', () => {
      this.ui.destroy();
      this.input.off('pointerdown', this.down, this);
      this.input.off('pointermove', this.move, this);
      this.input.off('pointerup', this.up, this);
      this.input.off('wheel', this.wheel, this);
      this.game.canvas.style.cursor = '';
    });
  }

  private exit() { this.m.playing = false; consumePressed(); this.scene.start(this.from); }

  // ---------- camera ----------
  /** Game pixels per CSS pixel. */
  private k() { return this.scale.width / Math.max(1, this.game.canvas.getBoundingClientRect().width); }
  /** CSS pixels → world units, so handles keep their on-screen size at any zoom. */
  private px(n: number) { return n * this.k() / this.cameras.main.zoom; }

  private applyCam() {
    const cam = this.cameras.main, s = this.scale, cr = this.game.canvas.getBoundingClientRect(), vr = this.ui.viewRect(), k = this.k();
    cam.setSize(s.width, s.height);
    const z = Math.max(0.05, Math.min((vr.width * k) / 430, (vr.height * k) / 290) * this.zoomMul);
    cam.setZoom(z);
    const ox = vr.left + vr.width / 2 - (cr.left + cr.width / 2), oy = vr.top + vr.height / 2 - (cr.top + cr.height / 2);
    cam.centerOn(FX + this.pan.x - (ox * k) / z, FY - 52 + this.pan.y - (oy * k) / z);
  }

  private fit() { this.zoomMul = 1; this.pan = { x: 0, y: 0 }; this.applyCam(); }

  private zoomBy(f: number, sx?: number, sy?: number) {
    const cam = this.cameras.main;
    const at = sx === undefined ? null : cam.getWorldPoint(sx, sy!);
    this.zoomMul = Phaser.Math.Clamp(this.zoomMul * f, 0.3, 6);
    this.applyCam();
    if (at) { const after = cam.getWorldPoint(sx!, sy!); this.pan.x += at.x - after.x; this.pan.y += at.y - after.y; this.applyCam(); }
  }

  private wheel(p: Phaser.Input.Pointer, _o: unknown, _dx: number, dy: number) { this.zoomBy(dy > 0 ? 1 / 1.12 : 1.12, p.x, p.y); }

  // ---------- geometry helpers ----------
  private center(b: Hitbox) { const g = this.m.geom(b); return { x: FX + g.x, y: FY - g.y, r: g.r }; }

  private sgn(b: Hitbox, cx: number) { return b.dir === 'back' ? -1 : b.dir === 'away' ? Math.sign(cx - FX) || 1 : 1; }

  /** Arrow tip (the angle handle), or null for types without an aimable angle. */
  private tipOf(b: Hitbox): [number, number] | null {
    const ty = typeOf(b);
    if (ty === 'grab' || ty === 'autolink' || ty === 'sakurai') return null;
    const c = this.center(b), a = launchAngle(b, 0, false) * Math.PI / 180, len = c.r + this.px(30);
    return [c.x + Math.cos(a) * len * this.sgn(b, c.x), c.y - Math.sin(a) * len];
  }

  private dummyRect() {
    const v = this.m.view, x = FX + v.dummyX, y = FY - v.dummyY;
    return { x1: x - 16, x2: x + 16, y1: y - 78, y2: y };
  }

  private hitTest(x: number, y: number): Target {
    const m = this.m, bx = m.mv().hitboxes, sel = m.box();
    if (sel) {
      const tp = this.tipOf(sel);
      if (tp && Math.hypot(x - tp[0], y - tp[1]) <= this.px(10)) return { kind: 'tip', i: m.sel };
      const c = this.center(sel);
      if (Math.abs(Math.hypot(x - c.x, y - c.y) - c.r) <= this.px(6)) return { kind: 'edge', i: m.sel };
    }
    let best = -1, score = Infinity;
    bx.forEach((b, i) => {
      const on = m.active(b);
      if (!on && !m.view.inactive && i !== m.sel) return;
      const c = this.center(b), d = Math.hypot(x - c.x, y - c.y);
      if (d > c.r) return;
      const s = d / c.r + (on ? 0 : 1) - (i === m.sel ? 0.05 : 0);
      if (s < score) { score = s; best = i; }
    });
    if (best >= 0) return { kind: 'body', i: best };
    if (m.view.dummy) { const r = this.dummyRect(); if (x >= r.x1 && x <= r.x2 && y >= r.y1 && y <= r.y2) return { kind: 'dummy', i: -1 }; }
    return { kind: 'none', i: -1 };
  }

  // ---------- pointer ----------
  private wp(p: Phaser.Input.Pointer) { const v = this.cameras.main.getWorldPoint(p.x, p.y); return { x: v.x, y: v.y }; }
  private ev(p: Phaser.Input.Pointer) { return p.event as MouseEvent; }

  private down(p: Phaser.Input.Pointer) {
    const m = this.m;
    (document.activeElement as HTMLElement | null)?.blur?.();
    if (m.helpOpen || p.button !== 0) return;
    const { x, y } = this.wp(p), t = this.hitTest(x, y), now = performance.now();
    const dbl = now - this.lastDown.t < 320 && Math.hypot(p.x - this.lastDown.x, p.y - this.lastDown.y) < 8;
    this.lastDown = { t: now, x: p.x, y: p.y };
    m.playing = false;
    if (t.kind === 'none' && dbl) { this.drag = null; m.add({ x: Math.round(x - FX), y: Math.round(FY - y) }); return; }
    if (t.kind === 'tip') { m.begin(); this.drag = { kind: 'angle' }; return; }
    if (t.kind === 'edge') { m.begin(); this.drag = { kind: 'size' }; return; }
    if (t.kind === 'body') {
      if (t.i !== m.sel) m.select(t.i);
      const c = this.center(m.box()!);
      m.begin();
      this.drag = { kind: 'move', dx: x - c.x, dy: y - c.y };
      return;
    }
    if (t.kind === 'dummy') { const v = m.view; this.drag = { kind: 'dummy', dx: x - (FX + v.dummyX), dy: y - (FY - v.dummyY) }; return; }
    this.drag = { kind: 'pan', sx: p.x, sy: p.y, px: this.pan.x, py: this.pan.y, moved: false };
  }

  private move(p: Phaser.Input.Pointer) {
    const m = this.m, d = this.drag, { x, y } = this.wp(p), e = this.ev(p);
    if (!d) {
      if (!p.isDown) {
        const t = this.hitTest(x, y);
        this.game.canvas.style.cursor = { tip: 'crosshair', edge: 'ew-resize', body: 'move', dummy: 'grab', none: 'default' }[t.kind];
        m.setHover(t.kind === 'body' || t.kind === 'edge' || t.kind === 'tip' ? t.i : -1);
      }
      return;
    }
    if (!p.isDown) { this.up(); return; }
    const b = m.box(), shift = e.shiftKey;
    if (d.kind === 'pan') {
      const zk = this.k() / this.cameras.main.zoom;
      if (Math.abs(p.x - d.sx) + Math.abs(p.y - d.sy) > 3) d.moved = true;
      this.pan.x = d.px - ((p.x - d.sx) / this.k()) * zk;
      this.pan.y = d.py - ((p.y - d.sy) / this.k()) * zk;
      this.applyCam();
      this.game.canvas.style.cursor = 'grabbing';
      return;
    }
    if (d.kind === 'dummy') {
      const snap = (v: number) => (shift ? Math.round(v / 5) * 5 : Math.round(v));
      m.view.dummyX = snap(x - d.dx - FX); m.view.dummyY = Math.max(0, snap(FY - (y - d.dy)));
      this.ui.tip(e.clientX, e.clientY, `dummy ${m.view.dummyX}, ${m.view.dummyY}`);
      return;
    }
    if (!b) return;
    const c = this.center(b);
    if (d.kind === 'move') {
      const snap = (v: number) => (shift ? Math.round(v / 5) * 5 : Math.round(v));
      const nx = snap(x - d.dx - FX), ny = snap(FY - (y - d.dy));
      m.setGeom({ x: nx, y: ny });
      this.ui.tip(e.clientX, e.clientY, `x ${nx}  y ${ny}`);
    } else if (d.kind === 'size') {
      const r = Math.max(2, Math.round(Math.hypot(x - c.x, y - c.y)));
      m.setGeom({ r });
      this.ui.tip(e.clientX, e.clientY, `radius ${r}`);
    } else {
      let a = (Math.atan2(c.y - y, (x - c.x) * this.sgn(b, c.x)) * 180) / Math.PI;
      a = shift ? Math.round(a / 15) * 15 : Math.round(a);
      if (a === -180) a = 180;
      if (typeOf(b) === 'spike') a = -Math.abs(a);
      m.edit((_, bb) => { if (bb) bb.angle = a; });
      this.ui.tip(e.clientX, e.clientY, `${a}°`);
    }
  }

  private up() {
    const d = this.drag;
    this.drag = null;
    this.ui.tip(0, 0, null);
    if (d?.kind === 'pan' && !d.moved) this.m.select(-1);
    this.game.canvas.style.cursor = 'default';
  }

  // ---------- loop ----------
  update(_time: number, delta: number) {
    const m = this.m;
    if (DATA.version !== this.version) { this.version = DATA.version; m.emit(); }
    this.keys();
    if (!this.scene.isActive()) return;
    if (m.playing) {
      this.playAcc += delta * SPEEDS[m.speedI];
      let t = m.frame;
      while (this.playAcc >= 1000 / 60) { this.playAcc -= 1000 / 60; t = (t % m.total()) + 1; }
      if (t !== m.frame) m.setFrame(t);
    } else this.playAcc = 0;
    this.applyCam();
    this.draw();
  }

  private keys() {
    const m = this.m, shift = hh('ShiftLeft', 'ShiftRight'), alt = hh('AltLeft', 'AltRight'), mod = hh('MetaLeft', 'MetaRight', 'ControlLeft', 'ControlRight');
    if (m.helpOpen) { consumePressed(); return; }
    if (mod) { consumePressed(); return; }
    if (pp('Escape')) { this.exit(); return; }
    const b = m.box();
    if (alt) {
      const dx = (pp('ArrowRight') ? 1 : 0) - (pp('ArrowLeft') ? 1 : 0), dy = (pp('ArrowUp') ? 1 : 0) - (pp('ArrowDown') ? 1 : 0), s = shift ? 5 : 1;
      if (b && (dx || dy)) { const g = m.geom(b, m.editFrame(b)); m.begin(); m.setGeom({ x: g.x + dx * s, y: g.y + dy * s }); }
    } else {
      if (pp('ArrowLeft', 'Comma')) { if (shift) m.jump(-1); else m.setFrame(m.frame - 1, true); }
      if (pp('ArrowRight', 'Period')) { if (shift) m.jump(1); else m.setFrame(m.frame + 1, true); }
      if (pp('ArrowUp')) m.stepMove(-1);
      if (pp('ArrowDown')) m.stepMove(1);
    }
    if (pp('Home')) m.setFrame(1);
    if (pp('End')) m.setFrame(m.total());
    const n = m.mv().hitboxes.length;
    if (pp('Tab') && n) m.select((m.sel + (shift ? n - 1 : 1) + n) % n);
    for (let d = 1; d <= 9; d++) if (pp(`Digit${d}`) && d <= n) m.select(d - 1);
    if (pp('KeyN')) m.add();
    if (pp('KeyD')) m.duplicate();
    if (pp('Delete', 'Backspace')) m.remove();
    if (pp('Space')) { m.playing = !m.playing; m.emit(); }
    if (pp('KeyP') && b) m.setPerFrame(!m.perFrame(b));
    if (pp('KeyC')) m.copyToNext();
    if (pp('KeyO')) { m.view.onion = !m.view.onion; m.emit(); }
    if (pp('KeyG')) { m.view.dummy = !m.view.dummy; m.emit(); }
    if (pp('KeyF')) this.fit();
    consumePressed();
  }

  // ---------- drawing ----------
  private label(i: number, s: string, x: number, y: number, alpha: number, color = '#fff') {
    const l = this.labels[i] ?? (this.labels[i] = txt(this, 0, 0, '', 12, '#fff', 'center', 800).setResolution(RES * 3).setDepth(10));
    l.setText(s).setColor(color).setPosition(x, y).setScale(this.k() / this.cameras.main.zoom).setAlpha(alpha).setVisible(true);
  }

  private arrow(x1: number, y1: number, x2: number, y2: number, col: number, a: number, w: number) {
    const gb = this.gb, ang = Math.atan2(y2 - y1, x2 - x1), hs = this.px(7 + w);
    gb.lineStyle(this.px(w), col, a).lineBetween(x1, y1, x2 - Math.cos(ang) * hs * 0.6, y2 - Math.sin(ang) * hs * 0.6);
    gb.fillStyle(col, a).fillTriangle(x2, y2, x2 - Math.cos(ang - 0.45) * hs, y2 - Math.sin(ang - 0.45) * hs, x2 - Math.cos(ang + 0.45) * hs, y2 - Math.sin(ang + 0.45) * hs);
  }

  private draw() {
    const m = this.m, mv = m.mv(), f = this.f, gb = this.gb, t = m.frame, px = (n: number) => this.px(n);
    Object.assign(f, { ch: m.fi, x: FX, y: FY, vx: 0, vy: 0, face: 1, ground: true, state: 'attack', lag: 0, flash: 0, inv: 0, blink: 0 });
    const ms: MoveState = { hit: new Set(), id: m.id, t, ph: 0, t2: 0, charge: 0 };
    f.sd = ms; f.move = ms;
    this.g.clear();
    drawFighter(this.g, this.w, f, false);
    gb.clear();
    let li = 0;
    if (m.view.hurtbox) { const h = hurt(f); gb.lineStyle(px(1.5), 0xffee55, 0.55).strokeRect(h.x1, h.y1, h.x2 - h.x1, h.y2 - h.y1); }
    gb.lineStyle(px(1.5), 0xffffff, 0.8).lineBetween(FX - px(6), FY, FX + px(6), FY).lineBetween(FX, FY - px(6), FX, FY + px(6));

    // reference dummy
    if (m.view.dummy) {
      const r = this.dummyRect();
      const hitBy = mv.hitboxes.findIndex(b => m.active(b) && typeOf(b) !== 'windbox' && (() => { const c = this.center(b); return circRect(c.x, c.y, c.r, r); })());
      const c = hitBy >= 0 ? 0xff4466 : 0x66aaff;
      gb.fillStyle(c, hitBy >= 0 ? 0.22 : 0.1).fillRect(r.x1, r.y1, 32, 78);
      gb.lineStyle(px(2), c, 0.9).strokeRect(r.x1, r.y1, 32, 78);
      gb.fillStyle(c, 0.6).fillCircle((r.x1 + r.x2) / 2, r.y1 + 12, 9);
      this.label(li++, hitBy >= 0 ? `HIT by ${hitBy + 1}` : `dummy · ${m.view.dummyX}px`, (r.x1 + r.x2) / 2, r.y2 + px(14), 1, hitBy >= 0 ? '#ff8095' : '#9cc4ff');
    }

    // boxes: inactive first, then active, then the selected one on top
    const order = mv.hitboxes.map((b, i) => i).sort((a, b) => this.rank(a) - this.rank(b));
    for (const i of order) {
      const b = mv.hitboxes[i], on = m.active(b), sel = i === m.sel, hov = i === m.hover, ty = typeOf(b), col = HIT_TYPES[ty].color;
      if (!on && !sel && !m.view.inactive) continue;
      const c = this.center(b), dim = on || sel ? 1 : 0.35;
      if (sel && m.perFrame(b) && m.view.onion) this.onion(b);
      if (on) gb.fillStyle(col, sel ? 0.4 : hov ? 0.36 : 0.26).fillCircle(c.x, c.y, c.r);
      gb.lineStyle(px(sel ? 3 : hov ? 2.5 : 1.5), sel ? 0xffd35a : col, dim).strokeCircle(c.x, c.y, c.r);
      if (!on && sel) gb.lineStyle(px(1), col, 0.6).strokeCircle(c.x, c.y, Math.max(1, c.r - px(4)));
      const aw = sel ? 2.5 : 1.5, aa = on || sel ? 0.95 : 0.3, tip = this.tipOf(b);
      if (tip) this.arrow(c.x, c.y, tip[0], tip[1], sel ? 0xffffff : 0xe8ecff, aa, aw);
      if (ty === 'sakurai') for (const deg of [0, 45]) { const a = deg * Math.PI / 180, L = c.r + px(22); this.arrow(c.x, c.y, c.x + Math.cos(a) * L, c.y - Math.sin(a) * L, 0xffffff, aa * 0.6, 1.5); }
      if (ty === 'autolink') for (let k = 0; k < 4; k++) { const a = k * Math.PI / 2 + Math.PI / 4, o = c.r + px(12), n = c.r * 0.45; this.arrow(c.x + Math.cos(a) * o, c.y + Math.sin(a) * o, c.x + Math.cos(a) * n, c.y + Math.sin(a) * n, 0xffffff, aa * 0.8, 1.5); }
      if (b.dir === 'away' && tip) gb.lineStyle(px(1.5), 0xffffff, aa).strokeCircle(c.x, c.y, px(5));
      if (sel) {
        if (tip) gb.fillStyle(0xffd35a, 1).fillCircle(tip[0], tip[1], px(6));
        gb.fillStyle(0x0b0e22, 1).fillCircle(c.x + c.r, c.y, px(5.5));
        gb.lineStyle(px(2), 0xffd35a, 1).strokeCircle(c.x + c.r, c.y, px(5.5));
      }
      this.label(li++, String(i + 1), c.x, c.y, on || sel ? 1 : 0.5, sel ? '#ffd35a' : '#ffffff');
    }
    for (; li < this.labels.length; li++) this.labels[li].setVisible(false);
  }

  private rank(i: number) {
    const m = this.m, b = m.mv().hitboxes[i];
    return i === m.sel ? 3 : i === m.hover ? 2 : m.active(b) ? 1 : 0;
  }

  /** Motion path through every per-frame position, with ghosts of the neighbouring frames. */
  private onion(b: Hitbox) {
    const m = this.m, gb = this.gb, col = HIT_TYPES[typeOf(b)].color, pts: { x: number; y: number }[] = [];
    for (let t = b.start; t <= b.end; t++) { const g = m.geom(b, t); pts.push({ x: FX + g.x, y: FY - g.y }); }
    gb.lineStyle(this.px(1.5), col, 0.5);
    for (let k = 1; k < pts.length; k++) gb.lineBetween(pts[k - 1].x, pts[k - 1].y, pts[k].x, pts[k].y);
    pts.forEach((p, k) => gb.fillStyle(b.start + k === m.editFrame(b) ? 0xffd35a : col, 0.8).fillCircle(p.x, p.y, this.px(2.5)));
    const t0 = m.editFrame(b);
    for (const [t, a] of [[t0 - 1, 0.35], [t0 + 1, 0.2]] as const) {
      if (t < b.start || t > b.end) continue;
      const g = m.geom(b, t);
      gb.lineStyle(this.px(1), col, a).strokeCircle(FX + g.x, FY - g.y, g.r);
    }
  }
}
