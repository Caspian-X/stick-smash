// Hitbox editor DOM shell: top bar, move list, viewport overlays (HUD, tools, toasts, drag readouts), help, and the
// inspector + timeline. Renders from the model at most once per animation frame.

import { DATA, moveTotal } from '../data';
import { type MoveId, SPECIAL_DIR } from '../data/types';
import { CSS, dragFrom, h } from './dom';
import { Inspector } from './inspector';
import { type EditorModel, MOVE_GROUPS } from './model';
import { Timeline } from './timeline';

export interface ViewActions { zoom(f: number): void; fit(): void; exit(): void }

const LAYOUT_KEY = 'stick-smash.editor-layout';
type Layout = { lw?: number; rw?: number; th?: number };
const PHASE_NAME = { startup: 'Startup', active: 'Active', endlag: 'End lag' };
const isMac = /Mac|iPhone|iPad/.test(navigator.platform);
const MOD = isMac ? '⌘' : 'Ctrl';

export class EditorUI {
  root = h('div', { id: 'hbe' });
  view = h('div', { className: 'view' });
  private style = h('style', { textContent: CSS });
  private insp: Inspector;
  private time: Timeline;
  private tabs = h('div', { className: 'tabs' });
  private status = h('span');
  private undoB = h('button', { textContent: '↶ Undo', title: `Undo (${MOD}+Z)` });
  private redoB = h('button', { textContent: '↷ Redo', title: `Redo (${MOD}+Shift+Z)` });
  private saveB = h('button', { className: 'primary' });
  private left = h('div', { className: 'left pane' });
  private hud = h('div', { className: 'hud' });
  private hint = h('div', { className: 'hint' });
  private toastEl = h('div', { className: 'toast hide' });
  private tools = h('div', { className: 'vtools' });
  private tipEl = h('div', { className: 'tip', style: 'display:none' });
  private modal: HTMLElement | null = null;
  private toastId = 0;
  private toastTimer = 0;
  private queued = false;
  private tabKey = '';
  private leftKey = '';
  private toolKey = '';
  private off: () => void;
  private onKey = (e: KeyboardEvent) => this.key(e);
  private onUnload = (e: BeforeUnloadEvent) => { if (this.m.dirty.size) { e.preventDefault(); e.returnValue = ''; } };

  constructor(private m: EditorModel, private act: ViewActions) {
    this.insp = new Inspector(m);
    this.time = new Timeline(m);
    document.head.append(this.style);
    this.undoB.onclick = () => m.undo();
    this.redoB.onclick = () => m.redo();
    this.saveB.onclick = () => void m.save();
    if (!import.meta.env.DEV) this.saveB.title = 'Saving needs the dev server (npm run dev)';
    const top = h('div', { className: 'top pane' },
      h('div', { className: 'brand' }, h('i'), 'Hitbox Editor'),
      this.tabs, h('div', { className: 'spacer' }), this.status,
      this.undoB, this.redoB,
      h('button', { className: 'ghost', textContent: 'Revert move', title: 'Discard unsaved changes to this move', onclick: () => m.revert() }),
      this.saveB,
      h('button', { className: 'icon ghost', textContent: '?', title: 'Shortcuts (?)', onclick: () => this.toggleHelp() }),
      h('button', { textContent: 'Exit', title: 'Back (Esc). Unsaved edits stay active in game until you reload.', onclick: () => act.exit() }));
    this.view.append(this.hud, this.hint, this.toastEl, this.tools);
    this.root.append(top, this.left, this.view, this.insp.el, this.time.el, this.tipEl,
      this.splitter('l', 'lw'), this.splitter('r', 'rw'), this.splitter('row', 'th'));
    this.applyLayout();
    document.body.append(this.root);
    this.off = m.on(() => this.schedule());
    addEventListener('keydown', this.onKey, true);
    addEventListener('beforeunload', this.onUnload);
    this.render();
  }

  destroy() {
    this.off();
    removeEventListener('keydown', this.onKey, true);
    removeEventListener('beforeunload', this.onUnload);
    clearTimeout(this.toastTimer);
    this.root.remove(); this.style.remove();
  }

