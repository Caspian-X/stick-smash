import Phaser from 'phaser';
import { DATA, frameSummary } from '../data';
import { COLORS } from '../data/colors';
import { WORLD_H, WORLD_W } from '../data/stage';
import { playSfx } from '../game/audio';
import { consumePressed, pp, readHuman, unpress } from '../game/input';
import { type DummyMode, S, WINS, save } from '../game/settings';
import type { MoveId } from '../data/types';
import { type Fighter, type Input, type World, MV, blankInput, createWorld } from '../sim/core';
import { stepWorld } from '../sim/step';
import { DebugOverlay } from '../render/debug';
import { drawFighter, drawProjectiles } from '../render/draw';
import { FxLayer } from '../render/fx';
import { Hud } from '../render/hud';
import { drawBackground, drawStage, drawStars } from '../render/stage';
import { setupCamera, txt, viewOf } from './common';

const STEP = 1000 / 60;
const SPEEDS = [1, 0.5, 0.25];
const DUMMIES: DummyMode[] = ['stand', 'shield', 'jump', 'cpu', 'p2'];
const DUMMY_NAMES: Record<DummyMode, string> = { stand: 'Stand', shield: 'Shield', jump: 'Jump', cpu: 'CPU', p2: 'P2 controls' };
const TRAINING_KEYS = ['KeyT', 'KeyY', 'KeyM', 'KeyR', 'KeyG', 'BracketLeft', 'BracketRight', 'KeyH', 'KeyE', 'Escape'];

export class FightScene extends Phaser.Scene {
  private w!: World;
  private fx = new FxLayer();
  private layer!: Phaser.GameObjects.Container;
  private stars!: Phaser.GameObjects.Graphics;
  private gWorld!: Phaser.GameObjects.Graphics;
  private debug!: DebugOverlay;
  private hud!: Hud;
  private overlay!: Phaser.GameObjects.Graphics;
  private bigText!: Phaser.GameObjects.Text;
  private subText!: Phaser.GameObjects.Text;
  private panel!: Phaser.GameObjects.Text;
  private toast!: Phaser.GameObjects.Text;
  private acc = 0;
  private inFrame = 0;
  private paused = false;
  private frozen = false;
  private speedI = 0;
  private overT = 0;
  private debugOn = false;
  private training = false;
  private dataVersion = 0;
  private toastT = 0;
  private lastMove: MoveId | undefined;

  constructor() { super('fight'); }

  create() {
    this.training = S.mode === 'training';
    this.debugOn = this.training || S.debug;
    this.paused = this.frozen = false; this.speedI = 0; this.acc = 0; this.overT = 0;
    this.dataVersion = DATA.version;
    this.fx = new FxLayer();
    setupCamera(this);
    this.layer = this.add.container(0, 0);
    const bg = this.add.graphics(); drawBackground(bg);
    this.stars = this.add.graphics();
    const stage = this.add.graphics(); drawStage(stage);
    this.gWorld = this.add.graphics();
    this.layer.add([bg, this.stars, stage, this.gWorld]);
    this.newMatch();
    this.debug = new DebugOverlay(this, this.w, this.layer);
    this.hud = new Hud(this, this.w, this.layer);
    this.overlay = this.add.graphics();
    this.bigText = txt(this, WORLD_W / 2, WORLD_H / 2 - 60, '', 120);
    this.subText = txt(this, WORLD_W / 2, WORLD_H / 2 + 20, '', 22, '#ccd', 'center', 700);
    this.panel = txt(this, 0, 0, '', 14, '#e8ecff', 'left', 600).setOrigin(0, 0).setLineSpacing(4);
    this.panel.setBackgroundColor('#00000099').setPadding(10, 8, 10, 8);
    this.toast = txt(this, WORLD_W / 2, 30, '', 16, '#9effa8', 'center', 800).setAlpha(0);
  }

