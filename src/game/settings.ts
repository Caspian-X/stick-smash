// Menu settings and win records, persisted to localStorage.

const KEY = 'stick-smash.v2';

export type Mode = 'cpu' | '2p' | 'training';
export type DummyMode = 'stand' | 'shield' | 'jump' | 'cpu' | 'p2';

export interface Settings {
  mode: Mode;
  level: number;
  stocks: number;
  c1: number;
  c2: number;
  dummy: DummyMode;
  debug: boolean;
}

const DEFAULTS: Settings = { mode: 'cpu', level: 1, stocks: 3, c1: 0, c2: 1, dummy: 'stand', debug: false };

export const S: Settings = { ...DEFAULTS };
export const WINS: Record<'cpu' | '2p', [number, number]> = { cpu: [0, 0], '2p': [0, 0] };

try {
  const sv = JSON.parse(localStorage.getItem(KEY) || 'null');
  if (sv) { Object.assign(S, sv.settings); Object.assign(WINS, sv.wins); }
} catch { /* ignore */ }

let saveT = 0;
export function save() {
  clearTimeout(saveT);
  saveT = window.setTimeout(() => {
    try { localStorage.setItem(KEY, JSON.stringify({ settings: S, wins: WINS })); } catch { /* ignore */ }
  }, 200);
}
