# Stick Smash

A small Smash-style platform fighter: **Phaser 4 + TypeScript + Vite**. It runs in the browser and the build is plain static files.

```bash
npm install
npm run dev        # opens http://localhost:5173 with hot reload
npm test           # sim tests (headless, no browser)
npm run build      # static build in dist/ (host anywhere: itch.io, GitHub Pages, Netlify…)
npm run smoke      # headless Chrome smoke test against a running dev server (screenshots + console errors)
```

The original single-file prototype is kept at `reference/stick-smash.html`.

## Balancing workflow

All tuning lives in `src/data/`. Run `npm run dev`, start a **Training** match, edit a data file and save.
The new values apply to the running match without a reload. You'll see a green "Tuning data reloaded" toast.

| File | What's in it |
|---|---|
| `src/data/global.ts` | Formulas and shared rules: knockback, **hitstun**, hitlag, shield (damage/stun/pushback), dodges, tech, ledge, grabs, landing lag |
| `src/data/fighters/brawler.ts`, `swordfighter.ts`, `king.ts` | Per-fighter `stats` (weight, speeds, jumps, gravity) and every move's frame data |
| `src/data/fighters/common.ts` | Default stats, grabs and throws shared by every fighter |
| `src/data/types.ts` | Documented shape of all of the above |
| `src/data/stage.ts` | Stage geometry, platforms, blast zones, ledge-grab region |

### Move frame data

```ts
ftilt: {
  name: 'Forward Tilt', endLag: 15,
  hitboxes: [{ start: 7, end: 11, x: 46, y: 42, r: 22, damage: 9, bkb: 5, kbg: 0.12, angle: 38 }],
},
```

- **Startup** = the first hitbox's `start`. **Active** = `start`–`end` (inclusive).
- **End lag** = `endLag`: frames after the last active frame before you can act. Total length = last `end` + `endLag`.
- **Landing lag** (aerials) = `landingLag`.
- **Knockback** = `(bkb + victim% × kbg × 1.4) × victimWeight`, launched at `angle` (0 forward, 90 up, negative = spike).
- **Hitstun** = `floor(knockback × 1.7) + 4` (globals in `global.ts → hitstun`), then per-hitbox `hitstunMult` / `hitstunAdd`.
- Per-hitbox extras: `hitlagMult`, `shieldDmgMult`, `shieldStunMult`, `dir: 'away' | 'back'`.
- Smash attacks: `charge: { frame, max, button }`. Charge scales damage via `knockback.chargeDamageDivisor`.
- Special moves use a `kind` (`projectile`, `gordo`, `groundPound`, `counter`, `superJump`) with their own documented fields.
- When hitboxes overlap the victim, the **first listed** hitbox wins (sweetspots go first).

### Training mode

Pick **Mode → Training** in the menu.

| Key | Action |
|---|---|
| `T` | Freeze / unfreeze |
| `Y` | Step one frame while frozen (inputs pressed while frozen apply to the stepped frame) |
| `M` | Speed 1× / 0.5× / 0.25× |
| `R` | Reset positions and damage |
| `G` | Dummy behavior: stand / shield / jump / CPU / P2 controls |
| `[` `]` | Dummy damage −10 / +10 |
| `H` | Debug view on/off (works in every mode) |

The panel shows your current move's frame and its startup/active/end lag, plus the **frame advantage** of the last hit, on hit or on shield. Positive means the attacker can act first. It also shows true-combo counts and combo damage.

The debug view draws hurtboxes (yellow; cyan = invulnerable; orange = hitstun), active hitboxes (red; purple = grab), and each hitbox's launch angle (line length grows with base knockback). It also shows ledge-grab zones and per-fighter state/frame/hitlag/hitstun labels.

## Controls

- **P1:** A/D move · W jump · S fall/drop · J attack · K special · I grab · L shield
- **P2:** arrows · `.` attack · `/` special · `;` grab · `'` shield (Numpad 1/2/3/0 also work)
- Tap a direction + attack = smash (hold to charge). Shield + tap direction = roll/spot dodge. Press shield in the air = air dodge, and just before landing = tech.

## Architecture

```
src/data/      tuning data (hot-reloaded through DATA in data/index.ts)
src/sim/       deterministic game logic: no Phaser, no DOM, no Math.random
  core.ts        types, world, accessors, RNG, effect events
  step.ts        one 60Hz tick
  states.ts      fighter state machine
  moves.ts       attacks + special-move behaviors
  combat.ts      hitboxes, hits, shields, grabs, counters
  physics.ts     movement, stage collision, landing, ledges
  projectiles.ts energy ball + gordo
  ai.ts          CPU
src/render/    Phaser Graphics drawing (poses, fighters, fx, HUD, debug overlay)
src/scenes/    MenuScene, FightScene (fixed-timestep loop, pause, training)
src/game/      keyboard, synthesized audio, saved settings
```

The sim is a pure function of `(world, inputs)`. It emits visual/audio effects as events (`world.fx`), which the renderer consumes. That makes it testable headlessly and ready for rollback netcode.

Online play from the prototype isn't ported yet. The deterministic sim was built so it can be added later.
