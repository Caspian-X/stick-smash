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

/** Ledge grab region relative to the stage corner. */
export const LEDGE = { minBelowTop: 20, maxBelowTop: 110, reachOut: 58, reachIn: 4, hangX: 12, hangY: 80 };

export function stageBottom(x: number): number {
  const k = Math.min(x - STAGE.L, STAGE.R - x) / UNDER.inset;
  return STAGE.bot + UNDER.depth * Math.max(0, Math.min(1, k));
}
