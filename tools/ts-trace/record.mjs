// Node 26 imports the read-only sibling TS sim using native type stripping.
import { register } from 'node:module';
import { mkdir, readFile, writeFile } from 'node:fs/promises';
import assert from 'node:assert/strict';
register(`data:text/javascript,${encodeURIComponent(`
  export async function resolve(specifier, context, next) {
    try { return await next(specifier, context); }
    catch (original) {
      if (!specifier.startsWith('.')) throw original;
      for (const suffix of ['.ts', '/index.ts']) {
        try { return await next(specifier + suffix, context); } catch {}
      }
      throw original;
    }
  }
`)}`, import.meta.url);
const source = new URL('../../../stick-smash/src/', import.meta.url);
const [{ DATA }, { createWorld, blankInput }, { stepWorld }, { STAGE, PLATS }] = await Promise.all([
  import(new URL('data/index.ts', source)), import(new URL('sim/core.ts', source)),
  import(new URL('sim/step.ts', source)), import(new URL('data/stage.ts', source)),
]);
const output = new URL('../../crates/sim/tests/fixtures/ts/', import.meta.url);
const ids = ['brawler', 'swordfighter', 'king'];
assert.deepEqual(DATA.fighters.map(f => f.id), ids);
const Z = [0, 0, 0], J = [0, 0, 1], A = [0, 0, 2], S = [0, 0, 4], SH = [0, 0, 16];
const raw = (x = 0, y = 0, b = 0) => [x * 127, y * 127, b];
const air = (x, y, extra = {}) => ({ player: 0, x, y, ground: false, surf: null, state: 'air', ...extra });