  private layout(): Layout { try { return JSON.parse(localStorage.getItem(LAYOUT_KEY) ?? '{}') as Layout; } catch { return {}; } }

  private applyLayout(l = this.layout()) {
    const st = this.root.style, set = (k: string, v?: number) => (v ? st.setProperty(k, `${v}px`) : st.removeProperty(k));
    set('--lw', l.lw); set('--rw', l.rw); set('--th', l.th);
    if (l.th) st.setProperty('--thmax', 'none'); else st.removeProperty('--thmax');
  }

  /** Drag handle on a panel edge. Sizes persist in localStorage; double-click resets to the default. */
  private splitter(side: 'l' | 'r' | 'row', k: keyof Layout) {
    const el = h('div', { className: `split ${side === 'row' ? 'row' : 'col ' + side}`, title: 'Drag to resize · double-click to reset' });
    const pane = { l: this.left, r: this.insp.el, row: this.time.el }[side];
    const [lo, hi] = { l: [120, 480], r: [260, 720], row: [70, innerHeight - 220] }[side];
    el.onmousedown = e => {
      const v0 = side === 'row' ? pane.offsetHeight : pane.offsetWidth;
      el.classList.add('drag');
      dragFrom(e, (dx, dy) => {
        const v = Math.round(Math.max(lo, Math.min(hi, v0 + (side === 'l' ? dx : side === 'r' ? -dx : -dy))));
        const l = { ...this.layout(), [k]: v };
        localStorage.setItem(LAYOUT_KEY, JSON.stringify(l));
        this.applyLayout(l);
      }, () => el.classList.remove('drag'), side === 'row' ? 'row-resize' : 'col-resize');
    };
    el.ondblclick = () => { const l = this.layout(); delete l[k]; localStorage.setItem(LAYOUT_KEY, JSON.stringify(l)); this.applyLayout(l); };
    return el;
  }

  /** Viewport rectangle in CSS pixels (the canvas area not covered by panels). */
  viewRect() { return this.view.getBoundingClientRect(); }

  /** Floating readout next to the cursor (null hides it). */
  tip(x: number, y: number, text: string | null) {
    this.tipEl.style.display = text ? '' : 'none';
    if (text) { this.tipEl.textContent = text; this.tipEl.style.left = `${x}px`; this.tipEl.style.top = `${y}px`; }
  }

  private schedule() {
    if (this.queued) return;
    this.queued = true;
    requestAnimationFrame(() => { this.queued = false; this.render(); });
  }

  render() {
    const m = this.m;
    this.renderTop();
    this.renderLeft();
    this.renderView();
    this.insp.render();
    this.time.render();
    if (m.helpOpen !== !!this.modal) this.toggleHelp(m.helpOpen);
  }

  private renderTop() {
    const m = this.m, tabKey = `${m.fi}|${[...m.dirty]}`;
    // Rebuild only on change: during playback this runs every frame, and replacing a button mid-click eats the click.
    if (tabKey !== this.tabKey) {
      this.tabKey = tabKey;
      this.tabs.replaceChildren(...DATA.fighters.map((f, i) => h('button', {
        className: i === m.fi ? 'sel' : '', onclick: () => m.selectFighter(i),
      }, f.name, [...m.dirty].some(k => k.startsWith(`${i}:`)) ? h('span', { className: 'dot' }) : null)));
    }
    const n = m.dirty.size;
    const names = [...m.dirty].map(k => { const [fi, id] = k.split(':'); return `${DATA.fighters[+fi].name} ${id}`; });
    this.status.className = n ? 'badge' : 'clean';
    this.status.textContent = n ? `${n} unsaved` : 'All changes saved';
    this.status.title = n ? `Unsaved: ${names.join(', ')}` : '';
    this.undoB.disabled = !m.canUndo();
    this.redoB.disabled = !m.canRedo();
    this.saveB.replaceChildren('Save to code ', h('kbd', { textContent: `${MOD}S` }));
    this.saveB.disabled = !n || !import.meta.env.DEV;
  }

