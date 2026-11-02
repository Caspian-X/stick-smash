# Playtesting

How to run a playtest session, the tools that help, how to capture what went wrong, and how to tune what you find.
Controls and the basic flow are in the [README](../README.md#how-to-play).

## Running a session

- **Use a release build**: `just release`. Debug builds (`just run`, `just dev`) are fine for checking menus, but
  timing and feel should be judged at full speed. Run it from the repository root (the game reads `assets/` from
  the working directory).
- **Devices**: by default the keyboard is one player. For two keyboard players, set Settings → Keyboard → *Two
  players (split)*: WASD + J K I L + Space + Esc on the left, arrows + . / ; ' + Right Shift + Enter (or the numpad)
  on the right. Gamepads join with any button. Up to 8 players.
- **Filling the lobby**: on character select, point at **Add CPU** and press attack. Each CPU card has a fighter,
  a level (Easy, Normal, Hard) and, with Teams on, a team. Your picks, CPUs and rules are remembered for next time.
- Turn on Settings → *Hitboxes in versus* if you want the debug overlay in every match, or press **F1** in a match
  to toggle it.

## Tools

### Training

Title → **Training**: you against a dummy. On character select pick your fighter, the
dummy's fighter and behaviour on its card, and the stage on the rules panel. Hitboxes start on.

The **Training** panel (top right; click its title to collapse it) shows:

- your current move, phase and frame (or your state when not attacking),
- the last hit, whether it was on hit or on shield, and its **frame advantage**: green when you can act first,
  red when the dummy can. It's the dummy's first actionable frame minus yours.
- the **combo** counter and its damage when more than one hit connected.

And controls for:

- **Dummy**: *Stand*, *Shield*, *Jump*, *CPU* (with a level) or *Second player* (press a button on another device to
  take it over).
- **Damage**: the dummy's percent (0–300%).
- **Reset positions** puts both fighters back at their spawns.
- **Speed**: 1×, 0.5×, 0.25×. Sounds keep their pitch.
- **Freeze / Step**: stop time and advance one frame at a time.
- **Hitboxes** overlay toggle (same as F1).

The pause menu (Start / Esc) has the dummy settings too, for when you're on a gamepad.

### Debug overlay

**F1** toggles hitboxes, hurtboxes and the rest of the debug overlay in any match.

### Editor → Test

In the editor, **▶ Test** (⌘↵) starts training with your edits, saved or not. On the stage editor there's also
**▶ Match** (⇧⌘↵): three stocks against a CPU. Pause → **Back to editor** returns with everything as you left it.
This is the fastest loop for move and stage tuning. See [MODDING.md](MODDING.md#the-editor).

### `just balance`

```bash
just balance            # every fighter, 16 seeds per matchup
just balance 4 lancer   # one fighter, 4 seeds (quicker)
```

Runs headless Hard-CPU duels (two stocks) for each fighter against every other fighter on every stage, swapping
sides between seeds, and prints one `BALANCE <fighter> {json}` line per fighter:

- `wins`, `matches`, `unfinished` (matches that hit the time cap, also printed as `TIMEOUT` lines; the run fails if
  any match doesn't finish),
- `dealt` / `taken` damage, `deaths`, `self_kos` (deaths with no hit in the last 4 seconds), `recovery_deaths` (died
  shortly after going helpless or using up special off stage),
- `moves`, per move: `starts`, `hits`, `damage`, `kos`, `tips` (hits with a sweet-spot hitbox), `blocked`, `whiffs`
  and `punished_whiffs` (whiffs that got hit before the move ended), `punish_starts` / `punish_hits` (started while
  the opponent was in its endlag), and `start_distance` (total distance at start; divide by `starts`).

CPU numbers are a smoke test, not the truth: they flag moves nobody can land, recoveries that fail and matches that
stall. `cargo test -p sim --test stages -- --nocapture` also prints per-stage KO, self-destruct and camera stats.

## Capturing issues with replays

Every versus match is recorded. When a match ends:

- it's **auto-saved** to the replays folder; only the newest 10 auto-saves are kept (`global.replay.autoSaveCount`
  in `assets/global.json`),
- **Save replay** on the results screen saves a copy that's never cleaned up. Do this for anything worth a bug report.
- **Watch replay** plays it straight away.

Replays live in:

| Platform | Folder |
|---|---|
| macOS | `~/Library/Application Support/stick-smash/replays/` |
| Linux | `~/.local/share/stick-smash/replays/` |
| Windows | `%APPDATA%\stick-smash\replays\` |

Title → **Replays** lists them (saved ones first) to watch or delete. While watching: play / pause, frame step, speed
(0.25×–2×), hide the HUD, and seek with left / right (one second) or by dragging the bar. Note the tick number
shown on screen when you report something.

Replays are tied to the build that recorded them. When the simulation changes, older replays show "recorded with a
different version" and can't be played, so attach the replay *and* the commit you were on.

To check that a folder of replays still plays back identically (no desync) and every match finishes:

```bash
just verify-replays ~/Library/Application\ Support/stick-smash/replays
```

## Session checklist

- **Movement and feel**: walk, dash, turnaround, jump heights (short hop, full hop, air jumps), fast fall, landing
  lag, platforms (land, drop through), ledge grab and getup options, shield, roll, spot-dodge, air-dodge, techs.
- **Every fighter's kit**: Archer, Brawler, Dancer, Golem, King, Lancer, Ninja, Swordfighter, Tinker, Wizard,
  Wrangler, Yo-yo. Tilts, smashes (charged), aerials, specials, grab and throws, and their recovery from far away.
- **Every stage**: Arena, Citadel, Tidebreak Isles, Foundry Lifts (moving platforms). Blast zones, camera framing
  with players far apart, respawn platforms.
- **CPUs** at Easy, Normal and Hard: do they recover, approach, shield, and finish matches?
- **Select flow with several devices**: joining, dropping and picking up chips, Add CPU, carrying a CPU's chip,
  the options popover (Grab / Shield), Teams and Friendly fire, the random tile, stage voting and the roulette,
  backing out from stage select.
- **Disconnects**: unplug a gamepad on character select (an unready player leaves; a ready player's slot waits and
  the next device that presses a button reclaims it), on stage select (back to character select) and mid-match.
- **Pause**: only the device that paused controls the menu; unplug it and control moves to another device.
- **Stage too small for the lineup**: make a stage with fewer spawns in the editor and check it's dimmed for big
  lineups (all built-in stages take 8).
- **Results and replays**: rematch, save, watch, seek, back to results.

## Reporting

```markdown
**What happened**:
**What I expected**:
**Build**: commit `git rev-parse --short HEAD`, release / debug
**Where**: stage, fighters, CPU levels, devices (keyboard / which gamepad)
**Replay**: attached `saved-….replay.json`, around tick ____
**How often**: every time / sometimes / once
**Feel notes** (optional): too floaty, too slow, hard to read, …
```

For feel feedback, name the move or action and what you'd change ("Golem's down smash comes out too slowly to
punish anything") rather than a number.

## The tuning loop

Where the numbers live:

- `assets/global.json`: shared gameplay tuning in sections (`movement`, `knockback`, `hitstun`, `hitlag`,
  `hitTypes`, `shield`, `dodge`, `landing`, `tech`, `knockdown`, `ledge`, `grab`, `respawn`, `match`, `input`,
  `sim`, `ai`), presentation in `render` (`camera`, `overlays`, `fx`, `hud`, `select`, `interpolate`), `audio`,
  `palette` and `replay`.
- `assets/fighters/<id>/fighter.json`: a fighter's stats, moves, throws, poses and CPU behaviour. Shared defaults are
  in `fighters/base`.
- `assets/stages/<id>/stage.json`: stage geometry and art.

[FORMAT.md](FORMAT.md) documents every field; the schemas give hover docs in your editor.

To try changes without touching the repository, put copies in your [content folder](MODDING.md#your-content-folder):
they override the built-in files by path (`global.json` is replaced whole, so copy the full file). For moves and
stages, the editor's Test button is quicker still. When a change works, move it into `assets/`.

After changing gameplay numbers in `assets/`:

1. `just test`: includes the TS trace comparison (the original game's behaviour, run with newer features switched
   off) and a golden CPU-match hash. If one fails, your change altered behaviour the port has to match: either put
   the change behind a setting that the traces turn off, or update the expectations deliberately. See
   [DEVELOPMENT.md](DEVELOPMENT.md).
2. `just balance` to compare matchups before and after.
3. `just verify-replays <dir>` on your saved replays; changes to simulation behaviour invalidate them by design.