  private newMatch() {
    const cpu = S.mode === 'cpu', dummyCpu = this.training && S.dummy === 'cpu';
    this.w = createWorld({
      fighters: [
        { ch: S.c1, cpu: false, name: 'P1' },
        { ch: S.c2, cpu: cpu || dummyCpu, name: cpu ? 'CPU' : this.training ? 'DUMMY' : 'P2' },
      ],
      stocks: S.stocks, cpuLevel: S.level, training: this.training, seed: (Math.random() * 2 ** 31) | 0,
    });
    this.overT = 0;
  }

  update(_time: number, delta: number) {
    this.acc += Math.min(100, delta) * SPEEDS[this.speedI];
    let steps = 0;
    while (this.acc >= STEP && steps < 6) { this.tick(); this.acc -= STEP; steps++; }
    if (steps === 6) this.acc = 0;
    this.renderFrame();
  }

  // ---------- one 60Hz tick ----------
  private tick() {
    this.inFrame++;
    if (DATA.version !== this.dataVersion) { this.dataVersion = DATA.version; this.toastT = 120; this.toast.setText('Tuning data reloaded'); }
    if (pp('KeyH')) { this.debugOn = !this.debugOn; if (!this.training) { S.debug = this.debugOn; save(); } }

    if (this.paused) {
      if (pp('Escape', 'KeyP', 'Enter')) this.paused = false;
      else if (pp('KeyQ')) { consumePressed(); this.scene.start('menu'); return; }
      consumePressed();
      return;
    }
    const w = this.w;
    if (w.over) {
      this.overT++;
      if (this.overT > 30 && pp('Enter', 'KeyJ', 'Space')) this.newMatch();
      else if (pp('Escape')) { consumePressed(); this.scene.start('menu'); return; }
      this.fx.tick();
      consumePressed();
      return;
    }
    if (pp('Escape', 'KeyP') && !w.endT) { this.paused = true; consumePressed(); return; }

    let stepNow = !this.frozen;
    if (this.training) {
      if (pp('KeyT')) this.frozen = !this.frozen;
      if (pp('KeyY') && this.frozen) stepNow = true;
      if (pp('KeyM')) this.speedI = (this.speedI + 1) % SPEEDS.length;
      if (pp('KeyR')) { this.newMatch(); unpress(...TRAINING_KEYS); return; }
      if (pp('KeyE') && import.meta.env.DEV) {
        consumePressed();
        this.scene.start('editor', { fi: w.fighters[0].ch, move: this.lastMove, from: 'fight' });
        return;
      }
      if (pp('KeyG')) {
        S.dummy = DUMMIES[(DUMMIES.indexOf(S.dummy) + 1) % DUMMIES.length]; save();
        w.fighters[1].cpu = S.dummy === 'cpu';
      }
      const dummy = w.fighters[1];
      if (pp('BracketLeft')) dummy.dmg = Math.max(0, dummy.dmg - 10);
      if (pp('BracketRight')) dummy.dmg = Math.min(999, dummy.dmg + 10);
      unpress(...TRAINING_KEYS);
      // While frozen, keep other presses buffered so they apply on the next stepped frame.
      if (!stepNow) return;
    }

    const tap = DATA.g.movement.tapWindow;
    const inputs: (Input | null)[] = w.fighters.map((f, i) => {
      if (f.cpu) return null;
      if (this.training && i === 1) return this.dummyInput(f);
      return readHuman(i, this.inFrame, tap);
    });
    const wasOver = w.over;
    stepWorld(w, inputs);
    this.fx.consume(w.fx, viewOf(this));
    this.fx.tick();
    if (!wasOver && w.over && w.winner && (S.mode === 'cpu' || S.mode === '2p')) { WINS[S.mode][w.winner.i]++; save(); }
    if (w.fighters[0].move) this.lastMove = w.fighters[0].move.id;
    consumePressed();
  }

  private dummyInput(f: Fighter): Input {
    const inp = blankInput();
    switch (S.dummy) {
      case 'shield': inp.shield = true; break;
      case 'jump': if (f.state === 'idle' && f.t > 20) { inp.jump = inp.jumpHeld = true; } break;
      case 'p2': return readHuman(1, this.inFrame, DATA.g.movement.tapWindow);
    }
    return inp;
  }

