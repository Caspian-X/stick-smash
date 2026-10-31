// One-shot migration from the sibling TS checkout. No source files are rewritten.
import { register } from 'node:module';
import { mkdir, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { resolve } from 'node:path';

// TS uses both extensionless files and directory imports (../data -> index.ts).
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

const root = fileURLToPath(new URL('../../', import.meta.url));
const source = new URL('../../../stick-smash/src/', import.meta.url);
const ts = path => import(new URL(path, source));
const [{ GLOBAL }, stage, { COLORS }, { DATA }, common, { pose, BODY, kfAt }, { mkFighter }] = await Promise.all([
  ts('data/global.ts'), ts('data/stage.ts'), ts('data/colors.ts'), ts('data/index.ts'),
  ts('data/fighters/common.ts'), ts('render/pose.ts'), ts('sim/core.ts'),
]);
const { BASE_STATS, COMMON_GRABS, COMMON_THROWS } = common;
const deg = x => x * 180 / Math.PI;
const hex = x => `#${x.toString(16).padStart(6, '0')}`;
const round = x => Number(x.toFixed(3));
const clean = v => JSON.parse(JSON.stringify(v, (_, x) => {
  if (typeof x !== 'number') return x;
  if (!Number.isFinite(x)) throw Error(`Non-finite content number: ${x}`);
  return round(x);
}));
const equal = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const point = ([x, y]) => [x, -y];
function convertPose(p) {
  const out = {};
  for (const [a, b] of [['hd', 'head'], ['sh', 'shoulder'], ['hp', 'hip']]) if (p[a]) out[b] = point(p[a]);
  for (const [a, b, c] of [['aF', 'elbowF', 'handF'], ['aB', 'elbowB', 'handB'], ['lF', 'kneeF', 'footF'], ['lB', 'kneeB', 'footB']]) {
    if (p[a]) { out[b] = point(p[a][0]); out[c] = point(p[a][1]); }
  }
  if (p.rot !== undefined) out.rot = deg(p.rot);
  if (p.oy !== undefined) out.offsetY = -p.oy;
  if (p.pv !== undefined) out.pivot = -p.pv;
  return clean(out);
}
function mock(ch = 0, state = 'idle', t = 0, sd = {}) {
  return Object.assign(mkFighter(0, ch, false, '', 3), { state, t, sd: { hit: new Set(), ...sd } });
}
const sample = (f, world = 0) => convertPose(pose(f, world));
const stand = sample(mock());
function partial(p) {
  return Object.fromEntries(Object.entries(p).filter(([k, v]) => !equal(v, stand[k])));
}
function key(at, p, lerp = false) { return { at, pose: partial(p), ...(lerp ? { lerp: true } : {}) }; }
// Remove identical holds and exactly linear interior samples, retaining step boundaries.
function compress(keys) {
  const out = [];
  for (const k of keys) {
    out.push(k);
    while (out.length >= 3) {
      const [a, b, c] = out.slice(-3);
      const flat = p => Object.values(p).flat();
      let redundant = equal(a.pose, b.pose) && equal(b.pose, c.pose);
      if (b.lerp && c.lerp && equal(Object.keys(a.pose), Object.keys(b.pose)) && equal(Object.keys(a.pose), Object.keys(c.pose))) {
        const av = flat(a.pose), bv = flat(b.pose), cv = flat(c.pose), u = (b.at - a.at) / (c.at - a.at);
        redundant ||= av.every((v, i) => Math.abs(bv[i] - (v + (cv[i] - v) * u)) <= 0.002);
      }
      if (!redundant) break;
      out.splice(-2, 1);
    }
  }
  // A held final key adds nothing, unless it is the endpoint of a lerp.
  if (out.length > 1 && !out.at(-1).lerp && equal(out.at(-1).pose, out.at(-2).pose)) out.pop();
  return out;
}
function timeline(end, fn, smooth = () => false) {
  return compress(Array.from({ length: end + 1 }, (_, t) => key(t, fn(t), t > 0 && smooth(t))));
}
function loop(clock, period, fn, count = 8) {
  return { clock, loop: period, keys: Array.from({ length: count + 1 }, (_, i) => key(i * period / count, fn(i * period / count), i > 0)) };
}
function stateAnim(state, end, sd = {}, smooth = () => false, setup = () => {}) {
  return { keys: timeline(end, t => { const f = mock(0, state, t, sd); setup(f, t); return sample(f); }, smooth) };
}
const poses = Object.fromEntries(Object.entries(BODY).map(([n, p]) => [n, convertPose(p)]));
poses.stand = stand;
poses.airTuck = convertPose(BODY.air);
poses.curl = partial(sample(mock(0, 'airdodge')));
poses.shield = partial(sample(mock(0, 'shield')));
poses.lie = partial(sample(mock(0, 'knockdown')));
const g = GLOBAL;
const anims = {
  idle: loop('world', 2 * Math.PI / 0.08, t => sample(mock(), t)),
  run: loop('run', 360, t => { const f = mock(); f.vx = 2; f.runPh = t * Math.PI / 180; return sample(f); }),
};
for (const [name, state, end] of [
  ['jumpsquat', 'jumpsquat', g.movement.jumpsquat], ['landing', 'landing', g.landing.hard],
  ['helpless', 'helpless', 1], ['shield', 'shield', 1],
  ['spot', 'spot', g.dodge.spotFrames], ['airdodge', 'airdodge', g.dodge.airDodgeFrames],
  ['hold', 'hold', g.grab.holdBase], ['knockdown', 'knockdown', g.knockdown.autoGetup],
  ['respawn', 'respawn', g.respawn.maxWait],
]) anims[name] = stateAnim(state, end);
anims.shieldDrop = stateAnim('landing', g.shield.dropLag, { kind: 'shield' });
anims['air.rise'] = stateAnim('air', 1, {}, () => false, f => { f.vy = -1; });
anims['air.fall'] = stateAnim('air', 1, {}, () => false, f => { f.vy = 1; });
anims['air.flip'] = { loop: 20, ...stateAnim('air', 20, {}, () => true, (f, t) => { f.flipT = 20 - t; f.vy = -1; }) };
// At the loop endpoint use the equivalent 360 degree pose, rather than flipT=0's reset.
anims['air.flip'].keys.at(-1).pose.rot = 360;
anims.roll = stateAnim('roll', g.dodge.rollFrames, {}, () => true);
anims.pummel = stateAnim('hold', g.grab.pummelFrames, {}, () => false, (f, t) => { f.sd.pt = g.grab.pummelFrames - t; });
for (const dir of ['f', 'b', 'u', 'd']) {
  const d = COMMON_THROWS[dir];
  anims[`throw.${dir}`] = stateAnim('throw', d.total, { kind: dir }, t => dir === 'b' ? t < d.release : dir === 'u' && t <= d.release);
}
for (const name of ['grabbed', 'hitstun', 'shieldbreak']) anims[name] = loop('world', 2 * Math.PI / 0.6, t => sample(mock(0, name), t));
// Representative positive horizontal velocity; TS reverses tumble spin when vx < 0.
anims.tumble = loop('state', 2 * Math.PI / 0.3, t => { const f = mock(0, 'hitstun'); f.tumble = true; f.spin = t * 0.3; return sample(f); });
anims.getup = stateAnim('getup', g.knockdown.getupFrames, {}, t => t <= 12);
anims.getupAttack = stateAnim('getup', g.knockdown.attackFrames, { kind: 'atk' }, t => t < 12);
anims.tech = stateAnim('tech', g.tech.frames, {}, t => t <= 8);
anims.ledge = loop('world', 2 * Math.PI / 0.05, t => sample(mock(0, 'ledge'), t));
anims.dizzy = loop('world', 2 * Math.PI / 0.1, t => sample(mock(0, 'dizzy'), t));
anims.win = loop('world', Math.PI / 0.12, t => sample(mock(0, 'win'), t));
for (const [name, kind] of [['climb', 'climb'], ['ledgeAttack', 'attack'], ['ledgeRoll', 'roll'], ['ledgeJump', 'jump']]) {
  const c = g.ledge[kind];
  anims[name] = stateAnim('climb', c.end, { kind, side: -1 }, t => t <= c.a || (kind === 'roll' && t > c.a + 1 && t < c.b), (f, t) => {
    // Representative left ledge pull-up: reproduce stClimb's easeOut position for hand IK.
    // This bakes the default stage's hang geometry; custom stages may need their own animation.
    const k = 1 - (1 - Math.min(t / c.a, 1)) ** 2;
    f.x = stage.STAGE.L - stage.LEDGE.hangX + (stage.LEDGE.hangX + 4) * k;
    f.y = stage.STAGE.top + stage.LEDGE.hangY * (1 - k);
  });
}
function hit(h, shift = 0) {
  const out = { ...h };
  if (out.start !== undefined) out.start -= shift;
  if (out.end !== undefined) out.end -= shift;
  if (out.frames) out.frames = out.frames.map(([t, ...v]) => [t - shift, ...v]);
  for (const [k, v] of Object.entries({ start: 0, dir: 'forward', type: 'normal', setKb: false, hitstunMult: 1, hitstunAdd: 0, hitlagMult: 1, shieldDmgMult: 1, shieldStunMult: 1 })) if (out[k] === v) delete out[k];
  return out;
}
const dust = (x, n) => ({ type: 'dust', x, y: 0, n });
const ring = (x, y, r, grow, life, color) => ({ type: 'ring', x, y, r, grow, life, ...(color ? { color } : {}) });
const part = (x, y, vx, vy, life, color, size) => ({ type: 'part', x, y, vx, vy, life, color, size });
const quake = () => ({ at: 0, sfx: 'quake', fx: [ring(0, 6, 10, 7, 16, 'player'), dust(-30, 8), dust(30, 8), { type: 'shake', k: 8 }] });
const transition = (on, to, rest = {}) => ({ on, to, ...rest });
const wave = d => ({ name: 'wave', frames: d.recovery, airAccelMult: d.kind === 'groundPound' ? 0 : 0.5,
  events: [quake()], hitboxes: d.wave.map(h => ({ ...hit(h), end: d.waveFrames - 1 })) });

function move(ch, id, d) {
  const f = DATA.fighters[ch], weapon = f.weapon, total = d.kind ? d.total : Math.max(0, ...d.hitboxes.map(h => h.end)) + d.endLag;
  const m = { name: d.name ?? id, ...(d.aerial ? { air: 'aerial' } : id === 'uspec' ? { air: 'upSpecial' } : {}), ...(id === 'nspec' ? { bReverse: true } : {}) };
  let phases;
  if (!d.kind) {
    const events = [], first = d.hitboxes[0];
    if (first && id !== 'uspec' && !weapon) events.push({ at: 0, sfx: 'swing' });
    if (d.charge && first) events.push({ at: first.start, sfx: 'smash' });
    if (weapon) for (const t of new Set(d.hitboxes.map(h => h.start))) events.push({ at: t, sfx: weapon === 'sword' ? 'slash' : 'whack' });
    if (d.forwardStep && first) events.push({ at: first.start, setVel: { x: d.forwardStep } });
    if (d.quakeFrame) events.push({ at: d.quakeFrame, if: 'grounded', sfx: 'quake', fx: [dust(76, 10), ring(76, 4, 8, 5, 12), { type: 'shake', k: 6 }] });
    if (d.dash) {
      events.push({ at: d.dash.frame, if: 'grounded', setVel: { x: d.dash.groundSpeed } }, { at: d.dash.frame, if: 'airborne', setVel: { x: d.dash.airSpeed } }, { at: d.dash.frame, fx: [dust(0, 4)] });
    }
    const p = { name: 'main', frames: total, hitboxes: d.hitboxes.map(h => hit(h)), events };
    if (d.slide) p.slide = d.slide;
    if (d.charge) { p.charge = { ...d.charge }; if (p.charge.max === 60) delete p.charge.max; }
    if (d.aerial) p.transitions = [transition('land', 'landing', { lag: d.landingLag ?? 0 })];
    if (d.helpless) (p.transitions ??= []).push(transition('timeout', 'helpless'));
    phases = [p];
    if (d.rise) {
      const r = d.rise;
      // moves.ts:97-105 launches at r.frame, particles strictly after it; physics.ts:78 requires t > frame+2.
      phases = [
        { name: 'startup', frames: r.frame, hover: true, events: [{ from: 1, to: r.frame - 1, mulVel: { x: 0.5 } }], transitions: [transition('timeout', 'rise')] },
        { ...p, name: 'rise', frames: total - r.frame, ledgeGrab: Math.max(0, 7 - r.frame),
          hitboxes: d.hitboxes.map(h => hit(h, r.frame)),
          events: [
            // Preserve same-frame slash/smash ordering before launch.
            ...events.filter(e => e.at >= r.frame).map(e => ({ ...e, at: e.at - r.frame })),
            { at: 0, setVel: { x: r.vx, y: r.vy }, leaveGround: true, sfx: 'djump' },
            { from: 1, to: 13, every: 2, fx: [part(0, 40, 0, -1, 16, 'player', 6)] },
          ], transitions: [transition('land', 'landing', { from: 3, lag: g.landing.upSpecial }), transition('timeout', 'helpless')] },
      ];
    }
  } else if (d.kind === 'projectile' || d.kind === 'gordo') {
    phases = [{ name: 'main', frames: total, events: [{ if: 'airborne', capFall: d.airFallCap }, { at: d.spawnFrame, spawn: d.kind === 'gordo' ? 'gordo' : 'energyBall' }] }];
  } else if (d.kind === 'groundPound') {
    phases = [
      { name: 'startup', frames: d.startup, hover: true, airAccelMult: 0,
        events: [{ if: 'airborne', mulVel: { x: 0.8 } }], transitions: [transition('timeout', 'wave', { if: 'grounded' }), transition('timeout', 'fall')] },
      // moves.ts:85-93: fall starts at startup, but sets speed from startup+1; cancels at timeout+1.
      { name: 'fall', frames: d.timeout - d.startup + 1, fallCap: d.fallSpeed, airAccelMult: 0,
        events: [{ setVel: { y: -d.fallSpeed }, mulVel: { x: 0.9 } }, { every: 2, fx: [part([-6, 6], 30, 0, 2, 14, 'player', 5)] }],
        hitboxes: [hit(d.fall)], transitions: [transition('grounded', 'wave'), transition('timeout', 'end')] }, wave(d),
    ];
  } else if (d.kind === 'superJump') {
    phases = [
      { name: 'squat', frames: d.launchFrame, hover: true, ledgeGrab: 7,
        events: [{ from: 1, to: d.launchFrame, mulVel: { x: 0.6 } }], transitions: [transition('timeout', 'rise')] },
      { name: 'rise', ledgeGrab: Math.max(0, 7 - d.launchFrame),
        events: [{ at: 0, setVel: { x: d.launchVx, y: d.launchVy }, leaveGround: true, sfx: 'djump', fx: [dust(0, 8)] },
          { every: 2, fx: [part(0, 30, 0, -1, 16, 'player', 6)] }],
        hitboxes: [{ ...hit(d.rise), end: d.riseHitUntil - d.launchFrame - 1 }],
        transitions: [transition('falling', 'plunge'), transition('land', 'wave')] },
      { name: 'plunge', ledgeGrab: 0, fallCap: d.plungeSpeed, airAccelMult: 0.5,
        events: [{ at: 0, mulVel: { x: 0.4 }, sfx: 'smash' }, { setVel: { y: -d.plungeSpeed } },
          { every: 2, fx: [part([-8, 8], 50, 0, 2, 14, '#ffe680', 5)] }],
        hitboxes: [hit(d.fall)], transitions: [transition('shieldPress', 'helpless', { from: 1 }), transition('land', 'wave')] },
      { ...wave(d), ledgeGrab: 0, airAccelMult: 1 },
    ];
  } else if (d.kind === 'counter') {
    phases = [
      { name: 'stance', frames: total, counter: { from: d.window[0], to: d.window[1], damageMult: d.damageMult, minDamage: d.minDamage, attackerFreeze: d.attackerFreeze, invuln: d.invuln, reflectSpeed: d.reflectSpeed, reflectLife: 75 }, transitions: [transition('counter', 'strike')] },
      { name: 'strike', frames: d.recovery, hitboxes: [{ ...hit(d.hit), counterDamage: true, start: d.strike[0], end: d.strike[1] }], events: [
        { at: 0, sfx: 'counter', fx: [ring(0, 40, 14, 5, 14), { type: 'burst', x: 0, y: 40, color: '#bbffff', n: 16, speed: 6, life: 20, size: 3 }, { type: 'shake', k: 4 }] },
        { at: d.strike[0], if: 'countered', sfx: 'slash' },
      ] },
    ];
  } else throw Error(`Unsupported TS move kind: ${d.kind}`);
  for (let i = 0; i < phases.length; i++) visuals(phases[i], ch, id, d, i);
  return { ...m, phases };
}
function visuals(p, ch, id, d, phaseIndex) {
  const weapon = DATA.fighters[ch].weapon;
  const shift = d.rise && phaseIndex === 1 ? d.rise.frame : 0;
  if (weapon && (d.anim || d.kind === 'superJump' || d.kind === 'counter')) {
    if (d.kind === 'superJump') {
      p.anim = [{ at: 0, pose: ['crouch', 'rise', 'stretch', 'crouch'][phaseIndex] }];
      p.attach = { [weapon]: { angle: [[0, [-150, 100, -90, -25][phaseIndex]]] } };
    } else {
      const strike = d.kind === 'counter' && phaseIndex === 1;
      const first = !d.kind && d.hitboxes.length ? Math.min(...d.hitboxes.map(h => h.start)) : 99;
      const body = strike ? 'lean' : d.anim.body;
      const wind = !strike && d.anim.windup;
      p.anim = [{ at: 0, pose: wind && shift < first ? wind : body }];
      if (wind && first > shift && first - shift < (p.frames ?? Infinity)) p.anim.push({ at: first - shift, pose: body });
      const swing = strike ? d.strikeSwing ?? [[0, -120], [3, -120], [7, 40]] : d.anim.swing;
      p.attach = { [weapon]: { angle: [[0, -kfAt(swing, shift)], ...swing.filter(([t]) => t > shift && t <= shift + (p.frames ?? Infinity)).map(([t, a]) => [t - shift, -a])] } };
    }
  } else {
    const end = p.frames ?? 1;
    p.anim = timeline(end, t => {
      const f = mock(ch, 'attack');
      f.ground = !d.aerial && id !== 'uspec' && d.kind !== 'groundPound';
      f.move = { id, t: t + shift, ph: d.kind === 'groundPound' ? phaseIndex : 0, t2: t, charge: 0, cd: 10 };
      return sample(f);
    }, t => (id === 'nair' && t + shift > d.hitboxes[0].start && t + shift <= d.hitboxes[0].end)
      || (d.kind === 'groundPound' && phaseIndex === 0));
  }
}
const line = (from, to, width, color, extra = {}) => ({ shape: 'line', from, to, width, color, ...extra });
const circle = (r, fill, extra = {}) => ({ shape: 'circle', at: [0, 0], r, fill, ...extra });
const rect = (x, y, w, h, fill, extra = {}) => ({ shape: 'rect', x, y, w, h, fill, ...extra });
function gordoShapes(r) {
  const shapes = [];
  for (let k = 0; k < 8; k++) shapes.push({ shape: 'poly', fill: '#e8e8f0', points: [[k * Math.PI / 4 - 0.3, r * 0.8], [k * Math.PI / 4, r * 1.45], [k * Math.PI / 4 + 0.3, r * 0.8]].map(([a, r]) => point([Math.cos(a) * r, Math.sin(a) * r])) });
  shapes.push(circle(r + 5, 'player', { alpha: 0.25 }), circle(r, '#3a2f5a', { stroke: 'player', width: 2.5 }));
  return shapes;
}
const gordoEyes = r => [-1, 1].map(s => rect(s * r * 0.35 - 1.5, -r * 0.1, 3, r * 0.45, '#ffffff'));
function projectiles(f) {
  const out = {};
  for (const d of Object.values(f.moves)) {
    if (d.kind === 'projectile') {
      const p = d.projectile;
      out.energyBall = { spawn: [p.x, p.y], vel: [p.speed, 0], r: p.r, life: p.life, max: p.max, hit: hit(p.hit), spawnSfx: 'shoot',
        trail: [{ every: 2, fx: [{ ...part(0, 0, 0, 0, 14, 'player', 6), inheritVel: -0.05 }] }],
        shapes: [circle(p.r + 10, 'player', { alpha: 0.2 }), circle(p.r + 5, 'player', { alpha: 0.3 }), circle(p.r, '#ffffff'), circle(p.r * 0.55, 'player')] };
    } else if (d.kind === 'gordo') {
      const p = d.gordo;
      out.gordo = { spawn: [p.x, p.y], vel: [p.vx, -p.vy], r: p.r, life: p.life, max: 1, hit: hit(p.hit), spawnSfx: 'toss', fullSfx: 'bonk',
        gravity: p.gravity, maxFall: p.maxFall, reflectVy: 4,
        bounce: { restitution: p.bounce, minSpeed: p.minBounce, friction: p.friction, maxBounces: p.maxBounces, fizzleLife: 20, wallRestitution: 0.6, sfx: 'bonk' },
        bat: { speed: p.batSpeed, vy: -p.batVy, life: p.batLife, cooldown: p.batCooldown, hitlag: p.batHitlag },
        spin: -deg(0.06), shapes: gordoShapes(p.r), overlay: gordoEyes(p.r) };
    }
  }
  return out;
}
function attachments(f) {
  const out = [];
  if (f.weapon) {
    const sword = f.weapon === 'sword';
    const shapes = sword ? [
      ...[['noGlow', 'player', 8], ['glow', '#bbffff', 12]].map(([when, color, width]) => line([3, 0], [54, 0], width, color, { when, alpha: 0.25 })),
      ...[['noGlow', '#e6ecff'], ['glow', '#ffffff']].map(([when, color]) => line([3, 0], [54, 0], 4, color, { when })),
      line([-7, 0], [3, 0], 4, '#7a5530'), line([3, -8], [3, 8], 3.5, '#e0b84a'),
    ] : [
      line([-6, 0], [40, 0], 4.5, '#8a5a2b'),
      rect(32, -19, 26, 38, 'player', { radius: 6, alpha: 0.15, when: 'noGlow' }),
      rect(32, -19, 26, 38, '#ff9a3c', { radius: 6, alpha: 0.35, when: 'glow' }),
      ...[['noGlow', '#b98545'], ['glow', '#ffcf7a']].map(([when, fill]) => rect(36, -15, 18, 30, fill, { radius: 4, stroke: '#4a2f14', width: 2, when })),
      rect(38, -15, 3, 30, '#e0b84a'), rect(49, -15, 3, 30, '#e0b84a'),
    ];
    out.push({ id: f.weapon, joint: 'handF', angle: deg(sword ? 0.8 : 3.2), hold: { elbow: 15, hand: 28, bend: -deg(0.35) }, trail: { inner: sword ? 34 : 50, outer: sword ? 84 : 96 }, glow: sword ? 'countering' : 'jetCharging', shapes });
  }
  if (f.look === 'king') out.push({ id: 'crown', joint: 'head', shapes: [
    { shape: 'poly', fill: '#ffd35a', points: [[-9, 7], [-10, 19], [-4, 12], [0, 21], [4, 12], [10, 19], [9, 7]] },
    { ...circle(2, '#ff5d8a'), at: [0, 10] },
  ] });
  if (f.look === 'sword') out.push({ id: 'hairTie', joint: 'head', shapes: [line([-9, 4], [-20, 1], 3, 'player')] });
  return out;
}
// Inheritance compares recursively, replacing arrays whole just like the Rust loader.
function diff(value, base) {
  if (equal(value, base)) return undefined;
  if (!value || typeof value !== 'object' || Array.isArray(value) || !base || typeof base !== 'object' || Array.isArray(base)) return value;
  const out = {};
  for (const [k, v] of Object.entries(value)) { const d = diff(v, base[k]); if (d !== undefined) out[k] = d; }
  for (const k of Object.keys(base)) if (!(k in value)) out[k] = null;
  return Object.keys(out).length ? out : undefined;
}
async function write(path, kind, value) {
  const file = resolve(root, 'assets', path);
  await mkdir(resolve(file, '..'), { recursive: true });
  await writeFile(file, JSON.stringify(clean({ $schema: path === 'global.json' ? 'schema/global.schema.json' : `../../schema/${kind}.schema.json`, schemaVersion: kind === 'stage' ? 2 : 1, ...value }), null, 2) + '\n');
  console.log(`Wrote assets/${path}`);
}
const global = structuredClone(g);
delete global.landing.upSpecial;
global.palette = [
  ...COLORS.map(c => ({ main: hex(c.c), dark: hex(c.d), light: hex(c.l) })),
  ...[
    [0x5cdb88, 0x348650, 0xd1ffe0], [0xffd65d, 0xa68935, 0xfff2c5], [0xaf80ff, 0x6c48a6, 0xe9d8ff],
    [0xff9b50, 0xa65e2e, 0xffdfc5], [0x50dce3, 0x2e8990, 0xc9fbff], [0xf77fbc, 0xa64878, 0xffd5eb],
  ].map(([main, dark, light]) => ({ main: hex(main), dark: hex(dark), light: hex(light) })),
];
await write('global.json', 'global', global);
await write('stages/arena/stage.json', 'stage', {
  name: 'Arena', width: stage.WORLD_W, height: stage.WORLD_H,
  blocks: [{ left: stage.STAGE.L, right: stage.STAGE.R, top: stage.STAGE.top, bottom: stage.STAGE.bot, underDepth: stage.UNDER.depth, underInset: stage.UNDER.inset, leftLedge: true, rightLedge: true }],
  platforms: stage.PLATS, ledge: stage.LEDGE,
  blast: { left: stage.BLAST.L, right: stage.BLAST.R, top: stage.BLAST.T, bottom: stage.BLAST.B },
  camera: { left: -1400, right: 2700, top: -900, bottom: 1500 },
  spawns: [...stage.SPAWNS, ...[360, 920, 550, 730, 620, 660].map(x => ({ x, face: x < 640 ? 1 : -1 }))],
  respawns: [stage.RESPAWN, ...[360, 920, 440, 840, 520, 760, 600].map(x => ({ ...stage.RESPAWN, x }))],
  art: { groundTop: '#5b6386', groundBottom: '#3b4060', underside: '#2c3048', edge: '#b8c4ff', platform: '#c9d4ff', platformShadow: '#8fa2ff' },
});
const base = {
  name: 'Base', blurb: '', stats: BASE_STATS, body: { halfWidth: 14, height: 74 },
  hurtboxes: { default: { x1: -16, x2: 16, y1: 0, y2: 78 }, knockdown: { x1: -30, x2: 30, y1: 0, y2: 16 } },
  ai: { reach: DATA.fighters[0].aiReach },
  moves: Object.fromEntries(Object.entries(COMMON_GRABS).map(([id, d]) => [id, move(0, id, d)])),
  throws: Object.fromEntries(Object.entries(COMMON_THROWS).map(([id, d]) => [id, { ...d, hit: hit(d.hit) }])),
  style: { headRadius: 10, torsoWidth: 6, frontLimbWidth: 5.5, backLimbWidth: 5, glowWidth: 14 }, poses, anims,
};
await write('fighters/base/fighter.json', 'fighter', { abstract: true, ...base });
for (let ch = 0; ch < DATA.fighters.length; ch++) {
  const f = DATA.fighters[ch];
  const value = {
    ...base, name: f.name, blurb: f.blurb, stats: f.stats, ai: { reach: f.aiReach },
    moves: Object.fromEntries(Object.entries(f.moves).map(([id, d]) => [id, move(ch, id, d)])),
    throws: Object.fromEntries(Object.entries(f.throws).map(([id, d]) => [id, { ...d, hit: hit(d.hit) }])),
    projectiles: projectiles(f), attachments: attachments(f),
  };
  // Optional held props use the same drawing geometry as their projectile.
  const ball = f.moves.nspec;
  if (ball.kind === 'projectile') {
    value.attachments.push(
      { id: 'chargeGlow', joint: 'handF', hidden: true, shapes: [circle(11, 'player', { alpha: 0.3 })] },
      { id: 'chargeOrb', joint: 'handF', hidden: true, shapes: [circle(3, '#ffffff')] },
    );
    value.moves.nspec.phases[0].attach = Object.fromEntries([['chargeGlow', 11], ['chargeOrb', 3]].map(([id, r]) => [id, {
      show: [0, ball.spawnFrame - 1], scale: [[0, 1], [ball.spawnFrame - 1, (r + (ball.spawnFrame - 1) * 0.6) / r]],
    }]));
  } else if (ball.kind === 'gordo') {
    value.attachments.push({ id: 'heldGordo', joint: 'root', hidden: true, shapes: [...gordoShapes(12), ...gordoEyes(12)] });
    value.moves.nspec.phases[0].attach.heldGordo = { show: [0, ball.spawnFrame - 1], offset: Array.from({ length: 8 }, (_, i) => { const t = i * (ball.spawnFrame - 1) / 7, k = t / ball.spawnFrame; return [t, 20 + k * 36, 50 + Math.sin(Math.PI * k) * 34]; }) };
  }
  if (f.look === 'king') value.fx = { airJump: Array.from({ length: 6 }, () => part([-14, 14], 4, [-1.5, 1.5], [-2, -0.5], 18, '#ddddee', 5)) };
  await write(`fighters/${f.id}/fighter.json`, 'fighter', { extends: 'base', ...diff(value, base) });
}
