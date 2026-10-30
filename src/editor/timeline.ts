// Bottom timeline: transport controls, frame ruler, startup/active/end-lag band, and one draggable bar per hitbox.

import { HIT_TYPES, typeOf } from '../render/hitTypes';
import { dragFrom, h } from './dom';
import { type EditorModel, SPEEDS } from './model';

const LABEL_W = 132;

export class Timeline {
  el = h('div', { className: 'time pane' });
  private frameLbl = h('span', { className: 'fr' });
  private playBtn = h('button', { className: 'icon', title: 'Play / pause (Space)' });
  private speed = h('select', { title: 'Playback speed' });
  private grid = h('div', { className: 'tl' });

  constructor(private m: EditorModel) {
    const b = (t: string, title: string, fn: () => void) => h('button', { className: 'icon', textContent: t, title, onclick: fn });
    this.speed.append(...SPEEDS.map((s, i) => h('option', { value: String(i), textContent: `${s}×` })));
    this.speed.onchange = () => { m.speedI = +this.speed.value; this.speed.blur(); m.emit(); };
    this.playBtn.onclick = () => { m.playing = !m.playing; m.emit(); };
    const legend = (cls: string, t: string) => h('span', { style: 'display:inline-flex;align-items:center;gap:4px;color:var(--dim);white-space:nowrap' },
      h('span', { style: `width:10px;height:10px;border-radius:2px;background:var(--${cls})` }), t);
    this.el.append(
      h('div', { className: 'transport' },
        b('⏮', 'First frame (Home)', () => m.setFrame(1)),
        b('⇤', 'Previous change (Shift+←)', () => m.jump(-1)),
        b('◀', 'Previous frame (←)', () => m.setFrame(m.frame - 1, true)),
        this.playBtn,
        b('▶', 'Next frame (→)', () => m.setFrame(m.frame + 1, true)),
        b('⇥', 'Next change (Shift+→)', () => m.jump(1)),
        b('⏭', 'Last frame (End)', () => m.setFrame(m.total())),
        this.frameLbl, this.speed,
        h('span', { className: 'spacer', style: 'flex:1' }),
        legend('startup', 'Startup'), legend('active', 'Active'), legend('endlag', 'End lag')),
      this.grid,
    );
  }

