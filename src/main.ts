import Phaser from 'phaser';
import { initAudio } from './game/audio';
import { initKeyboard } from './game/input';
import { MenuScene } from './scenes/MenuScene';
import { FightScene } from './scenes/FightScene';
import { EditorScene } from './editor/EditorScene';
import { RES } from './scenes/common';
import { WORLD_H, WORLD_W } from './data/stage';

initKeyboard();
initAudio();

const game = new Phaser.Game({
  type: Phaser.AUTO,
  parent: 'game',
  backgroundColor: '#0d1030',
  scale: {
    mode: Phaser.Scale.EXPAND,
    autoCenter: Phaser.Scale.CENTER_BOTH,
    width: WORLD_W * RES,
    height: WORLD_H * RES,
  },
  input: { keyboard: false, gamepad: false },
  audio: { noAudio: true },
  render: { antialias: true },
  scene: [MenuScene, FightScene, EditorScene],
});

// Dev-only handle for debugging and scripted browser tests (window.game.scene.getScene('editor')…).
if (import.meta.env.DEV) Object.assign(window, { game });
