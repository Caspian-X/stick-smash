// Shapes of all balance data. Frame numbers are 1-based move frames at 60fps.
// Coordinates are in stage pixels relative to the fighter's feet: x = forward, y = up.

export type MoveId =
  | 'jab' | 'ftilt' | 'utilt' | 'dtilt' | 'dashatk'
  | 'fsmash' | 'usmash' | 'dsmash'
  | 'nair' | 'fair' | 'bair' | 'uair' | 'dair'
  | 'grab' | 'dashgrab'
  | 'nspec' | 'uspec' | 'dspec';

export interface Hitbox {
  /** First active frame. */
  start: number;
  /** Last active frame (inclusive). */
  end: number;
  /** Offset forward from the feet. Negative = behind. */
  x: number;
  /** Offset up from the feet. */
  y: number;
  /** Radius. */
  r: number;
  /** Percent dealt. */
  damage: number;
  /** Base knockback. */
  bkb: number;
  /** Knockback growth: knockback added per victim percent. */
  kbg: number;
  /** Launch angle in degrees. 0 = forward, 90 = straight up, negative = down (spike). */
  angle: number;
  /** 'away' launches away from the attacker's center (multi-direction hits); 'back' launches backward. */
  dir?: 'away' | 'back';
  /** Multiplies the hitstun this box causes (default 1). */
  hitstunMult?: number;
  /** Flat frames added to hitstun (default 0). */
  hitstunAdd?: number;
  /** Multiplies hitlag (freeze frames) on hit (default 1). */
  hitlagMult?: number;
  /** Multiplies shield damage (default 1). */
  shieldDmgMult?: number;
  /** Multiplies shield stun (default 1). */
  shieldStunMult?: number;
  /** Grab box: grabs instead of hitting. */
  grab?: boolean;
}

/** Hitbox for phases that aren't frame-windowed (projectiles, shockwaves, throws). start/end are ignored. */
export type PhaseHitbox = Omit<Hitbox, 'start' | 'end'> & { start?: number; end?: number };

export interface ChargeData {
  /** Move frame that freezes while the button is held. */
  frame: number;
  /** Max frames of charge (default 60). */
  max?: number;
  /** Which button charges it. */
  button?: 'attack' | 'special';
}

/** Weapon animation: angle keyframes [frame, degrees] (0 forward, -90 up, 90 down) and body poses. */
export interface WeaponAnim {
  swing: [number, number][];
  /** Body pose during the move. */
  body: BodyPose;
  /** Body pose before the first hitbox (wind-up). */
  windup?: BodyPose;
}
export type BodyPose = 'stand' | 'lean' | 'back' | 'crouch' | 'stretch' | 'lunge' | 'air' | 'rise' | 'guard';

interface MoveBase {
  /** Name shown in the debug overlay. */
  name?: string;
  /** Frames on the ground with reduced friction (dash attacks, dash grab). */
  slide?: number;
  /** Aerial: can fast-fall, gets air drift, and suffers landingLag if it lands mid-move. */
  aerial?: boolean;
  landingLag?: number;
  charge?: ChargeData;
  /** Ground speed burst at the first active frame (smash lunges). */
  forwardStep?: number;
  /** Ends in helpless (special fall). */
  helpless?: boolean;
  /** Visual/sfx: ground thud at this frame. */
  quakeFrame?: number;
  anim?: WeaponAnim;
}

/** Ordinary move: hitboxes on frame windows. Total length = last active frame + endLag. */
export interface StandardMove extends MoveBase {
  kind?: undefined;
  hitboxes: Hitbox[];
  /** Frames after the last active frame before the fighter can act. */
  endLag: number;
  /** Up-special style launch. */
  rise?: { frame: number; vy: number; vx: number };
  /** Lunge burst. */
  dash?: { frame: number; groundSpeed: number; airSpeed: number };
}