  render() {
    const m = this.m, mv = m.mv(), N = m.timelineTotal(), bx = mv.hitboxes, pct = (f: number) => `${(f / N) * 100}%`;
    this.frameLbl.textContent = `${m.frame} / ${m.total()}`;
    this.playBtn.textContent = m.playing ? '❚❚' : '▶';
    this.playBtn.classList.toggle('on', m.playing);
    this.speed.value = String(m.speedI);

    const rows: Node[] = [];
    // ruler
    const ruler = h('div', { className: 'trk ruler', title: 'Click or drag to scrub' });
    const every = N > 60 ? 10 : 5;
    for (let f = 1; f <= N; f++) {
      ruler.append(h('div', { className: 'tick', style: { left: pct(f - 1), borderLeftColor: f % every === 0 || f === 1 ? '#ffffff30' : '#ffffff10' } },
        f % every === 0 || f === 1 ? h('span', { textContent: String(f) }) : null));
    }
    ruler.onmousedown = e => this.scrub(e, ruler);
    rows.push(h('div', {}), ruler);

    // phase band
    const s0 = bx.length ? Math.min(...bx.map(b => b.start)) : 1, e0 = bx.length ? Math.max(...bx.map(b => b.end)) : 0, tot = m.total();
    const band = h('div', { className: 'trk', title: 'Whole move: startup, active frames, end lag' });
    const seg = (cls: string, a: number, b: number, text: string) => { if (b >= a) band.append(h('div', { className: `seg ${cls}`, style: { left: pct(a - 1), width: pct(b - a + 1) }, textContent: text, title: text })); };
    seg('startup', 1, s0 - 1, s0 > 2 ? `Startup 1–${s0 - 1}` : 'Startup');
    seg('active', s0, e0, `Active ${s0}–${e0}`);
    seg('endlag', e0 + 1, tot, `End lag ${e0 + 1}–${tot} (${mv.endLag}f)`);
    const lagH = h('div', { className: 'lagh', style: { left: pct(tot) }, title: 'Drag to change end lag' });
    lagH.onmousedown = e => {
      e.stopPropagation();
      const fpx = band.clientWidth / N, l0 = mv.endLag;
      m.begin(); m.lockTotal = N;
      dragFrom(e, dx => m.edit(x => { x.endLag = Math.max(0, l0 + Math.round(dx / fpx)); }), () => { m.lockTotal = 0; m.emit(); }, 'ew-resize');
    };
    band.append(lagH);
    band.onmousedown = e => this.scrub(e, band);
    rows.push(h('div', { className: 'lbl', textContent: 'Move' }), band);

    // hitbox tracks
    bx.forEach((b, i) => {
      const info = HIT_TYPES[typeOf(b)], sel = i === m.sel;
      const lbl = h('div', { className: `lbl${sel ? ' sel' : ''}`, onclick: () => m.select(i), onmouseenter: () => m.setHover(i), onmouseleave: () => m.setHover(-1) },
        h('span', { className: 'sw', style: { width: '9px', height: '9px', borderRadius: '50%', background: info.css, flex: 'none' } }),
        `${i + 1} · ${info.label}`);
      const trk = h('div', { className: `trk${sel ? ' sel' : ''}`, onmouseenter: () => m.setHover(i), onmouseleave: () => m.setHover(-1) });
      const bar = h('div', {
        className: `bar${sel ? ' sel' : ''}`, title: `Frames ${b.start}–${b.end} (${b.end - b.start + 1} active). Drag to move, drag the edges to resize.`,
        style: { left: pct(b.start - 1), width: pct(b.end - b.start + 1), background: info.css + (sel ? 'ee' : 'aa') },
      }, h('div', { className: 'hd l' }), h('div', { className: 'hd r' }));
      if (b.frames?.length) {
        let prev: number[] | null = null;
        for (const e of b.frames) {
          const moved = !!prev && (prev[1] !== e[1] || prev[2] !== e[2] || prev[3] !== e[3]);
          trk.append(h('div', { className: `key${moved ? ' moved' : ''}`, style: { left: pct(e[0] - 0.5) } }));
          prev = e;
        }
      }
      bar.onmousedown = e => this.dragBar(e, i, trk, (e.target as HTMLElement).classList.contains('l') ? 'l' : (e.target as HTMLElement).classList.contains('r') ? 'r' : 'move');
      trk.onmousedown = e => { if (e.target === trk) { m.select(i); this.scrub(e, trk); } };
      trk.prepend(bar);
      rows.push(lbl, trk);
    });

    const head = h('div', { className: 'playhead', style: { left: `calc(${LABEL_W}px + (100% - ${LABEL_W}px) * ${(m.frame - 0.5) / N})` } });
    this.grid.replaceChildren(...rows, head);
    this.grid.style.gridTemplateColumns = `${LABEL_W}px 1fr`;
  }

  private scrub(e: MouseEvent, el: HTMLElement) {
    const m = this.m, N = m.timelineTotal();
    const at = (x: number) => { const r = el.getBoundingClientRect(); m.setFrame(Math.floor(((x - r.left) / r.width) * N) + 1); };
    m.playing = false;
    at(e.clientX);
    dragFrom(e, (_dx, _dy, ev) => at(ev.clientX), undefined, 'ew-resize');
  }

  private dragBar(e: MouseEvent, i: number, trk: HTMLElement, mode: 'l' | 'r' | 'move') {
    e.stopPropagation();
    const m = this.m, b = m.mv().hitboxes[i], N = m.timelineTotal(), fpx = trk.clientWidth / N, s0 = b.start, e0 = b.end;
    const r = trk.getBoundingClientRect(), clicked = Math.floor(((e.clientX - r.left) / r.width) * N) + 1;
    m.begin(); m.lockTotal = N;
    if (m.sel !== i) { m.sel = i; m.emit(); }
    dragFrom(e, dx => {
      const d = Math.round(dx / fpx);
      if (mode === 'l') m.setWindow(i, Math.min(s0 + d, e0), e0);
      else if (mode === 'r') m.setWindow(i, s0, Math.max(s0, e0 + d));
      else { const dd = Math.max(1 - s0, d); m.setWindow(i, s0 + dd, e0 + dd); }
    }, moved => {
      m.lockTotal = 0;
      if (!moved) m.setFrame(clicked); else m.emit();
    }, mode === 'move' ? 'grabbing' : 'ew-resize');
  }
}
