// Core sim types, world creation, accessors and effect events.
// The sim never touches Phaser, the DOM, or Math.random: it is a pure function of (world, inputs),
// which keeps it testable and ready for rollback netcode.

import { DATA } from '../data';
import type { FighterData, FighterStats, GordoMove, MoveData, MoveId, PhaseHitbox } from '../data/types';
import { type Surface, GROUND, SPAWNS, STAGE } from '../data/stage';
import { COLORS } from '../data/colors';

export interface Input {
  left: boolean; right: boolean; up: boolean; down: boolean;
  /** Jump pressed this frame / jump button held (full hop vs short hop). */
  jump: boolean; jumpHeld: boolean;
  atk: boolean; atkHeld: boolean; sp: boolean; spHeld: boolean; grab: boolean;
  shield: boolean; shieldP: boolean; downP: boolean;
  /** Direction pressed within the smash-input window and still held. */
  tapL: boolean; tapR: boolean; tapU: boolean; tapD: boolean;
  /** Any button pressed this frame (mashing). */
  anyP: boolean;
}
export const EMPTY: Readonly<Input> = Object.freeze({
  left: false, right: false, up: false, down: false, jump: false, jumpHeld: false, atk: false, atkHeld: false, sp: false, spHeld: false,
  grab: false, shield: false, shieldP: false, downP: false, tapL: false, tapR: false, tapU: false, tapD: false, anyP: false,
});
export const blankInput = (): Input => ({ ...EMPTY });

export type FState =
  | 'idle' | 'jumpsquat' | 'air' | 'attack' | 'helpless' | 'landing' | 'shield' | 'roll' | 'spot' | 'airdodge'
  | 'hitstun' | 'knockdown' | 'getup' | 'tech' | 'ledge' | 'climb' | 'hold' | 'throw' | 'grabbed'
  | 'shieldbreak' | 'dizzy' | 'respawn' | 'dead' | 'out' | 'win';

/** Per-state scratch data; reset on every state change. */
export interface StateData {
  hit: Set<number>;
  id?: MoveId; t?: number; ph?: number; t2?: number; charge?: number; cd?: number;
  n?: number; kind?: string; side?: number; dir?: number; dirl?: boolean; dx?: number; dy?: number;
  v?: Fighter | null; by?: Fighter | null; timer?: number; pt?: number; stun?: number; tech?: boolean; getup?: boolean;
  /** Move frame each victim (by index) was last hit, for multi-hit boxes. */
  hitAt?: number[];
  /** Neutral special already B-reversed. */
  rev?: boolean;
}
export interface MoveState extends StateData { id: MoveId; t: number; ph: number; t2: number; charge: number }

export interface AiState { cool: number; wait: number; shieldT: number; chargeT: number; spT: number; upT: number; holdT: number; pum: number }

export interface Fighter {
  i: number; ch: number; name: string; cpu: boolean;
  x: number; y: number; vx: number; vy: number; face: number;
  dmg: number; stocks: number;
  ground: boolean; surf: Surface | null; jumps: number;
  state: FState; t: number; sd: StateData; move: MoveState | null;
  hitstun: number; tumble: boolean; spin: number;
  /** Hitlag: frozen frames. */
  lag: number;
  inv: number; blink: number;
  upB: boolean; adUsed: boolean; ff: boolean;
  drop: number; ledgeCD: number;
  runPh: number; flipT: number; flash: number; hudShake: number;
  sh: number; lastSP: number; inDir: number;
  ai: AiState;
}

export interface Projectile {
  o: Fighter; gordo: boolean;
  x: number; y: number; vx: number; vy: number; life: number; r: number;
  hit: PhaseHitbox; bn: number; hcd: number;
  gp?: GordoMove['gordo'];
}

export type SfxName =
  | 'jump' | 'djump' | 'swing' | 'smash' | 'charge' | 'hit' | 'shoot' | 'slash' | 'counter' | 'whack' | 'toss' | 'bonk'
  | 'jet' | 'quake' | 'ko' | 'land' | 'grab' | 'pummel' | 'throw' | 'shieldHit' | 'shieldBreak' | 'tech' | 'roll'
  | 'beep' | 'go';

/** Presentation events emitted by the sim and consumed by the renderer each frame. */
export type Fx =
  | { t: 'sfx'; name: SfxName; arg?: number }
  | { t: 'burst'; x: number; y: number; col: number; n: number; spd: number; life: number; sz: number; g: number }
  | { t: 'dust'; x: number; y: number; n: number }
  | { t: 'ring'; x: number; y: number; r: number; vr: number; life: number; col: number }
  | { t: 'beam'; x: number; y: number; col: number }
  | { t: 'part'; x: number; y: number; vx: number; vy: number; life: number; col: number; sz: number; g: number }
  | { t: 'shake'; k: number };

/** Training: frame advantage of the most recent hit/shield hit. */
export interface Trace {
  attacker: number; defender: number; move: string; onShield: boolean;
  hitFrame: number; aFree: number; dFree: number; done: boolean;
  combo: number; comboDmg: number;
}

export interface MatchConfig {
  fighters: { ch: number; cpu: boolean; name: string }[];
  stocks: number;
  cpuLevel: number;
  training?: boolean;
  seed?: number;
}

