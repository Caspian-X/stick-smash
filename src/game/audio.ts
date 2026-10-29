// Synthesized sound effects (Web Audio, no asset files).

import type { SfxName } from '../sim/core';

let actx: AudioContext | null = null;
let noiseBuf: AudioBuffer | null = null;
export let muted = false;
export const setMuted = (m: boolean) => { muted = m; };

export function initAudio() {
  const unlock = () => {
    if (!actx) { try { actx = new AudioContext(); } catch { /* no audio */ } }
    if (actx && actx.state === 'suspended') void actx.resume();
  };
  addEventListener('pointerdown', unlock);
  addEventListener('keydown', unlock);
}

function tone(f0: number, f1: number, dur: number, type: OscillatorType = 'square', vol = 0.08) {
  if (!actx || muted) return;
  const t = actx.currentTime, o = actx.createOscillator(), g = actx.createGain();
  o.type = type; o.frequency.setValueAtTime(f0, t); o.frequency.exponentialRampToValueAtTime(Math.max(20, f1), t + dur);
  g.gain.setValueAtTime(vol, t); g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  o.connect(g).connect(actx.destination); o.start(t); o.stop(t + dur + 0.02);
}

function noise(dur: number, vol = 0.2, freq = 1200) {
  if (!actx || muted) return;
  if (!noiseBuf) {
    noiseBuf = actx.createBuffer(1, actx.sampleRate, actx.sampleRate);
    const d = noiseBuf.getChannelData(0); for (let i = 0; i < d.length; i++) d[i] = Math.random() * 2 - 1;
  }
  const t = actx.currentTime, s = actx.createBufferSource(), f = actx.createBiquadFilter(), g = actx.createGain();
  s.buffer = noiseBuf; f.type = 'lowpass'; f.frequency.setValueAtTime(freq, t); f.frequency.exponentialRampToValueAtTime(100, t + dur);
  g.gain.setValueAtTime(vol, t); g.gain.exponentialRampToValueAtTime(0.0001, t + dur);
  s.connect(f).connect(g).connect(actx.destination); s.start(t); s.stop(t + dur);
}

const SFX: Record<SfxName | 'menu', (k?: number) => void> = {
  jump() { tone(300, 600, 0.1, 'triangle', 0.06); },
  djump() { tone(420, 950, 0.12, 'triangle', 0.06); },
  swing() { noise(0.08, 0.05, 3500); },
  smash() { noise(0.2, 0.12, 3000); tone(320, 110, 0.18, 'sawtooth', 0.05); },
  charge() { tone(500, 700, 0.06, 'sine', 0.03); },
  hit(k = 5) { noise(0.1 + k * 0.012, 0.25, 1800 + k * 90); tone(170, 45, 0.12 + k * 0.006, 'sine', 0.3); },
  shoot() { tone(900, 280, 0.16, 'square', 0.045); },
  slash() { noise(0.1, 0.08, 6000); tone(1500, 500, 0.08, 'sine', 0.025); },
  counter() { tone(1800, 2400, 0.12, 'triangle', 0.08); noise(0.15, 0.1, 8000); },
  whack() { noise(0.14, 0.1, 1400); tone(240, 90, 0.12, 'triangle', 0.05); },
  toss() { tone(260, 520, 0.12, 'square', 0.04); noise(0.1, 0.08, 2000); },
  bonk() { tone(700, 300, 0.07, 'square', 0.04); },
  jet() { noise(0.07, 0.05, 900); },
  quake() { noise(0.35, 0.3, 700); tone(120, 35, 0.35, 'sine', 0.35); },
  ko() { noise(0.9, 0.4, 5000); tone(600, 40, 0.9, 'sawtooth', 0.1); },
  land() { noise(0.05, 0.05, 600); },
  grab() { tone(220, 330, 0.08, 'square', 0.06); },
  pummel() { noise(0.06, 0.12, 1500); },
  throw() { noise(0.18, 0.12, 2500); },
  shieldHit() { tone(1100, 700, 0.08, 'triangle', 0.08); },
  shieldBreak() { noise(0.6, 0.35, 7000); tone(1400, 200, 0.6, 'sawtooth', 0.08); },
  tech() { tone(1200, 1700, 0.08, 'triangle', 0.06); },
  roll() { noise(0.12, 0.05, 1500); },
  menu() { tone(620, 620, 0.05, 'square', 0.04); },
  beep() { tone(440, 440, 0.14, 'square', 0.05); },
  go() { tone(520, 1040, 0.35, 'square', 0.06); },
};

export const playSfx = (name: SfxName | 'menu', arg?: number) => SFX[name]?.(arg);
