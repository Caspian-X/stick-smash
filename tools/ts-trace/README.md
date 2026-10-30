# TS reference traces

Run from the Rust checkout with **Node 26** and the read-only sibling `../stick-smash`:

```sh
node tools/ts-trace/record.mjs
node tools/ts-trace/record.mjs --verify
```

The recorder imports the actual TS sim/data using native type stripping and the converter's `module.register` extensionless/directory resolve hook. It changes no TS, converter, or asset files. Normal execution writes `crates/sim/tests/fixtures/ts/<name>.json`, `coverage.json`, and [REPORT.md](REPORT.md). Verification checks existing fixture bytes without writing. Both commands independently build the whole suite twice and require byte-identical output. Scenario assertions run before writing. No dependencies or browser are needed; Node 26 currently prints a deprecation warning for the deliberately reused register hook.

## Replay contract

- Each input tick contains two `[sx,sy,buttons]` triples. Integer stick axes range from -127 through 127; **positive y means up**. Buttons: jump=1, attack=2, special=4, grab=8, shield=16. Scripts/fuzz use digital -127/0/127; derivation also checks threshold boundaries at ±63/64.
- Direction thresholds are inclusive ±64. Presses are held-state rising edges per player; history starts released. Last direction press ticks start at -99 and update **before** taps. Taps require still-held direction and `w.frame + 1 - lastPress <= DATA.g.movement.tapWindow` (currently 5).
- Jump presses/holds include up only when that player's `tapJump` is true. Attack/special expose both press and hold; grab exposes press; shield exposes hold and press; down exposes hold and press. `anyP` includes all direction/button rising edges.
- `readHuman` uses `i !== 0 || S.wJump`. **The actual checkout's settings default is `wJump: true`**, unlike the task's stated default. Fixtures deliberately follow the requested shared contract: P1 `tapJump: false` (`wJump=false`), P2 `true`. Every fixture records those values, including CPU slots.
- Human history advances every tick, including countdown, hitlag, dead/respawn, and match-end freezes. CPU slots skip human derivation; `stepWorld` selects AI and suppresses inputs during its own frozen frames. `countdown-held-input` covers presses consumed before GO. CPU raw inputs are zero.
- Fighter IDs use the asserted TS order: brawler=0, swordfighter=1, king=2. All fixtures use the arena, seed/config exactly as recorded, two players, and three starting stocks.
- Apply `config.setup` **once immediately after createWorld**, before tick 1. There are no subsequent state edits. `x/y` and `vx/vy` use TS pixels and y-down coordinates; `face` is ±1; `dmg` is percent. Air setups explicitly set `state: "air", ground: false, surf: null`, retaining all other creation defaults (including jumps). No other state/surface overrides are used. [REPORT.md](REPORT.md) lists every exact override. Most setups only move the dummy; tech/getup setups use a 120% victim and an airborne attacker that lands a real down-air hit.
- Frames are snapshots **after** each `stepWorld` call. Fighter tuples are `[x,y,vx,vy,state,dmg,stocks,lag,hitstun,face,moveSlotOrNull,charge]`; absent moves have null slot and zero charge. Projectile tuples preserve TS array order and use owner **index**, not fighter ID. RNG is normalized with `>>> 0`. `w.fx` is drained every tick, after observing scenario evidence. Stop immediately at `w.over`; the stock KO fixture asserts it reaches that state.
- Numbers use unrounded `JSON.stringify` precision; non-finite snapshots fail. Each frame occupies one line. No timestamps or platform-dependent paths enter fixtures.

## Coverage and limits

The suite includes all ground normals, all five aerials, tap smashes and a held charge, all specials (sword side special included), every throw with pummel, shield hits, both rolls, spot dodge, neutral/directional air dodge, fast fall, natural platform drop, and short/full hops for every fighter. Ground and aerial groups run several moves sequentially: the initially adjacent dummy can be knocked away, so **every move executes, but every move is not guaranteed to connect**. Separate throw/shield fixtures preserve reliable contact.

Focused cases cover brawler up-special landing/ledge, ground/air pound and offstage timeout before KO; King super-jump landing and plunge cancel, charged Jet Hammer, Gordo bounce/bat-back; sword melee counter and energy-ball reflection; all ledge options plus walking offstage; real-hit tech/tech-roll and missed-tech getup options; shield decay through break/dizzy; training and stock KO/respawn, match completion, and countdown input history. Assertions check key moves/states, effect events, phases, ownership, and charge. `coverage.json` records observed evidence and configs outside the shared fixture schema.

Three deterministic xorshift32 fuzz scripts hold inputs for 1–24 ticks, independently of the sim RNG, for 1,500 ticks each. All nine ordered CPU matchups run 2,500 ticks at level 2, plus brawler/King at levels 0 and 1; a scripted human also faces a CPU. CPU matches keep natural countdowns and run until the tick limit or match end.

No requested scenario proved impossible in TS. The total is **63 fixtures / 8,597,772 bytes** (about 8.20 MiB), slightly above the 8 MB aim to retain the requested CPU/fuzz durations. Exact per-fixture sizes, tick counts, and setup overrides are in [REPORT.md](REPORT.md). The TS source is imported live: changed source/data intentionally cause `--verify` to fail until fixtures are regenerated. No Rust code or tests are changed or run by this tool.
