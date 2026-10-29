// Stick-figure poses. Local space: +x = facing direction, y up is negative, feet at (0,0).

import { STAGE } from '../data/stage';
import type { BodyPose } from '../data/types';
import { DATA } from '../data';
import { type Fighter, FD, MV, clamp } from '../sim/core';

export type Pt = [number, number];
export type Limb = [Pt, Pt];
export interface Pose {
  hd: Pt; sh: Pt; hp: Pt; aF: Limb; aB: Limb; lF: Limb; lB: Limb;
  rot: number; oy: number; pv: number;
  /** Weapon angle override (radians). */
  sw?: number;
}
type PosePart = Partial<Pose>;

const AIRTUCK: PosePart = { aF: [[10, -48], [18, -56]], aB: [[-10, -48], [-18, -56]], lF: [[10, -22], [6, -8]], lB: [[-2, -18], [-10, -6]] };
const CROUCH: PosePart = { hd: [6, -50], sh: [3, -38], hp: [0, -18], aF: [[12, -28], [18, -20]], aB: [[-6, -30], [-10, -22]], lF: [[10, -12], [14, 0]], lB: [[-8, -10], [-12, 0]] };
const CURL: PosePart = { hd: [8, -40], sh: [3, -36], hp: [0, -20], aF: [[12, -30], [16, -18]], aB: [[8, -30], [12, -18]], lF: [[14, -26], [8, -12]], lB: [[10, -24], [4, -12]], pv: -28 };
const SHIELDP: PosePart = { hd: [2, -60], sh: [1, -47], hp: [0, -26], aF: [[12, -44], [6, -56]], aB: [[10, -40], [4, -52]], lF: [[8, -13], [11, 0]], lB: [[-6, -13], [-10, 0]] };
const LIE: PosePart = { hd: [0, -64], sh: [0, -50], hp: [0, -28], aF: [[6, -40], [4, -28]], aB: [[-6, -40], [-4, -28]], lF: [[4, -14], [6, 0]], lB: [[-4, -14], [-2, 0]], rot: -Math.PI / 2, oy: 33 };

export const BODY: Record<BodyPose, PosePart> = {
  stand: {},
  lean: { hd: [8, -61], sh: [6, -48], hp: [0, -28], lF: [[14, -14], [20, 0]], lB: [[-10, -14], [-20, 0]], aB: [[-6, -40], [-16, -36]] },
  back: { hd: [-10, -60], sh: [-8, -47], hp: [-2, -27], lF: [[10, -14], [16, 0]], lB: [[-10, -14], [-18, 0]], aB: [[-4, -40], [4, -34]] },
  crouch: CROUCH,
  stretch: { hd: [0, -66], sh: [0, -52], hp: [0, -30], lF: [[4, -14], [2, 0]], lB: [[-4, -14], [-2, 0]], aB: [[-8, -44], [-14, -36]] },
  lunge: { hd: [14, -56], sh: [12, -44], hp: [0, -26], lF: [[20, -14], [30, 0]], lB: [[-16, -12], [-30, 0]], aB: [[-6, -40], [-20, -44]] },
  air: AIRTUCK,
  rise: { lF: [[4, -12], [0, 4]], lB: [[-6, -10], [-10, 4]], aB: [[-8, -44], [-12, -36]] },
  guard: { hd: [-2, -62], sh: [-2, -49], hp: [0, -27], lF: [[8, -13], [11, 0]], lB: [[-6, -13], [-10, 0]], aB: [[6, -42], [12, -38]] },
};

function runPose(f: Fighter): PosePart {
  const s = Math.sin(f.runPh), c = Math.cos(f.runPh);
  return {
    hd: [6, -63], sh: [4, -50], hp: [0, -29],
    lF: [[6 + s * 10, -15], [s * 18, -Math.max(0, c) * 8]], lB: [[6 - s * 10, -15], [-s * 18, -Math.max(0, -c) * 8]],
    aF: [[4 - s * 10, -40], [10 - s * 14, -34]], aB: [[4 + s * 10, -40], [10 + s * 14, -34]],
  };
}