export interface ProjectileData {
  /** Spawn offset from feet. */
  x: number; y: number;
  speed: number;
  life: number;
  r: number;
  /** Max alive per owner. */
  max: number;
  hit: PhaseHitbox;
}

export interface ProjectileMove extends MoveBase {
  kind: 'projectile';
  total: number;
  spawnFrame: number;
  /** While airborne, fall speed is capped at this during the move. */
  airFallCap: number;
  projectile: ProjectileData;
}

export interface GordoMove extends MoveBase {
  kind: 'gordo';
  total: number;
  spawnFrame: number;
  airFallCap: number;
  gordo: {
    x: number; y: number; vx: number; vy: number; life: number; r: number;
    gravity: number; maxFall: number; bounce: number; minBounce: number; friction: number;
    /** After this many bounces it fizzles. */
    maxBounces: number;
    /** When batted by any attack. */
    batSpeed: number; batVy: number; batLife: number; batCooldown: number; batHitlag: number;
    hit: PhaseHitbox;
  };
}

export interface GroundPoundMove extends MoveBase {
  kind: 'groundPound';
  /** Hover frames before plunging (air) or slamming (ground). */
  startup: number;
  fallSpeed: number;
  /** If it hasn't landed by this frame, it cancels. */
  timeout: number;
  /** Shockwave active frames after landing. */
  waveFrames: number;
  /** Frames after landing until actionable. */
  recovery: number;
  fall: PhaseHitbox;
  wave: PhaseHitbox[];
}

export interface CounterMove extends MoveBase {
  kind: 'counter';
  total: number;
  /** Frames during which a hit is countered. */
  window: [number, number];
  /** Counter-attack active frames (counted from the counter trigger). */
  strike: [number, number];
  /** Frames from trigger until actionable. */
  recovery: number;
  /** Damage = max(minDamage, incoming × damageMult). */
  damageMult: number;
  minDamage: number;
  /** Freeze frames for the countered attacker. */
  attackerFreeze: number;
  invuln: number;
  hit: PhaseHitbox;
  /** Reflected projectile speed multiplier. */
  reflectSpeed: number;
  /** Weapon swing keyframes for the counter-attack (frames counted from the trigger). */
  strikeSwing?: [number, number][];
}

export interface SuperJumpMove extends MoveBase {
  kind: 'superJump';
  launchFrame: number;
  launchVy: number;
  launchVx: number;
  /** Rise hitbox active until this move frame. */
  riseHitUntil: number;
  plungeSpeed: number;
  waveFrames: number;
  recovery: number;
  rise: PhaseHitbox;
  fall: PhaseHitbox;
  wave: PhaseHitbox[];
}

export type MoveData = StandardMove | ProjectileMove | GordoMove | GroundPoundMove | CounterMove | SuperJumpMove;

export interface ThrowData {
  /** Frame the victim is released and hit. */
  release: number;
  total: number;
  hit: PhaseHitbox;
}

export interface FighterStats {
  /** Knockback taken multiplier. Lower = heavier. */
  weight: number;
  runSpeed: number;
  runAccel: number;
  groundFriction: number;
  /** Full hop launch speed (positive number, upward). */
  jumpSpeed: number;
  shortHopSpeed: number;
  airJumpSpeed: number;
  airJumpDrift: number;
  /** Mid-air jumps. */
  airJumps: number;
  airSpeed: number;
  airAccel: number;
  gravity: number;
  fallSpeed: number;
  fastFallSpeed: number;
}

export interface FighterData {
  id: string;
  name: string;
  weapon?: 'sword' | 'hammer';
  /** Cosmetic: sword hair-tie, king crown. */
  look?: 'sword' | 'king';
  stats: FighterStats;
  moves: Record<MoveId, MoveData>;
  throws: Record<'f' | 'b' | 'u' | 'd', ThrowData>;
  /** How close the CPU wants to be to attack. */
  aiReach: number;
  /** Menu help line. */
  blurb: string;
}