  private renderLeft() {
    const m = this.m, fd = m.fighter();
    const rows: { id: MoveId; name: string; meta: string; ok: boolean; title: string; dir?: string }[] = [];
    for (const [, ids] of MOVE_GROUPS) for (const id of ids) {
      const d = fd.moves[id], dir = SPECIAL_DIR[id];
      if (!d) {
        if (dir) rows.push({ id, name: '(uses neutral)', meta: '', ok: false, dir, title: `${fd.name} has no side special: side + special does the neutral special` });
        continue;
      }
      const ok = d.kind === undefined;
      const meta = ok && d.hitboxes.length ? `${Math.min(...d.hitboxes.map(b => b.start))}f · ${moveTotal(d)}` : ok ? '—' : d.kind;
      const title = (dir ? `${dir} special · ` : '') + (ok ? `${d.name ?? id}: startup · total frames` : `${d.name ?? id} uses '${d.kind}' special logic (edit its numbers in the fighter file)`);
      rows.push({ id, name: d.name ?? id, meta, ok, title, dir });
    }
    const key = JSON.stringify([m.fi, m.id, [...m.dirty], rows]);
    if (key === this.leftKey) return;
    this.leftKey = key;
    const keep = this.left.scrollTop, kids: Node[] = [];
    for (const [grp, ids] of MOVE_GROUPS) {
      kids.push(h('div', { className: 'grp', textContent: grp }));
      for (const r of rows.filter(x => ids.includes(x.id))) {
        kids.push(h('div', {
          className: `mv${r.id === m.id ? ' sel' : ''}${r.ok ? '' : ' off'}`, onclick: () => m.selectMove(r.id), title: r.title,
        }, r.dir ? h('span', { className: 'sdir', textContent: r.dir }) : null, h('span', { className: 'nm', textContent: r.name }),
        m.isDirty(m.fi, r.id) ? h('span', { className: 'dot', title: 'Unsaved' }) : null, h('span', { className: 'meta', textContent: r.meta })));
      }
    }
    this.left.replaceChildren(...kids);
    this.left.scrollTop = keep;
  }

  private renderTools() {
    const m = this.m;
    const t = (label: string, key: keyof EditorModel['view'], title: string) => h('button', {
      className: `icon${m.view[key] ? ' on' : ''}`, textContent: label, title, onclick: () => { (m.view[key] as boolean) = !m.view[key]; m.emit(); },
    });
    this.tools.replaceChildren(
      t('◌', 'onion', 'Onion skin: show a per-frame box\'s path and neighbouring frames (O)'),
      t('◐', 'inactive', 'Show boxes that aren\'t active on this frame'),
      t('▭', 'hurtbox', 'Show the fighter\'s hurtbox'),
      t('☺', 'dummy', 'Reference dummy: drag it around to check spacing. It glows red when a box would hit (G)'),
      h('button', { className: 'icon', textContent: '−', title: 'Zoom out (wheel)', onclick: () => this.act.zoom(1 / 1.2) }),
      h('button', { className: 'icon', textContent: '⤢', title: 'Fit (F)', onclick: () => this.act.fit() }),
      h('button', { className: 'icon', textContent: '+', title: 'Zoom in (wheel)', onclick: () => this.act.zoom(1.2) }),
    );
  }

  private renderView() {
    const m = this.m, ph = m.phase(), b = m.box(), mv = m.mv();
    this.hud.replaceChildren(...[
      h('span', { className: 'chip', textContent: `${mv.name ?? m.id}` }),
      h('span', { className: `chip ph-${ph}`, textContent: `Frame ${m.frame} / ${m.total()} · ${PHASE_NAME[ph]}` }),
      b && m.perFrame(b) ? h('span', { className: 'chip', style: { color: 'var(--accent)' }, textContent: `◆ Per-frame box ${m.sel + 1}: editing frame ${m.editFrame(b)}` }) : null,
    ].filter(x => x !== null));
    this.hint.textContent = b
      ? 'Drag: move · Edge: resize · Arrow tip: aim · Double-click: add · ? shortcuts'
      : mv.hitboxes.length ? 'Click a hitbox to select · double-click to add · drag empty space to pan' : 'Double-click anywhere to add a hitbox there';
    const v = m.view, toolKey = `${v.onion}${v.inactive}${v.hurtbox}${v.dummy}`;
    if (toolKey !== this.toolKey) {
      this.toolKey = toolKey;
      this.renderTools();
    }
    if (m.toast && m.toast.id !== this.toastId) {
      this.toastId = m.toast.id;
      this.toastEl.className = `toast ${m.toast.kind}`;
      this.toastEl.textContent = m.toast.text;
      clearTimeout(this.toastTimer);
      this.toastTimer = window.setTimeout(() => this.toastEl.classList.add('hide'), m.toast.kind === 'err' ? 6000 : 2600);
    }
  }