/** Angle (degrees) at frame t from [frame, angle] keyframes. */
export function kfAt(kf: [number, number][], t: number): number {
  for (let k = 1; k < kf.length; k++) {
    if (t > kf[k][0]) continue;
    const [t0, a0] = kf[k - 1], [t1, a1] = kf[k];
    return a0 + (a1 - a0) * (t - t0) / (t1 - t0);
  }
  return kf[kf.length - 1][1];
}

/** Weapon swing timing for the current move: [keyframes, t, first active frame, active windows], or null. */
export function swingOf(f: Fighter): [[number, number][], number, number, [number, number][]] | null {
  const m = f.move!, d = MV(f);
  if (d.kind === 'counter' && m.ph === 1) return [d.strikeSwing ?? [[0, -120], [3, -120], [7, 40]], m.t2, m.cd ? d.strike[0] : 99, m.cd ? [d.strike] : []];
  if (!d.anim) return null;
  const ws: [number, number][] = d.kind === undefined ? d.hitboxes.map(b => [b.start, b.end]) : [];
  return [d.anim.swing, m.t, ws.length ? Math.min(...ws.map(w => w[0])) : 99, ws];
}

function movePose(f: Fighter, set: (o: PosePart) => Pose) {
  const m = f.move!, d = MV(f), b0 = d.kind === undefined ? d.hitboxes[0] : undefined;
  const st = b0 ? b0.start : 0, en = b0 ? b0.end : 0, wind = !!b0 && m.t < st, act = !!b0 && m.t >= st && m.t <= en;
  switch (m.id) {
    case 'jab': if (wind) set({ aF: [[4, -46], [8, -50]] }); else set({ hd: [4, -64], sh: [3, -50], aF: [[18, -50], [act ? 36 : 30, -52]] }); break;
    case 'ftilt': if (wind) set({ hd: [-3, -63], sh: [-2, -50], lF: [[12, -26], [6, -20]] });
      else set({ hd: [-8, -62], sh: [-6, -49], hp: [0, -30], lF: [[22, -36], [act ? 48 : 40, -40]], lB: [[-4, -14], [-6, 0]], aF: [[-2, -40], [4, -32]], aB: [[-16, -46], [-24, -52]] }); break;
    case 'utilt': if (wind) set({ ...CROUCH, hd: [0, -58], sh: [0, -44], hp: [0, -24] });
      else set({ aF: [[12, -70], [act ? 10 : 8, act ? -98 : -88]], hd: [-3, -64] }); break;
    case 'dtilt': set({ ...CROUCH, lF: wind ? [[10, -12], [14, 0]] : [[22, -10], [act ? 46 : 38, -4]] }); break;
    case 'dashatk': if (wind) set({ hd: [8, -60], sh: [6, -47] });
      else set({ hd: [-8, -56], sh: [-6, -44], hp: [0, -26], lF: [[20, -24], [act ? 42 : 36, -18]], lB: [[-6, -12], [-12, -2]], aF: [[4, -36], [-4, -28]], aB: [[-14, -42], [-22, -36]] }); break;
    case 'fsmash': if (wind) set({ hd: [-10, -60], sh: [-8, -47], hp: [-2, -27], aF: [[-16, -46], [-30, -42]], aB: [[-14, -44], [-26, -38]], lF: [[10, -14], [16, 0]], lB: [[-10, -14], [-18, 0]] });
      else set({ hd: [12, -60], sh: [9, -47], hp: [2, -27], aF: [[28, -50], [act ? 56 : 46, -48]], aB: [[-8, -44], [-18, -38]], lF: [[16, -14], [22, 0]], lB: [[-12, -14], [-24, 0]] }); break;
    case 'usmash': if (wind) set(CROUCH);
      else set({ hd: [0, -66], sh: [0, -52], hp: [0, -30], aF: [[8, -78], [act ? 8 : 10, act ? -106 : -96]], aB: [[-4, -78], [-6, act ? -104 : -94]], lF: [[4, -14], [2, 0]], lB: [[-4, -14], [-2, 0]], oy: act ? -8 : 0 }); break;
    case 'dsmash': if (wind) set({ ...CROUCH, aF: [[10, -30], [6, -20]], aB: [[-6, -30], [-10, -20]] });
      else set({ hd: [0, -48], sh: [0, -36], hp: [0, -16], aF: [[10, -30], [6, -14]], aB: [[-10, -30], [-6, -14]], lF: [[22, -10], [act ? 48 : 40, -6]], lB: [[-22, -10], [act ? -48 : -40, -6]] }); break;
    case 'nair': if (act) set({ rot: (m.t - st) / (en - st + 1) * Math.PI * 2, aF: [[16, -50], [32, -52]], aB: [[-16, -50], [-32, -52]], lF: [[14, -18], [28, -8]], lB: [[-14, -18], [-28, -8]] });
      else set(AIRTUCK); break;
    case 'fair': if (wind) set({ ...AIRTUCK, lF: [[10, -28], [2, -24]] });
      else set({ hd: [-6, -62], sh: [-4, -49], lF: [[22, -34], [act ? 46 : 38, -40]], lB: [[-4, -18], [-12, -8]], aB: [[-14, -46], [-22, -52]], aF: [[2, -42], [8, -34]] }); break;
    case 'bair': if (wind) set({ ...AIRTUCK, lB: [[-6, -28], [-2, -20]] });
      else set({ hd: [8, -62], sh: [6, -49], lB: [[-22, -34], [act ? -46 : -38, -40]], lF: [[8, -18], [12, -6]], aF: [[16, -46], [22, -52]], aB: [[2, -42], [-2, -34]] }); break;
    case 'uair': if (wind) set(AIRTUCK);
      else set({ aF: [[12, -70], [10, -98]], aB: [[-6, -70], [-2, -96]], lF: [[10, -22], [6, -8]], lB: [[-4, -18], [-10, -6]] }); break;
    case 'dair': if (wind) set({ ...AIRTUCK, lF: [[8, -30], [4, -22]], lB: [[-4, -30], [-6, -22]] });
      else set({ aF: [[10, -60], [16, -72]], aB: [[-10, -60], [-16, -72]], lF: [[4, -12], [3, act ? 8 : 2]], lB: [[-4, -12], [-3, act ? 8 : 2]] }); break;
    case 'grab': case 'dashgrab': if (wind) set({ aF: [[6, -44], [10, -40]], aB: [[4, -42], [8, -38]] });
      else set({ hd: [4, -63], aF: [[18, -48], [act ? 36 : 30, -46]], aB: [[16, -46], [act ? 34 : 28, -44]] }); break;
    case 'nspec': {
      const spawn = d.kind === 'projectile' || d.kind === 'gordo' ? d.spawnFrame : 12;
      if (m.t < spawn) set({ hd: [-2, -63], aF: [[-6, -44], [-14, -42]], aB: [[-10, -44], [-18, -46]] });
      else set({ hd: [3, -64], aF: [[16, -50], [32, -50]], aB: [[14, -48], [30, -46]] });
      break;
    }
    case 'uspec': if (m.t < 4) set({ ...CROUCH, hd: [0, -58], sh: [0, -44], hp: [0, -22] });
      else set({ aF: [[12, -72], [12, -100]], aB: [[-8, -44], [-12, -36]], lF: [[4, -12], [0, 4]], lB: [[-6, -10], [-10, 4]] }); break;
    case 'dspec':
      if (m.ph === 0) set({ ...AIRTUCK, rot: f.ground ? 0 : m.t * 0.8 });
      else if (m.ph === 1) set({ aF: [[10, -62], [16, -76]], aB: [[-10, -62], [-16, -76]], lF: [[3, -12], [2, 6]], lB: [[-3, -12], [-2, 6]] });
      else set({ ...CROUCH, aF: [[14, -26], [20, -2]], aB: [[2, -26], [8, -2]] });
      break;
  }
}

