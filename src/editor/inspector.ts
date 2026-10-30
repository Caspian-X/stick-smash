// Right-hand inspector: move frame data, hitbox list, and every field of the selected hitbox.
// Sections are rebuilt only when their structure changes. Otherwise registered sync functions refresh values, so
// typing into a field or scrubbing a label is never interrupted.

import { frameSummary } from '../data';
import { type Hitbox, SPECIAL_DIR } from '../data/types';
import { G } from '../sim/core';
import { hitlagOf, hitstunOf, knockback, launchAngle, shieldStunOf } from '../sim/combat';
import { HIT_TYPES, HIT_TYPE_ORDER, typeOf } from '../render/hitTypes';
import { dragFrom, h, round } from './dom';
import type { EditorModel } from './model';

interface NumOpts {
  get: () => number | undefined;
  set: (v: number | undefined) => void;
  step: number;
  min?: number;
  max?: number;
  int?: boolean;
  optional?: boolean;
  title?: string;
  disabled?: () => boolean;
}

const fmt = (v: number | undefined) => (v === undefined ? '' : String(+v.toFixed(4)));

export class Inspector {
  el = h('div', { className: 'right pane' });
  private syncs: (() => void)[] = [];
  private key = '';

  constructor(private m: EditorModel) {}

  render() {
    const m = this.m, b = m.box(), mv = m.mv();
    const key = [m.fi, m.id, m.sel, mv.hitboxes.length, b && typeOf(b), b && m.perFrame(b), b?.rehit !== undefined, b?.setKb, !!mv.aerial].join('|');
    if (key !== this.key) { this.key = key; this.build(); }
    for (const s of this.syncs) s();
  }

  /** Register a function that refreshes some live content on every change. */
  private live(fn: () => void) { this.syncs.push(fn); fn(); }

  private build() {
    const m = this.m, mv = m.mv(), b = m.box();
    this.syncs = [];
    const keep = this.el.scrollTop;
    this.el.replaceChildren(this.header(), this.list(), b ? this.boxEditor(b) : this.emptyBox());
    this.el.scrollTop = keep;
    void mv;
  }

  // ---------- move header ----------
  private header() {
    const m = this.m, mv = m.mv();
    const stat = (label: string, val: () => string) => {
      const b = h('b');
      this.live(() => { b.textContent = val(); });
      return h('div', { className: 'stat' }, b, h('span', { textContent: label }));
    };
    const editStat = (label: string, o: NumOpts) => h('div', { className: 'stat', title: 'Drag the label or type a value' }, this.num('', o).querySelector('input')!, h('span', { textContent: label }));
    const startup = () => { const bx = m.mv().hitboxes; return bx.length ? Math.min(...bx.map(x => x.start)) : 0; };
    const lastEnd = () => Math.max(0, ...m.mv().hitboxes.map(x => x.end));
    const summary = h('div', { className: 'note' });
    this.live(() => { summary.textContent = frameSummary(m.mv()) + (m.mv().charge ? ` · charges on frame ${m.mv().charge!.frame}` : ''); });
    return h('div', {},
      h('h2', {}, mv.name ?? m.id, h('small', { textContent: `${m.fighter().name} · ${SPECIAL_DIR[m.id] ? `${SPECIAL_DIR[m.id]} special` : m.id}` })),
      h('div', { className: 'stats' },
        stat('Startup', () => String(startup() || '–')),
        stat('Active', () => (startup() ? `${startup()}–${lastEnd()}` : '–')),
        editStat('End lag', { get: () => m.mv().endLag, set: v => m.edit(x => { x.endLag = Math.max(0, Math.round(v ?? 0)); }), step: 1, min: 0, int: true }),
        stat('Total', () => String(m.total())),
      ),
      mv.aerial ? h('div', { className: 'grid2', style: 'margin-top:8px' },
        this.num('Landing lag', { get: () => m.mv().landingLag, set: v => m.edit(x => { x.landingLag = Math.max(0, Math.round(v ?? 0)); }), step: 1, min: 0, int: true, title: 'Frames of lag if this aerial lands mid-move' })) : null,
      summary,
    );
  }

