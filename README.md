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
- **Knockback** = `(bkb + victim% × kbg × 1.4) × victimWeight`, launched at `angle` (0 forward, 90 up, negative = spike). `setKb: true` = fixed knockback (`bkb × weight`, ignores %).
- **Hitstun** = `floor(knockback × 1.7) + 4` (globals in `global.ts → hitstun`), then per-hitbox `hitstunMult` / `hitstunAdd`.
- Per-hitbox extras: `hitlagMult`, `shieldDmgMult`, `shieldStunMult`, `dir: 'away' | 'back'`, `rehit: N` (multi-hit: can hit the same victim again every N frames).
- **Per-frame boxes:** `frames: [[frame, x, y, r], …]` gives the box its own position/size on each frame. A frame with no entry uses the closest earlier one.
- Smash attacks: `charge: { frame, max, button }`. Charge scales damage via `knockback.chargeDamageDivisor`.
- Special moves use a `kind` (`projectile`, `gordo`, `groundPound`, `counter`, `superJump`) with their own documented fields.
- When hitboxes overlap the victim, the **first listed** hitbox wins (sweetspots go first).

### Hitbox types

Set with `type` on a hitbox (default `normal`). Their tuning is in `global.ts → hitTypes`.

| Type | Effect |
|---|---|
| `normal` | Launches at `angle`. |
| `spike` | Meteor: always launches downward (a positive angle is mirrored). Grounded victims bounce up. |
| `trip` | Grounded victims are knocked prone (knockdown with getup options). Airborne victims are hit normally. |
| `sakurai` | Automatic angle: flat on weak grounded hits, higher on strong ones, 45° in the air. |
| `autolink` | Pulls the victim toward the box center, plus the attacker's momentum. Pair it with `rehit` for multi-hit moves. |
| `windbox` | No damage, hitstun or hitlag. Pushes anything it touches every frame along `angle`, with strength `bkb`. |
| `electric` | Longer hitlag for the victim, with sparks. |
| `grab` | Grabs instead of hitting. |

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
| `E` | Open the hitbox editor on your fighter's last move (dev server only) |

The panel shows your current move's frame and its startup/active/end lag, plus the **frame advantage** of the last hit, on hit or on shield. Positive means the attacker can act first. It also shows true-combo counts and combo damage.

The debug view draws hurtboxes (yellow; cyan = invulnerable; orange = hitstun), active hitboxes colored by type, and each hitbox's launch angle (line length grows with base knockback). It also shows the half-circle ledge-grab zones (a fighter grabs when their hand point, the cyan dot, enters one) and per-fighter state/frame/hitlag/hitstun labels.

### Hitbox editor (dev)

Pick **Mode → Hitbox Editor** in the menu, or press `E` in Training. Press `?` inside it for every shortcut.

- **Layout:** fighter tabs and Save on top, the move list on the left, the posed fighter in the middle, the inspector on the right, and the timeline at the bottom.
- **In the view:** click a box to select it, drag it to move it, drag its edge to resize it, and drag the arrow tip to aim (Shift snaps). Double-click empty space to add a box there. Drag empty space to pan, and use the wheel to zoom. `Alt+arrows` nudges a box.
- **Per-frame positions:** in the inspector's *Position & size*, choose **Per frame**. Every active frame starts at the current position. Scrub to any frame and drag the box: only that frame changes. **Apply to all frames** copies the current frame to every frame. **Copy → next frame** (`C`) copies it forward and steps there. Onion skin (`O`) shows the box's path and neighbouring frames.
- **Types:** pick Regular, Spike, Trip, Sakurai angle, Autolink, Windbox, Electric or Grab. Each one sets sensible defaults when you choose it.
- **Inspector:** number fields can be scrubbed by dragging their label (Shift = ×10). There's an angle dial, multi-hit, fixed knockback and advanced multipliers. A preview shows knockback, hitstun and tumble at 0/50/100/150%, plus the move's frame advantage on shield.
- **Timeline:** drag a bar to move its active frames, drag its edges to resize it, and drag the end of the blue band to change end lag. Diamonds mark per-frame keys; amber ones differ from the frame before.
- **Reference dummy** (`G`): a hurtbox you can drag around. It turns red when a box would hit it.
- `⌘/Ctrl+Z` undoes, `⌘/Ctrl+Shift+Z` redoes, and `⌘/Ctrl+S` saves.

Edits apply live, so Training uses them immediately. **Save** writes them into `src/data/fighters/<fighter>.ts` through a dev-server endpoint (`vite.config.ts`). It only rewrites that move's `hitboxes`, `endLag` and `landingLag`, so comments and everything else stay as they are. A move inherited from `common.ts`, like grab, is saved as a new entry in that fighter's own file.

## Controls

- **P1:** A/D move · W or Space jump · S fall/drop · J attack · K special · I grab · L shield. The menu option **P1 W Jump → Off** makes W up-only (tilts/aerials/specials), so only Space jumps.
- **P2:** arrows · `.` attack · `/` special · `;` grab · `'` shield (Numpad 1/2/3/0 also work)
- Jumps are a **short hop** (jump released by the end of jumpsquat) or a **full hop** (still held). Height is fixed after takeoff.
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
src/editor/    hitbox editor: EditorScene (viewport), model.ts (state + edit ops + undo), ui.ts / inspector.ts / timeline.ts / dom.ts (DOM panels), source.ts (rewrites fighter files)
src/game/      keyboard, synthesized audio, saved settings
```

The sim is a pure function of `(world, inputs)`. It emits visual/audio effects as events (`world.fx`), which the renderer consumes. That makes it testable headlessly and ready for rollback netcode.

Online play from the prototype isn't ported yet. The deterministic sim was built so it can be added later.
