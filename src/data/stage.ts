// Stage geometry (world is 1280×720; y grows downward).

export interface Surface { x1: number; x2: number; y: number; main?: boolean }

export const WORLD_W = 1280;
export const WORLD_H = 720;

export const STAGE = { L: 300, R: 980, top: 520, bot: 580 };
export const GROUND: Surface = { x1: STAGE.L, x2: STAGE.R, y: STAGE.top, main: true };
export const PLATS: Surface[] = [
  { x1: 390, x2: 570, y: 400 },
  { x1: 710, x2: 890, y: 400 },
  { x1: 550, x2: 730, y: 285 },
];
export const SURFS: Surface[] = [GROUND, ...PLATS];
export const BLAST = { L: -220, R: WORLD_W + 220, T: -380, B: WORLD_H + 220 };
/** Underside: vertical walls down to STAGE.bot, then slopes inward so rising fighters slide out to the wall. */
export const UNDER = { depth: 100, inset: 60 };
export const SPAWNS = [{ x: 470, face: 1 }, { x: 810, face: -1 }];
export const RESPAWN = { x: 640, startY: -80, endY: 150 };

/**
 * Ledge grab zone: a half circle on the outside of each stage corner, centered `cy` below the corner.
 * A falling fighter grabs when their hand point (`handY` above the feet) is inside it. `reachIn` lets the
 * flat side extend a few pixels over the stage. hangX/hangY = where the feet sit while hanging.
 */
export const LEDGE = { r: 60, cy: 20, handY: 50, reachIn: 4, hangX: 12, hangY: 80 };

/** Is the point inside the ledge-grab half circle of the stage corner on `side` (-1 left, 1 right)? */
export function inLedgeZone(side: number, x: number, y: number): boolean {
  const ex = side < 0 ? STAGE.L : STAGE.R, dx = x - ex, dy = y - (STAGE.top + LEDGE.cy);
  return dx * side >= -LEDGE.reachIn && dx * dx + dy * dy <= LEDGE.r * LEDGE.r;
}

export function stageBottom(x: number): number {
  const k = Math.min(x - STAGE.L, STAGE.R - x) / UNDER.inset;
  return STAGE.bot + UNDER.depth * Math.max(0, Math.min(1, k));
}
