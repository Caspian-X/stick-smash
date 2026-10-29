// DOM side panel for the hitbox editor: move/fighter pickers, frame timeline, hitbox list and field inputs.

import { DATA, frameSummary, moveTotal } from '../data';
import type { Hitbox, StandardMove } from '../data/types';

export interface PanelState {
  fi: number;
  ids: string[];
  id: string;
  mv: StandardMove;
  frame: number;
  sel: number;
  playing: boolean;
  speed: number;
  dirty: string[];
  status: string;
  canSave: boolean;
}

export interface PanelActions {
  fighter(fi: number): void;
  move(id: string): void;
  frame(t: number): void;
  select(i: number): void;
  /** Called before a field edit starts (undo snapshot). */
  begin(): void;
  /** A field changed; `apply` mutates the move. */
  edit(apply: (mv: StandardMove, b: Hitbox | undefined) => void): void;
  add(): void; dup(): void; del(): void; prio(dir: number): void;
  play(): void; save(): void; undo(): void; revert(): void; exit(): void;
}

const NUM_FIELDS: [keyof Hitbox, string, number][] = [
  ['start', 'Start', 1], ['end', 'End', 1], ['x', 'X (fwd)', 1], ['y', 'Y (up)', 1], ['r', 'Radius', 1],
  ['damage', 'Damage %', 0.5], ['angle', 'Angle °', 1], ['bkb', 'Base KB', 0.5], ['kbg', 'KB growth', 0.005],
];
const OPT_FIELDS: [keyof Hitbox, string, number][] = [
  ['hitstunMult', 'Hitstun ×', 0.1], ['hitstunAdd', 'Hitstun +', 1], ['hitlagMult', 'Hitlag ×', 0.1],
  ['shieldDmgMult', 'Shield dmg ×', 0.1], ['shieldStunMult', 'Shield stun ×', 0.1],
];

const CSS = `
#hbe { position: fixed; top: 0; right: 0; bottom: 0; width: 360px; overflow-y: auto; box-sizing: border-box; padding: 12px 14px;
  background: #0b0e24ee; color: #dde2ff; font: 12px/1.35 system-ui, -apple-system, sans-serif; border-left: 1px solid #2a3060; z-index: 10; }
#hbe h1 { font-size: 15px; margin: 0 0 8px; letter-spacing: .5px; display: flex; justify-content: space-between; align-items: center; }
#hbe h2 { font-size: 12px; margin: 12px 0 5px; color: #9aa3d8; text-transform: uppercase; letter-spacing: .6px; }
#hbe select, #hbe input, #hbe button { font: inherit; color: #fff; background: #1a1f45; border: 1px solid #343b78; border-radius: 4px; }
#hbe select { padding: 3px 4px; }
#hbe input[type=number] { width: 100%; box-sizing: border-box; padding: 3px 5px; }
#hbe button { padding: 4px 8px; cursor: pointer; }
#hbe button:hover { background: #2a3170; }
#hbe button.pri { background: #2d6b3a; border-color: #3f9a52; }
#hbe button:disabled { opacity: .45; cursor: default; }
#hbe .row { display: flex; gap: 6px; align-items: center; margin: 4px 0; flex-wrap: wrap; }
#hbe .grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 5px 8px; }
#hbe .grid label { display: flex; flex-direction: column; gap: 2px; color: #9aa3d8; font-size: 11px; }
#hbe .sum { color: #aab; margin: 4px 0; }
#hbe .tl { display: grid; gap: 1px; margin: 4px 0; cursor: pointer; user-select: none; }
#hbe .tl div { height: 9px; background: #1a1f45; }
#hbe .tl .su { background: #4a5070; } #hbe .tl .ac { background: #e0405a; } #hbe .tl .el { background: #3a68c0; }
#hbe .tl .on { background: #e0405a; } #hbe .tl .gr { background: #b070ff; } #hbe .tl .sel.on, #hbe .tl .sel.gr { background: #ffd35a; }
#hbe .tl .cur { outline: 1px solid #fff; outline-offset: -1px; }
#hbe .list div { padding: 3px 6px; border-radius: 3px; cursor: pointer; display: flex; justify-content: space-between; }
#hbe .list div:hover { background: #1a1f45; } #hbe .list div.sel { background: #3a3410; color: #ffd35a; }
#hbe .status { margin-top: 8px; padding: 6px; border-radius: 4px; background: #14183a; color: #cfd; white-space: pre-wrap; }
#hbe .help { color: #8890b8; font-size: 11px; margin-top: 10px; }
#hbe kbd { background: #1a1f45; border: 1px solid #343b78; border-radius: 3px; padding: 0 3px; font-size: 10px; }
`;

