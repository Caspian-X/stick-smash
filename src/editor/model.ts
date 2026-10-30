// Hitbox editor state and every edit operation. The Phaser viewport and the DOM panels both read from it and
// call its methods; `on()` subscribers re-render after each change.

import { DATA, boxAt, moveTotal } from '../data';
import type { Hitbox, HitboxType, MoveId, StandardMove } from '../data/types';

export const MOVE_GROUPS: [string, MoveId[]][] = [
  ['Ground', ['jab', 'ftilt', 'utilt', 'dtilt', 'dashatk']],
  ['Smash', ['fsmash', 'usmash', 'dsmash']],
  ['Aerial', ['nair', 'fair', 'bair', 'uair', 'dair']],
  ['Grab', ['grab', 'dashgrab']],
  ['Special', ['nspec', 'sspec', 'uspec', 'dspec']],
];
export const ALL_MOVES = MOVE_GROUPS.flatMap(([, ids]) => ids);
export const SPEEDS = [0.1, 0.25, 0.5, 1];

const NEW_BOX: Hitbox = { start: 1, end: 4, x: 40, y: 45, r: 20, damage: 6, bkb: 5, kbg: 0.08, angle: 40 };
type Snap = { fi: number; id: MoveId; json: string };
export type Geom = { x: number; y: number; r: number };

export class EditorModel {
  fi = 0;
  id: MoveId = 'jab';
  frame = 1;
  sel = 0;
  hover = -1;
  playing = false;
  speedI = 2;
  view = { onion: true, inactive: true, hurtbox: true, dummy: false, dummyX: 70, dummyY: 0 };
  helpOpen = false;
  /** Locks the timeline scale while dragging bars (the move's total length changes mid-drag). */
  lockTotal = 0;
  toast: { text: string; kind: 'ok' | 'err' | 'info'; id: number } | null = null;
  readonly dirty = new Set<string>();
  private orig = new Map<string, string>();
  private undos: Snap[] = [];
  private redos: Snap[] = [];
  private owned = new WeakSet<object>();
  private subs = new Set<() => void>();
  private toastN = 0;

  on(fn: () => void) { this.subs.add(fn); return () => this.subs.delete(fn); }
  emit() { for (const fn of this.subs) fn(); }

  // ---------- reading ----------
  fighter() { return DATA.fighters[this.fi]; }
  editable(id: MoveId) { const d = this.fighter().moves[id]; return !!d && d.kind === undefined; }
  ids(): MoveId[] { return ALL_MOVES.filter(id => this.editable(id)); }

  /** Current move. Cloned once so edits never leak into shared objects (e.g. COMMON_GRABS). */
  mv(fi = this.fi, id = this.id): StandardMove {
    const fd = DATA.fighters[fi];
    let m = fd.moves[id]!;
    if (!this.owned.has(m)) { m = structuredClone(m); fd.moves[id] = m; this.owned.add(m); }
    return m as StandardMove;
  }
  box(i = this.sel): Hitbox | undefined { return this.mv().hitboxes[i]; }
  total() { return moveTotal(this.mv()); }
  timelineTotal() { return Math.max(this.total(), this.lockTotal); }
  geom(b: Hitbox, t = this.frame): Geom { const g = boxAt(b, t); return { x: g.x, y: g.y, r: g.r }; }
  active(b: Hitbox, t = this.frame) { return t >= b.start && t <= b.end; }
  perFrame(b: Hitbox) { return !!b.frames?.length; }
  /** Frame whose geometry an edit changes: the playhead, clamped into the box's active window. */
  editFrame(b: Hitbox) { return Math.max(b.start, Math.min(b.end, this.frame)); }
  key(fi = this.fi, id = this.id) { return `${fi}:${id}`; }
  isDirty(fi: number, id: MoveId) { return this.dirty.has(this.key(fi, id)); }
  canUndo() { return this.undos.length > 0; }
  canRedo() { return this.redos.length > 0; }
  phase(t = this.frame): 'startup' | 'active' | 'endlag' {
    const bx = this.mv().hitboxes;
    if (bx.some(b => this.active(b, t))) return 'active';
    return bx.length && t < Math.min(...bx.map(b => b.start)) ? 'startup' : 'endlag';
  }

  // ---------- history ----------
  /** Snapshot for undo. Call once at the start of each edit gesture (drag, typing session, button). */
  begin() {
    const json = JSON.stringify(this.mv()), k = this.key();
    if (!this.orig.has(k)) this.orig.set(k, json);
    const top = this.undos[this.undos.length - 1];
    if (top && top.fi === this.fi && top.id === this.id && top.json === json) return;
    this.undos.push({ fi: this.fi, id: this.id, json });
    if (this.undos.length > 400) this.undos.shift();
    this.redos.length = 0;
  }

  /** Apply a change to the current move (after `begin()`) and notify. */
  edit(fn: (mv: StandardMove, b: Hitbox | undefined) => void) {
    const mv = this.mv();
    fn(mv, mv.hitboxes[this.sel]);
    this.changed();
  }

  /** One-shot edit: begin + edit. */
  op(fn: (mv: StandardMove, b: Hitbox | undefined) => void) { this.begin(); this.edit(fn); }

