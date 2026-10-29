import type Phaser from 'phaser';

/** Graphics.fillPoints accepts plain {x, y} objects at runtime; this only satisfies the typings. */
export const vec = (p: { x: number; y: number }[]) => p as unknown as Phaser.Math.Vector2[];