const h = <K extends keyof HTMLElementTagNameMap>(tag: K, props: Record<string, unknown> = {}, ...kids: (Node | string)[]): HTMLElementTagNameMap[K] => {
  const e = Object.assign(document.createElement(tag), props) as HTMLElementTagNameMap[K];
  e.append(...kids);
  return e;
};

export class EditorPanel {
  root: HTMLDivElement;
  private style: HTMLStyleElement;
  private fighterSel = h('select');
  private moveSel = h('select');
  private sum = h('div', { className: 'sum' });
  private frameLbl = h('span');
  private playBtn = h('button');
  private tl = h('div', { className: 'tl' });
  private moveFields = h('div', { className: 'grid' });
  private list = h('div', { className: 'list' });
  private boxFields = h('div');
  private status = h('div', { className: 'status' });
  private saveBtn = h('button', { className: 'pri', textContent: 'Save to code (⌘S)' });
  private st!: PanelState;
  private key = '';
  private optKey = '';

  constructor(private act: PanelActions) {
    this.style = h('style', { textContent: CSS });
    document.head.append(this.style);
    this.fighterSel.onchange = () => { act.fighter(+this.fighterSel.value); this.fighterSel.blur(); };
    this.moveSel.onchange = () => { act.move(this.moveSel.value); this.moveSel.blur(); };
    this.playBtn.onclick = () => act.play();
    this.saveBtn.onclick = () => act.save();
    const btn = (t: string, f: () => void, title = '') => h('button', { textContent: t, title, onclick: f });
    this.tl.onmousedown = e => this.timelineClick(e);
    this.tl.onmousemove = e => { if (e.buttons & 1) this.timelineClick(e); };
    this.root = h('div', { id: 'hbe' },
      h('h1', {}, 'HITBOX EDITOR', btn('Esc: back', () => act.exit())),
      h('div', { className: 'row' }, this.fighterSel, this.moveSel),
      this.sum,
      h('div', { className: 'row' }, btn('◀', () => act.frame(this.st.frame - 1), '← key'), this.frameLbl, btn('▶', () => act.frame(this.st.frame + 1), '→ key'), this.playBtn),
      this.tl,
      h('h2', {}, 'Move'), this.moveFields,
      h('h2', {}, 'Hitboxes (first listed wins on overlap)'), this.list,
      h('div', { className: 'row' }, btn('+ Add', () => act.add(), 'N'), btn('Duplicate', () => act.dup()), btn('Delete', () => act.del(), 'Del'),
        btn('↑ Priority', () => act.prio(-1)), btn('↓', () => act.prio(1))),
      this.boxFields,
      h('div', { className: 'row' }, this.saveBtn, btn('Undo (⌘Z)', () => act.undo()), btn('Revert move', () => act.revert())),
      this.status,
      h('div', { className: 'help' }, ...helpLines()),
    );
    document.body.append(this.root);
  }

  destroy() { this.root.remove(); this.style.remove(); }

