# Modding

Fighters, stages and the game's tuning are plain JSON files. You can edit them by hand or with the built-in editor
(**Editor** on the title screen), and you never have to touch the game's own `assets/` folder to do it.
For the full field-by-field format, see [FORMAT.md](FORMAT.md).

## Your content folder

The game loads the built-in content from `assets/`, then your own content folder:

| Platform | Folder |
|---|---|
| macOS | `~/Library/Application Support/stick-smash/content/` |
| Linux | `~/.local/share/stick-smash/content/` |
| Windows | `%APPDATA%\stick-smash\content\` |

**Settings** on the title screen shows the exact path. The folder uses the same layout as `assets/`:

```
content/
  global.json                  shared tuning (optional)
  fighters/<id>/fighter.json   one folder per fighter; <id> is its id
  stages/<id>/stage.json       one folder per stage
```

A file in your folder **replaces** the built-in file with the same path. So `fighters/brawler/fighter.json` replaces
the Brawler, and a new id like `fighters/pirate/fighter.json` adds a fighter. Note that `global.json` is replaced
whole, not merged: copy the built-in one and change what you need. Content loads at startup, so restart the game
after editing files by hand (the editor reloads for you, see below).

## Packages and `extends`

Each fighter or stage folder is a *package*. A fighter can build on another one:

```json
{
  "$schema": "../../schema/fighter.schema.json",
  "schemaVersion": 1,
  "extends": "brawler",
  "name": "Heavy Brawler",
  "stats": { "weight": 1.3 }
}
```

The child is merged over its parent before it's read:

- objects merge key by key (so `"stats": { "weight": 1.3 }` keeps every other stat),
- arrays and plain values replace the parent's value whole,
- `null` deletes a key.

Every built-in fighter extends `base`, an `"abstract": true` package that holds the shared stats, grabs, throws,
hurtboxes, poses and state animations. Abstract packages aren't playable. Stages don't inherit; a stage file
is always complete.

## Schemas and autocomplete

Every file starts with `"$schema"`, a relative path to its JSON schema in `assets/schema/`. Editors such as VS Code
use it for autocomplete, hover docs and error squiggles. In your own content folder the relative path has nothing to
point at, so either:

- copy `assets/schema/` into your content folder as `content/schema/` (the loader ignores `schema` folders), or
- map the schemas in your editor, e.g. a `.vscode/settings.json` in your content folder:

```json
{
  "json.schemas": [
    { "fileMatch": ["fighters/*/fighter.json"], "url": "/path/to/stick-smash-rs/assets/schema/fighter.schema.json" },
    { "fileMatch": ["stages/*/stage.json"], "url": "/path/to/stick-smash-rs/assets/schema/stage.schema.json" },
    { "fileMatch": ["global.json"], "url": "/path/to/stick-smash-rs/assets/schema/global.schema.json" }
  ]
}
```

## When something is wrong

Content is checked when it loads. Every problem names the file and the JSON path, like
`fighters/pirate/fighter.json: moves.jab.phases: needs at least one phase`.

- A broken fighter or stage is **skipped**; everything else still loads. A toast in the bottom-right corner lists
  the first few problems for 15 seconds, and the log has all of them.
- A broken `global.json` is **fatal**: the game stops at startup and prints the errors.
- The editor runs the same checks live and refuses to save a package with errors (see below).

## The editor

The editor is for keyboard and mouse. It has two modes in one window, **Fighters** and **Stages**: switch with the
buttons in the top bar or ⌘1 / ⌘2 (Ctrl on Windows and Linux). Press `?` for the shortcut sheet of the current mode.

Both modes share:

- **Undo / redo**: ⌘Z, ⇧⌘Z (or ⌘Y). Drags and typing collapse into one step.
- **Problems**: the top bar shows "✔ valid" or "⚠ N problems"; click to jump to the field. Stages can also show
  "ℹ notes" for legal but likely-wrong setups (fewer than 8 spawns, spawns in the air, geometry past the blast
  zone, KOs off camera).
- **Save** (⌘S) and **Save all** (⇧⌘S) write into your content folder. Hover Save to see the target file. **Revert**
  goes back to the saved version (undoable). A dot (●) marks unsaved packages.
- **Test** plays your edits, saved or not, then brings you back: pause → **Back to editor** (see below).
- **Esc** deselects, then leaves. Leaving or closing the window with unsaved changes asks first. After saving,
  leaving the editor reloads the game's content so the new data is live.

### Saving writes only what you changed

The editor saves the package's *own* file, not the merged fighter. It compares against the last save and patches
only the changed values into the file, so `extends`, `$schema`, untouched values and inherited data stay as they
were. Edit one hitbox of an inherited move and the file gets just that move's override. A new fighter starts as a
thin file with just `extends` and `name`.

### Fighters

Pick a fighter in the top bar (or **+ New fighter…**: an id, a name and which fighter to extend).

- **Browser** (left): the moves grouped Ground / Smash / Air / Special / Grab, the four throws, poses and
  fighter-wide sections (stats and the rest). ⌘F searches. Right-click a move to copy it to another slot or revert
  it; right-click a pose to rename, duplicate or delete it.
- **Viewport** (centre): the move played by the real game simulation, not an approximation. Toolbar options start it
  in the air (with a start height), add a standing dummy (distance and damage) to see real hits and knockback, or
  hold the button for a number of frames to see a charge.
  - Drag a hitbox to move it, its rim to resize it, the gold knob at the tip of its launch arrow to aim it. ⇧ snaps (whole pixels,
    15°), Alt drags finely with no snapping. Click or Tab selects a hitbox; N adds one on this frame, ⌘D duplicates,
    Del deletes, Alt + arrows nudge (⇧ ×10).
  - On a pose key the joints get handles you can drag; K adds a pose key on this frame.
  - Wheel or pinch zooms, right or middle drag pans, F frames the fighter. H, I and O toggle the hurtbox, inactive
    hitboxes and the onion skin (3 frames before in red, after in green).
- **Timeline** (bottom): phases as they really played, each hitbox's active window, events and pose keys. Click or
  drag to scrub. Space plays, ← → (or , .) step a frame (⇧ ×10), Home / End jump, [ ] go to the previous / next phase,
  L loops, ↑ ↓ switch moves.
- **Inspector** (right): every field of the selection (hitbox, phase, move, throw, pose, stats), with problems shown
  next to the field. Data without a hand-made inspector (projectiles, attachments, effects, style, AI…) gets forms
  generated from the schema, with the field docs as hover help. A value that doesn't parse stays red and doesn't
  reach the fighter.
- **▶ Test** (⌘↵) opens training with your fighter against the dummy you last set up in training.

### Stages

Pick a stage in the top bar, or **+ New stage…** / **Duplicate** (a full copy of an existing stage, unsaved edits
included) / **Rename…**. Only stages that were never saved can change their id.

- **Viewport**: the real rendered stage. Click or Tab selects a part; drag to move it, and drag edges, corners or
  ends to resize. Parts are blocks (solid ground, with ledge toggles on each top corner of a selected block),
  platforms, moving-platform waypoints, spawns (click the arrow to flip which way the fighter faces), respawns,
  the ledge zone, and the blast zone and camera bounds (dashed rectangles).
  - The **add** toolbar places a block, a platform (or N), a motion waypoint on the selected platform, a spawn or
    a respawn. ⌘D duplicates, Del deletes.
  - Drags snap to whole pixels; ⇧ (or the **Grid** toggle, G) snaps to the grid, Alt moves 0.1 px. Alt + arrows
    nudge 1 px (⇧ ×10).
  - **Mirror** (M) mirrors every edit around the stage's centre: the part's mirror partner follows, centred parts stay
    centred, and a mirrored pair is deleted together.
  - B and P show the bounds and the spawn / respawn points. Wheel or pinch zooms, right drag (or dragging empty
    space) pans, F frames the stage.
  - Moving platforms have a play button and a scrub bar so you can see their path with the sandbox off.
- **Sandbox**: up to 4 CPUs (at a level you pick) play on the stage while you edit, so you see how it really plays.
  Space starts 2 CPUs (or pauses them), C cycles the count, ⟲ restarts.
- **Browser** (left) lists the stage's sections, geometry and players; the **inspector** has fields for geometry and
  schema forms (with colour pickers) for the art and the background.
- **▶ Training** (⌘↵) or **▶ Match** (⇧⌘↵, three stocks against a CPU) tests the stage.

## Tips

- Start from a copy: extend a built-in fighter, or duplicate a stage, and change one thing at a time.
- Stages need one spawn and one respawn per player; a stage with fewer is offered only to lineups that fit
  (it's dimmed on stage select).
- Behaviour differences between moves come from data (`kind` fields and phase settings), so anything a built-in
  fighter does, yours can do. [FORMAT.md](FORMAT.md) explains move phases, events, projectiles and the rest.
- For tuning workflows and testing your changes, see [PLAYTESTING.md](PLAYTESTING.md).
