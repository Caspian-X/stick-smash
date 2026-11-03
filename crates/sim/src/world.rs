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
}
impl PlayerConfig {
    pub fn human(fighter: &str) -> Self {
        Self { fighter: fighter.into(), cpu: None, team: None, tap_jump: true, controls: Controls::default() }
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
        Self { stage: stage.into(), players, seed, training: true, stocks: 1, friendly_fire: false }
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
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
    hit_at: Vec<(usize, u32)>,
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
}
impl StateData {
    pub(crate) fn can_hit(&self, victim: usize, tick: u32, rehit: Option<u32>) -> bool {
        self.hit_at.iter().find(|(v, _)| *v == victim).is_none_or(|(_, at)| rehit.is_some_and(|n| tick - *at >= n))
    }
    pub(crate) fn record_hit(&mut self, victim: usize, tick: u32) {
        if let Some((_, at)) = self.hit_at.iter_mut().find(|(v, _)| *v == victim) {
            *at = tick;
        } else {
            self.hit_at.push((victim, tick));
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
    }
    pub fn hittable(&self) -> bool {
        !matches!(self.state, State::Dead | State::Respawn | State::Grabbed | State::Out | State::Win)
            && self.inv == 0
            && self.blink == 0
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
    pub count: u32,
    pub go_t: u32,
    pub end_t: u32,
    pub winner: Option<usize>,
    pub over: bool,
    pub trace: Option<Trace>,
}
impl World {
    /// Fails for unknown content ids, no players, nonpositive stocks, or insufficient stage capacity.
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
                Fighter {
                    data: di,
                    x: sp.x,
                    y,
                    vx: 0.,
                    vy: 0.,
                    face: f64::from(sp.face),
                    dmg: 0.,
                    last_hit_by: None,
                    stocks: cfg.stocks,
                    ground: surf.is_some(),
                    surf,
                    jumps: d.stats.air_jumps,
                    state: if surf.is_some() { State::Idle } else { State::Air },
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
            count: if cfg.training { 0 } else { data.global.match_.countdown },
            go_t: 0,
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