// History belongs to the recorder, never to the mutable TS world or its RNG.
function history() { return { held: {}, taps: { l: -99, r: -99, u: -99, d: -99 } }; }
function derive([sx, sy, buttons], h, tick, tapJump) {
  assert(Number.isInteger(sx) && Math.abs(sx) <= 127 && Number.isInteger(sy) && Math.abs(sy) <= 127);
  assert(Number.isInteger(buttons) && buttons >= 0 && buttons < 32);
  const held = { l: sx <= -64, r: sx >= 64, u: sy >= 64, d: sy <= -64,
    j: !!(buttons & 1), a: !!(buttons & 2), s: !!(buttons & 4), g: !!(buttons & 8), sh: !!(buttons & 16) };
  const p = Object.fromEntries(Object.entries(held).map(([k, v]) => [k, v && !h.held[k]]));
  for (const k of ['l', 'r', 'u', 'd']) if (p[k]) h.taps[k] = tick;
  const tap = k => held[k] && tick - h.taps[k] <= DATA.g.movement.tapWindow;
  const inp = { ...blankInput(), left: held.l, right: held.r, up: held.u, down: held.d,
    jump: p.j || (tapJump && p.u), jumpHeld: held.j || (tapJump && held.u),
    atk: p.a, atkHeld: held.a, sp: p.s, spHeld: held.s, grab: p.g,
    shield: held.sh, shieldP: p.sh, downP: p.d,
    tapL: tap('l'), tapR: tap('r'), tapU: tap('u'), tapD: tap('d'), anyP: Object.values(p).some(Boolean) };
  h.held = held;
  return inp;
}
// Boundary/edge checks independent of scenario outcomes.
{
  const h = history();
  assert.equal(derive([63, 63, 0], h, 1, false).anyP, false);
  assert.equal(derive([64, 64, 2], h, 2, false).tapU, true);
  const held = derive([64, 64, 2], h, 7, false);
  assert(held.tapR && held.atkHeld && !held.atk && !held.jump);
  assert.equal(derive([64, 64, 2], h, 8, false).tapR, false);
  assert(derive([-64, -64, 0], h, 9, false).downP);
  assert(derive([0, 64, 0], history(), 1, true).jump);
}
class Trace {
  constructor(name, ch = 0, setup = [], opts = {}) {
    this.config = { stage: 'arena', seed: 12345, stocks: 3, training: true, cpuLevel: 2,
      players: [{ fighter: ids[ch], cpu: false, tapJump: false }, { fighter: 'king', cpu: false, tapJump: true }],
      ...opts, ...(setup.length ? { setup } : {}) };
    this.name = name;
    const c = this.config;
    assert.equal(c.players.length, 2);
    this.w = createWorld({ ...c, fighters: c.players.map((p, i) => ({ ch: ids.indexOf(p.fighter), cpu: p.cpu, name: `P${i + 1}` })) });
    for (const { player, ...fields } of setup) {
      assert(Object.keys(fields).every(k => ['x', 'y', 'vx', 'vy', 'face', 'dmg', 'state', 'ground', 'surf'].includes(k)));
      Object.assign(this.w.fighters[player], fields);
    }
    this.h = [history(), history()]; this.inputs = []; this.frames = [];
    this.states = new Set(); this.moves = new Set(); this.events = new Set(); this.details = new Set();
  }
  get f() { return this.w.fighters[0]; }
  tick(a = Z, b = Z) {
    if (this.w.over) return;
    const inputs = [a, b].map(v => [...v]);
    const ins = inputs.map((v, i) => this.config.players[i].cpu ? null : derive(v, this.h[i], this.w.frame + 1, this.config.players[i].tapJump));
    stepWorld(this.w, ins);
    this.inputs.push(inputs);
    this.frames.push({ rng: this.w.rng >>> 0,
      f: this.w.fighters.map(f => [f.x, f.y, f.vx, f.vy, f.state, f.dmg, f.stocks, f.lag, f.hitstun, f.face, f.move?.id ?? null, f.move?.charge ?? 0]),
      p: this.w.projs.map(p => [p.o.i, p.x, p.y, p.vx, p.vy, p.life]) });
    for (const f of this.w.fighters) {
      this.states.add(f.state); if (f.move) { this.moves.add(f.move.id); this.details.add(`${f.move.id}:phase${f.move.ph}`); }
      if (f.ff) this.details.add('fastFall');
      if (f.drop) this.details.add('platformDrop');
      if (f.move?.charge > 25) this.details.add('charged');
      if (f.state === 'climb' || f.state === 'getup' || f.state === 'throw') this.details.add(`${f.state}:${f.sd.kind}`);
    }
    for (const p of this.w.projs) { if (p.bn) this.details.add('gordoBounce'); if (p.gordo && p.o.i === 1) this.details.add('gordoBatBack'); if (!p.gordo && p.o.i === 0) this.details.add('energyOwner0'); }
    for (const fx of this.w.fx) if (fx.t === 'sfx') this.events.add(fx.name);
    assert(this.frames.at(-1).f.flat().concat(this.frames.at(-1).p.flat()).every(v => typeof v !== 'number' || Number.isFinite(v)), `${this.name}: non-finite frame`);
    this.w.fx.length = 0;
  }
  run(n, a = Z, b = Z) { for (let i = 0; i < n && !this.w.over; i++) this.tick(a, b); return this; }
  until(pred, limit = 240, input = () => [Z, Z]) {
    for (let i = 0; i < limit && !pred() && !this.w.over; i++) this.tick(...input(i));
    assert(pred(), `${this.name}: condition timed out at ${this.w.frame} (${this.f.state}, ${this.f.x}, ${this.f.y})`);
    return this;
  }
  move(id, input, hold = 1) {
    this.tick(input); assert.equal(this.f.move?.id, id, `${this.name} frame ${this.w.frame}`);
    this.run(hold - 1, input); return this;
  }
  ready() { this.until(() => this.f.state === 'idle'); this.run(1); return this; }
  need(kind, ...values) { for (const v of values) assert(this[kind].has(v), `${this.name}: missing ${kind} ${v}`); return this; }
  encode() {
    return `{"name":${JSON.stringify(this.name)},"config":${JSON.stringify(this.config)},"inputs":${JSON.stringify(this.inputs)},"frames":[\n${this.frames.map(f => JSON.stringify(f)).join(',\n')}\n]}\n`;
  }
}
const fixtures = [];
const make = (...args) => { const t = new Trace(...args); fixtures.push(t); return t; };
const far = [{ player: 1, x: 920 }], adjacent = [{ player: 1, x: 510 }];
function build() {
  fixtures.length = 0;
  for (let ch = 0; ch < 3; ch++) {
    const id = ids[ch];
    const t = make(`${id}-ground`, ch, adjacent);
    t.run(1).move('jab', A).run(30).ready();
    // Age direction taps during shield and shield-drop lag, without building run speed.
    for (const [move, x, y] of [['ftilt', 1, 0], ['utilt', 0, 1], ['dtilt', 0, -1]]) {
      t.run(8, SH).run(1).run(7, raw(x, y)).move(move, raw(x, y, 2)).run(30).ready();
    }
    for (const [move, x, y] of [['fsmash', -1, 0], ['usmash', 0, 1], ['dsmash', 0, -1]]) {
      t.move(move, raw(x, y, 2)).run(30).ready();
    }
    t.move('fsmash', raw(1, 0, 2), 48).run(30).ready();
    t.run(15, raw(1)).move('dashatk', raw(1, 0, 2)).run(90);
    t.need('moves', 'jab', 'ftilt', 'utilt', 'dtilt', 'fsmash', 'usmash', 'dsmash', 'dashatk').need('details', 'charged').need('events', 'hit');
    const a = make(`${id}-aerial-movement`, ch, far);
    for (const [move, dx, dy] of [['nair', 0, 0], ['fair', 1, 0], ['bair', -1, 0], ['uair', 0, 1], ['dair', 0, -1]]) {
      a.ready().run(7, J).move(move, raw(dx * a.f.face, dy, 2)).run(35);
    }
    a.ready().run(1, J).run(35); // short hop
    a.ready().run(9, J).run(35); // full hop
    a.ready().run(9, J).until(() => a.f.vy > 0).run(1, raw(0, -1)).run(35);
    for (const dx of [0, -1]) a.ready().run(9, J).run(1, raw(dx, 0, 16)).run(35);
    a.ready().run(2, SH).run(1, raw(1, 0, 16)).run(35);
    a.ready().run(2, SH).run(1, raw(-1, 0, 16)).run(35);
    a.ready().run(2, SH).run(1, raw(0, -1, 16)).run(35);
    // Jump naturally onto a platform, then drop through it.
    a.ready().until(() => Math.abs(a.f.x - 480) < 6, 250, () => [raw(Math.sign(480 - a.f.x)), Z]);
    a.run(15).run(8, J).run(30).ready().run(1, raw(0, -1)).run(55);
    a.need('moves', 'nair', 'fair', 'bair', 'uair', 'dair').need('states', 'roll', 'spot', 'airdodge').need('details', 'fastFall', 'platformDrop');
    const s = make(`${id}-specials`, ch, adjacent);
    s.move('nspec', S).run(30).ready();
    if (ch === 1) s.move('sspec', raw(1, 0, 4)).run(30).ready();
    s.move('dspec', raw(0, -1, 4), ch === 2 ? 70 : 1).run(120).ready();
    s.move('uspec', raw(0, 1, 4)).run(180);
    s.need('moves', 'nspec', 'dspec', 'uspec');
    if (ch === 0) s.need('details', 'dspec:phase2').need('states', 'landing');
    if (ch === 2) s.need('details', 'uspec:phase3', 'charged');
    for (const [dir, x, y] of [['f', 1, 0], ['b', -1, 0], ['u', 0, 1], ['d', 0, -1]]) {
      const g = make(`${id}-throw-${dir}`, ch, adjacent);
      g.move('grab', raw(0, 0, 8)).until(() => g.f.state === 'hold').run(6).run(1, A).run(22).run(1, raw(x, y)).run(110);
      g.need('events', 'grab', 'pummel', 'throw').need('details', `throw:${dir}`);
    }
    const sh = make(`${id}-shield-hit`, ch, adjacent);
    sh.run(2, Z, SH).run(1, A, SH).run(50, Z, SH).need('events', 'shieldHit');
  }
  for (const [option, input] of [['climb', raw(-1)], ['attack', A], ['roll', SH], ['jump', J], ['drop', raw(0, -1)]]) {
    const t = make(`ledge-${option}`, 0, [air(STAGE.R + 30, STAGE.top - 20, { vy: 2 }), ...far]);
    t.until(() => t.f.state === 'ledge').run(9).run(1, input).run(110).need('states', 'ledge');
    if (option !== 'drop') t.need('details', `climb:${option}`);
  }
  const walk = make('walk-off-ledge', 0, [{ player: 0, x: 950 }, ...far]);
  walk.until(() => !walk.f.ground, 60, () => [raw(1), Z]).run(12, raw(-1)).run(100).need('states', 'ledge');
  const ledge = make('brawler-uspec-ledge', 0, [air(1010, 650, { face: -1 }), ...far]);
  ledge.move('uspec', raw(-1, 1, 4)).run(120).need('states', 'ledge');
  const pound = make('brawler-pound-air', 0, far);
  pound.run(10, J).move('dspec', raw(0, -1, 4)).run(130).need('details', 'dspec:phase1', 'dspec:phase2');
  const timeout = make('brawler-pound-blast', 0, [air(1200, -300), ...far]);
  timeout.move('dspec', raw(0, -1, 4)).until(() => !timeout.f.move);
  assert.equal(timeout.f.state, 'air', 'ground pound must time out before KO'); timeout.run(120).need('states', 'dead');
  const cancel = make('king-plunge-cancel', 2, far);
  cancel.move('uspec', raw(0, 1, 4)).until(() => cancel.f.move?.ph === 2).run(1, SH);
  assert.equal(cancel.f.state, 'helpless'); cancel.run(120);
  const bounce = make('king-gordo-bounce', 2, [{ player: 0, x: 340 }, { player: 1, x: 950 }]);
  bounce.move('nspec', S).run(150).need('details', 'gordoBounce');
  const bat = make('king-gordo-bat', 2, [{ player: 0, x: 340 }, { player: 1, x: 620 }], { players: [{ fighter: 'king', cpu: false, tapJump: false }, { fighter: 'swordfighter', cpu: false, tapJump: true }] });
  bat.move('nspec', S).run(25).run(1, Z, raw(-1, 0, 2)).run(125).need('details', 'gordoBatBack');
  const melee = make('sword-counter-melee', 1, adjacent);
  melee.move('dspec', raw(0, -1, 4)).run(4).run(1, Z, A).run(100).need('events', 'counter');
  const reflect = make('sword-counter-reflect', 1, [{ player: 1, x: 650 }], { players: [{ fighter: 'swordfighter', cpu: false, tapJump: false }, { fighter: 'brawler', cpu: false, tapJump: true }] });
  reflect.run(1, Z, S).run(10).move('dspec', raw(0, -1, 4)).run(100).need('events', 'counter').need('details', 'energyOwner0');
  const dizzy = make('shield-break-dizzy', 0, far);
  dizzy.run(390, SH).run(250).need('states', 'shieldbreak', 'dizzy');
  for (const option of ['tech', 'tech-roll', 'stand', 'attack', 'roll', 'auto']) {
    // A real aerial spike produces tumble/knockdown; no synthetic hitstun/scratch state.
    const t = make(`knockdown-${option}`, 0, [{ player: 0, dmg: 120, x: 640 }, { ...air(640, 330), player: 1 }], { players: [{ fighter: 'brawler', cpu: false, tapJump: false }, { fighter: 'brawler', cpu: false, tapJump: true }] });
    t.run(1, Z, raw(0, -1, 2));
    if (option.startsWith('tech')) {
      t.until(() => t.f.state === 'hitstun' && t.f.vy > 0 && [...PLATS, { x1: 300, x2: 980, y: 520 }].some(p => t.f.x >= p.x1 && t.f.x <= p.x2 && p.y > t.f.y && p.y - t.f.y < 50)).run(1, option === 'tech' ? SH : raw(1, 0, 16)).run(100).need('events', 'tech');
    } else {
      t.until(() => t.f.state === 'knockdown').run(14);
      const input = { stand: J, attack: A, roll: raw(1), auto: Z }[option];
      t.run(1, input).run(100).need('states', 'knockdown');
      if (option === 'attack') t.need('details', 'getup:atk');
      if (option === 'stand' || option === 'auto') t.need('details', 'getup:stand');
      if (option === 'roll') t.need('states', 'roll');
    }
  }
  for (const training of [true, false]) {
    const t = make(`ko-respawn-${training ? 'training' : 'stocks'}`, 0, [], { training });
    t.run(1100, raw(-1)).need('states', 'dead', 'respawn');
    if (!training) { assert(t.w.over, 'stock fixture must reach w.over'); assert(t.f.stocks === 0); }
  }
  // Press edges during countdown must be consumed before GO, not buffered into it.
  make('countdown-held-input', 0, [], { training: false }).run(190, raw(1, 1, 3)).run(10).run(1, A).run(80);
  for (let k = 0; k < 3; k++) {
    const t = make(`fuzz-${k + 1}`, k % 3, [], { seed: 6100 + k, training: false });
    let rng = 0x12340000 + k;
    const next = n => { rng ^= rng << 13; rng ^= rng >>> 17; rng ^= rng << 5; return (rng >>> 0) % n; };
    const held = [Z, Z], left = [0, 0];
    for (let tick = 0; tick < 1500 && !t.w.over; tick++) {
      for (let p = 0; p < 2; p++) if (--left[p] <= 0) {
        held[p] = raw(next(3) - 1, next(3) - 1, next(32)); left[p] = 1 + next(24);
      }
      t.tick(...held);
    }
  }
  for (const level of [2, 0, 1]) for (let a = 0; a < 3; a++) for (let b = 0; b < 3; b++) {
    if (level !== 2 && (a !== 0 || b !== 2)) continue;
    make(`cpu-${ids[a]}-${ids[b]}-level${level}`, a, [], { training: false, cpuLevel: level, seed: 7 + a * 3 + b,
      players: [{ fighter: ids[a], cpu: true, tapJump: false }, { fighter: ids[b], cpu: true, tapJump: true }] }).run(2500);
  }
  const human = make('human-vs-cpu', 0, [], { training: false, players: [{ fighter: 'brawler', cpu: false, tapJump: false }, { fighter: 'king', cpu: true, tapJump: true }] });
  for (let i = 0; i < 700 && !human.w.over; i++) {
    const dx = Math.sign(human.w.fighters[1].x - human.f.x);
    human.tick(raw(dx, i % 120 < 8 ? 1 : 0, i % 40 === 0 ? 2 : i % 90 === 0 ? 4 : 0));
  }
  return fixtures;
}
const first = build().map(t => ({ name: t.name, text: t.encode(), ticks: t.frames.length,
  config: t.config, states: [...t.states].sort(), moves: [...t.moves].sort(), details: [...t.details].sort(), events: [...t.events].sort() }));
