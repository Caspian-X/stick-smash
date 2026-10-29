// Brawler: all-rounder. Energy ball, rising uppercut, ground pound.
// Hitbox fields: start/end = active frames, x/y = offset forward/up from feet, r = radius,
// damage = %, bkb = base knockback, kbg = knockback growth, angle = launch degrees (0 forward, 90 up).
// Move length = last active frame + endLag. If two boxes overlap a victim, the first listed wins.

import type { FighterData } from '../types';
import { BASE_STATS, COMMON_GRABS, COMMON_THROWS } from './common';

const brawler: FighterData = {
  id: 'brawler',
  name: 'Brawler',
  blurb: 'Brawler special: energy ball · ↑ rising uppercut (recovery) · ↓ ground slam',
  aiReach: 75,
  stats: { ...BASE_STATS },
  throws: COMMON_THROWS,
  moves: {
    ...COMMON_GRABS,
    jab: {
      name: 'Jab', endLag: 10,
      hitboxes: [{ start: 3, end: 6, x: 36, y: 52, r: 18, damage: 4, bkb: 4, kbg: 0.05, angle: 30 }],
    },
    ftilt: {
      name: 'Forward Tilt', endLag: 15,
      hitboxes: [{ start: 7, end: 11, x: 46, y: 42, r: 22, damage: 9, bkb: 5, kbg: 0.12, angle: 38 }],
    },
    utilt: {
      name: 'Up Tilt', endLag: 13,
      hitboxes: [{ start: 6, end: 11, x: 10, y: 96, r: 26, damage: 8, bkb: 6, kbg: 0.1, angle: 88 }],
    },
    dtilt: {
      name: 'Down Tilt', endLag: 12,
      hitboxes: [{ start: 5, end: 8, x: 44, y: 8, r: 20, damage: 6, bkb: 4, kbg: 0.08, angle: 18 }],
    },
    dashatk: {
      name: 'Dash Attack', endLag: 20, slide: 14,
      hitboxes: [{ start: 6, end: 14, x: 32, y: 24, r: 24, damage: 9, bkb: 6, kbg: 0.08, angle: 60 }],
    },
    fsmash: {
      name: 'Forward Smash', endLag: 29, charge: { frame: 8 }, forwardStep: 4,
      hitboxes: [{ start: 14, end: 17, x: 54, y: 46, r: 26, damage: 15, bkb: 7, kbg: 0.165, angle: 38 }],
    },
    usmash: {
      name: 'Up Smash', endLag: 28, charge: { frame: 6 },
      hitboxes: [{ start: 11, end: 16, x: 6, y: 100, r: 32, damage: 14, bkb: 8, kbg: 0.16, angle: 88 }],
    },
    dsmash: {
      name: 'Down Smash', endLag: 31, charge: { frame: 6 },
      hitboxes: [
        { start: 10, end: 13, x: 46, y: 10, r: 26, damage: 13, bkb: 6, kbg: 0.15, angle: 25, dir: 'away' },
        { start: 10, end: 13, x: -46, y: 10, r: 26, damage: 13, bkb: 6, kbg: 0.15, angle: 25, dir: 'away' },
      ],
    },
    nair: {
      name: 'Neutral Air', endLag: 16, aerial: true, landingLag: 6,
      hitboxes: [{ start: 4, end: 12, x: 0, y: 40, r: 44, damage: 7, bkb: 4, kbg: 0.09, angle: 45, dir: 'away' }],
    },
    fair: {
      name: 'Forward Air', endLag: 18, aerial: true, landingLag: 8,
      hitboxes: [{ start: 6, end: 10, x: 44, y: 40, r: 24, damage: 10, bkb: 5, kbg: 0.12, angle: 40 }],
    },
    bair: {
      name: 'Back Air', endLag: 18, aerial: true, landingLag: 8,
      hitboxes: [{ start: 6, end: 10, x: -44, y: 40, r: 24, damage: 12, bkb: 6, kbg: 0.13, angle: 40, dir: 'back' }],
    },
    uair: {
      name: 'Up Air', endLag: 14, aerial: true, landingLag: 6,
      hitboxes: [{ start: 5, end: 10, x: 8, y: 96, r: 26, damage: 8, bkb: 5, kbg: 0.1, angle: 85 }],
    },
    dair: {
      name: 'Down Air', endLag: 18, aerial: true, landingLag: 12,
      hitboxes: [{ start: 9, end: 14, x: 0, y: -2, r: 24, damage: 11, bkb: 4, kbg: 0.1, angle: -80 }],
    },
    nspec: {
      name: 'Energy Ball', kind: 'projectile', total: 34, spawnFrame: 12, airFallCap: 2.5,
      projectile: {
        x: 30, y: 50, speed: 10, life: 75, r: 11, max: 2,
        hit: { x: 0, y: 0, r: 11, damage: 6, bkb: 3.5, kbg: 0.05, angle: 28 },
      },
    },
    uspec: {
      name: 'Rising Uppercut', endLag: 20, helpless: true,
      rise: { frame: 4, vy: 17.5, vx: 2.5 },
      hitboxes: [{ start: 4, end: 16, x: 12, y: 60, r: 32, damage: 10, bkb: 7, kbg: 0.08, angle: 82 }],
    },
    dspec: {
      name: 'Ground Slam', kind: 'groundPound', startup: 8, fallSpeed: 22, timeout: 50, waveFrames: 6, recovery: 22,
      fall: { x: 0, y: 6, r: 26, damage: 9, bkb: 5, kbg: 0.07, angle: 70 },
      wave: [
        { x: 58, y: 14, r: 36, damage: 8, bkb: 6, kbg: 0.11, angle: 48, dir: 'away' },
        { x: -58, y: 14, r: 36, damage: 8, bkb: 6, kbg: 0.11, angle: 48, dir: 'away' },
      ],
    },
  },
};

export default brawler;