  private toggleHelp(open = !this.modal) {
    this.m.helpOpen = open;
    if (!open) { this.modal?.remove(); this.modal = null; return; }
    if (this.modal) return;
    const k = (what: string, ...keys: string[]) => h('div', { className: 'k' }, h('span', { textContent: what }), h('span', {}, ...keys.map(x => h('kbd', { textContent: x }))));
    this.modal = h('div', { className: 'modal', onclick: (e: MouseEvent) => { if (e.target === this.modal) this.toggleHelp(false); } },
      h('div', {},
        h('h3', { textContent: 'Hitbox editor shortcuts' }),
        h('div', { className: 'cols' },
          k('Select box', 'Click'), k('Move box', 'Drag'), k('Resize', 'Drag edge'), k('Aim launch angle', 'Drag arrow tip'),
          k('Snap angle 15° / move 5px', 'Shift'), k('Add box here', 'Double-click'), k('Pan view', 'Drag empty'), k('Zoom', 'Wheel'),
          k('Previous / next frame', '←', '→'), k('Previous / next change', 'Shift', '←/→'), k('First / last frame', 'Home', 'End'),
          k('Play / pause', 'Space'), k('Previous / next move', '↑', '↓'), k('Nudge box 1px (5 with Shift)', 'Alt', 'arrows'),
          k('Next / previous box', 'Tab', 'Shift+Tab'), k('Select box 1–9', '1…9'), k('Add / duplicate / delete', 'N', 'D', 'Del'),
          k('Per-frame on/off', 'P'), k('Copy → next frame', 'C'), k('Onion skin / dummy', 'O', 'G'), k('Fit view', 'F'),
          k('Undo / redo', `${MOD}Z`, `${MOD}⇧Z`), k('Save to code', `${MOD}S`), k('Exit', 'Esc')),
        h('p', { className: 'note', textContent: 'Number fields: drag the label left/right to scrub (Shift = ×10), or type. Timeline: drag bars to move, edges to resize, the end of the blue band to change end lag.' }),
        h('p', { className: 'note', textContent: 'Edits apply live, so Training uses them immediately. Saving writes the edited moves into src/data/fighters/*.ts.' }),
        h('div', { className: 'row', style: 'justify-content:flex-end' }, h('button', { textContent: 'Close', onclick: () => this.toggleHelp(false) }))));
    this.root.append(this.modal);
  }

  /** Shortcuts that must work even while a field has focus (capture phase, before the game's key tracker). */
  private key(e: KeyboardEvent) {
    const mod = e.metaKey || e.ctrlKey, m = this.m;
    const typing = e.target instanceof HTMLElement && !!e.target.closest('input, select, textarea');
    if (mod && e.code === 'KeyS') { e.preventDefault(); (document.activeElement as HTMLElement | null)?.blur?.(); void m.save(); }
    else if (mod && e.code === 'KeyZ') { e.preventDefault(); (document.activeElement as HTMLElement | null)?.blur?.(); if (e.shiftKey) m.redo(); else m.undo(); }
    else if (mod && e.code === 'KeyY') { e.preventDefault(); m.redo(); }
    else if (!typing && !mod && e.key === '?') { e.preventDefault(); e.stopImmediatePropagation(); this.toggleHelp(); }
    else if (this.modal && e.key === 'Escape') { e.preventDefault(); e.stopImmediatePropagation(); this.toggleHelp(false); }
  }
}
