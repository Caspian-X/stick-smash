// Shared scene helpers: resolution, camera framing, text.

import Phaser from 'phaser';
import { WORLD_H, WORLD_W } from '../data/stage';
import type { View } from '../render/fx';

/** Render scale: the canvas is RES× the 1280×720 world so lines stay sharp on hi-DPI screens. */
export const RES = Math.min(2, Math.max(1, window.devicePixelRatio || 1));

/** Frame the 1280×720 world in the middle; extra screen space shows more world around it. */
export function setupCamera(scene: Phaser.Scene) {
  const cam = scene.cameras.main;
  const fit = () => {
    cam.setSize(scene.scale.width, scene.scale.height);
    cam.setZoom(RES * Math.min(scene.scale.width / (WORLD_W * RES), scene.scale.height / (WORLD_H * RES)));
    cam.centerOn(WORLD_W / 2, WORLD_H / 2);
  };
  fit();
  scene.scale.on('resize', fit);
  scene.events.once('shutdown', () => scene.scale.off('resize', fit));
}

export function viewOf(scene: Phaser.Scene): View {
  const cam = scene.cameras.main, vw = cam.width / cam.zoom, vh = cam.height / cam.zoom;
  return { x0: WORLD_W / 2 - vw / 2, x1: WORLD_W / 2 + vw / 2, y0: WORLD_H / 2 - vh / 2, y1: WORLD_H / 2 + vh / 2 };
}

export type Align = 'left' | 'center' | 'right';
export function txt(scene: Phaser.Scene, x: number, y: number, s: string, size: number, color = '#fff', align: Align = 'center', weight = 900) {
  const t = scene.add.text(x, y, s, {
    fontFamily: 'system-ui, -apple-system, Segoe UI, sans-serif', fontSize: `${size}px`, fontStyle: String(weight),
    color, stroke: '#00000088', strokeThickness: Math.max(3, size / 9), align,
  });
  t.setOrigin(align === 'left' ? 0 : align === 'right' ? 1 : 0.5, 0.5);
  t.setResolution(RES * 1.5);
  return t;
}

export const hex = (n: number) => '#' + n.toString(16).padStart(6, '0');