  // ---------- hitbox list ----------
  private list() {
    const m = this.m, wrap = h('div', { className: 'boxes', onmouseleave: () => m.setHover(-1) });
    let key = '';
    this.live(() => {
      const bx = m.mv().hitboxes, k = JSON.stringify([m.sel, m.hover, bx]);
      if (k === key) return; // unchanged: keep the rows so clicks during playback aren't lost
      key = k;
      wrap.replaceChildren(...bx.map((b, i) => {
        const ty = HIT_TYPES[typeOf(b)];
        const ord = h('span', { className: 'ord' },
          h('button', { textContent: '▲', title: 'Higher priority', disabled: i === 0, onclick: (e: MouseEvent) => { e.stopPropagation(); m.reorder(i, i - 1); } }),
          h('button', { textContent: '▼', title: 'Lower priority', disabled: i === bx.length - 1, onclick: (e: MouseEvent) => { e.stopPropagation(); m.reorder(i, i + 1); } }));
        const meta = ty.label === 'Grab' ? `f${b.start}–${b.end}` : ty.label === 'Windbox' ? `f${b.start}–${b.end} · push ${b.bkb}` : `f${b.start}–${b.end} · ${+b.damage.toFixed(2)}% · ${b.angle}°`;
        return h('div', {
          className: `brow${i === m.sel ? ' sel' : ''}${i === m.hover ? ' hov' : ''}`, onclick: () => m.select(i), onmouseenter: () => m.setHover(i),
          title: 'Click to select. Earlier boxes win when several touch the victim.',
        }, h('span', { className: 'ix', textContent: String(i + 1) }), h('span', { className: 'sw', style: { background: ty.css } }),
        h('span', { className: 'nm' }, ty.label, m.perFrame(b) ? h('span', { className: 'mt', textContent: ' · per-frame' }) : null), h('span', { className: 'mt', textContent: meta }), ord);
      }));
      if (!bx.length) wrap.append(h('div', { className: 'empty', textContent: 'This move has no hitboxes yet.' }));
    });
    return h('div', { className: 'sec' },
      h('div', { className: 'st' }, 'Hitboxes', h('span', { className: 'grow' }),
        h('button', { className: 'icon', textContent: '+', title: 'Add hitbox (N, or double-click the view)', onclick: () => m.add() }),
        h('button', { className: 'icon', textContent: '⧉', title: 'Duplicate (D)', disabled: !m.box(), onclick: () => m.duplicate() }),
        h('button', { className: 'icon', textContent: '🗑', title: 'Delete (Del)', disabled: !m.box(), onclick: () => m.remove() })),
      wrap,
      h('div', { className: 'note', textContent: 'Top of the list = highest priority when boxes overlap the victim.' }),
    );
  }

  private emptyBox() {
    return h('div', { className: 'sec' }, h('div', { className: 'empty' }, 'Select a hitbox in the view or the list,', h('br'), 'or double-click the view to add one.'));
  }