  /** Full refresh. Field inputs are rebuilt only when the move/selection changes, so typing isn't interrupted. */
  update(st: PanelState) {
    this.st = st;
    const mv = st.mv, total = moveTotal(mv), optKey = `${st.fi}|${st.id}|${st.ids}|${st.dirty}`;
    if (optKey !== this.optKey) {
      this.optKey = optKey;
      this.fighterSel.replaceChildren(...DATA.fighters.map((f, i) => h('option', { value: String(i), textContent: f.name, selected: i === st.fi })));
      this.moveSel.replaceChildren(...st.ids.map(id => {
        const d = DATA.fighters[st.fi].moves[id as keyof typeof DATA.fighters[0]['moves']];
        return h('option', { value: id, textContent: `${d.name ?? id} (${id})${st.dirty.includes(`${st.fi}:${id}`) ? ' •' : ''}`, selected: id === st.id });
      }));
    }
    this.sum.textContent = frameSummary(mv);
    this.frameLbl.textContent = `frame ${st.frame} / ${total}`;
    this.playBtn.textContent = `${st.playing ? '❚❚ Pause' : '▶ Play'} ${st.speed}x`;
    this.playBtn.title = 'Space = play/pause · M = speed';
    this.timeline(total);
    this.list.replaceChildren(...mv.hitboxes.map((b, i) => h('div', {
      className: i === st.sel ? 'sel' : '', onclick: () => this.act.select(i),
    }, h('span', { textContent: `#${i + 1}  f${b.start}–${b.end}` }), h('span', { textContent: b.grab ? 'grab' : `${num(b.damage)}%  ${b.angle}°${b.dir ? ' ' + b.dir : ''}` }))));
    if (!mv.hitboxes.length) this.list.append(h('div', { textContent: 'No hitboxes. Press + Add.' }));
    const key = `${st.fi}:${st.id}:${st.sel}:${mv.hitboxes.length}`;
    if (key !== this.key) { this.key = key; this.buildFields(); } else this.syncFields();
    this.saveBtn.disabled = !st.canSave || !st.dirty.length;
    this.status.textContent = st.status;
  }

  private timeline(total: number) {
    const st = this.st, mv = st.mv, bx = mv.hitboxes;
    const s0 = bx.length ? Math.min(...bx.map(b => b.start)) : 1, e0 = bx.length ? Math.max(...bx.map(b => b.end)) : 0;
    this.tl.style.gridTemplateColumns = `repeat(${total}, 1fr)`;
    const cells: HTMLElement[] = [];
    for (let t = 1; t <= total; t++) cells.push(h('div', { className: (t < s0 ? 'su' : t <= e0 ? 'ac' : 'el') + (t === st.frame ? ' cur' : '') }));
    bx.forEach((b, i) => {
      for (let t = 1; t <= total; t++) {
        const on = t >= b.start && t <= b.end;
        cells.push(h('div', { className: `${on ? (b.grab ? 'gr' : 'on') : ''}${i === st.sel ? ' sel' : ''}${t === st.frame ? ' cur' : ''}` }));
      }
    });
    this.tl.replaceChildren(...cells);
    this.tl.title = 'Top row: startup (grey) / active (red) / end lag (blue). One row per hitbox. Click to scrub.';
  }

  private timelineClick(e: MouseEvent) {
    const r = this.tl.getBoundingClientRect(), total = moveTotal(this.st.mv), rows = this.st.mv.hitboxes.length + 1;
    const t = Math.max(1, Math.min(total, Math.floor((e.clientX - r.left) / r.width * total) + 1));
    const row = Math.floor((e.clientY - r.top) / r.height * rows);
    if (row > 0 && row - 1 !== this.st.sel) this.act.select(row - 1);
    if (t !== this.st.frame) this.act.frame(t);
  }

  private field(label: string, step: number, get: () => number | undefined, set: (v: number | undefined) => void, optional = false) {
    const inp = h('input', { type: 'number', step: String(step), placeholder: optional ? 'default' : '' });
    inp.dataset.get = '1';
    (inp as HTMLInputElement & { sync?: () => void }).sync = () => { if (document.activeElement !== inp) inp.value = fmt(get()); };
    (inp as HTMLInputElement & { sync?: () => void }).sync!();
    inp.onfocus = () => this.act.begin();
    inp.oninput = () => {
      const v = inp.value.trim() === '' ? undefined : Number(inp.value);
      if (v !== undefined && !Number.isFinite(v)) return;
      if (v === undefined && !optional) return;
      set(v);
    };
    inp.onkeydown = e => { if (e.key === 'Enter' || e.key === 'Escape') inp.blur(); };
    return h('label', {}, label, inp);
  }