export interface World {
  frame: number;
  fighters: Fighter[];
  projs: Projectile[];
  fx: Fx[];
  rng: number;
  count: number; goT: number; endT: number;
  winner: Fighter | null; over: boolean;
  cfg: MatchConfig;
  trace: Trace | null;
}

// ---------- accessors (always read live data so hot-reloaded tuning applies immediately) ----------
export const G = () => DATA.g;
export const FD = (f: Fighter): FighterData => DATA.fighters[f.ch] ?? DATA.fighters[0];
export const ST = (f: Fighter): FighterStats => FD(f).stats;
export const MV = (f: Fighter, id: MoveId = f.move!.id): MoveData => FD(f).moves[id] ?? FD(f).moves.nspec;
export const maxJ = (f: Fighter) => ST(f).airJumps;
export const other = (w: World, f: Fighter) => w.fighters.find(g => g !== f) ?? null;
export const col = (f: Fighter) => COLORS[f.i % COLORS.length].c;

export const clamp = (v: number, a: number, b: number) => (v < a ? a : v > b ? b : v);
export const lerp = (a: number, b: number, k: number) => a + (b - a) * k;
export const easeOut = (k: number) => 1 - (1 - k) * (1 - k);

/** Deterministic RNG (mulberry32) stored on the world. */
export function rand(w: World): number {
  let t = (w.rng = (w.rng + 0x6d2b79f5) | 0);
  t = Math.imul(t ^ (t >>> 15), t | 1);
  t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
}
export const rnd = (w: World, a: number, b: number) => a + rand(w) * (b - a);

// ---------- fx helpers ----------
export const sfx = (w: World, name: SfxName, arg?: number) => w.fx.push({ t: 'sfx', name, arg });
export const burst = (w: World, x: number, y: number, c: number, n: number, spd: number, life: number, sz: number, g = 0.15) =>
  w.fx.push({ t: 'burst', x, y, col: c, n, spd, life, sz, g });
export const dust = (w: World, x: number, y: number, n = 6) => w.fx.push({ t: 'dust', x, y, n });
export const ring = (w: World, x: number, y: number, r: number, vr: number, life: number, c = 0xffffff) =>
  w.fx.push({ t: 'ring', x, y, r, vr, life, col: c });
export const part = (w: World, x: number, y: number, vx: number, vy: number, life: number, c: number, sz: number, g = 0) =>
  w.fx.push({ t: 'part', x, y, vx, vy, life, col: c, sz, g });
export const shake = (w: World, k: number) => w.fx.push({ t: 'shake', k });

// ---------- creation ----------
export function setState(f: Fighter, s: FState, sd: Partial<StateData> = {}) {
  f.state = s; f.t = 0; f.sd = { hit: new Set(), ...sd };
  if (s !== 'attack') f.move = null;
}

export function mkFighter(i: number, ch: number, cpu: boolean, name: string, stocks: number): Fighter {
  const sp = SPAWNS[i] ?? SPAWNS[0];
  const f: Fighter = {
    i, ch: DATA.fighters[ch] ? ch : 0, name, cpu,
    x: sp.x, y: STAGE.top, vx: 0, vy: 0, face: sp.face, dmg: 0, stocks,
    ground: true, surf: GROUND, jumps: 0, state: 'idle', t: 0, sd: { hit: new Set() }, move: null,
    hitstun: 0, tumble: false, spin: 0, lag: 0, inv: 0, blink: 0, upB: false, adUsed: false, ff: false,
    drop: 0, ledgeCD: 0, runPh: 0, flipT: 0, flash: 0, hudShake: 0,
    sh: DATA.g.shield.max, lastSP: -99, inDir: 0,
    ai: { cool: 30, wait: 0, shieldT: 0, chargeT: 0, spT: 0, upT: 0, holdT: 0, pum: 0 },
  };
  f.jumps = maxJ(f);
  return f;
}

export function createWorld(cfg: MatchConfig): World {
  return {
    frame: 0,
    fighters: cfg.fighters.map((c, i) => mkFighter(i, c.ch, c.cpu, c.name, cfg.stocks)),
    projs: [], fx: [], rng: cfg.seed ?? 12345,
    count: cfg.training ? 0 : DATA.g.match.countdown, goT: 0, endT: 0, winner: null, over: false,
    cfg, trace: null,
  };
}

export const isInv = (f: Fighter) => f.inv > 0 || f.blink > 0;
export const hittable = (v: Fighter) => !['dead', 'respawn', 'grabbed', 'out', 'win'].includes(v.state) && !isInv(v);

/** Counter-stance active: the swordfighter-style down special inside its window. */
export function countering(v: Fighter): boolean {
  if (v.state !== 'attack' || !v.move || v.move.ph !== 0) return false;
  const d = MV(v);
  return d.kind === 'counter' && v.move.t >= d.window[0] && v.move.t <= d.window[1];
}

/** Can act freely this frame (used for frame-advantage tracking). */
export function actionable(f: Fighter): boolean {
  if (f.lag > 0) return false;
  if (f.state === 'idle' || f.state === 'air' || f.state === 'ledge') return true;
  return f.state === 'shield' && !(f.sd.stun! > 0);
}
