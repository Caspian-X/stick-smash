// Rewrites move data inside a fighter source file (src/data/fighters/*.ts) so the hitbox editor can save to code.
// Pure string code: no Node/DOM. Used by the Vite dev-server endpoint (vite.config.ts) and tests.
// Only the edited fields are replaced (hitboxes, endLag, landingLag); comments and everything else stay as written.

import type { Hitbox, StandardMove } from '../data/types.ts';

/** Key order used when writing a hitbox. Unknown keys go last. */
export const HITBOX_KEYS: (keyof Hitbox)[] = [
  'start', 'end', 'x', 'y', 'r', 'damage', 'bkb', 'kbg', 'angle', 'dir', 'type', 'setKb', 'rehit',
  'hitstunMult', 'hitstunAdd', 'hitlagMult', 'shieldDmgMult', 'shieldStunMult', 'frames',
];
const EDITED = ['endLag', 'landingLag', 'hitboxes'] as const;

const num = (n: number) => String(Math.round(n * 10000) / 10000);

/** Compact TS literal, single line. */
export function lit(v: unknown): string {
  if (typeof v === 'number') return num(v);
  if (typeof v === 'string') return `'${v.replace(/\\/g, '\\\\').replace(/'/g, "\\'")}'`;
  if (typeof v === 'boolean' || v === null) return String(v);
  if (Array.isArray(v)) return `[${v.map(lit).join(', ')}]`;
  if (typeof v === 'object') {
    const e = Object.entries(v as object).filter(([, x]) => x !== undefined);
    return e.length ? `{ ${e.map(([k, x]) => `${k}: ${lit(x)}`).join(', ')} }` : '{}';
  }
  throw new Error(`can't serialize ${typeof v}`);
}

export function hitboxLit(b: Hitbox): string {
  const rec = b as unknown as Record<string, unknown>;
  const keys = [...HITBOX_KEYS.filter(k => k in b), ...Object.keys(b).filter(k => !HITBOX_KEYS.includes(k as keyof Hitbox))];
  return `{ ${keys.filter(k => rec[k] !== undefined).map(k => `${k}: ${lit(rec[k])}`).join(', ')} }`;
}

/** `[{...}]` on one line for a single box, one box per line otherwise. `indent` = indentation of the `hitboxes:` line. */
export function hitboxesLit(boxes: Hitbox[], indent: string): string {
  if (boxes.length === 0) return '[]';
  if (boxes.length === 1) return `[${hitboxLit(boxes[0])}]`;
  return `[\n${boxes.map(b => `${indent}  ${hitboxLit(b)},`).join('\n')}\n${indent}]`;
}

// ---------- minimal scanner for object literals ----------

/** Index after a string/comment starting at i, or -1 if none starts there. */
function skipNonCode(s: string, i: number): number {
  const c = s[i];
  if (c === '/' && s[i + 1] === '/') { const e = s.indexOf('\n', i); return e < 0 ? s.length : e; }
  if (c === '/' && s[i + 1] === '*') { const e = s.indexOf('*/', i + 2); return e < 0 ? s.length : e + 2; }
  if (c === "'" || c === '"' || c === '`') {
    let j = i + 1;
    while (j < s.length && s[j] !== c) j += s[j] === '\\' ? 2 : 1;
    return j + 1;
  }
  return -1;
}

/** Index of the bracket that closes the one at `open`. */
export function matchClose(s: string, open: number): number {
  let depth = 0;
  for (let i = open; i < s.length; i++) {
    const k = skipNonCode(s, i);
    if (k >= 0) { i = k - 1; continue; }
    const c = s[i];
    if (c === '{' || c === '[' || c === '(') depth++;
    else if (c === '}' || c === ']' || c === ')') { if (--depth === 0) return i; }
  }
  throw new Error('unbalanced brackets');
}

export interface Entry { key: string | null; keyStart: number; valStart: number; valEnd: number }

