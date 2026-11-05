use crate::{
    GameData,
    data::{Color, Hitbox},
    input::{Buffer, Controls, History},
    rng::Rng,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlayerConfig {
    pub fighter: String,
    pub cpu: Option<u8>,
    pub team: Option<u8>,
    pub tap_jump: bool,
    #[serde(default)]
    pub controls: Controls,
    /// Sudden death only: this side lost the time-up and starts out (no stocks, out of the fight).
    #[serde(default)]
    pub eliminated: bool,
}
impl PlayerConfig {
    pub fn human(fighter: &str) -> Self {
        Self {
            fighter: fighter.into(),
            cpu: None,
            team: None,
            tap_jump: true,
            controls: Controls::default(),
            eliminated: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MatchConfig {
    pub seed: u32,
    pub stage: String,
    pub stocks: i32,
    pub training: bool,
    pub players: Vec<PlayerConfig>,
    pub friendly_fire: bool,
    /// Time limit in seconds; `None` plays until one side has stocks left (training always does).
    #[serde(default)]
    pub time_limit: Option<u32>,
    /// Sudden death (SSBU): one stock each at `match.suddenDeathDamage`, no time limit.
    #[serde(default)]
    pub sudden_death: bool,
}
impl MatchConfig {
    /// The same lineup as sudden death: one stock each at `match.suddenDeathDamage` and no timer. The seed is kept,
    /// so the same inputs play out the same way.
    pub fn sudden_death(&self) -> Self {
        Self { stocks: 1, time_limit: None, sudden_death: true, ..self.clone() }
    }
}
impl MatchConfig {
    /// Distinct teams plus one side per unteamed player. Match lobbies should require at least two.
    pub fn opposing_sides(&self) -> usize {
        self.players.iter().filter(|p| p.team.is_none()).count()
            + self.players.iter().filter_map(|p| p.team).collect::<std::collections::BTreeSet<_>>().len()
    }
}
impl MatchConfig {
    /// One-stock training world with the supplied stage, lineup and deterministic seed.
    pub fn sandbox(stage: impl Into<String>, players: Vec<PlayerConfig>, seed: u32) -> Self {
        Self {
            stage: stage.into(),
            players,
            seed,
            training: true,
            stocks: 1,
            friendly_fire: false,
            time_limit: None,
            sudden_death: false,
        }
    }
}
impl Default for MatchConfig {
    fn default() -> Self {
        Self {
            seed: 12345,
            stage: "arena".into(),
            stocks: 3,
            training: false,
            players: vec![PlayerConfig::human("brawler"), PlayerConfig { cpu: Some(2), ..PlayerConfig::human("king") }],
            friendly_fire: false,
            time_limit: None,
            sudden_death: false,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Fx {
    Sfx { name: String, arg: Option<f64>, x: Option<f64> },
    Burst { x: f64, y: f64, color: Color, player: usize, n: f64, speed: f64, life: f64, size: f64, gravity: f64 },
    Dust { x: f64, y: f64, n: u32 },
    Ring { x: f64, y: f64, r: f64, grow: f64, life: u32, color: Color, player: usize },
    Beam { x: f64, y: f64, player: usize },
    Part { x: f64, y: f64, vx: f64, vy: f64, life: f64, color: Color, player: usize, size: f64, gravity: f64 },
    Shake { k: f64 },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum State {
    #[default]
    Idle,
    Jumpsquat,
    Air,
    Attack,
    Helpless,
    Landing,
    Shield,
    Roll,
    Spot,
    Airdodge,
    Hitstun,
    Knockdown,
    Getup,
    Tech,
    Ledge,
    Climb,
    Hold,
    Throw,
    // Throwing a held item (`sd.kind`: `f`, `b`, `u`, `d` or `drop`; `sd.n` 1 for a smash throw).
    Toss,
    Grabbed,
    Shieldbreak,
    Dizzy,
    Buried,
    Respawn,
    Dead,
    Out,
    Win,
}
impl State {
    pub fn name(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Jumpsquat => "jumpsquat",
            Self::Air => "air",
            Self::Attack => "attack",
            Self::Helpless => "helpless",
            Self::Landing => "landing",
            Self::Shield => "shield",
            Self::Roll => "roll",
            Self::Spot => "spot",
            Self::Airdodge => "airdodge",
            Self::Hitstun => "hitstun",
            Self::Knockdown => "knockdown",
            Self::Getup => "getup",
            Self::Tech => "tech",
            Self::Ledge => "ledge",
            Self::Climb => "climb",
            Self::Hold => "hold",
            Self::Throw => "throw",
            Self::Toss => "toss",
            Self::Grabbed => "grabbed",
            Self::Shieldbreak => "shieldbreak",
            Self::Dizzy => "dizzy",
            Self::Buried => "buried",
            Self::Respawn => "respawn",
            Self::Dead => "dead",
            Self::Out => "out",
            Self::Win => "win",
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct StateData {
    hit_at: Vec<(usize, u8, u32)>,
    pub n: f64,
    pub kind: String,
    pub side: f64,
    /// Block index while hanging or climbing.
    pub ledge_block: usize,
    pub dir: f64,
    pub dirl: bool,
    pub dx: f64,
    pub dy: f64,
    pub victim: Option<usize>,
    pub by: Option<usize>,
    pub timer: f64,
    pub pt: u32,
    pub stun: i32,
    pub jump_aerial: Option<String>,
    /// Jumpsquat ends in a short hop regardless of the jump button (second jump press).
    pub short: bool,
    /// World frame the state was entered (ledge hang, for trump timing).
    pub entered: u64,
    /// Dodge length after staling (0 = the global `frames`).
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub end: u32,
    /// Invulnerability starting later in this state: `inv_len` frames from state frame `inv_at`.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub inv_at: u32,
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub inv_len: u32,
}
impl StateData {
    pub(crate) fn can_hit(&self, victim: usize, group: u8, tick: u32, rehit: Option<u32>) -> bool {
        self.hit_at
            .iter()
            .find(|(v, g, _)| *v == victim && *g == group)
            .is_none_or(|(_, _, at)| rehit.is_some_and(|n| tick - *at >= n))
    }
    pub(crate) fn record_hit(&mut self, victim: usize, group: u8, tick: u32) {
        if let Some((_, _, at)) = self.hit_at.iter_mut().find(|(v, g, _)| *v == victim && *g == group) {
            *at = tick;
        } else {
            self.hit_at.push((victim, group, tick));
        }
    }
    pub(crate) fn clear_hits(&mut self) {
        self.hit_at.clear();
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MoveState {
    pub slot: String,
    #[serde(default)]
    pub started: u64,
    pub phase: usize,
    /// Phase timeline chosen at move start from `Move.variants` (0 = the default `phases`).
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub variant: u8,
    pub t: u32,
    pub elapsed: u32,
    pub charge: u32,
    /// Saturated charge hold time, separate from strength.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub charge_hold: u32,
    pub counter_damage: f64,
    pub reversed: bool,
    /// The attack stick (not the button) started it, so only the stick holds its charge.
    #[serde(default)]
    pub stick: bool,
}
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct AiState {
    pub cool: f64,
    pub wait: f64,
    pub shield_t: f64,
    pub charge_t: f64,
    pub sp_t: f64,
    pub up_t: f64,
    pub hold_t: f64,
    pub pum: f64,
    #[serde(default)]
    pub pursuit_ticks: u32,
    #[serde(default)]
    pub pursuit_x: f64,
    #[serde(default)]
    pub pursuit: bool,
    /// Ticks a held item has been carried (CPU throws it after `items.cpuHold`).
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub item_t: u32,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fighter {
    pub data: usize,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub face: f64,
    pub dmg: f64,
    pub last_hit_by: Option<usize>,
    pub stocks: i32,
    pub ground: bool,
    pub surf: Option<usize>,
    pub jumps: u32,
    pub state: State,
    pub t: u32,
    pub sd: StateData,
    pub mov: Option<MoveState>,
    pub hitstun: i32,
    pub tumble: bool,
    pub spin: f64,
    pub lag: u32,
    pub inv: u32,
    pub blink: u32,
    pub up_b: bool,
    pub ad_used: bool,
    pub ff: bool,
    pub drop: u32,
    pub ledge_cd: u32,
    pub run_ph: f64,
    pub flip_t: u32,
    pub flash: u32,
    pub hud_shake: u32,
    pub sh: f64,
    pub last_sp: i64,
    pub in_dir: f64,
    pub in_down: bool,
    pub ai: AiState,
    pub history: History,
    pub buffer: Buffer,
    /// Ground gait while idle (reset by every state change).
    pub gait: Gait,
    pub gait_t: u32,
    /// A launch waiting for directional influence when hitlag ends.
    pub di: bool,
    /// The next ledge grab grants invulnerability (see `ledge.freshInvuln`).
    pub ledge_inv: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub used_air_moves: Vec<String>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub paralyze_used: bool,
    /// Remaining uses of each content charge pool, in content order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charges: Vec<u32>,
    /// Ticks until each pool is full again (0 = not recharging).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub charge_wait: Vec<u32>,
    /// The part of `vx`/`vy` left from the last launch (`hitstun.ultimate`), shrinking linearly to zero. Kept through
    /// hitstun and the air state after it; any other state, landing, a ledge or an air jump clears it.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub launch: (f64, f64),
    /// World frame until which shield presses in hitstun cannot tech (`tech.lockout`).
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub tech_lock: u64,
    /// Recent dodges in a row (`dodge.staling`) and the world frame the last one started.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub dodge_stale: u8,
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub dodge_at: u64,
    /// Ticks left of a full hop's initial rise (`stats.jumpInitialHeight`); the last one launches at `jumpSpeed`.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub rise: u8,
    /// The current launch uses `hitstun.ultimate.topLaunch` physics (and, after hitstun, slows back to its own fall
    /// speed).
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub top_launch: bool,
    /// Float ticks left (`stats.float`); 0 = not floating.
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub floating: u32,
    /// The float was used this airtime (landing restores it).
    #[serde(default, skip_serializing_if = "crate::data::common::is_default")]
    pub float_used: bool,
    /// The item in hand (`items.rs`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<Held>,
}
/// An item by content: the package (`GameData::fighters` index) that defines it and its id there.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Held {
    pub data: usize,
    pub id: String,
}
/// An item lying in the world or falling freely (not thrown): no hitbox, picked up or caught by anyone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LooseItem {
    pub item: Held,
    /// Last thrower (who cannot catch it again for `items.regrab` ticks).
    pub owner: usize,
    /// Centre of the item.
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    /// Resting on this surface.
    pub surf: Option<usize>,
    pub life: u32,
    pub age: u32,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Gait {
    #[default]
    Stand,
    Walk,
    Dash,
    Run,
    Turn,
    Brake,
    Crouch,
    Rise,
}
impl Fighter {
    pub fn set_state(&mut self, state: State) {
        self.state = state;
        self.t = 0;
        self.sd = StateData::default();
        self.gait = Gait::Stand;
        self.gait_t = 0;
        if state != State::Attack {
            self.mov = None;
        }
        if !matches!(state, State::Hitstun | State::Air) {
            self.launch = (0., 0.);
            self.top_launch = false;
        }
        // An aerial keeps the initial rise and the float (`moves::start_move` ends them for other moves).
        if !matches!(state, State::Air | State::Attack) {
            self.rise = 0;
            self.floating = 0;
        }
    }
    pub fn hittable(&self) -> bool {
        !matches!(self.state, State::Dead | State::Respawn | State::Grabbed | State::Out | State::Win)
            && self.inv == 0
            && self.blink == 0
    }
    /// The hitstun cancel rule that applies to this fighter right now: airborne in tumble with one configured.
    pub fn hitstun_cancel(&self, g: &crate::data::global::Global) -> Option<crate::data::global::HitstunCancel> {
        g.hitstun.ultimate.as_ref().and_then(|u| u.cancel).filter(|_| self.tumble && !self.ground)
    }
    /// Can take a new action this tick (before the state clock advances). Shared by buffering, frame advantage
    /// and the editor sandbox.
    pub fn actionable(&self, g: &crate::data::global::Global) -> bool {
        self.lag == 0
            && match self.state {
                State::Idle | State::Air => true,
                State::Shield => self.sd.stun <= 0,
                State::Ledge => self.t.saturating_add(1) >= g.ledge.min_hang,
                State::Knockdown => self.t >= g.knockdown.min_frames,
                State::Hitstun => self
                    .hitstun_cancel(g)
                    .is_some_and(|c| self.t.saturating_add(1) >= c.air_dodge_after.min(c.aerial_after)),
                _ => false,
            }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Projectile {
    pub owner: usize,
    pub fighter_data: usize,
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub vx: f64,
    pub vy: f64,
    pub life: i32,
    pub r: f64,
    pub hit: Hitbox,
    pub bounces: u32,
    pub cooldown: u32,
    /// Elapsed ticks, independent of fuse resets and bounce limits.
    #[serde(default)]
    pub age: u32,
    #[serde(default)]
    pub born: u64,
    #[serde(default)]
    pub surf: Option<usize>,
    #[serde(default)]
    pub released: Option<u32>,
    #[serde(default)]
    pub automatic: bool,
    #[serde(default)]
    pub stage: usize,
    #[serde(default)]
    pub face: f64,
    #[serde(default)]
    pub damage: f64,
    /// Victim and projectile age of the last contact, shared by overlapping article hitboxes.
    #[serde(default)]
    pub hits: Vec<(usize, u32)>,
    #[serde(default)]
    pub damage_hits: Vec<(usize, u64, usize, u32)>,
    #[serde(default = "unit_multiplier")]
    pub power: f64,
    #[serde(default = "unit_multiplier")]
    pub speed: f64,
    /// A thrown item (id in `fighter_data`'s `items`): it becomes a loose item when it lands, connects or expires.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<String>,
}
fn unit_multiplier() -> f64 {
    1.
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub attacker: usize,
    pub defender: usize,
    pub label: String,
    pub on_shield: bool,
    #[serde(default)]
    pub armored: bool,
    pub hit_frame: u64,
    pub a_free: i64,
    pub d_free: i64,
    pub done: bool,
    pub combo: u32,
    pub combo_dmg: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HitRecord {
    pub attacker: Option<usize>,
    pub victim: usize,
    pub damage: f64,
    pub shielded: bool,
}
/// Entire rollback snapshot; all content and fighter relationships are indices.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct World {
    pub frame: u64,
    pub rng: Rng,
    /// None preserves legacy shared gameplay/effect randomness.
    pub cosmetic_rng: Option<Rng>,
    pub fx: Vec<Fx>,
    /// Per-victim hits; the caller drains these after each tick, like `fx`.
    pub hits: Vec<HitRecord>,
    /// Actions accepted this tick (input display); drained like `fx`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub acts: Vec<crate::actions::Accepted>,
    pub cfg: MatchConfig,
    pub stage: usize,
    pub fighters: Vec<Fighter>,
    pub projs: Vec<Projectile>,
    /// Loose items (`items.rs`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<LooseItem>,
    pub count: u32,
    pub go_t: u32,
    /// Ticks left on the match timer (`None`: no time limit, out of time, or training).
    #[serde(default)]
    pub time_left: Option<u32>,
    /// The timer ran out; the winner came from the time-up standings.
    #[serde(default)]
    pub time_up: bool,
    /// The fighters tied for the lead at the moment the timer ran out, fixed then so the end animation cannot
    /// change who plays sudden death. Empty when the time up was decided (see [`World::time_leaders`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub time_tied: Vec<usize>,
    pub end_t: u32,
    pub winner: Option<usize>,
    pub over: bool,
    pub trace: Option<Trace>,
}
impl World {
    /// Fails for unknown content ids, no players, nonpositive stocks or time limit, or insufficient stage capacity.
    /// One-side configurations are allowed for training; lobbies should check `opposing_sides()`.
    pub fn new(cfg: MatchConfig, data: &GameData) -> Result<Self, String> {
        let stage = data
            .stages
            .iter()
            .position(|s| s.id == cfg.stage)
            .ok_or_else(|| format!("unknown stage {:?}", cfg.stage))?;
        let s = &data.stages[stage];
        if cfg.players.is_empty() || cfg.players.len() > s.max_players() {
            return Err(format!(
                "stage {:?} supports 1..={} players, got {}",
                s.id,
                s.max_players(),
                cfg.players.len()
            ));
        }
        if cfg.stocks <= 0 {
            return Err("stocks must be positive".into());
        }
        let time_left = match cfg.time_limit {
            None => None,
            Some(0) => return Err("time limit must be positive".into()),
            Some(s) => Some(s.checked_mul(crate::TICK_HZ).ok_or("time limit is too long")?),
        };
        for p in &cfg.players {
            if data.fighter(&p.fighter).is_none() {
                return Err(format!("unknown fighter {:?}", p.fighter));
            }
        }
        let fighters = cfg
            .players
            .iter()
            .enumerate()
            .map(|(i, p)| {
                let di = data.fighters.iter().position(|f| f.id == p.fighter).expect("unknown fighter");
                let d = &data.fighters[di];
                let sp = &s.spawns[i];
                let y = sp.y.unwrap_or_else(|| s.spawn_y(sp.x).unwrap_or(0.));
                let surf =
                    s.blocks.iter().position(|b| sp.x >= b.left && sp.x <= b.right && y == b.top).or_else(|| {
                        s.platforms
                            .iter()
                            .position(|p| {
                                let (x1, x2, py) = p.position(0.);
                                sp.x >= x1 && sp.x <= x2 && y == py
                            })
                            .map(|i| i + s.blocks.len())
                    });
                let out = cfg.sudden_death && p.eliminated;
                Fighter {
                    data: di,
                    x: sp.x,
                    y,
                    vx: 0.,
                    vy: 0.,
                    face: f64::from(sp.face),
                    dmg: if !out && cfg.sudden_death { data.global.match_.sudden_death_damage } else { 0. },
                    last_hit_by: None,
                    stocks: if cfg.sudden_death { i32::from(!out) } else { cfg.stocks },
                    ground: surf.is_some(),
                    surf,
                    jumps: d.stats.air_jumps,
                    state: if out {
                        State::Out
                    } else if surf.is_some() {
                        State::Idle
                    } else {
                        State::Air
                    },
                    t: 0,
                    sd: StateData::default(),
                    mov: None,
                    hitstun: 0,
                    tumble: false,
                    spin: 0.,
                    lag: 0,
                    inv: 0,
                    blink: 0,
                    up_b: false,
                    ad_used: false,
                    ff: false,
                    drop: 0,
                    ledge_cd: 0,
                    run_ph: 0.,
                    flip_t: 0,
                    flash: 0,
                    hud_shake: 0,
                    sh: data.global.shield.max,
                    last_sp: -99,
                    in_dir: 0.,
                    in_down: false,
                    ai: AiState { cool: data.global.ai.initial_cool, ..Default::default() },
                    history: History::default(),
                    buffer: Buffer::default(),
                    gait: Gait::Stand,
                    gait_t: 0,
                    di: false,
                    ledge_inv: true,
                    used_air_moves: Vec::new(),
                    paralyze_used: false,
                    charges: crate::charges::full(data, di),
                    charge_wait: vec![0; data.fighters[di].charges.len()],
                    launch: (0., 0.),
                    tech_lock: 0,
                    dodge_stale: 0,
                    dodge_at: 0,
                    rise: 0,
                    top_launch: false,
                    floating: 0,
                    float_used: false,
                    item: None,
                }
            })
            .collect();
        Ok(Self {
            frame: 0,
            rng: Rng(cfg.seed),
            cosmetic_rng: data.global.sim.separate_cosmetic_rng.then_some(Rng(cfg.seed ^ 0x9e37_79b9)),
            fx: vec![],
            hits: vec![],
            acts: vec![],
            stage,
            fighters,
            projs: vec![],
            items: vec![],
            count: if cfg.training { 0 } else { data.global.match_.countdown },
            go_t: 0,
            time_left: time_left.filter(|_| !cfg.training && !cfg.sudden_death),
            time_up: false,
            time_tied: vec![],
            end_t: 0,
            winner: None,
            over: false,
            trace: None,
            cfg,
        })
    }
    pub fn enemies(&self, a: usize, b: usize) -> bool {
        a != b && (self.cfg.players[a].team.is_none() || self.cfg.players[a].team != self.cfg.players[b].team)
    }
    pub fn opponents(&self, a: usize, b: usize) -> bool {
        a != b
            && (self.cfg.friendly_fire
                || self.cfg.players[a].team.is_none()
                || self.cfg.players[a].team != self.cfg.players[b].team)
    }
    /// One entry per side, in deterministic key order: (stocks left, pooled damage, the side's fighters). Teamed
    /// players share a key; unteamed players are alone. A fighter in the death sequence has already spent the stock
    /// they died on, so their damage no longer counts.
    fn time_sides(&self) -> Vec<(i32, f64, Vec<usize>)> {
        let mut sides: std::collections::BTreeMap<(u8, usize), (i32, f64, Vec<usize>)> = Default::default();
        for (i, f) in self.fighters.iter().enumerate() {
            let key = self.cfg.players[i].team.map_or((1, i), |t| (0, usize::from(t)));
            let side = sides.entry(key).or_default();
            side.0 += f.stocks;
            if f.stocks > 0 && f.state != State::Dead {
                side.1 += f.dmg;
            }
            side.2.push(i);
        }
        sides.into_values().collect()
    }
    /// Time up (SSBU): the side with the most stocks left wins, then the side with the lowest pooled damage; `None`
    /// on a tie or when nobody has a stock left. The winner is a surviving member of that side.
    pub fn time_winner(&self) -> Option<usize> {
        let sides = self.time_sides();
        let best = sides.iter().map(|s| (s.0, s.1)).reduce(time_better)?;
        let mut leaders = sides.iter().filter(|s| (s.0, s.1) == best);
        let lead = leaders.next()?;
        if best.0 <= 0 || leaders.next().is_some() {
            return None;
        }
        lead.2.iter().copied().find(|&i| self.fighters[i].stocks > 0).or_else(|| lead.2.first().copied())
    }
    /// The fighters still tied for the lead when time runs out, for sudden death: every member of each tied side.
    /// Empty when the sides are not tied or nobody has a stock left.
    pub fn time_leaders(&self) -> Vec<usize> {
        let sides = self.time_sides();
        let Some(best) = sides.iter().map(|s| (s.0, s.1)).reduce(time_better) else { return vec![] };
        if best.0 <= 0 || sides.iter().filter(|s| (s.0, s.1) == best).count() < 2 {
            return vec![];
        }
        sides.iter().filter(|s| (s.0, s.1) == best).flat_map(|s| s.2.iter().copied()).collect()
    }
    pub(crate) fn effect_rng(&mut self) -> &mut Rng {
        self.cosmetic_rng.as_mut().unwrap_or(&mut self.rng)
    }
    pub fn sfx(&mut self, name: impl Into<String>) {
        self.fx.push(Fx::Sfx { name: name.into(), arg: None, x: None });
    }
    pub fn dust(&mut self, x: f64, y: f64, n: u32) {
        self.fx.push(Fx::Dust { x, y, n });
    }
    pub fn ring(&mut self, x: f64, y: f64, r: f64, grow: f64, life: u32) {
        self.fx.push(Fx::Ring { x, y, r, grow, life, color: Color::Rgb(0xffffff), player: 0 });
    }
    pub fn part(&mut self, (x, y): (f64, f64), (vx, vy): (f64, f64), life: f64, color: Color, size: f64, gravity: f64) {
        self.fx.push(Fx::Part { x, y, vx, vy, life, color, size, gravity, player: 0 });
    }
}
pub fn sign(v: f64) -> f64 {
    if v > 0. {
        1.
    } else if v < 0. {
        -1.
    } else {
        0.
    }
}
/// Time-up order between two sides' `(stocks, damage)` scores: more stocks, then less damage.
fn time_better(a: (i32, f64), b: (i32, f64)) -> (i32, f64) {
    if (b.0, -b.1) > (a.0, -a.1) { b } else { a }
}