  private changed() {
    const k = this.key();
    if (this.orig.get(k) === JSON.stringify(this.mv())) this.dirty.delete(k); else this.dirty.add(k);
    this.emit();
  }

  private restore(s: Snap) {
    const m = JSON.parse(s.json) as StandardMove;
    DATA.fighters[s.fi].moves[s.id] = m; this.owned.add(m);
    this.fi = s.fi; this.id = s.id;
    this.sel = Math.min(this.sel, m.hitboxes.length - 1);
  }

  private swap(from: Snap[], to: Snap[], what: string) {
    let s = from.pop();
    while (s && s.json === JSON.stringify(this.mv(s.fi, s.id))) s = from.pop();
    if (!s) { this.notify(`Nothing to ${what}`, 'info'); return; }
    to.push({ fi: s.fi, id: s.id, json: JSON.stringify(this.mv(s.fi, s.id)) });
    this.restore(s);
    this.changed();
  }
  undo() { this.swap(this.undos, this.redos, 'undo'); }
  redo() { this.swap(this.redos, this.undos, 'redo'); }

  revert() {
    const o = this.orig.get(this.key());
    if (!o || !this.dirty.has(this.key())) { this.notify('No unsaved changes on this move', 'info'); return; }
    this.begin();
    this.restore({ fi: this.fi, id: this.id, json: o });
    this.changed();
    this.notify('Reverted to the last saved version', 'info');
  }

  notify(text: string, kind: 'ok' | 'err' | 'info' = 'ok') { this.toast = { text, kind, id: ++this.toastN }; this.emit(); }

  // ---------- navigation ----------
  setFrame(t: number, wrap = false) {
    const n = this.total();
    this.frame = wrap ? (t < 1 ? n : t > n ? 1 : t) : Math.max(1, Math.min(n, t));
    this.emit();
  }
  firstActive() { const bx = this.mv().hitboxes; return bx.length ? Math.min(...bx.map(b => b.start)) : 1; }
  /** Frames where something starts or stops, for jump-to-next. */
  marks() {
    const s = new Set([1, this.total()]);
    for (const b of this.mv().hitboxes) { s.add(b.start); s.add(b.end); s.add(b.end + 1); }
    return [...s].filter(t => t >= 1 && t <= this.total()).sort((a, b) => a - b);
  }
  jump(d: number) {
    const m = this.marks(), t = d > 0 ? m.find(x => x > this.frame) : [...m].reverse().find(x => x < this.frame);
    if (t !== undefined) this.setFrame(t);
  }

  selectMove(id: MoveId) {
    if (!this.editable(id)) return;
    this.id = id; this.sel = this.mv().hitboxes.length ? 0 : -1; this.hover = -1;
    // while playing, restart from frame 1 so the whole animation shows
    this.frame = this.playing ? 1 : this.firstActive();
    this.emit();
  }
  stepMove(d: number) {
    const ids = this.ids(), i = ids.indexOf(this.id);
    this.selectMove(ids[(i + d + ids.length) % ids.length]);
  }
  selectFighter(fi: number) {
    this.fi = fi;
    if (!this.editable(this.id)) this.id = this.ids()[0];
    this.sel = Math.min(this.sel, this.mv().hitboxes.length - 1);
    this.setFrame(this.frame);
  }
  select(i: number) {
    const b = this.box(i);
    this.sel = b ? i : -1;
    if (b && !this.active(b)) this.frame = b.start;
    this.emit();
  }
  setHover(i: number) { if (i !== this.hover) { this.hover = i; this.emit(); } }

  // ---------- hitbox edits (call begin() first for gesture edits) ----------
  /** Move/resize the selected box: the shared geometry, or this frame's entry in per-frame mode. */
  setGeom(g: Partial<Geom>) {
    this.edit((_, b) => {
      if (!b) return;
      if (!b.frames?.length) { Object.assign(b, g); return; }
      const t = this.editFrame(b), cur = this.geom(b, t), next = { ...cur, ...g };
      const e = b.frames.find(x => x[0] === t);
      if (e) { e[1] = next.x; e[2] = next.y; e[3] = next.r; }
      else { b.frames.push([t, next.x, next.y, next.r]); b.frames.sort((p, q) => p[0] - q[0]); }
      if (t === b.start) Object.assign(b, next);
    });
  }

  /** Per-frame mode on: every active frame starts as this frame's geometry. Off: keep this frame's geometry for all. */
  setPerFrame(on: boolean) {
    this.op((_, b) => {
      if (!b) return;
      const g = this.geom(b, this.editFrame(b));
      if (on) { b.frames = []; for (let t = b.start; t <= b.end; t++) b.frames.push([t, g.x, g.y, g.r]); }
      else delete b.frames;
      Object.assign(b, g);
    });
    this.notify(on ? `Per-frame: every frame starts at this position. Adjust any frame on its own.` : 'Same position on every frame', 'info');
  }