/** Top-level entries of the object literal whose `{` is at `open`. Spreads have key null. valEnd is exclusive. */
export function entries(s: string, open: number): Entry[] {
  const close = matchClose(s, open), out: Entry[] = [];
  let i = open + 1;
  while (i < close) {
    const k = skipNonCode(s, i);
    if (k >= 0 && s[i] !== "'" && s[i] !== '"') { i = k; continue; }
    if (/[\s,]/.test(s[i])) { i++; continue; }
    const keyStart = i;
    let key: string | null = null;
    if (s.startsWith('...', i)) i += 3;
    else {
      const m = /^([A-Za-z_$][\w$]*|'[^']*'|"[^"]*")\s*:\s*/.exec(s.slice(i, i + 200));
      if (!m) throw new Error(`can't parse object key near: ${s.slice(i, i + 40)}`);
      key = m[1].replace(/^['"]|['"]$/g, '');
      i += m[0].length;
    }
    const valStart = i;
    let depth = 0;
    for (; i < close; i++) {
      const j = skipNonCode(s, i);
      if (j >= 0) { i = j - 1; continue; }
      const c = s[i];
      if (c === '{' || c === '[' || c === '(') depth++;
      else if (c === '}' || c === ']' || c === ')') depth--;
      else if (c === ',' && depth === 0) break;
    }
    let valEnd = i;
    while (valEnd > valStart && /\s/.test(s[valEnd - 1])) valEnd--;
    out.push({ key, keyStart, valStart, valEnd });
  }
  return out;
}

const lineIndent = (s: string, at: number) => /^[ \t]*/.exec(s.slice(s.lastIndexOf('\n', at - 1) + 1))![0];

function movesOpen(src: string): number {
  const m = /\bmoves\s*:\s*\{/.exec(src);
  if (!m) throw new Error('no `moves: {` block found');
  return m.index + m[0].length - 1;
}

/** Full move literal, used when a move is inherited (e.g. from COMMON_GRABS) and needs its own entry. */
function moveLit(mv: StandardMove, indent: string): string {
  const rec = mv as unknown as Record<string, unknown>;
  const head = Object.keys(mv).filter(k => k !== 'hitboxes' && rec[k] !== undefined).map(k => `${k}: ${lit(rec[k])}`).join(', ');
  return `{\n${indent}  ${head},\n${indent}  hitboxes: ${hitboxesLit(mv.hitboxes, indent + '  ')},\n${indent}}`;
}

/** Replace a standard move's hitboxes/endLag/landingLag in the source, or add the move if the file doesn't define it. */
export function patchMove(src: string, id: string, mv: StandardMove): string {
  const open = movesOpen(src), top = entries(src, open), e = top.find(x => x.key === id);
  if (!e) {
    const first = top.find(x => x.key !== null) ?? top[top.length - 1];
    if (!first) throw new Error('empty moves block');
    const at = first.key !== null ? first.keyStart : first.valEnd + 1, ind = lineIndent(src, first.keyStart);
    const text = `${id}: ${moveLit(mv, ind)},`;
    return first.key !== null ? src.slice(0, at) + text + '\n' + ind + src.slice(at) : src.slice(0, at) + '\n' + ind + text + src.slice(at);
  }
  if (src[e.valStart] !== '{') throw new Error(`move ${id} isn't an object literal`);
  const inner = entries(src, e.valStart), reps: [number, number, string][] = [];
  const rec = mv as unknown as Record<string, unknown>;
  for (const k of EDITED) {
    if (rec[k] === undefined) continue;
    const x = inner.find(n => n.key === k);
    const val = k === 'hitboxes' ? hitboxesLit(mv.hitboxes, lineIndent(src, x?.keyStart ?? e.valStart)) : lit(rec[k]);
    if (x) reps.push([x.valStart, x.valEnd, val]);
    else {
      const hb = inner.find(n => n.key === 'hitboxes');
      if (!hb) throw new Error(`move ${id} has no hitboxes entry`);
      reps.push([hb.keyStart, hb.keyStart, `${k}: ${val}, `]);
    }
  }
  reps.sort((a, b) => b[0] - a[0]);
  for (const [a, b, t] of reps) src = src.slice(0, a) + t + src.slice(b);
  return src;
}

export function patchFighterSource(src: string, moves: Record<string, StandardMove>): string {
  for (const [id, mv] of Object.entries(moves)) src = patchMove(src, id, mv);
  return src;
}
