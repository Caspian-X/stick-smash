# AGENTS.md

Stick Smash: Smash-style platform fighter. Phaser 4 + TypeScript + Vite. See README.md for the balancing workflow and layout.

## Commands
- `npm run dev`: dev server with hot reload. `npm test`: vitest sim tests. `npm run typecheck`. `npm run build`.
- `npm run smoke [url] [outDir]`: headless system Chrome (Playwright, `channel: 'chrome'`) plays a bit, saves screenshots, prints console errors. Needs a dev server running. Use it after render/scene changes, because typecheck can't catch Phaser runtime errors.

## Rules
- **`src/sim/` must stay deterministic and Phaser-free.** No `Math.random` (use `rand(w)`/`rnd(w, a, b)`), no DOM, no wall-clock time. Visuals/audio go out as `Fx` events (`sfx`, `burst`, `part`, `ring`, `shake`…) that `render/fx.ts` consumes. This keeps tests headless and makes future rollback netcode possible.
- **All tunable numbers live in `src/data/`** (`global.ts` for shared rules and formulas, `fighters/*.ts` for stats and moves). Don't hard-code new balance constants in sim code; add a documented field instead.
- **Read tuning through accessors** (`G()`, `FD(f)`, `ST(f)`, `MV(f)` in `sim/core.ts`, or `DATA.*`), never by importing `GLOBAL` or a fighter file directly from sim/render code. That's what makes hot reload work (`data/index.ts` accepts HMR for the data modules).
- New data files must be added to the HMR `accept` list in `src/data/index.ts`.
- Move slots are fixed ids (`MoveId` in `data/types.ts`). Special behavior is chosen by `kind`, never by fighter or move id. Each fighter maps its own specials into `nspec`/`uspec`/`dspec`.
- Frame data convention: hitbox `start`/`end` are inclusive active frames; move total = last `end` + `endLag`. First listed hitbox has priority.
- Phaser 4 docs ship as AI skills in `node_modules/phaser/skills/*/SKILL.md`. Read those before using unfamiliar Phaser APIs, because v4 differs from v3 (e.g. filters replace preFX/postFX).
- `Graphics.fillPoints` typings want `Vector2[]`; wrap plain `{x, y}` arrays with `vec()` from `render/vec.ts`.
- Scenes are reused by Phaser on restart (same class instance), so reset per-run state in `create()` and don't touch objects left over from a previous run.
- Keyboard input is our own `KeyboardEvent.code` tracker (`game/input.ts`), not Phaser's. Call `consumePressed()` once per sim tick.
- Match the existing compact code style. Keep comments sparse and only where behavior isn't obvious.
