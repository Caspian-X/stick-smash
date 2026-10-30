// CPU opponent. Uses the world's seeded RNG so matches stay deterministic.

import { STAGE, UNDER } from '../data/stage';
import { type Fighter, type Input, type World, FD, MV, blankInput, clamp, other, rand, rnd } from './core';

const MID = (STAGE.L + STAGE.R) / 2;

export function aiInput(w: World, f: Fighter, level = w.cfg.cpuLevel): Input {
  const ai = f.ai, o = other(w, f), lv = clamp(level, 0, 2), st = f.state, R = () => rand(w);
  const inp = blankInput();
  const react = [26, 14, 6][lv], aggr = [0.05, 0.12, 0.25][lv];
  if (ai.chargeT > 0) { ai.chargeT--; inp.atkHeld = true; }
  if (ai.spT > 0) { ai.spT--; inp.spHeld = true; }
  if (ai.upT > 0) { ai.upT--; inp.jumpHeld = true; }
  if (ai.shieldT > 0) ai.shieldT--;
  const toCenter = () => { const d = f.x < MID ? 1 : -1; inp.right = d > 0; inp.left = d < 0; };
  const C = FD(f), nspec = C.moves.nspec, dspec = C.moves.dspec, sspec = C.moves.sspec ?? nspec;
  const isSword = sspec.kind === undefined && !!sspec.dash, isKing = nspec.kind === 'gordo';
  if (st === 'attack' && f.move!.id === 'uspec' && f.y > STAGE.top - 200) {
    toCenter();
    if (MV(f).kind === 'superJump' && f.move!.ph === 2 && (f.x < STAGE.L - 20 || f.x > STAGE.R + 20)) inp.shieldP = true;
    return inp;
  }

  if (st === 'grabbed' || st === 'dizzy') { inp.anyP = R() < [0.12, 0.3, 0.55][lv]; return inp; }
  if (st === 'hitstun') {
    if (f.tumble && R() < [0.03, 0.1, 0.25][lv]) inp.shieldP = true;
    if (R() < 0.5) toCenter();
    return inp;
  }
  if (st === 'knockdown') {
    if (f.t > 12 + R() * 30) {
      const r = R();
      if (r < 0.3) inp.atk = true; else if (r < 0.6) toCenter(); else inp.jump = true;
    }
    return inp;
  }
  if (st === 'ledge') {
    if (!ai.wait) ai.wait = Math.floor(rnd(w, 10, 50));
    if (f.t > ai.wait) {
      ai.wait = 0;
      const r = R();
      if (r < 0.25) inp.jump = true; else if (r < 0.5) inp.atk = true; else if (r < 0.7) inp.shieldP = true; else toCenter();
    }
    return inp;
  }
  if (st === 'hold') {
    if (f.t === 1) { ai.holdT = Math.floor(rnd(w, 6, 30)); ai.pum = Math.floor(rnd(w, 0, 3)); }
    if (f.t > ai.holdT && f.sd.pt === 0) {
      if (ai.pum > 0 && R() < 0.5) { ai.pum--; inp.atk = true; }
      else {
        const fwd = () => { inp[f.face > 0 ? 'right' : 'left'] = true; }, back = () => { inp[f.face > 0 ? 'left' : 'right'] = true; };
        const edgeDir = f.x < MID ? -1 : 1;
        if (o && o.dmg > 90) { if (edgeDir === f.face) fwd(); else back(); }
        else { const r = R(); if (r < 0.35) inp.down = true; else if (r < 0.6) inp.up = true; else if (r < 0.8) fwd(); else back(); }
      }
    }
    return inp;
  }
  if (st === 'shield') {
    const oAtk = !!o && o.state === 'attack', close = !!o && Math.abs(o.x - f.x) < 70;
    if (!oAtk && close && R() < [0.1, 0.3, 0.6][lv]) {
      const r = R();
      if (r < 0.5) inp.grab = true; else if (r < 0.75) { inp.up = true; inp.sp = true; } else { inp.jump = true; ai.upT = 14; }
      ai.shieldT = 0; return inp;
    }
    if (f.sh > 12 && (ai.shieldT > 0 || oAtk)) inp.shield = true;
    return inp;
  }

  const under = f.x > STAGE.L + UNDER.inset - 10 && f.x < STAGE.R - UNDER.inset + 10 && f.y > STAGE.bot - 5;
  const offstage = (st === 'air' || st === 'helpless') && (f.x < STAGE.L || f.x > STAGE.R || f.y > STAGE.top + 5);
  if (offstage) {
    const dir = under ? (f.x < MID ? -1 : 1) : (f.x < MID ? 1 : -1);
    inp.left = dir < 0; inp.right = dir > 0; inp.up = false;
    if (st === 'helpless') return inp;
    const edge = f.x < MID ? STAGE.L : STAGE.R;
    if (f.vy > 0.5 && f.jumps > 0 && f.y > STAGE.top - 80) inp.jump = true;
    else if (f.jumps === 0 && !f.upB && f.vy > 1 && !under && f.y > STAGE.top - 30 && (Math.abs(f.x - edge) < 220 || f.y > STAGE.top + 150)) { inp.up = true; inp.sp = true; }
    return inp;
  }
  if (!o || ['out', 'dead', 'respawn'].includes(o.state)) {
    const dx = MID - f.x; if (Math.abs(dx) > 40) inp[dx > 0 ? 'right' : 'left'] = true;
    return inp;
  }
  const dx = o.x - f.x, dy = o.y - f.y, adx = Math.abs(dx), tdir = Math.sign(dx) || 1;
  const toward = () => { inp.left = tdir < 0; inp.right = tdir > 0; };
  const tx = clamp(o.x, STAGE.L + 30, STAGE.R - 30);
  if (Math.abs(tx - f.x) > 50) inp[tx > f.x ? 'right' : 'left'] = true;

  if (f.ground && st === 'idle') {
    if (o.state === 'attack' && adx < 110 && Math.abs(dy) < 80 && R() < [0.04, 0.15, 0.35][lv]) {
      if (dspec.kind === 'counter' && R() < 0.35) return { ...blankInput(), down: true, sp: true };
      ai.shieldT = Math.floor(rnd(w, 12, 30));
    }
    for (const p of w.projs) {
      if (p.o !== f && Math.sign(p.vx) === Math.sign(f.x - p.x) && Math.abs(p.x - f.x) < 120 && Math.abs(p.y - (f.y - 40)) < 40 && R() < [0.03, 0.15, 0.45][lv]) {
        if (R() < 0.5) ai.shieldT = 15; else { inp.jump = true; ai.upT = 14; }
      }
    }
    if (ai.shieldT > 0) return { ...blankInput(), shield: true };
  }
  if (dy < -80 && adx < 180 && f.ground && R() < 0.06) { inp.jump = true; ai.upT = 14; }
  if (f.ground && f.surf && !f.surf.main && dy > 60 && adx < 140 && R() < 0.06) inp.downP = true;
  if (ai.cool > 0) { ai.cool--; return inp; }
  const punish = o.state === 'dizzy' || o.state === 'knockdown';
  if (!punish && R() > aggr) return inp;
  const r = R(), reach = C.aiReach;
  if (adx < reach && Math.abs(dy) < 60) {
    inp.jump = false; inp.up = false;
    if (f.ground && o.state === 'shield') { toward(); inp.grab = true; }
    else if (f.ground && punish) { toward(); inp[tdir > 0 ? 'tapR' : 'tapL'] = true; inp.atk = true; ai.chargeT = o.state === 'dizzy' ? 55 : 6; }
    else if (f.ground && r < 0.22) {
      const s = R();
      if (s < 0.5) { toward(); inp[tdir > 0 ? 'tapR' : 'tapL'] = true; }
      else { inp.left = inp.right = false; inp[s < 0.75 ? 'tapD' : 'tapU'] = true; }
      inp.atk = true; ai.chargeT = Math.floor(rnd(w, 0, [5, 15, 25][lv]));
    }
    else if (f.ground && r < 0.32) { toward(); inp.grab = true; }
    else if (r < 0.6 || tdir !== f.face) { toward(); inp.atk = true; }
    else if (r < 0.75) { inp.left = inp.right = false; inp.down = true; inp.atk = true; }
    else if (r < 0.9) { inp.left = inp.right = false; inp.atk = true; }
    else { inp.left = inp.right = false; inp.down = true; inp.sp = true; }
  } else if (dy < -60 && adx < 60) { inp.up = true; inp.atk = true; }
  else if (dy > 50 && adx < 40 && !f.ground) { inp.down = true; inp.atk = true; }
  else if (isSword && f.ground && adx > 100 && adx < 190 && Math.abs(dy) < 50 && R() < 0.3) { toward(); inp.sp = true; }
  else if (isKing && f.ground && adx > 100 && adx < 200 && Math.abs(dy) < 40 && R() < 0.15) { toward(); inp.down = true; inp.sp = true; ai.spT = Math.floor(rnd(w, 10, 70)); }
  else if (isKing && adx > 160 && adx < 480 && Math.abs(dy) < 120 && !w.projs.some(p => p.o === f) && R() < 0.3) { toward(); inp.sp = true; }
  else if (nspec.kind === 'projectile' && adx > 220 && Math.abs(dy) < 90 && R() < 0.35) { toward(); inp.sp = true; }
  else return inp;
  ai.cool = react + Math.floor(rnd(w, 0, react));
  return inp;
}