  /** Copy this frame's position/size to every active frame. */
  applyToAll() {
    this.op((_, b) => {
      if (!b) return;
      const g = this.geom(b, this.editFrame(b));
      Object.assign(b, g);
      if (b.frames) b.frames = b.frames.map(([t]) => [t, g.x, g.y, g.r]);
    });
    this.notify('Applied this frame\'s position and size to all frames');
  }

  /** Copy this frame's position/size onto the next frame and step there (for nudging frame by frame). */
  copyToNext() {
    const b = this.box();
    if (!b?.frames || this.editFrame(b) >= b.end) return;
    const g = this.geom(b, this.editFrame(b));
    this.frame = this.editFrame(b) + 1;
    this.begin(); this.setGeom(g);
  }

  /** Change the active window; per-frame entries outside it are dropped and new frames get the nearest geometry. */
  setWindow(i: number, start: number, end: number) {
    this.edit(mv => {
      const b = mv.hitboxes[i];
      if (!b) return;
      start = Math.max(1, Math.round(start)); end = Math.max(start, Math.round(end));
      if (b.frames) {
        const fr: Hitbox['frames'] = [];
        for (let t = start; t <= end; t++) { const g = this.geom(b, Math.max(b.start, Math.min(b.end, t))); fr.push([t, g.x, g.y, g.r]); }
        b.frames = fr;
        Object.assign(b, this.geom(b, start));
      }
      b.start = start; b.end = end;
    });
  }

  setType(ty: HitboxType) {
    this.op((_, b) => {
      if (!b) return;
      const was = b.type ?? 'normal';
      if (ty === 'normal') delete b.type; else b.type = ty;
      if (ty === 'grab') Object.assign(b, { damage: 0, bkb: 0, kbg: 0, angle: 0 });
      else if (was === 'grab' && !b.damage && !b.bkb) Object.assign(b, { damage: 5, bkb: 5, kbg: 0.08, angle: 40 });
      if (ty === 'spike' && b.angle >= 0) b.angle = -80;
      if (ty !== 'spike' && was === 'spike' && b.angle < 0) b.angle = Math.abs(b.angle);
      if (ty === 'windbox') { b.damage = 0; b.kbg = 0; if (!b.bkb) b.bkb = 3; }
      if (ty === 'autolink' && !b.rehit) b.rehit = 4;
    });
  }

  add(at?: { x: number; y: number }) {
    this.begin();
    const mv = this.mv(), grab = this.id === 'grab' || this.id === 'dashgrab';
    const b: Hitbox = { ...NEW_BOX, start: this.frame, end: Math.min(this.frame + 3, Math.max(this.frame, this.total())), ...(at ?? {}) };
    if (grab) Object.assign(b, { damage: 0, bkb: 0, kbg: 0, angle: 0, type: 'grab' });
    mv.hitboxes.push(b);
    this.sel = mv.hitboxes.length - 1;
    this.edit(() => {});
    this.notify(`Added hitbox #${this.sel + 1}. Drag it, or edit it in the inspector.`, 'info');
  }

  duplicate() {
    const b = this.box();
    if (!b) return;
    this.begin();
    const c: Hitbox = structuredClone(b);
    c.x += 12;
    if (c.frames) c.frames = c.frames.map(([t, x, y, r]) => [t, x + 12, y, r]);
    this.mv().hitboxes.splice(this.sel + 1, 0, c);
    this.sel++;
    this.edit(() => {});
  }

  remove(i = this.sel) {
    if (!this.box(i)) return;
    this.begin();
    this.mv().hitboxes.splice(i, 1);
    this.sel = Math.min(this.sel, this.mv().hitboxes.length - 1);
    this.edit(() => {});
  }

  /** Move a box earlier/later in the list (earlier = higher priority). */
  reorder(i: number, j: number) {
    const bx = this.mv().hitboxes;
    if (!bx[i] || j < 0 || j >= bx.length || i === j) return;
    this.begin();
    const [b] = bx.splice(i, 1);
    bx.splice(j, 0, b);
    this.sel = j;
    this.edit(() => {});
  }

  // ---------- saving ----------
  async save() {
    if (!import.meta.env.DEV) { this.notify('Saving needs the dev server (npm run dev)', 'err'); return; }
    if (!this.dirty.size) { this.notify('Nothing to save', 'info'); return; }
    const byFighter = new Map<number, Record<string, StandardMove>>();
    for (const k of this.dirty) {
      const [fi, id] = k.split(':'), n = +fi;
      if (!byFighter.has(n)) byFighter.set(n, {});
      byFighter.get(n)![id] = this.mv(n, id as MoveId);
    }
    const files: string[] = [];
    try {
      for (const [fi, moves] of byFighter) {
        const res = await fetch('/__editor/save', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ fighter: DATA.fighters[fi].id, moves }) });
        const j = await res.json() as { ok: boolean; file?: string; error?: string };
        if (!j.ok) throw new Error(j.error);
        files.push(j.file!);
        for (const id of Object.keys(moves)) { const k = this.key(fi, id as MoveId); this.dirty.delete(k); this.orig.delete(k); }
      }
      this.notify(`Saved to ${files.join(', ')}`);
    } catch (e) {
      this.notify(`Save failed: ${e instanceof Error ? e.message : e}`, 'err');
    }
  }
}
