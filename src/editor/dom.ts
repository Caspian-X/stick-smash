// Tiny DOM helpers and the stylesheet for the hitbox editor UI.

type Kid = Node | string | null | undefined | false;

/** Create an element: props are assigned (className, onclick, title…), `style` may be an object. */
export function h<K extends keyof HTMLElementTagNameMap>(tag: K, props: Record<string, unknown> = {}, ...kids: Kid[]): HTMLElementTagNameMap[K] {
  const { style, dataset, ...rest } = props as { style?: Partial<CSSStyleDeclaration> | string; dataset?: Record<string, string> };
  const e = Object.assign(document.createElement(tag), rest) as HTMLElementTagNameMap[K];
  if (typeof style === 'string') e.setAttribute('style', style); else if (style) Object.assign(e.style, style);
  if (dataset) Object.assign(e.dataset, dataset);
  for (const k of kids) if (k !== null && k !== undefined && k !== false) e.append(k);
  return e;
}

/** Track a mouse drag from a mousedown, with document-level move/up listeners. */
export function dragFrom(e: MouseEvent, move: (dx: number, dy: number, ev: MouseEvent) => void, up?: (moved: boolean) => void, cursor = 'grabbing') {
  const x0 = e.clientX, y0 = e.clientY;
  let moved = false;
  const mm = (ev: MouseEvent) => { if (Math.abs(ev.clientX - x0) + Math.abs(ev.clientY - y0) > 2) moved = true; move(ev.clientX - x0, ev.clientY - y0, ev); };
  const mu = () => { removeEventListener('mousemove', mm); removeEventListener('mouseup', mu); document.body.classList.remove('hbe-dragging'); document.body.style.cursor = ''; up?.(moved); };
  addEventListener('mousemove', mm);
  addEventListener('mouseup', mu);
  document.body.classList.add('hbe-dragging');
  document.body.style.cursor = cursor;
  e.preventDefault();
}

export const round = (v: number, step: number) => {
  const d = Math.max(0, -Math.floor(Math.log10(step)) + 1);
  return +(Math.round(v / step) * step).toFixed(Math.min(6, d));
};

