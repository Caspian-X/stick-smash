import Phaser from 'phaser';
import { DATA } from '../data';
import { COLORS } from '../data/colors';
import { WORLD_W } from '../data/stage';
import { playSfx } from '../game/audio';
import { consumePressed, pp } from '../game/input';
import { type Mode, S, WINS, save } from '../game/settings';
import { drawBackground, drawStage, drawStars } from '../render/stage';
import { setupCamera, txt } from './common';

const MODES: Mode[] = ['cpu', '2p', 'training'];
const MODE_NAMES: Record<Mode, string> = { cpu: '1P vs CPU', '2p': '2 Players', training: 'Training' };
const DUMMY_NAMES = { stand: 'Stand', shield: 'Shield', jump: 'Jump', cpu: 'CPU', p2: 'P2 controls' };
const cycle = (v: number, ch: number, n: number) => (v + ch + n) % n;
type Row = [string, string, ((dir: number) => void) | null];

export class MenuScene extends Phaser.Scene {
  private sel = 0;
  private stars!: Phaser.GameObjects.Graphics;
  private rowsUi: { bg: Phaser.GameObjects.Graphics; k: Phaser.GameObjects.Text; v: Phaser.GameObjects.Text }[] = [];
  private record!: Phaser.GameObjects.Text;
  private t = 0;

  constructor() { super('menu'); }

  create() {
    setupCamera(this);
    drawBackground(this.add.graphics());
    this.stars = this.add.graphics();
    drawStage(this.add.graphics());
    this.add.graphics().fillStyle(0x000000, 0.78).fillRect(-2000, -2000, 6000, 6000);
    const cx = WORLD_W / 2;
    txt(this, cx, 90, 'STICK SMASH', 88);
    txt(this, cx, 148, 'a tiny platform fighter', 20, '#aab', 'center', 500);
    this.rowsUi = [];
    for (let i = 0; i < 7; i++) {
      const y = 200 + i * 42;
      this.rowsUi.push({ bg: this.add.graphics(), k: txt(this, cx - 190, y, '', 23, '#ccd', 'left', 700), v: txt(this, cx + 190, y, '', 23, '#fff', 'right', 800) });
    }
    this.record = txt(this, cx, 500, '', 16, '#99a', 'center', 600);
    const lines: [string, string][] = [
      ['P1: A/D move · W jump · S fall/drop · J attack · K special · I grab · L shield', COLORS[0].css],
      ["P2: ←/→ move · ↑ jump · ↓ fall · . attack · / special · ; grab · ' shield   (Num 1/2/3/0 also work)", COLORS[1].css],
      ['Attack: tap a direction + attack together = SMASH (hold attack to charge) · hold a direction = tilt', '#ccd'],
      ['Shield: tap ←/→ roll · tap ↓ dodge · jump/grab out of shield · in air = air dodge · press before landing to tech', '#ccd'],
      ['Grab: attack = pummel · ←/→/↑/↓ = throw · mash to escape  ·  Ledge: toward/up climb · jump · attack · shield = roll', '#ccd'],
      ...DATA.fighters.map(f => [f.blurb, '#ccd'] as [string, string]),
      ['H: toggle hitbox/frame debug view (any mode)', '#ffd35a'],
    ];
    lines.forEach(([s, c], i) => txt(this, cx, 528 + i * 18, s, 13, c, 'center', 600));
    txt(this, cx, 706, 'W/S select · A/D change · Enter start', 13, '#778', 'center', 600);
    this.sel = Math.min(this.sel, this.rows().length - 1);
  }

  private rows(): Row[] {
    const nm = (c: number) => (DATA.fighters[c] ?? DATA.fighters[0]).name;
    const r: Row[] = [['Mode', MODE_NAMES[S.mode], d => { S.mode = MODES[cycle(MODES.indexOf(S.mode), d, MODES.length)]; }]];
    if (S.mode === 'cpu') r.push(['CPU Level', ['Easy', 'Normal', 'Hard'][S.level], d => { S.level = cycle(S.level, d, 3); }]);
    r.push(['P1 Fighter', nm(S.c1), d => { S.c1 = cycle(S.c1, d, DATA.fighters.length); }]);
    const p2 = S.mode === 'cpu' ? 'CPU Fighter' : S.mode === 'training' ? 'Dummy Fighter' : 'P2 Fighter';
    r.push([p2, nm(S.c2), d => { S.c2 = cycle(S.c2, d, DATA.fighters.length); }]);
    if (S.mode === 'training') {
      const ds = Object.keys(DUMMY_NAMES) as (keyof typeof DUMMY_NAMES)[];
      r.push(['Dummy', DUMMY_NAMES[S.dummy], d => { S.dummy = ds[cycle(ds.indexOf(S.dummy), d, ds.length)]; }]);
    } else r.push(['Stocks', String(S.stocks), d => { S.stocks = cycle(S.stocks - 1, d, 5) + 1; }]);
    r.push(['START', '', null]);
    return r;
  }

  update() {
    this.t++;
    drawStars(this.stars, this.t);
    const up = pp('KeyW', 'ArrowUp'), dn = pp('KeyS', 'ArrowDown'), lf = pp('KeyA', 'ArrowLeft'), rt = pp('KeyD', 'ArrowRight');
    const ok = pp('Enter', 'KeyJ', 'Space', 'Period');
    let rows = this.rows();
    if (up || dn) { this.sel = cycle(this.sel, up ? -1 : 1, rows.length); playSfx('menu'); }
    const row = rows[this.sel], ch = rt ? 1 : lf ? -1 : ok && row[2] ? 1 : 0;
    if (ch && row[2]) { row[2](ch); save(); playSfx('menu'); }
    if (pp('KeyR') && S.mode !== 'training') { WINS[S.mode] = [0, 0]; save(); playSfx('menu'); }
    if (pp('KeyH')) { S.debug = !S.debug; save(); playSfx('menu'); }
    const start = ok && !row[2];
    consumePressed();
    if (start) { playSfx('go'); this.scene.start('fight'); return; }

    rows = this.rows();
    this.sel = Math.min(this.sel, rows.length - 1);
    this.rowsUi.forEach((ui, i) => {
      const r = rows[i], sel = i === this.sel;
      ui.bg.clear();
      if (!r) { ui.k.setText(''); ui.v.setText(''); return; }
      const y = 200 + i * 42;
      if (sel) ui.bg.fillStyle(0xffffff, 0.09).fillRoundedRect(WORLD_W / 2 - 220, y - 20, 440, 40, 10);
      if (!r[2]) {
        ui.k.setText(sel ? `▶  ${r[0]}  ◀` : r[0]).setFontSize(30).setOrigin(0.5).setX(WORLD_W / 2).setColor(sel ? '#ffd35a' : '#fff');
        ui.v.setText('');
      } else {
        ui.k.setText(r[0]).setFontSize(23).setOrigin(0, 0.5).setX(WORLD_W / 2 - 190).setColor('#ccd');
        ui.v.setText((sel ? '◀ ' : '') + r[1] + (sel ? ' ▶' : ''));
      }
    });
    const w = S.mode === 'training' ? null : WINS[S.mode];
    this.record.setText(w ? `Record  P1 ${w[0]} – ${w[1]} ${S.mode === 'cpu' ? 'CPU' : 'P2'}   (R to reset)   ·   Debug view: ${S.debug ? 'ON' : 'off'}`
      : `Training: frame data, hitboxes, frame advantage   ·   Debug view: ${S.debug ? 'ON' : 'off'}`);
  }
}