  // ---------- selected hitbox ----------
  private boxEditor(b: Hitbox) {
    const m = this.m, ty = typeOf(b), info = HIT_TYPES[ty], box = () => m.box()!;
    const sec = (title: string, ...kids: (Node | null)[]) => h('div', { className: 'sec' }, h('div', { className: 'st' }, title), ...kids);
    const field = <K extends keyof Hitbox>(k: K, label: string, step: number, extra: Partial<NumOpts> = {}) => this.num(label, {
      get: () => box()[k] as number | undefined,
      set: v => m.edit((_, x) => { if (!x) return; const r = x as unknown as Record<string, unknown>; if (v === undefined) delete r[k]; else r[k] = v; }),
      step, ...extra,
    });

    // type picker
    const types = h('div', { className: 'types' }, ...HIT_TYPE_ORDER.map(t => {
      const i = HIT_TYPES[t], sel = t === ty;
      return h('button', {
        className: sel ? 'sel' : '', title: i.desc, onclick: () => m.setType(t),
        style: sel ? { borderColor: i.css, background: i.css + '26' } : {},
      }, h('span', { className: 'ic', textContent: i.icon, style: { color: i.css } }), i.label);
    }));

    // timing
    const timing = h('div', { className: 'grid2' },
      this.num('Start frame', { get: () => box().start, set: v => m.setWindow(m.sel, v ?? 1, Math.max(v ?? 1, box().end)), step: 1, min: 1, int: true, title: 'First active frame' }),
      this.num('End frame', { get: () => box().end, set: v => m.setWindow(m.sel, Math.min(box().start, v ?? 1), v ?? 1), step: 1, min: 1, int: true, title: 'Last active frame (inclusive)' }));
    const multi = h('input', { type: 'checkbox', checked: b.rehit !== undefined, onchange: (e: Event) => m.op((_, x) => { if (!x) return; if ((e.target as HTMLInputElement).checked) x.rehit = 4; else delete x.rehit; }) });
    const rehit = b.rehit !== undefined ? field('rehit', 'Every N frames', 1, { min: 1, int: true, title: 'Frames between repeat hits on the same victim' }) : null;

    // shape
    const pf = m.perFrame(b);
    const g = (k: 'x' | 'y' | 'r', label: string, min?: number) => this.num(label, {
      get: () => m.geom(box(), m.editFrame(box()))[k], set: v => m.setGeom({ [k]: v ?? 0 }), step: 1, int: true, min,
    });
    const mode = h('div', { className: 'seg2' },
      h('button', { className: pf ? '' : 'sel', textContent: 'Same every frame', onclick: () => pf && m.setPerFrame(false), title: 'One position and size for the whole active window' }),
      h('button', { className: pf ? 'sel' : '', textContent: 'Per frame', onclick: () => !pf && m.setPerFrame(true), title: 'Give each active frame its own position and size (P)' }));
    const pfNote = h('div', { className: 'note' });
    const nextBtn = h('button', { textContent: 'Copy → next frame', title: 'Copy this frame\'s position to the next frame and step there', onclick: () => m.copyToNext() });
    this.live(() => {
      const x = m.box();
      if (!x) return;
      const t = m.editFrame(x);
      pfNote.replaceChildren(...(m.perFrame(x)
        ? [`Editing `, h('b', { textContent: `frame ${t}` }), ` of ${x.start}–${x.end}. `, m.frame !== t ? `(playhead is outside this box, showing its nearest frame) ` : '', 'Diamonds in the timeline mark each frame; amber ones differ from the frame before.']
        : ['Same position on every active frame. Switch to ', h('b', { textContent: 'Per frame' }), ' to animate it.']));
      nextBtn.disabled = t >= x.end;
    });
    const shape = h('div', {}, mode, h('div', { className: 'grid3', style: 'margin-top:8px' }, g('x', 'X (forward)'), g('y', 'Y (up)'), g('r', 'Radius', 1)), pfNote,
      pf ? h('div', { className: 'row' },
        h('button', { textContent: 'Apply to all frames', title: 'Set every frame to this frame\'s position and size', onclick: () => m.applyToAll() }), nextBtn) : null);

    // launch
    const angleOff = ty === 'sakurai' || ty === 'autolink';
    const launch = ty === 'grab' ? null : sec(ty === 'windbox' ? 'Push direction' : 'Launch',
      h('div', { className: 'launch' }, this.dial(angleOff),
        h('div', {},
          this.num('Angle °', { get: () => box().angle, set: v => m.edit((_, x) => { if (x) x.angle = Math.round(v ?? 0); }), step: 1, int: true, min: -180, max: 180, disabled: () => angleOff,
            title: '0 = forward, 90 = up, negative = down' }),
          h('div', { className: 'note', textContent: 'Direction' }),
          this.seg([['', 'Forward'], ['away', 'Away'], ['back', 'Back']], () => box().dir ?? '', v => m.op((_, x) => { if (!x) return; if (v) x.dir = v as Hitbox['dir']; else delete x.dir; }), ty === 'autolink',
            'Forward: the way the attacker faces · Away: away from the attacker\'s center (boxes on both sides) · Back: behind the attacker'))),
      angleOff ? h('div', { className: 'note', textContent: ty === 'sakurai' ? 'Angle is automatic for this type (global.ts → hitTypes.sakurai).' : 'Autolink ignores the angle: it pulls toward the box center.' }) : null,
      ty === 'spike' ? h('div', { className: 'note', textContent: 'Spikes always go downward: a positive angle is mirrored below the horizon.' }) : null);

    // power
    const fixed = h('input', { type: 'checkbox', checked: !!b.setKb, onchange: (e: Event) => m.op((_, x) => { if (!x) return; if ((e.target as HTMLInputElement).checked) x.setKb = true; else delete x.setKb; }) });
    const power = ty === 'grab' ? null : sec('Power',
      h('div', { className: 'grid3' },
        ty === 'windbox' ? null : field('damage', 'Damage %', 0.5, { min: 0 }),
        field('bkb', ty === 'windbox' ? 'Push strength' : 'Base KB', 0.5, { min: 0, title: ty === 'windbox' ? 'Push per frame (× hitTypes.windbox.push)' : 'Knockback at 0%' }),
        ty === 'windbox' ? null : field('kbg', 'KB growth', 0.005, { min: 0, disabled: () => !!m.box()?.setKb, title: 'Extra knockback per victim percent' })),
      ty === 'windbox' ? null : h('label', { className: 'chk', style: 'margin-top:8px', title: 'Knockback = base KB at any percent (jabs, multi-hit links)' }, fixed, 'Fixed knockback (ignores the victim\'s %)'),
      ty === 'windbox' ? null : this.preview());

    const adv = ty === 'grab' || ty === 'windbox' ? null : h('div', { className: 'sec' },
      h('details', {}, h('summary', { textContent: 'Advanced multipliers' }),
        h('div', { className: 'grid3' },
          field('hitstunMult', 'Hitstun ×', 0.1, { optional: true, min: 0 }), field('hitstunAdd', 'Hitstun +', 1, { optional: true, int: true }),
          field('hitlagMult', 'Hitlag ×', 0.1, { optional: true, min: 0 }), field('shieldDmgMult', 'Shield dmg ×', 0.1, { optional: true, min: 0 }),
          field('shieldStunMult', 'Shield stun ×', 0.1, { optional: true, min: 0 })),
        h('div', { className: 'note', textContent: 'Blank = default (1×, +0).' })));

    const desc = h('div', { className: 'desc', textContent: info.desc });
    return h('div', {},
      h('div', { className: 'sec' }, h('div', { className: 'st' }, `Hitbox ${m.sel + 1}`, h('span', { className: 'grow' }),
        h('span', { style: { color: info.css, textTransform: 'none', letterSpacing: '0', fontSize: '11px' }, textContent: `${info.icon} ${info.label}` })), types, desc),
      sec('Timing', timing, h('label', { className: 'chk', style: 'margin-top:8px', title: 'Let this box hit the same victim more than once' }, multi, 'Multi-hit'), rehit ? h('div', { className: 'grid2', style: 'margin-top:6px' }, rehit) : null),
      sec('Position & size', shape),
      launch, power, adv,
    );
  }

