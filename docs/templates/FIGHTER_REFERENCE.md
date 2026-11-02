# <Name> reference and conversion record

Copy to `docs/imports/<id>.md`, replace placeholders, and remove template instructions.
Follow [the import workflow](../CHARACTER_IMPORTS.md).
The ledger records expected source behavior independently of fighter JSON and tests.

## Target and status

- Package/id: `assets/fighters/<id>/fighter.json`; display name: <name>.
- Branch: <branch>; source game/patch: <game and version>.
- Intended identity and fidelity: <movement, rhythm, range, recovery; what should match closely>.
- State: <research / authoring / validation / ready>; remaining work: <explicit list>.

## Sources

| Source | URL / local path | Patch / commit / hash | Access date | What it establishes |
| --- | --- | --- | --- | --- |
| Frame-data page | <url> | <version or unknown> | <date> | <timing/damage/etc.> |
| Per-frame visuals | <original links and retained inputs> | <revision/hash> | <date> | <coverage/pose/timeline> |
| Numeric scripts / stats | <url/path> | <patch and pinned commit> | <date> | <geometry/launch/etc.> |

Acquisition/generation command and retained inputs: <paths, just recipe, regeneration scope>.
Missing evidence: <properties requiring approximation or further research>.

### Disagreements

| Property / move | Source A | Source B | Chosen value and reason |
| --- | --- | --- | --- |
| <property> | <value + citation> | <value + citation> | <decision> |

## Conversion rules

| Property | Source units / convention | Destination mapping | Evidence / limitation |
| --- | --- | --- | --- |
| Frames | <gameplay vs animation; motion rates> | <phase map and entry convention> | <example boundary> |
| Geometry | <root/bone axes, origin and scale> | <forward X / up Y, px/unit> | <numeric or fitted transforms> |
| Capsules / priority | <segments, masks, hit ids> | <circle spacing, ordering, situations> | <coverage and overlap checks> |
| Poses / weapon | <visuals or animation data> | <tracks and angle unwrapping> | <fitted vs measured> |
| Movement | <walk/run/air/gravity/jump units> | <independent scaling and targets> | <traversal checks> |
| Weight / knockback | <source formulas and codes> | <multiplier, bkb/kbg, hit types> | <adaptations> |
| Hurtboxes / body | <available volumes> | <authored/inherited groups and collision body> | <limitations> |

Label properties as **converted** (numeric evidence + stated transform), **fitted** (estimated from
visuals), **shared** (engine/base behavior), **adapted** (intentional gameplay change), or **unknown**.

## Move ledger

Cover every destination slot, all four throws, jab stages, charge levels, ground/air variants and
hit/whiff/landing branches. Mark inherited or omitted behavior explicitly. Expand tables per move if needed.

| Slot / variant | Source citation | Startup / active / total | Damage / launch / mask / priority | Landing / autocancel / charge | Phases / classification |
| --- | --- | --- | --- | --- | --- |
| <slot> | <source and section/script> | <inclusive windows> | <variants and parameters> | <boundaries> | <mapping and classification> |

Special behavior: <movement events, armor, counter, resource restoration, helpless/landing endings>.
Shared mechanics: <pummel, dodge/shield/ledge/input rules; differences from the source game>.

## Validation and handoff

- Reference tests: <target/functions, expected values and boundary coverage>.
- Raw-input tests: <jump+A, attack-stick modes, buffering, holds/releases, recovery>.
- Stage and CPU coverage: <roster entries or dedicated tests, stages/seeds/seats>.
- New engine semantics: <data types/validation/editor support/tests; compatibility and SIM_VERSION decision>.
- Commands and outcomes: <focused tests, bless, check, optional balance; dates>.
- Visual inspection: <capture paths, frames/variants actually opened and reviewed>.
- Controller playtest: <device/build and observations; state explicitly if not performed>.
- Known approximations / remaining work: <explicit list>.
- Final delivery: <branch, commit if any, local vs pushed>.
