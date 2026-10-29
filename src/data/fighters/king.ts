// King: heavy hammer fighter. Slow, huge hits, three air jumps.
// See brawler.ts for field meanings. `anim.swing` = hammer angle keyframes [frame, degrees].

import type { FighterData } from '../types';
import { BASE_STATS, COMMON_GRABS, COMMON_THROWS } from './common';

const king: FighterData = {
  id: 'king',
  name: 'King',
  weapon: 'hammer',
  look: 'king',
  blurb: 'King: heavy, 3 air jumps · special: throw a Gordo (any attack bats it back) · ↑ super jump · ↓ hold for Jet Hammer',
  aiReach: 100,
  stats: { ...BASE_STATS, weight: 0.9, runSpeed: 5, jumpSpeed: 14, airJumps: 3, airJumpSpeed: 11.5 },
  throws: COMMON_THROWS,
  moves: {
    ...COMMON_GRABS,
    jab: {
      name: 'Jab', endLag: 15, anim: { body: 'stand', swing: [[0, -70], [4, -10], [7, 15]] },
      hitboxes: [{ start: 4, end: 7, x: 62, y: 46, r: 24, damage: 5, bkb: 5, kbg: 0.05, angle: 35 }],
    },
    ftilt: {
      name: 'Forward Tilt', endLag: 24, anim: { windup: 'back', body: 'lean', swing: [[0, -30], [6, -140], [10, -125], [14, 30]] },
      hitboxes: [{ start: 11, end: 14, x: 78, y: 40, r: 32, damage: 13, bkb: 7, kbg: 0.12, angle: 36 }],
    },
    utilt: {
      name: 'Up Tilt', endLag: 20, anim: { body: 'stand', swing: [[0, 40], [8, 20], [10, 0], [16, -200]] },
      hitboxes: [{ start: 10, end: 16, x: 30, y: 100, r: 40, damage: 11, bkb: 7, kbg: 0.11, angle: 88 }],
    },
    dtilt: {
      name: 'Down Tilt', endLag: 20, anim: { body: 'crouch', swing: [[0, -40], [5, -60], [7, 15], [10, 20]] },
      hitboxes: [{ start: 7, end: 10, x: 76, y: 14, r: 26, damage: 9, bkb: 5, kbg: 0.08, angle: 22 }],
    },
    dashatk: {
      name: 'Dash Attack', endLag: 28, slide: 18, anim: { body: 'lean', swing: [[0, -120], [8, -120], [16, 30]] },
      hitboxes: [{ start: 10, end: 16, x: 68, y: 36, r: 32, damage: 12, bkb: 7, kbg: 0.09, angle: 50 }],
    },
    fsmash: {
      name: 'Forward Smash', endLag: 44, charge: { frame: 10 }, forwardStep: 4, quakeFrame: 25,
      anim: { windup: 'back', body: 'lean', swing: [[0, -20], [10, -150], [20, -140], [25, 35]] },
      hitboxes: [{ start: 22, end: 26, x: 82, y: 24, r: 36, damage: 24, bkb: 9, kbg: 0.19, angle: 40 }],
    },
    usmash: {
      name: 'Up Smash', endLag: 35, charge: { frame: 8 },
      anim: { windup: 'crouch', body: 'stretch', swing: [[0, -90], [8, -200], [16, -200], [24, -70]] },
      hitboxes: [{ start: 17, end: 23, x: 20, y: 112, r: 42, damage: 17, bkb: 8, kbg: 0.17, angle: 88 }],
    },
    dsmash: {
      name: 'Down Smash', endLag: 32, charge: { frame: 8 }, quakeFrame: 14,
      anim: { body: 'crouch', swing: [[0, -60], [8, -110], [14, 25], [17, 25], [19, -140], [24, -200]] },
      hitboxes: [
        { start: 14, end: 17, x: 74, y: 12, r: 30, damage: 15, bkb: 7, kbg: 0.15, angle: 28, dir: 'away' },
        { start: 21, end: 24, x: -74, y: 12, r: 30, damage: 15, bkb: 7, kbg: 0.15, angle: 28, dir: 'away' },
      ],
    },
    nair: {
      name: 'Neutral Air', endLag: 20, aerial: true, landingLag: 9, anim: { body: 'air', swing: [[0, -40], [6, -40], [14, 320]] },
      hitboxes: [{ start: 6, end: 14, x: 0, y: 46, r: 64, damage: 10, bkb: 5, kbg: 0.09, angle: 45, dir: 'away' }],
    },
    fair: {
      name: 'Forward Air', endLag: 21, aerial: true, landingLag: 10, anim: { body: 'air', swing: [[0, -90], [7, -140], [13, 70]] },
      hitboxes: [{ start: 9, end: 13, x: 68, y: 48, r: 32, damage: 12, bkb: 6, kbg: 0.12, angle: 40 }],
    },
    bair: {
      name: 'Back Air', endLag: 22, aerial: true, landingLag: 10, anim: { body: 'air', swing: [[0, -30], [8, -170], [12, -230]] },
      hitboxes: [{ start: 8, end: 12, x: -68, y: 46, r: 32, damage: 14, bkb: 7, kbg: 0.13, angle: 40, dir: 'back' }],
    },
    uair: {
      name: 'Up Air', endLag: 18, aerial: true, landingLag: 8, anim: { body: 'air', swing: [[0, 30], [6, 10], [12, -190]] },
      hitboxes: [{ start: 6, end: 12, x: 10, y: 114, r: 38, damage: 11, bkb: 6, kbg: 0.1, angle: 85 }],
    },
    dair: {
      name: 'Down Air', endLag: 26, aerial: true, landingLag: 18, anim: { body: 'air', swing: [[0, -70], [10, 60], [12, 90]] },
      hitboxes: [{ start: 12, end: 16, x: 2, y: -16, r: 30, damage: 14, bkb: 4, kbg: 0.1, angle: -80 }],
    },
    nspec: {
      name: 'Gordo Toss', kind: 'gordo', total: 40, spawnFrame: 14, airFallCap: 3,
      anim: { windup: 'back', body: 'lean', swing: [[0, -20], [10, -140], [15, 20], [20, 30]] },
      gordo: {
        x: 56, y: 58, vx: 5.5, vy: -7, life: 160, r: 16,
        gravity: 0.4, maxFall: 12, bounce: 0.7, minBounce: 4, friction: 0.85, maxBounces: 3,
        batSpeed: 10, batVy: -5, batLife: 120, batCooldown: 12, batHitlag: 4,
        hit: { x: 0, y: 0, r: 14, damage: 11, bkb: 6, kbg: 0.1, angle: 45 },
      },
    },
    dspec: {
      name: 'Jet Hammer', endLag: 25, charge: { frame: 10, max: 90, button: 'special' },
      anim: { windup: 'back', body: 'lean', swing: [[0, -20], [10, -170], [16, -160], [20, 30]] },
      hitboxes: [{ start: 17, end: 21, x: 70, y: 44, r: 36, damage: 16, bkb: 8, kbg: 0.15, angle: 38, shieldDmgMult: 1.5 }],
    },
    uspec: {
      name: 'Super Jump', kind: 'superJump', helpless: true,
      launchFrame: 8, launchVy: 20, launchVx: 2.5, riseHitUntil: 18, plungeSpeed: 19, waveFrames: 6, recovery: 28,
      rise: { x: 10, y: 60, r: 32, damage: 8, bkb: 6, kbg: 0.07, angle: 80 },
      fall: { x: 0, y: 20, r: 30, damage: 12, bkb: 5, kbg: 0.06, angle: -70 },
      wave: [
        { x: 60, y: 14, r: 36, damage: 9, bkb: 6, kbg: 0.1, angle: 50, dir: 'away' },
        { x: -60, y: 14, r: 36, damage: 9, bkb: 6, kbg: 0.1, angle: 50, dir: 'away' },
      ],
    },
  },
};

export default king;
