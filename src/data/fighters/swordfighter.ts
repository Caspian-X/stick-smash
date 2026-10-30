// Swordfighter: longer reach, slower smashes. Forward smash's first box is the sweet-spot tip.
// See brawler.ts for field meanings. `anim.swing` = sword angle keyframes [frame, degrees] (0 forward, -90 up, 90 down).

import type { FighterData } from '../types';
import { BASE_STATS, COMMON_GRABS, COMMON_THROWS } from './common';

const swordfighter: FighterData = {
  id: 'swordfighter',
  name: 'Swordfighter',
  weapon: 'sword',
  look: 'sword',
  blurb: 'Swordfighter: longer reach, forward smash tip hits hardest · special: hold for Flare Blade · → lunge stab · ↑ rising slash · ↓ counter',
  aiReach: 95,
  stats: { ...BASE_STATS },
  throws: COMMON_THROWS,
  moves: {
    ...COMMON_GRABS,
    jab: {
      name: 'Jab', endLag: 12, anim: { body: 'stand', swing: [[0, 40], [3, -15], [6, 10]] },
      hitboxes: [{ start: 3, end: 6, x: 58, y: 50, r: 22, damage: 4, bkb: 4, kbg: 0.05, angle: 80 }],
    },
    ftilt: {
      name: 'Forward Tilt', endLag: 19, anim: { body: 'lean', swing: [[0, 30], [4, -60], [7, -60], [11, 40]] },
      hitboxes: [{ start: 7, end: 11, x: 64, y: 54, r: 30, damage: 10, bkb: 6, kbg: 0.12, angle: 38 }],
    },
    utilt: {
      name: 'Up Tilt', endLag: 14, anim: { body: 'stand', swing: [[0, 40], [5, 10], [6, 0], [12, -180]] },
      hitboxes: [
        { start: 7, end: 12, x: 43, y: 65, r: 17, damage: 8, bkb: 6, kbg: 0.1, angle: 117, frames: [[7, 43, 65, 17], [8, 34, 85, 17], [9, 14, 97, 17], [10, -11, 93, 17], [11, -37, 78, 17], [12, -47, 58, 17]] },
        { start: 7, end: 12, x: 65, y: 74, r: 16, damage: 8, bkb: 6, kbg: 0.1, angle: 108, frames: [[7, 65, 74, 16], [8, 46, 106, 16], [9, 12, 118, 16], [10, -26, 117, 16], [11, -56, 93, 16], [12, -74, 65, 16]] },
      ],
    },
    dtilt: {
      name: 'Down Tilt', endLag: 14, anim: { body: 'crouch', swing: [[0, 60], [4, 40], [5, 15], [8, 10]] },
      hitboxes: [
        { start: 5, end: 14, x: 72, y: 17, r: 15, damage: 7, bkb: 4, kbg: 0.07, angle: -15, type: 'trip', rehit: 4 },
        { start: 5, end: 14, x: 45, y: 25, r: 14, damage: 6, bkb: 5, kbg: 0.08, angle: -9 },
      ],
    },
    dashatk: {
      name: 'Dash Attack', endLag: 22, slide: 14, anim: { body: 'lean', swing: [[0, -30], [6, -30], [14, 30]] },
      hitboxes: [{ start: 6, end: 14, x: 66, y: 48, r: 30, damage: 10, bkb: 6, kbg: 0.09, angle: 50 }],
    },
    fsmash: {
      name: 'Forward Smash', endLag: 37, charge: { frame: 8 }, forwardStep: 4,
      anim: { windup: 'back', body: 'lean', swing: [[0, 0], [8, -130], [14, -110], [17, 50]] },
      hitboxes: [
        { start: 14, end: 17, x: 86, y: 40, r: 20, damage: 19, bkb: 8, kbg: 0.18, angle: 36 },
        { start: 14, end: 17, x: 50, y: 58, r: 28, damage: 13, bkb: 6, kbg: 0.14, angle: 40 },
      ],
    },
    usmash: {
      name: 'Up Smash', endLag: 29, charge: { frame: 6 },
      anim: { windup: 'crouch', body: 'stretch', swing: [[0, 40], [6, 30], [12, 20], [16, -95]] },
      hitboxes: [
        { start: 12, end: 17, x: 4, y: 128, r: 24, damage: 16, bkb: 8, kbg: 0.165, angle: 88 },
        { start: 12, end: 17, x: 30, y: 80, r: 30, damage: 13, bkb: 7, kbg: 0.15, angle: 85 },
      ],
    },
    dsmash: {
      name: 'Down Smash', endLag: 25, charge: { frame: 6 },
      anim: { body: 'crouch', swing: [[0, 60], [6, -20], [10, -10], [13, 35], [17, 35], [18, -180], [21, -215]] },
      hitboxes: [
        { start: 10, end: 13, x: 64, y: 12, r: 26, damage: 12, bkb: 6, kbg: 0.14, angle: 25, dir: 'away' },
        { start: 18, end: 21, x: -64, y: 12, r: 26, damage: 14, bkb: 7, kbg: 0.16, angle: 25, dir: 'away' },
      ],
    },
    nair: {
      name: 'Neutral Air', endLag: 18, aerial: true, landingLag: 7, anim: { body: 'air', swing: [[0, -40], [5, -40], [12, 320]] },
      hitboxes: [{ start: 5, end: 12, x: 0, y: 46, r: 60, damage: 7, bkb: 4, kbg: 0.09, angle: 45, dir: 'away' }],
    },
    fair: {
      name: 'Forward Air', endLag: 16, aerial: true, landingLag: 8, anim: { body: 'air', swing: [[0, 0], [6, -110], [10, 60]] },
      hitboxes: [{ start: 6, end: 10, x: 60, y: 56, r: 30, damage: 9, bkb: 5, kbg: 0.11, angle: 42 }],
    },
    bair: {
      name: 'Back Air', endLag: 17, aerial: true, landingLag: 9, anim: { body: 'air', swing: [[0, -30], [7, -150], [11, -220]] },
      hitboxes: [{ start: 7, end: 11, x: -62, y: 48, r: 30, damage: 12, bkb: 6, kbg: 0.13, angle: 40, dir: 'back' }],
    },
    uair: {
      name: 'Up Air', endLag: 14, aerial: true, landingLag: 6, anim: { body: 'air', swing: [[0, 30], [5, 10], [10, -190]] },
      hitboxes: [{ start: 5, end: 10, x: 6, y: 116, r: 34, damage: 9, bkb: 5, kbg: 0.1, angle: 85 }],
    },
    dair: {
      name: 'Down Air', endLag: 20, aerial: true, landingLag: 14, anim: { body: 'air', swing: [[0, -70], [8, 60], [10, 90]] },
      hitboxes: [{ start: 10, end: 14, x: 2, y: -18, r: 24, damage: 12, bkb: 4, kbg: 0.1, angle: -90, type: 'spike' }],
    },
    nspec: {
      name: 'Flare Blade', endLag: 26, charge: { frame: 10, max: 60, button: 'special' },
      anim: { windup: 'back', body: 'lean', swing: [[0, 20], [10, -160], [16, -150], [20, 40]] },
      hitboxes: [
        { start: 17, end: 20, x: 70, y: 50, r: 30, damage: 13, bkb: 7, kbg: 0.15, angle: 38 },
        { start: 17, end: 20, x: 32, y: 58, r: 24, damage: 10, bkb: 6, kbg: 0.12, angle: 42 },
      ],
    },
    sspec: {
      name: 'Lunge', endLag: 23, slide: 18, dash: { frame: 9, groundSpeed: 12, airSpeed: 8 },
      anim: { windup: 'back', body: 'lunge', swing: [[0, 0], [8, -15], [9, 0]] },
      hitboxes: [{ start: 9, end: 15, x: 66, y: 46, r: 22, damage: 11, bkb: 7, kbg: 0.11, angle: 30, shieldDmgMult: 2.5 }],
    },
    uspec: {
      name: 'Rising Slash', endLag: 20, helpless: true, rise: { frame: 4, vy: 19, vx: 2.8 },
      anim: { windup: 'crouch', body: 'rise', swing: [[0, 60], [4, 20], [10, -90]] },
      hitboxes: [
        { start: 4, end: 8, x: 50, y: 64, r: 30, damage: 11, bkb: 7, kbg: 0.1, angle: 80 },
        { start: 9, end: 18, x: 6, y: 104, r: 28, damage: 5, bkb: 5, kbg: 0.07, angle: 85 },
      ],
    },
    dspec: {
      name: 'Counter', kind: 'counter', total: 44, window: [4, 24], strike: [3, 7], recovery: 26,
      damageMult: 1.3, minDamage: 8, attackerFreeze: 14, invuln: 14, reflectSpeed: 1.25,
      anim: { body: 'guard', swing: [[0, -40], [4, -80]] },
      strikeSwing: [[0, -120], [3, -120], [7, 40]],
      hit: { x: 52, y: 48, r: 38, damage: 0, bkb: 7, kbg: 0.14, angle: 38 },
    },
  },
};

export default swordfighter;