function weaponPose(f: Fighter, set: (o: PosePart) => Pose) {
  const m = f.move!, d = MV(f);
  const arm = (P: Pose, th: number) => {
    const s = P.sh;
    set({ aF: [[s[0] + 15 * Math.cos(th + 0.35), s[1] + 15 * Math.sin(th + 0.35)], [s[0] + 28 * Math.cos(th), s[1] + 28 * Math.sin(th)]], sw: th });
  };
  if (d.kind === 'superJump') {
    const ph = m.ph;
    arm(set(BODY[ph === 0 || ph === 3 ? 'crouch' : ph === 1 ? 'rise' : 'stretch']), (ph === 0 ? 150 : ph === 1 ? -100 : ph === 2 ? 90 : 25) * Math.PI / 180);
    return;
  }
  const [kf, t, a] = swingOf(f)!;
  const body: BodyPose = d.kind === 'counter' && m.ph === 1 ? 'lean' : t < a && d.anim?.windup ? d.anim.windup : d.anim?.body ?? 'stand';
  arm(set(BODY[body]), kfAt(kf, t) * Math.PI / 180);
}

export function pose(f: Fighter, T: number): Pose {
  const st = f.state, sd = f.sd, b = Math.sin(T * 0.08) * 1.2;
  const P: Pose = {
    hd: [0, -64 + b], sh: [0, -50 + b], hp: [0, -28], aF: [[8, -38 + b], [13, -27 + b]], aB: [[-7, -38 + b], [-11, -27 + b]],
    lF: [[6, -14], [9, 0]], lB: [[-5, -14], [-9, 0]], rot: 0, oy: 0, pv: -38,
  };
  const set = (o: PosePart) => Object.assign(P, o);
  switch (st) {
    case 'idle': if (Math.abs(f.vx) > 0.8) set(runPose(f)); break;
    case 'jumpsquat': set(CROUCH); break;
    case 'landing': set(sd.kind === 'shield' ? SHIELDP : CROUCH); break;
    case 'air':
      if (f.vy < 0) set({ aF: [[10, -56], [18, -66]], aB: [[-10, -54], [-16, -62]], lF: [[10, -22], [6, -6]], lB: [[-2, -18], [-10, -4]] });
      else set({ aF: [[12, -48], [22, -54]], aB: [[-12, -48], [-22, -54]], lF: [[6, -14], [8, 0]], lB: [[-6, -12], [-10, -2]] });
      if (f.flipT > 0) P.rot = (20 - f.flipT) / 20 * Math.PI * 2;
      break;
    case 'helpless': set({ hd: [0, -62], aF: [[6, -38], [8, -26]], aB: [[-6, -38], [-8, -26]], lF: [[4, -14], [4, 0]], lB: [[-4, -14], [-4, 0]] }); break;
    case 'attack': {
      const d = MV(f);
      if (FD(f).weapon && (d.anim || d.kind === 'superJump' || d.kind === 'counter')) weaponPose(f, set); else movePose(f, set);
      break;
    }
    case 'shield': set(SHIELDP); break;
    case 'roll': set({ ...CURL, rot: clamp(f.t / 26, 0, 1) * Math.PI * 2 }); break;
    case 'spot': set({ hd: [-8, -62], sh: [-5, -50], aF: [[2, -40], [6, -30]], aB: [[-12, -40], [-14, -30]] }); break;
    case 'airdodge': set(CURL); break;
    case 'hold':
      set({ aF: [[16, -46], [28, -44]], aB: [[14, -44], [26, -42]] });
      if (sd.pt! > 3 && sd.pt! < 11) set({ lF: [[16, -32], [22, -22]], hd: [4, -63] });
      break;
    case 'throw': {
      const T2 = FD(f).throws[sd.kind as 'f'], p = clamp(f.t / T2.release, 0, 1), done = f.t >= T2.release;
      if (sd.kind === 'f') set(done ? { hd: [6, -63], aF: [[20, -50], [40, -50]], aB: [[18, -48], [38, -46]], lF: [[14, -14], [18, 0]] } : { aF: [[14, -46], [26, -44]], aB: [[12, -44], [24, -42]] });
      else if (sd.kind === 'b') {
        if (done) set({ hd: [-6, -62], aF: [[-14, -50], [-32, -48]], aB: [[-12, -48], [-30, -46]] });
        else { const a = p * Math.PI, hx = Math.cos(a) * 28, hy = -46 - Math.sin(a) * 30; set({ aF: [[hx * 0.5, (hy - 50) / 2], [hx, hy]], aB: [[hx * 0.5 - 2, (hy - 48) / 2], [hx - 2, hy + 2]] }); }
      } else if (sd.kind === 'u') {
        const hx = 28 * (1 - p), hy = -44 - p * 52;
        set({ aF: [[hx * 0.6 + 4, (hy - 50) / 2], [hx, hy]], aB: [[hx * 0.6, (hy - 48) / 2], [hx - 4, hy]] });
      } else set(done ? { ...CROUCH, aF: [[14, -30], [24, -8]], aB: [[10, -28], [20, -6]] } : { aF: [[12, -62], [20, -76]], aB: [[8, -60], [16, -74]] });
      break;
    }
    case 'grabbed': case 'shieldbreak': case 'hitstun': {
      const s = Math.sin(T * 0.6) * 4;
      set({ hd: [-4, -62], aF: [[10, -58], [16 + s, -70]], aB: [[-10, -58], [-14 - s, -70]], lF: [[8, -16], [14, -4 + s]], lB: [[-6, -14], [-12, -2 - s]] });
      if (st === 'hitstun') P.rot = f.tumble ? f.spin : -0.25;
      break;
    }
    case 'knockdown': set(LIE); break;
    case 'getup': case 'tech': {
      const k = clamp(f.t / (st === 'tech' ? 8 : 12), 0, 1);
      set({ ...CROUCH, rot: -Math.PI / 2 * (1 - k), oy: 33 * (1 - k) });
      if (st === 'getup' && sd.kind === 'atk' && f.t >= 12 && f.t <= 20) set({ hp: [0, -16], sh: [0, -34], hd: [0, -48], lF: [[22, -12], [42, -10]], lB: [[-22, -12], [-42, -10]], aF: [[8, -26], [10, -10]], aB: [[-8, -26], [-10, -10]] });
      break;
    }
    case 'ledge': {
      const s = Math.sin(T * 0.05) * 3;
      set({ hd: [0, -62], sh: [0, -50], aF: [[7, -66], [12, -80]], aB: [[3, -66], [8, -80]], lF: [[4 + s, -14], [2 + s, 0]], lB: [[-2 + s, -14], [-4 + s, 0]] });
      break;
    }
    case 'climb': {
      const c = DATA.g.ledge[sd.kind as 'climb'], ex = sd.side! < 0 ? STAGE.L : STAGE.R;
      const atk = DATA.g.ledge.attackActive;
      if (f.t <= c.a) {
        const hx = (ex - f.x) * f.face, hy = STAGE.top - f.y, k = f.t / c.a;
        set({ hd: [2, -60], sh: [2, -48], hp: [0, -28], aF: [[(hx + 2) / 2 + 6, (hy - 48) / 2], [hx, hy]], aB: [[(hx + 2) / 2 + 2, (hy - 48) / 2], [hx - 4, hy]],
          lF: [[12, -28 - k * 6], [8, -16 - k * 4]], lB: [[0, -14], [-4, 0]] });
      } else if (sd.kind === 'roll' && f.t < c.b) set({ ...CURL, rot: (f.t - c.a) / (c.b - c.a) * Math.PI * 2 });
      else if (sd.kind === 'attack' && f.t >= atk[0] - 2 && f.t <= atk[1] + 4) set({ hd: [-8, -62], sh: [-6, -49], hp: [0, -30], lF: [[22, -32], [44, -32]], lB: [[-4, -14], [-6, 0]], aB: [[-16, -46], [-24, -52]] });
      else if (f.t < c.a + 8) set(CROUCH);
      break;
    }
    case 'dizzy': {
      const s = Math.sin(T * 0.1);
      set({ hd: [s * 6, -62], sh: [s * 3, -49], aF: [[8, -38], [6 + s * 4, -24]], aB: [[-8, -38], [-6 + s * 4, -24]], rot: s * 0.08 });
      break;
    }
    case 'win': set({ aF: [[10, -62], [20, -82]], aB: [[-10, -62], [-20, -82]], oy: -Math.abs(Math.sin(T * 0.12)) * 16 }); break;
  }
  return P;
}