const second = build();
assert.equal(first.length, second.length);
for (let i = 0; i < first.length; i++) assert.equal(second[i].encode(), first[i].text, `Nondeterministic: ${first[i].name}`);
await mkdir(output, { recursive: true });
const verify = process.argv.includes('--verify');
for (const f of first) {
  const path = new URL(`${f.name}.json`, output);
  if (verify) assert.equal(await readFile(path, 'utf8'), f.text, `Fixture differs: ${f.name}`);
  else await writeFile(path, f.text);
  console.log(`${f.name.padEnd(36)} ${String(f.ticks).padStart(4)} ticks ${String(Buffer.byteLength(f.text)).padStart(7)} bytes`);
}
const report = first.map(({ text, ...f }) => ({ ...f, bytes: Buffer.byteLength(text) }));
if (!verify) {
  await writeFile(new URL('coverage.json', import.meta.url), JSON.stringify(report, null, 2) + '\n');
  const rows = report.map(f => `| ${f.name} | ${f.ticks} | ${f.bytes} |`).join('\n');
  const setups = report.filter(f => f.config.setup).map(f => `- **${f.name}**: \`${JSON.stringify(f.config.setup)}\``).join('\n');
  await writeFile(new URL('REPORT.md', import.meta.url), `# Recorded TS fixtures\n\n${report.length} fixtures, ${report.reduce((n, f) => n + f.bytes, 0)} bytes. Two independent recordings compared byte for byte before writing.\n\n| Fixture | Ticks | Bytes |\n| --- | ---: | ---: |\n${rows}\n\n## Exact setup overrides\n\nApplied once immediately after createWorld; player indices are zero based. All unlisted fields retain TS defaults.\n\n${setups}\n`);
}
console.log(`${verify ? 'Verified' : 'Wrote'} ${first.length} fixtures, ${first.reduce((n, f) => n + Buffer.byteLength(f.text), 0)} bytes; two independent runs identical.`);
