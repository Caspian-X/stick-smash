// Global tuning: every number that isn't specific to one fighter or move.
// Edit and save while the game runs (npm run dev): values hot-reload into the current match.
// Units: frames (60 per second), stage pixels, pixels/frame for speeds.

import type { PhaseHitbox } from './types';

export const GLOBAL = {
  /** Shared movement rules. Per-fighter speeds/gravity live in each fighter's `stats`. */
  movement: {
    /** Frames crouching before takeoff. Jump still held on the last one = full hop, released = short hop (fixed heights). */
    jumpsquat: 4,
    /** Air drift when no direction is held (multiplier per frame). */
    airDrag: 0.96,
    /** Air drift multiplier when a direction is held but already at max speed. */
    airDragHeld: 0.99,
    /** Fast fall allowed once vertical speed is above this (negative = still slightly rising). */
    fastFallMinVy: -3,
    /** Downward speed applied the frame you fast fall. */
    fastFallKick: 14,
    /** Air accel during specials/other attacks (aerials use aerialAccel). */
    attackAirAccel: 0.25,
    aerialAccel: 0.45,
    helplessAccel: 0.3,
    upSpecialAccel: 0.3,
    /** Ground friction multiplier during attacks; `slide` frames use attackSlideFriction. */
    attackFriction: 0.8,
    attackSlideFriction: 0.95,
    /** Air drift multiplier during attacks when not accelerating. */
    attackAirDrag: 0.98,
    /** Frames platforms are ignored after dropping through. */
    platformDrop: 10,
    /** Fighters overlapping closer than this push apart. */
    pushRadius: 26,
    pushStrength: 0.15,
    /** Ground speed needed for attack → dash attack and grab → dash grab. */
    dashAttackSpeed: 4.3,
    dashGrabSpeed: 3.9,
    /** Frames a direction press counts as a "tap" for smash attacks, rolls and spot dodges. */
    tapWindow: 5,
  },

  /** Knockback = (bkb + victimPercent × kbg × kbPercentScale) × victimWeight. */
  knockback: {
    kbPercentScale: 1.4,
    /** Knockback above this sends the victim tumbling (can tech, lands in knockdown). */
    tumbleThreshold: 13,
    /** Hitting a grounded victim downward bounces them up at this fraction. */
    groundBounce: 0.6,
    /** Smash charge: damage × (1 + charge / chargeDamageDivisor). */
    chargeDamageDivisor: 150,
    /** Fraction of the charge damage bonus that also applies to base knockback. */
    chargeKbRatio: 0.5,
  },

  /** Hitstun = floor(knockback × perKnockback) + base, then per-hitbox hitstunMult/hitstunAdd. */
  hitstun: {
    perKnockback: 1.7,
    base: 4,
    gravity: 0.6,
    maxFall: 16,
    airDrag: 0.97,
    groundDrag: 0.85,
    /** Horizontal drift you can add while in hitstun (per frame). */
    driftAccel: 0.08,
  },

  /** Hitlag (freeze frames for both fighters) = min(max, floor(base + damage × perDamage + knockback × perKnockback)). */
  hitlag: {
    base: 3,
    perDamage: 0.45,
    perKnockback: 0.1,
    max: 16,
  },

  /** Rules for each hitbox `type` (see HitboxType in types.ts). */
  hitTypes: {
    /** Grounded victim knocked prone: slide speed away from the attacker. */
    trip: { slide: 2.5 },
    /** Auto angle: grounded hits below tumble knockback use groundLow, stronger grounded hits groundHigh, airborne hits air. */
    sakurai: { groundLow: 0, groundHigh: 40, air: 45 },
    /** Victim velocity = (hitbox center − victim center) × pull + attacker velocity × momentum. */
    autolink: { pull: 0.22, momentum: 1 },
    /** Push per frame = bkb × push × victim weight, capped at maxSpeed. */
    windbox: { push: 0.6, maxSpeed: 9 },
    /** Victim hitlag multiplier. */
    electric: { victimHitlagMult: 1.6 },
  },

  shield: {
    max: 50,
    /** Shield HP lost per frame while held. */
    decay: 0.16,
    /** Shield HP regained per frame while not shielding. */
    regen: 0.1,
    /** Shield HP lost = damage × damageMult × hitbox.shieldDmgMult. */
    damageMult: 1.2,
    /** Shield stun = floor(damage × stunPerDamage + stunBase). */
    stunPerDamage: 0.8,
    stunBase: 3,
    /** Shield hitlag = floor(hitlagBase + damage × hitlagPerDamage). */
    hitlagBase: 2,
    hitlagPerDamage: 0.3,
    /** Defender slide = damage × pushPerDamage + pushBase. */
    pushPerDamage: 0.22,
    pushBase: 1.2,
    /** Attacker's own pushback on a shielded grounded hit. */
    attackerPushback: 1.5,
    /** Frames of lag after releasing shield. */
    dropLag: 7,
    breakLaunch: 15,
    breakGravity: 0.55,
    breakMaxFall: 10,
    /** Dizzy duration = max(dizzyMin, dizzyBase − percent × dizzyPerPercent). */
    dizzyBase: 480,
    dizzyMin: 240,
    dizzyPerPercent: 2,
    /** Frames removed from dizzy per button press. */
    dizzyMash: 3,
    /** Shield HP after recovering from dizzy. */
    dizzyRecoverHP: 30,
  },

  dodge: {
    rollInvuln: 31,
    rollFrames: 30,
    rollSpeed: 7,
    rollMoveStart: 3,
    rollMoveEnd: 18,
    spotInvuln: 16,
    spotFrames: 26,
    airDodgeInvuln: 28,
    airDodgeFrames: 30,
    airDodgeLandingLag: 4,
    dirAirDodgeInvuln: 24,
    dirAirDodgeFrames: 34,
    dirAirDodgeSpeed: 10,
    dirAirDodgeDecay: 0.88,
    dirAirDodgeDecayFrames: 18,
    dirAirDodgeLandingLag: 10,
  },

  landing: {
    /** Landing lag from a normal fall faster than softLandSpeed. */
    hard: 3,
    softLandSpeed: 4,
    helpless: 14,
    /** Landing during an up special after it launched. */
    upSpecial: 10,
  },

  tech: {
    /** Press shield within this many frames before landing in tumble to tech. */
    window: 12,
    invuln: 20,
    frames: 24,
  },

  knockdown: {
    /** Frames lying down before you can act. */
    minFrames: 12,
    /** Automatically stand up at this frame. */
    autoGetup: 70,
    getupFrames: 24,
    getupInvuln: 22,
    attackFrames: 32,
    attackInvuln: 14,
    attackActive: [14, 17] as [number, number],
    attack: [
      { x: 38, y: 14, r: 24, damage: 6, bkb: 6, kbg: 0.05, angle: 30, dir: 'away' },
      { x: -38, y: 14, r: 24, damage: 6, bkb: 6, kbg: 0.05, angle: 30, dir: 'away' },
    ] as PhaseHitbox[],
  },

  ledge: {
    invuln: 30,
    /** Frames hanging before you can act. */
    minHang: 8,
    /** Auto-drop after hanging this long. */
    maxHang: 300,
    regrabCooldown: 30,
    jumpSpeed: 15,
    /** Climb options: a = frames to pull up, b = frame movement ends, end = actionable, dist = slide, inv = invuln. */
    climb: { a: 14, b: 28, end: 30, dist: 26, inv: 30 },
    attack: { a: 12, b: 20, end: 40, dist: 26, inv: 24 },
    roll: { a: 10, b: 36, end: 40, dist: 120, inv: 34 },
    jump: { a: 7, b: 7, end: 7, dist: 0, inv: 8 },
    attackActive: [22, 26] as [number, number],
    attackHit: [{ x: 38, y: 30, r: 26, damage: 7, bkb: 6, kbg: 0.06, angle: 35 }] as PhaseHitbox[],
  },

  grab: {
    /** Hold time = holdBase + victimPercent × holdPerPercent. */
    holdBase: 90,
    holdPerPercent: 0.9,
    /** Hold frames removed per victim button press. */
    mashPerPress: 4,
    /** Frames before the grabber can pummel/throw. */
    minHold: 5,
    pummelFrames: 14,
    pummelHitFrame: 7,
    pummelDamage: 1.5,
    pummelHitlag: 3,
    /** Vertical tolerance for grab boxes. */
    heightTolerance: 40,
    releaseLag: 12,
  },

  respawn: {
    deadFrames: 70,
    descendFrames: 40,
    maxWait: 220,
    invuln: 100,
  },

  match: {
    countdown: 180,
    goFrames: 45,
    endFrames: 110,
  },
};

export type GlobalTuning = typeof GLOBAL;