  // ---------- drawing ----------
  private renderFrame() {
    const w = this.w, v = viewOf(this), sh = this.fx.shake;
    this.layer.setPosition(sh ? (Math.random() * 2 - 1) * sh : 0, sh ? (Math.random() * 2 - 1) * sh : 0);
    drawStars(this.stars, w.frame);
    const g = this.gWorld;
    g.clear();
    drawProjectiles(g, w);
    for (const f of w.fighters) drawFighter(g, w, f, !this.debugOn);
    this.fx.draw(g);
    this.debug.draw(w, this.debugOn);
    this.hud.update(w, v);
    this.hud.hint.setText(this.training ? 'Esc: pause' : 'Esc: pause  ·  H: debug view');

    const o = this.overlay;
    o.clear();
    let big = '', bigCol = '#fff', sub = '', alpha = 1;
    if (w.count > 0) big = String(Math.ceil(w.count / 60));
    else if (w.goT > 0) { big = 'GO!'; bigCol = '#ffd35a'; alpha = Math.min(1, w.goT / 15); }
    if (w.endT > 0 && w.winner) big = 'GAME!';
    if (w.over && w.winner) {
      o.fillStyle(0x000000, 0.4).fillRect(v.x0, v.y0, v.x1 - v.x0, v.y1 - v.y0);
      big = `${w.winner.name} WINS!`; bigCol = COLORS[w.winner.i].css;
      const rec = S.mode === 'cpu' || S.mode === '2p' ? WINS[S.mode] : null;
      sub = (rec ? `Record  ${rec[0]} – ${rec[1]}\n` : '') + (this.overT > 30 ? 'Enter: rematch  ·  Esc: menu' : '');
    }
    if (this.paused) {
      o.fillStyle(0x000000, 0.5).fillRect(v.x0, v.y0, v.x1 - v.x0, v.y1 - v.y0);
      big = 'PAUSED'; bigCol = '#fff'; alpha = 1;
      sub = 'Esc / Enter: resume  ·  Q: quit to menu';
    }
    this.bigText.setText(big).setColor(bigCol).setAlpha(alpha).setY(w.over ? 150 : WORLD_H / 2 - 60);
    this.subText.setText(sub).setY(w.over ? 235 : WORLD_H / 2 + 40);

    this.panel.setVisible(this.training);
    if (this.training) this.panel.setText(this.trainingText()).setPosition(v.x0 + 14, v.y0 + 14);
    if (this.toastT > 0) { this.toastT--; this.toast.setAlpha(Math.min(1, this.toastT / 30)).setY(v.y0 + 24); } else this.toast.setAlpha(0);
  }

  private trainingText(): string {
    const w = this.w, p1 = w.fighters[0], tr = w.trace;
    const lines = [`TRAINING  ·  speed ${SPEEDS[this.speedI]}x${this.frozen ? '  ·  FROZEN (Y = step)' : ''}`];
    if (p1.state === 'attack' && p1.move) {
      const d = MV(p1);
      lines.push(`P1 ${d.name ?? p1.move.id}: frame ${p1.move.t}  (${frameSummary(d)})`);
    } else lines.push(`P1 ${p1.state}`);
    if (tr) {
      const adv = tr.done ? tr.dFree - tr.aFree : null;
      const advS = adv === null ? '…' : (adv > 0 ? '+' : '') + adv;
      lines.push(`Last: ${tr.move} ${tr.onShield ? 'on shield' : 'on hit'}  ·  advantage ${advS}`);
      if (tr.combo > 1) lines.push(`Combo: ${tr.combo} hits  ·  ${Math.round(tr.comboDmg)}%`);
    }
    lines.push(`Dummy: ${DUMMY_NAMES[S.dummy]} (G)  ·  ${Math.floor(w.fighters[1].dmg)}% ([ / ])`);
    lines.push('T freeze · Y step · M speed · R reset · H hitboxes' + (import.meta.env.DEV ? ' · E edit hitboxes' : ''));
    return lines.join('\n');
  }
}