  // ---------- widgets ----------
  private num(label: string, o: NumOpts) {
    const m = this.m;
    const inp = h('input', { type: 'number', step: String(o.step), placeholder: o.optional ? 'default' : '', title: o.title ?? '' });
    const clampV = (v: number) => {
      if (o.min !== undefined) v = Math.max(o.min, v);
      if (o.max !== undefined) v = Math.min(o.max, v);
      return o.int ? Math.round(v) : round(v, o.step);
    };
    inp.onfocus = () => m.begin();
    inp.oninput = () => {
      const s = inp.value.trim();
      if (s === '') { if (o.optional) o.set(undefined); return; }
      const v = Number(s);
      if (Number.isFinite(v)) o.set(clampV(v));
    };
    inp.onblur = () => { inp.value = fmt(o.get()); };
    inp.onkeydown = e => { if (e.key === 'Enter' || e.key === 'Escape') inp.blur(); };
    const lab = h('label', { title: (o.title ? o.title + ' · ' : '') + 'Drag left/right to adjust (Shift = ×10)' }, label);
    lab.onmousedown = e => {
      if (o.disabled?.()) return;
      m.begin();
      const v0 = o.get() ?? (o.min ?? 0);
      dragFrom(e, (dx, _dy, ev) => o.set(clampV(v0 + Math.round(dx / 4) * o.step * (ev.shiftKey ? 10 : 1))), undefined, 'ew-resize');
    };
    const wrap = h('div', { className: 'fld' }, label ? lab : null, inp);
    this.live(() => {
      if (document.activeElement !== inp) inp.value = fmt(o.get());
      const dis = !!o.disabled?.();
      wrap.classList.toggle('dis', dis); inp.disabled = dis;
    });
    return wrap;
  }

  private seg(opts: [string, string][], get: () => string, set: (v: string) => void, disabled = false, title = '') {
    const wrap = h('div', { className: 'seg2', title, style: disabled ? 'opacity:.4;pointer-events:none' : '' });
    const btns = opts.map(([v, l]) => h('button', { textContent: l, onclick: () => set(v) }));
    wrap.append(...btns);
    this.live(() => btns.forEach((b, i) => b.classList.toggle('sel', opts[i][0] === get())));
    return wrap;
  }

