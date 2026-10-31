//! Content: global tuning, stages and fighters, loaded from JSON. The sim only parses strings; the game crate
//! does the file I/O. See `docs/FORMAT.md` for the format and the move phase semantics.

pub mod audio;
pub mod common;
pub mod fighter;
pub mod global;
pub mod hitbox;
pub mod json;
pub mod load;
pub mod moves;
pub mod render;
pub mod schema;
pub mod stage;
pub mod visual;

pub use common::{Color, Num, Pt, Rect};
pub use fighter::{Fighter, Projectile, Stats};
pub use global::Global;
pub use hitbox::{HitType, Hitbox, LaunchDir};
pub use load::{ContentError, Source, load};
pub use moves::{Event, Move, Moves, Phase, Transition, Trigger};
pub use stage::Stage;

/// Newest `schemaVersion` this build reads. Older files are migrated on load.
pub const SCHEMA_VERSION: u32 = 2;

/// Everything a match reads. `hash` identifies the exact content, so online peers can check they match.
#[derive(Clone, Debug)]
pub struct GameData {
    pub global: Global,
    pub stages: Vec<Stage>,
    /// Playable fighters (abstract packages excluded), sorted by id.
    pub fighters: Vec<Fighter>,
    pub hash: u64,
}

impl GameData {
    pub fn fighter(&self, id: &str) -> Option<&Fighter> {
        self.fighters.iter().find(|f| f.id == id)
    }
    pub fn stage(&self, id: &str) -> Option<&Stage> {
        self.stages.iter().find(|s| s.id == id)
    }
}