  private buildFields() {
    const st = this.st, act = this.act;
    this.moveFields.replaceChildren(
      this.field('End lag', 1, () => this.st.mv.endLag, v => act.edit(m => { m.endLag = Math.max(0, Math.round(v!)); })),
      ...(st.mv.aerial ? [this.field('Landing lag', 1, () => this.st.mv.landingLag, v => act.edit(m => { m.landingLag = Math.max(0, Math.round(v!)); }))] : []),
    );
    const b = st.mv.hitboxes[st.sel];
    if (!b) { this.boxFields.replaceChildren(h('div', { className: 'sum', textContent: 'Click a hitbox (in the view or list) to edit it.' })); return; }
    const box = () => this.st.mv.hitboxes[this.st.sel];
    const setB = (k: keyof Hitbox, int: boolean) => (v: number | undefined) => act.edit((_, bb) => {
      if (!bb) return;
      const r = bb as unknown as Record<string, unknown>;
      if (v === undefined) delete r[k]; else r[k] = int ? Math.round(v) : v;
      if (k === 'start' && bb.end < bb.start) bb.end = bb.start;
      if (k === 'end' && bb.end < bb.start) bb.start = bb.end;
      if (bb.start < 1) bb.start = 1;
    });
    const dir = h('select', {}, ...[['', 'forward'], ['away', 'away from attacker'], ['back', 'backward']].map(([v, t]) => h('option', { value: v, textContent: t, selected: (b.dir ?? '') === v })));
    dir.onfocus = () => act.begin();
    dir.onchange = () => { act.edit((_, bb) => { if (!bb) return; if (dir.value) bb.dir = dir.value as Hitbox['dir']; else delete bb.dir; }); dir.blur(); };
    const grab = h('input', { type: 'checkbox', checked: !!b.grab });
    grab.onchange = () => { act.begin(); act.edit((_, bb) => { if (!bb) return; if (grab.checked) bb.grab = true; else delete bb.grab; }); grab.blur(); };
    const ints = new Set<keyof Hitbox>(['start', 'end', 'x', 'y', 'r', 'angle', 'hitstunAdd']);
    this.boxFields.replaceChildren(
      h('h2', {}, `Hitbox #${st.sel + 1}`),
      h('div', { className: 'grid' }, ...NUM_FIELDS.map(([k, l, s]) => this.field(l, s, () => box()?.[k] as number, setB(k, ints.has(k))))),
      h('div', { className: 'row' }, 'Launch', dir, h('label', {}, grab, ' grab box')),
      h('h2', {}, 'Optional (blank = default)'),
      h('div', { className: 'grid' }, ...OPT_FIELDS.map(([k, l, s]) => this.field(l, s, () => box()?.[k] as number | undefined, setB(k, ints.has(k)), true))),
    );
  }

  private syncFields() {
    this.root.querySelectorAll<HTMLInputElement & { sync?: () => void }>('input[data-get]').forEach(i => i.sync?.());
  }
}

const num = (n: number) => String(Math.round(n * 100) / 100);
const fmt = (v: number | undefined) => (v === undefined ? '' : String(Math.round(v * 10000) / 10000));

function helpLines(): (Node | string)[] {
  const k = (s: string) => h('kbd', { textContent: s });
  return [
    'Drag a circle to move it · drag its ', h('b', { textContent: '○' }), ' edge handle to resize · drag the ', h('b', { textContent: '●' }),
    ' arrow tip to set the launch angle (Shift snaps 15°).', h('br'),
    k('←'), k('→'), ' frame  ', k('↑'), k('↓'), ' move  ', k('F'), ' fighter  ', k('Tab'), ' next box  ', k('N'), ' add  ', k('Del'), ' delete  ',
    k('Space'), ' play  ', k('M'), ' speed  ', k('Wheel'), ' zoom', h('br'),
    'Edits apply live (Training uses them right away). Save writes them into src/data/fighters/*.ts.',
  ];
}
