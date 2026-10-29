// Defaults shared by every fighter. A fighter file spreads these and overrides what it needs.

import type { FighterStats, MoveData, ThrowData } from '../types';

export const BASE_STATS: FighterStats = {
  weight: 1,
  runSpeed: 6.8,
  runAccel: 1.3,
  groundFriction: 0.78,
  jumpSpeed: 15,
  shortHopSpeed: 10.5,
  airJumpSpeed: 13.5,
  airJumpDrift: 4.5,
  airJumps: 1,
  airSpeed: 5.2,
  airAccel: 0.5,
  gravity: 0.75,
  fallSpeed: 10.5,
  fastFallSpeed: 15,
};

export const COMMON_GRABS: { grab: MoveData; dashgrab: MoveData } = {
  grab: {
    name: 'Grab', endLag: 22,
    hitboxes: [{ start: 6, end: 8, x: 34, y: 46, r: 20, damage: 0, bkb: 0, kbg: 0, angle: 0, grab: true }],
  },
  dashgrab: {
    name: 'Dash Grab', endLag: 28, slide: 8,
    hitboxes: [{ start: 8, end: 10, x: 40, y: 46, r: 22, damage: 0, bkb: 0, kbg: 0, angle: 0, grab: true }],
  },
};

export const COMMON_THROWS: Record<'f' | 'b' | 'u' | 'd', ThrowData> = {
  f: { release: 8, total: 24, hit: { x: 0, y: 0, r: 0, damage: 7, bkb: 7, kbg: 0.09, angle: 40 } },
  b: { release: 12, total: 28, hit: { x: 0, y: 0, r: 0, damage: 9, bkb: 7, kbg: 0.13, angle: 40 } },
  u: { release: 12, total: 28, hit: { x: 0, y: 0, r: 0, damage: 6, bkb: 8, kbg: 0.085, angle: 90 } },
  d: { release: 10, total: 26, hit: { x: 0, y: 0, r: 0, damage: 5, bkb: 6, kbg: 0.045, angle: 75 } },
};