export const CSS = `
:root { --bg: #0b0e22; --panel: #11152e; --panel2: #171c3b; --line: #262c55; --text: #e6e9ff; --dim: #8d94c4; --faint: #5d6491;
  --accent: #ffd35a; --ok: #43d17a; --err: #ff5470; --startup: #4b5276; --active: #ff3355; --endlag: #3f6fd1; }
#hbe { position: fixed; inset: 0; display: grid; z-index: 10; pointer-events: none; color: var(--text);
  grid-template: "top top top" 50px "left view right" 1fr "left time right" var(--th, auto) / var(--lw, 224px) 1fr var(--rw, 372px);
  font: 12px/1.4 system-ui, -apple-system, "Segoe UI", sans-serif; }
#hbe * { box-sizing: border-box; }
#hbe .pane { pointer-events: auto; background: var(--panel); }
#hbe button { font: inherit; color: var(--text); background: var(--panel2); border: 1px solid var(--line); border-radius: 6px;
  padding: 5px 10px; cursor: pointer; display: inline-flex; align-items: center; gap: 6px; white-space: nowrap; transition: background .12s, border-color .12s; }
#hbe button:hover:not(:disabled) { background: #232a55; border-color: #3a4384; }
#hbe button:disabled { opacity: .4; cursor: default; }
#hbe button.primary { background: #2a7a46; border-color: #3aa962; font-weight: 600; }
#hbe button.primary:hover:not(:disabled) { background: #31914f; }
#hbe button.ghost { background: transparent; border-color: transparent; }
#hbe button.ghost:hover:not(:disabled) { background: var(--panel2); }
#hbe button.icon { padding: 4px 7px; min-width: 28px; justify-content: center; }
#hbe button.on { border-color: var(--accent); color: var(--accent); }
#hbe kbd { font: 10px ui-monospace, monospace; background: #0003; border: 1px solid var(--line); border-bottom-width: 2px; border-radius: 4px; padding: 0 4px; color: var(--dim); }
#hbe input[type=number], #hbe input[type=text] { font: inherit; color: var(--text); background: var(--bg); border: 1px solid var(--line);
  border-radius: 5px; padding: 4px 6px; width: 100%; outline: none; }
#hbe input[type=number]:focus { border-color: var(--accent); }
#hbe input::-webkit-inner-spin-button { opacity: .35; }
#hbe input::placeholder { color: var(--faint); }

/* top bar */
#hbe .top { grid-area: top; display: flex; align-items: center; gap: 10px; padding: 0 12px; border-bottom: 1px solid var(--line); background: #0e1230; }
#hbe .brand { font-weight: 800; letter-spacing: .4px; font-size: 14px; display: flex; align-items: center; gap: 8px; margin-right: 6px; }
#hbe .brand i { width: 14px; height: 14px; border-radius: 50%; background: radial-gradient(circle at 35% 35%, #ff8aa0, #ff3355); box-shadow: 0 0 10px #ff335599; }
#hbe .tabs { display: flex; background: var(--bg); border: 1px solid var(--line); border-radius: 8px; padding: 2px; gap: 2px; }
#hbe .tabs button { border: 0; background: transparent; border-radius: 6px; padding: 5px 12px; color: var(--dim); }
#hbe .tabs button.sel { background: var(--panel2); color: var(--text); box-shadow: inset 0 0 0 1px #3a4384; }
#hbe .spacer { flex: 1; }
#hbe .badge { color: var(--accent); font-weight: 600; display: inline-flex; align-items: center; gap: 6px; }
#hbe .badge::before { content: ''; width: 7px; height: 7px; border-radius: 50%; background: var(--accent); }
#hbe .clean { color: var(--dim); }

/* move list */
#hbe .left { grid-area: left; border-right: 1px solid var(--line); overflow-y: auto; padding: 8px 6px 16px; }
#hbe .grp { font-size: 10px; text-transform: uppercase; letter-spacing: 1px; color: var(--faint); margin: 12px 8px 4px; font-weight: 700; }
#hbe .mv { display: flex; align-items: center; gap: 6px; padding: 6px 8px; border-radius: 6px; cursor: pointer; color: var(--dim); }
#hbe .mv:hover { background: var(--panel2); color: var(--text); }
#hbe .mv.sel { background: #272d5c; color: #fff; box-shadow: inset 2px 0 0 var(--accent); }
#hbe .mv.off { opacity: .4; cursor: default; }
#hbe .mv .sdir { font-size: 9.5px; font-weight: 700; text-transform: uppercase; letter-spacing: .5px; color: #ffb35a; background: #ffb35a1c; border-radius: 4px; padding: 1px 4px; flex: none; min-width: 46px; text-align: center; }
#hbe .mv .nm { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
#hbe .mv .meta { font: 10px ui-monospace, monospace; color: var(--faint); }
#hbe .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); flex: none; }

/* viewport */
#hbe .view { grid-area: view; position: relative; pointer-events: none; }
#hbe .view > * { pointer-events: auto; }
#hbe .hud { position: absolute; left: 12px; top: 10px; display: flex; gap: 8px; align-items: center; pointer-events: none; }
#hbe .chip { background: #0b0e22cc; border: 1px solid var(--line); border-radius: 999px; padding: 3px 10px; font-weight: 600; backdrop-filter: blur(4px); }
#hbe .chip.ph-startup { color: #c2c8ee; } #hbe .chip.ph-active { color: #ff8095; border-color: #ff335588; } #hbe .chip.ph-endlag { color: #8fb0ff; }
#hbe .vtools { position: absolute; right: 10px; bottom: 10px; display: flex; gap: 4px; background: #0b0e22cc; border: 1px solid var(--line); border-radius: 8px; padding: 4px; backdrop-filter: blur(4px); }
#hbe .hint { position: absolute; left: 12px; bottom: 12px; max-width: calc(100% - 300px); color: var(--dim); background: #0b0e22b0; border-radius: 10px; padding: 4px 10px; pointer-events: none; }
#hbe .toast { position: absolute; left: 50%; bottom: 56px; transform: translateX(-50%); padding: 7px 14px; border-radius: 8px; font-weight: 600; max-width: 80%; text-align: center;
  background: #173a26; border: 1px solid #2f8a52; color: #bff5cf; box-shadow: 0 6px 20px #0008; transition: opacity .3s, transform .3s; pointer-events: none; }
#hbe .toast.err { background: #3d1622; border-color: #a33a52; color: #ffc4cf; }
#hbe .toast.info { background: #1b2148; border-color: #3a4384; color: #dfe3ff; }
#hbe .toast.hide { opacity: 0; transform: translate(-50%, 6px); }

/* timeline */
#hbe .time { grid-area: time; border-top: 1px solid var(--line); padding: 6px 10px 10px; user-select: none; max-height: var(--thmax, 42vh); overflow-y: auto; }
#hbe .transport { display: flex; align-items: center; gap: 6px; margin-bottom: 6px; }
#hbe .transport .fr { font: 600 12px ui-monospace, monospace; min-width: 92px; text-align: center; }
#hbe .transport select { font: inherit; color: var(--text); background: var(--panel2); border: 1px solid var(--line); border-radius: 6px; padding: 4px; }
#hbe .tl { display: grid; grid-template-columns: 132px 1fr; row-gap: 3px; position: relative; }
#hbe .tl .lbl { display: flex; align-items: center; gap: 6px; padding-right: 8px; color: var(--dim); overflow: hidden; white-space: nowrap; cursor: pointer; height: 22px; }
#hbe .tl .lbl.sel { color: var(--accent); }
#hbe .tl .trk { position: relative; height: 22px; background: var(--bg); border-radius: 4px; cursor: pointer; }
#hbe .tl .trk.sel { box-shadow: inset 0 0 0 1px #ffd35a55; }
#hbe .tl .ruler { height: 26px; background: none; cursor: ew-resize; z-index: 2; }
#hbe .tl .cell { position: absolute; top: 0; bottom: 0; border-left: 1px solid #ffffff12; display: flex; justify-content: center; align-items: center; pointer-events: none; }
#hbe .tl .cell span { font: 10px/1 ui-monospace, monospace; color: var(--dim); padding: 2px 2px; border-radius: 3px; }
#hbe .tl .ruler.stag .cell.odd { align-items: flex-start; } #hbe .tl .ruler.stag .cell.even { align-items: flex-end; }
#hbe .tl .cell.act span { color: #ff8095; }
#hbe .tl .cell.cur span { background: var(--accent); color: #111; font-weight: 700; }
#hbe .tl .ruler:hover .cell.hov span:not(:empty) { background: #ffffff22; color: var(--text); }
#hbe .tl .seg { position: absolute; top: 3px; bottom: 3px; border-radius: 3px; font-size: 10px; font-weight: 600; color: #fffc; display: flex; align-items: center; justify-content: center; overflow: hidden; white-space: nowrap; }
#hbe .tl .seg.startup { background: var(--startup); } #hbe .tl .seg.active { background: var(--active); } #hbe .tl .seg.endlag { background: var(--endlag); }
#hbe .tl .bar { position: absolute; top: 3px; bottom: 3px; border-radius: 4px; cursor: grab; box-shadow: inset 0 0 0 1px #0004; }
#hbe .tl .bar.sel { box-shadow: 0 0 0 2px var(--accent); }
#hbe .tl .bar .hd { position: absolute; top: 0; bottom: 0; width: 7px; cursor: ew-resize; }
#hbe .tl .bar .hd.l { left: -2px; } #hbe .tl .bar .hd.r { right: -2px; }
#hbe .tl .bar:hover .hd { background: #fff5; border-radius: 3px; }
#hbe .tl .key { position: absolute; top: 50%; width: 7px; height: 7px; margin: -3.5px 0 0 -3.5px; transform: rotate(45deg); background: #fff; border: 1px solid #0006; pointer-events: none; }
#hbe .tl .key.moved { background: var(--accent); }
#hbe .tl .lagh { position: absolute; top: 2px; bottom: 2px; width: 8px; margin-left: -4px; cursor: ew-resize; border-radius: 3px; }
#hbe .tl .lagh:hover { background: #fff6; }
#hbe .playhead { position: absolute; top: 0; bottom: 0; width: 2px; margin-left: -1px; background: var(--accent); pointer-events: none; box-shadow: 0 0 6px #ffd35a88; }
#hbe .split { pointer-events: auto; z-index: 6; position: relative; }
#hbe .split::after { content: ''; position: absolute; inset: 0; margin: auto; background: transparent; transition: background .12s; }
#hbe .split:hover::after, #hbe .split.drag::after { background: var(--accent); }
#hbe .split.col { grid-row: 2 / 4; width: 8px; cursor: col-resize; }
#hbe .split.col::after { width: 2px; }
#hbe .split.l { grid-column: 1; justify-self: end; margin-right: -4px; }
#hbe .split.r { grid-column: 3; justify-self: start; margin-left: -4px; }
#hbe .split.row { grid-area: time; align-self: start; height: 8px; margin-top: -4px; cursor: row-resize; }
#hbe .split.row::after { height: 2px; }

/* inspector */
#hbe .right { grid-area: right; border-left: 1px solid var(--line); overflow-y: auto; padding: 12px 14px 24px; }
#hbe .right h2 { font-size: 16px; margin: 0; display: flex; align-items: baseline; gap: 8px; }
#hbe .right h2 small { font: 11px ui-monospace, monospace; color: var(--faint); font-weight: 400; }
#hbe .sec { margin-top: 14px; padding-top: 12px; border-top: 1px solid var(--line); }
#hbe .sec > .st { display: flex; align-items: center; gap: 8px; font-size: 10px; text-transform: uppercase; letter-spacing: 1px; color: var(--faint); font-weight: 700; margin-bottom: 8px; }
#hbe .sec > .st .grow { flex: 1; }
#hbe .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px; margin-top: 10px; }
#hbe .stat { background: var(--bg); border: 1px solid var(--line); border-radius: 7px; padding: 5px 7px; }
#hbe .stat b { display: block; font-size: 15px; } #hbe .stat span { font-size: 10px; color: var(--dim); }
#hbe .stat input { font-size: 15px !important; font-weight: 700; padding: 0 !important; border: 0 !important; background: transparent !important; }
#hbe .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: 8px 10px; }
#hbe .grid3 { display: grid; grid-template-columns: repeat(3, 1fr); gap: 8px 10px; }
#hbe .fld { display: flex; flex-direction: column; gap: 3px; min-width: 0; }
#hbe .fld > label { color: var(--dim); font-size: 11px; cursor: ew-resize; display: flex; gap: 4px; align-items: center; }
#hbe .fld > label:hover { color: var(--text); }
#hbe .fld.dis { opacity: .4; pointer-events: none; }
#hbe .boxes { display: flex; flex-direction: column; gap: 3px; }
#hbe .brow { display: flex; align-items: center; gap: 8px; padding: 6px 8px; border-radius: 7px; background: var(--bg); border: 1px solid transparent; cursor: pointer; }
#hbe .brow:hover, #hbe .brow.hov { border-color: var(--line); }
#hbe .brow.sel { border-color: var(--accent); background: #2a2610; }
#hbe .brow .ix { font: 700 11px ui-monospace, monospace; color: var(--dim); width: 18px; }
#hbe .brow .sw { width: 10px; height: 10px; border-radius: 50%; flex: none; }
#hbe .brow .nm { flex: 1; } #hbe .brow .mt { font: 11px ui-monospace, monospace; color: var(--dim); }
#hbe .brow .ord { display: none; gap: 2px; } #hbe .brow:hover .ord { display: flex; }
#hbe .brow .ord button { padding: 0 5px; font-size: 10px; }
#hbe .types { display: grid; grid-template-columns: repeat(4, 1fr); gap: 5px; }
#hbe .types button { flex-direction: column; gap: 2px; padding: 6px 2px; font-size: 10.5px; color: var(--dim); }
#hbe .types button .ic { font-size: 17px; line-height: 1; }
#hbe .types button.sel { color: #fff; }
#hbe .desc { color: var(--dim); margin-top: 8px; font-size: 11.5px; line-height: 1.45; }
#hbe .seg2 { display: flex; background: var(--bg); border: 1px solid var(--line); border-radius: 7px; padding: 2px; gap: 2px; }
#hbe .seg2 button { flex: 1; border: 0; background: transparent; padding: 4px 6px; color: var(--dim); justify-content: center; }
#hbe .seg2 button.sel { background: var(--panel2); color: var(--text); box-shadow: inset 0 0 0 1px #3a4384; }
#hbe .row { display: flex; gap: 6px; align-items: center; flex-wrap: wrap; margin-top: 8px; }
#hbe .note { color: var(--dim); font-size: 11px; margin-top: 6px; }
#hbe .note b { color: var(--text); }
#hbe .launch { display: grid; grid-template-columns: 92px 1fr; gap: 12px; align-items: center; }
#hbe .dial { width: 92px; height: 92px; cursor: crosshair; }
#hbe .dial.dis { opacity: .4; cursor: default; }
#hbe .chk { display: flex; align-items: center; gap: 6px; color: var(--dim); cursor: pointer; }
#hbe .chk input { accent-color: var(--accent); }
#hbe table.kb { width: 100%; border-collapse: collapse; font: 11px ui-monospace, monospace; margin-top: 4px; }
#hbe table.kb td, #hbe table.kb th { padding: 3px 4px; text-align: right; border-bottom: 1px solid #ffffff0d; }
#hbe table.kb th { color: var(--faint); font-weight: 600; } #hbe table.kb td:first-child, #hbe table.kb th:first-child { text-align: left; color: var(--dim); }
#hbe .good { color: var(--ok); } #hbe .bad { color: var(--err); }
#hbe details summary { cursor: pointer; color: var(--dim); font-size: 11px; margin: 4px 0 8px; }
#hbe .empty { color: var(--dim); text-align: center; padding: 18px 10px; border: 1px dashed var(--line); border-radius: 8px; line-height: 1.6; }

/* help */
#hbe .modal { position: fixed; inset: 0; background: #05061299; display: grid; place-items: center; pointer-events: auto; z-index: 20; }
#hbe .modal > div { background: var(--panel); border: 1px solid var(--line); border-radius: 12px; padding: 18px 22px; width: min(680px, 92vw); max-height: 86vh; overflow-y: auto; box-shadow: 0 20px 60px #000a; }
#hbe .modal h3 { margin: 0 0 10px; font-size: 16px; }
#hbe .modal .cols { display: grid; grid-template-columns: 1fr 1fr; gap: 4px 24px; }
#hbe .modal .k { display: flex; justify-content: space-between; gap: 10px; padding: 4px 0; border-bottom: 1px solid #ffffff0a; color: var(--dim); }
#hbe .modal .k span:last-child { display: flex; gap: 3px; }
#hbe .tip { position: fixed; pointer-events: none; z-index: 30; background: #000c; color: #fff; font: 600 11px ui-monospace, monospace; padding: 3px 7px; border-radius: 5px; transform: translate(14px, 14px); white-space: nowrap; }
body.hbe-dragging * { cursor: inherit !important; user-select: none !important; }
`;