  /** Angle dial: right = forward, up = 90°. Drag to aim (Shift snaps to 15°). */
  private dial(disabled: boolean) {
    const m = this.m, NS = 'http://www.w3.org/2000/svg';
    const svg = document.createElementNS(NS, 'svg');
    svg.setAttribute('viewBox', '-50 -50 100 100'); svg.setAttribute('class', 'dial' + (disabled ? ' dis' : ''));
    const el = (tag: string, a: Record<string, string | number>) => { const e = document.createElementNS(NS, tag); for (const k in a) e.setAttribute(k, String(a[k])); svg.append(e); return e; };
    el('circle', { r: 44, fill: '#0b0e22', stroke: '#262c55', 'stroke-width': 2 });
    for (let a = 0; a < 360; a += 45) { const r = a * Math.PI / 180; el('line', { x1: Math.cos(r) * 38, y1: -Math.sin(r) * 38, x2: Math.cos(r) * 44, y2: -Math.sin(r) * 44, stroke: '#3a4384', 'stroke-width': a % 90 ? 1 : 2 }); }
    el('text', { x: 30, y: 4, fill: '#5d6491', 'font-size': 9, 'text-anchor': 'middle' }).textContent = 'fwd';
    const line = el('line', { x1: 0, y1: 0, stroke: '#fff', 'stroke-width': 3, 'stroke-linecap': 'round' });
    const tip = el('circle', { r: 5, fill: '#ffd35a' });
    const label = el('text', { x: 0, y: 30, fill: '#e6e9ff', 'font-size': 11, 'font-weight': 700, 'text-anchor': 'middle' });
    el('circle', { r: 3, fill: '#fff' });
    this.live(() => {
      const b = m.box();
      if (!b) return;
      const ty = typeOf(b), deg = ty === 'sakurai' ? 45 : launchAngle(b, 0, false), r = deg * Math.PI / 180, x = Math.cos(r) * 34, y = -Math.sin(r) * 34;
      line.setAttribute('x2', String(x)); line.setAttribute('y2', String(y));
      line.setAttribute('stroke', HIT_TYPES[ty].css);
      tip.setAttribute('cx', String(x)); tip.setAttribute('cy', String(y));
      label.textContent = ty === 'sakurai' ? 'auto' : ty === 'autolink' ? 'pull' : `${deg}°`;
    });
    if (!disabled) svg.addEventListener('mousedown', e => {
      m.begin();
      const set = (ev: MouseEvent) => {
        const rc = svg.getBoundingClientRect();
        let a = Math.atan2(-(ev.clientY - (rc.top + rc.height / 2)), ev.clientX - (rc.left + rc.width / 2)) * 180 / Math.PI;
        a = ev.shiftKey ? Math.round(a / 15) * 15 : Math.round(a);
        if (a === -180) a = 180;
        m.edit((_, x) => { if (x) x.angle = a; });
      };
      set(e);
      dragFrom(e, (_dx, _dy, ev) => set(ev), undefined, 'crosshair');
    });
    return svg;
  }

  /** What this box does at different victim percents (weight 1.0), plus its on-shield frame advantage. */
  private preview() {
    const m = this.m, table = h('table', { className: 'kb' }), extra = h('div', { className: 'note' });
    this.live(() => {
      const b = m.box();
      if (!b) return;
      const ty = typeOf(b), tumble = G().knockback.tumbleThreshold, pcts = [0, 50, 100, 150];
      const kbs = pcts.map(p => knockback(b, p + b.damage, 1));
      const row = (name: string, cells: (string | Node)[]) => h('tr', {}, h('td', { textContent: name }), ...cells.map(c => h('td', {}, c)));
      table.replaceChildren(
        h('tr', {}, h('th', { textContent: 'Victim %' }), ...pcts.map(p => h('th', { textContent: `${p}%` }))),
        row('Knockback', kbs.map(k => k.toFixed(1))),
        row('Hitstun', kbs.map(k => (ty === 'trip' ? 'prone*' : `${hitstunOf(b, k)}f`))),
        row('Tumble', kbs.map(k => (ty !== 'autolink' && k > tumble ? h('span', { className: 'good', textContent: '✓' }) : '·'))),
        ...(ty === 'sakurai' ? [row('Angle (ground)', kbs.map(k => `${launchAngle(b, k, true)}°`))] : []),
      );
      const ss = shieldStunOf(b), adv = ss - (m.total() - b.start), hl = hitlagOf(b, kbs[0]);
      extra.replaceChildren(
        'On shield (hitting on frame ', h('b', { textContent: String(b.start) }), '): ', h('b', { className: adv >= 0 ? 'good' : 'bad', textContent: (adv > 0 ? '+' : '') + adv }),
        ` · shield stun ${ss}f · hitlag ${hl}f${ty === 'electric' ? ` (victim ×${G().hitTypes.electric.victimHitlagMult})` : ''}`,
        ty === 'trip' ? h('div', { textContent: '* Grounded victims are knocked prone. Airborne victims take the hitstun shown by the formula.' }) : '',
        ty === 'autolink' ? h('div', { textContent: 'Autolink: launch direction comes from the pull, not the angle.' }) : '',
      );
    });
    return h('div', { style: 'margin-top:10px' }, h('div', { className: 'note', textContent: 'Preview vs a weight 1.0 fighter:' }), table, extra);
  }
}
