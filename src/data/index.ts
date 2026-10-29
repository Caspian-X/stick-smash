// Live data registry. Game code reads tuning through DATA so edits hot-reload mid-match.

import { GLOBAL } from './global';
import brawler from './fighters/brawler';
import swordfighter from './fighters/swordfighter';
import king from './fighters/king';
import type { FighterData, MoveData } from './types';

export const DATA = {
  g: GLOBAL,
  fighters: [brawler, swordfighter, king] as FighterData[],
  /** Bumped on every hot reload. */
  version: 0,
};

/** Frame the move ends (actionable after this). Undefined for phase-driven moves. */
export function moveTotal(d: MoveData): number {
  switch (d.kind) {
    case undefined: return Math.max(0, ...d.hitboxes.map(h => h.end)) + d.endLag;
    case 'projectile': case 'gordo': case 'counter': return d.total;
    default: return 999;
  }
}

/** Startup / active window summary for the debug overlay and docs. */
export function frameSummary(d: MoveData): string {
  if (d.kind === undefined && d.hitboxes.length) {
    const s = Math.min(...d.hitboxes.map(h => h.start)), e = Math.max(...d.hitboxes.map(h => h.end));
    return `startup ${s} · active ${s}-${e} · end lag ${d.endLag} · total ${moveTotal(d)}` + (d.aerial ? ` · landing ${d.landingLag}` : '');
  }
  if (d.kind === 'projectile' || d.kind === 'gordo') return `spawn ${d.spawnFrame} · total ${d.total}`;
  if (d.kind === 'counter') return `window ${d.window[0]}-${d.window[1]} · total ${d.total}`;
  return d.kind ?? '';
}

if (import.meta.hot) {
  import.meta.hot.accept(
    ['./global', './fighters/brawler', './fighters/swordfighter', './fighters/king'],
    ([g, b, s, k]) => {
      if (g) DATA.g = g.GLOBAL;
      if (b) DATA.fighters[0] = b.default;
      if (s) DATA.fighters[1] = s.default;
      if (k) DATA.fighters[2] = k.default;
      DATA.version++;
      console.info('[stick-smash] tuning data hot-reloaded');
    },
  );
}
