// Keyboard state by KeyboardEvent.code. `pressed` holds keys newly pressed since the last sim frame.

import { type Input, blankInput } from '../sim/core';

const held = new Set<string>();
const pressed = new Set<string>();
const BLOCK = new Set(['ArrowUp', 'ArrowDown', 'ArrowLeft', 'ArrowRight', 'Space', 'Slash', 'Period', 'Quote', 'Semicolon', 'Backspace', 'Tab', 'Backquote']);

let ready = false;
export function initKeyboard() {
  if (ready) return;
  ready = true;
  addEventListener('keydown', e => {
    if (BLOCK.has(e.code)) e.preventDefault();
    if (!e.repeat) pressed.add(e.code);
    held.add(e.code);
  });
  addEventListener('keyup', e => held.delete(e.code));
  addEventListener('blur', () => held.clear());
}

export const pp = (...c: string[]) => c.some(k => pressed.has(k));
export const hh = (...c: string[]) => c.some(k => held.has(k));
/** Call once per sim frame after inputs are read. */
export const consumePressed = () => pressed.clear();
/** Drop specific presses (so UI hotkeys don't leak into gameplay while presses are being buffered). */
export const unpress = (...c: string[]) => c.forEach(k => pressed.delete(k));
export const pressedKeys = () => [...pressed];

export interface KeyMap { l: string[]; r: string[]; u: string[]; d: string[]; a: string[]; s: string[]; g: string[]; sh: string[] }
export const CONTROLS: KeyMap[] = [
  { l: ['KeyA'], r: ['KeyD'], u: ['KeyW'], d: ['KeyS'], a: ['KeyJ'], s: ['KeyK'], g: ['KeyI'], sh: ['KeyL'] },
  { l: ['ArrowLeft'], r: ['ArrowRight'], u: ['ArrowUp'], d: ['ArrowDown'], a: ['Period', 'Numpad1'], s: ['Slash', 'Numpad2'], g: ['Semicolon', 'Numpad3'], sh: ['Quote', 'Numpad0'] },
];

const taps = [{ l: -99, r: -99, u: -99, d: -99 }, { l: -99, r: -99, u: -99, d: -99 }];

/** Read player `i`'s keys for sim frame `frame`. A direction counts as a tap if pressed within `tapWindow` frames and still held. */
export function readHuman(i: number, frame: number, tapWindow: number): Input {
  const c = CONTROLS[i], ts = taps[i];
  for (const k of ['l', 'r', 'u', 'd'] as const) if (pp(...c[k])) ts[k] = frame;
  const tap = (k: 'l' | 'r' | 'u' | 'd') => hh(...c[k]) && frame - ts[k] <= tapWindow;
  const inp = blankInput();
  Object.assign(inp, {
    left: hh(...c.l), right: hh(...c.r), up: hh(...c.u), down: hh(...c.d),
    jump: pp(...c.u), atk: pp(...c.a), atkHeld: hh(...c.a), sp: pp(...c.s), spHeld: hh(...c.s), grab: pp(...c.g),
    shield: hh(...c.sh), shieldP: pp(...c.sh), downP: pp(...c.d),
    tapL: tap('l'), tapR: tap('r'), tapU: tap('u'), tapD: tap('d'),
    anyP: Object.values(c).some(ks => pp(...ks)),
  });
  return inp;
}
