//! Targeted balance diagnostics: original roster vs the imported reference fighters.
//!
//! Opt-in and ignored so it never runs in `just test`:
//!
//! ```sh
//! BALANCE_SEEDS=8 cargo test --release -p sim --test balance_original -- --ignored --nocapture
//! ```
//!
//! `BALANCE_SEEDS` seeds are played on every stage, in both seats, against each reference.
//! `BALANCE_STAGES` (comma-separated ids) and `BALANCE_ORIGINALS` narrow the matrix; `BALANCE_VERBOSE=1`
//! prints one line per match. Every line is a JSON object prefixed with `BALANCE `.
//!
//! The reference imports and their data are treated as read-only: this harness only reports on them.
mod common;

use sim::{GameData, MatchConfig, PlayerConfig, World, step_world, world::State};

/// Original playable fighters. King is excluded from this matrix while its package is replaced by an import.
const ORIGINALS: &[&str] =
    &["archer", "brawler", "dancer", "golem", "lancer", "ninja", "swordfighter", "tinker", "wrangler", "yoyo"];

/// SSBU-derived reference packages, preserved as-is for comparison.
const REFERENCES: &[&str] = &["mike", "whiplash", "wizard"];

#[derive(Default, Clone, Debug, serde::Serialize)]
struct MoveStats {
    starts: usize,
    hits: usize,
    tips: usize,
    blocked: usize,
    whiffs: usize,
    punished_whiffs: usize,
    punish_starts: usize,
    punish_hits: usize,
    damage: f64,
    kos: usize,
    ko_percent: f64,
    start_distance: f64,
}

impl MoveStats {
    fn merge(&mut self, o: &MoveStats) {
        self.starts += o.starts;
        self.hits += o.hits;
        self.tips += o.tips;
        self.blocked += o.blocked;
        self.whiffs += o.whiffs;
        self.punished_whiffs += o.punished_whiffs;
        self.punish_starts += o.punish_starts;
        self.punish_hits += o.punish_hits;
        self.damage += o.damage;
        self.kos += o.kos;
        self.ko_percent += o.ko_percent;
        self.start_distance += o.start_distance;
    }
}

#[derive(Default, Clone, Debug, serde::Serialize)]
struct FighterStats {
    matches: usize,
    wins: usize,
    unfinished: usize,
    dealt: f64,
    taken: f64,
    deaths: usize,
    self_kos: usize,
    recovery_deaths: usize,
    /// Sum of victim percent just before KOs this fighter caused.
    ko_percent: f64,
    kos: usize,
    /// Ticks survived by both fighters, summed over matches (match length).
    ticks: usize,
    moves: std::collections::BTreeMap<String, MoveStats>,
}

impl FighterStats {
    fn merge(&mut self, o: &FighterStats) {
        self.matches += o.matches;
        self.wins += o.wins;
        self.unfinished += o.unfinished;
        self.dealt += o.dealt;
        self.taken += o.taken;
        self.deaths += o.deaths;
        self.self_kos += o.self_kos;
        self.recovery_deaths += o.recovery_deaths;
        self.ko_percent += o.ko_percent;
        self.kos += o.kos;
        self.ticks += o.ticks;
        for (slot, m) in &o.moves {
            self.moves.entry(slot.clone()).or_default().merge(m);
        }
    }
}

#[derive(Default)]
struct Attempt {
    slot: String,
    hit: bool,
    punished: bool,
    punish: bool,
}

fn move_label(d: &GameData, w: &World, before: &sim::world::Fighter, a: usize, victim: usize) -> String {
    let label = w
        .trace
        .as_ref()
        .filter(|t| t.hit_frame == w.frame && t.attacker == a && t.defender == victim)
        .map(|t| t.label.as_str());
    let fighter = &d.fighters[w.fighters[a].data];
    label
        .and_then(|name| fighter.moves.iter().find(|(_, m)| m.name == name).map(|(s, _)| s.to_string()))
        .or_else(|| before.mov.as_ref().map(|m| m.slot.clone()))
        .unwrap_or_else(|| {
            if before.state == State::Hold { "Pummel".into() } else { sim::combat::attack_label(d, before) }
        })
}

fn tip_mult(d: &GameData, charge: u32) -> f64 {
    1. + f64::from(charge) / d.global.knockback.charge_damage_divisor
}

/// Both-seat duel: accumulates independent statistics for each slot.
fn duel(d: &GameData, stage: &str, ids: [&str; 2], seed: u32, stats: &mut [FighterStats; 2]) {
    let mut w = World::new(
        MatchConfig {
            stage: stage.into(),
            seed,
            stocks: 2,
            players: ids.iter().map(|id| PlayerConfig { cpu: Some(2), ..PlayerConfig::human(id) }).collect(),
            ..Default::default()
        },
        d,
    )
    .unwrap();
    let mut attempt: [Option<Attempt>; 2] = [None, None];
    let mut last_hit = [None; 2];
    let mut last_move = [String::new(), String::new()];
    let mut recovery = [None; 2];
    let mut dmg_before = [0.; 2];
    let mut stocks_before: [i32; 2] = [w.fighters[0].stocks, w.fighters[1].stocks];
    let mut ticks = 0;
    for tick in 0..30_000 {
        let before = w.fighters.clone();
        step_world(&mut w, d, &[]);
        ticks = tick + 1;
        for i in 0..2 {
            let f = &w.fighters[i];
            let prev = &before[i];
            let started = f.mov.as_ref().is_some_and(|m| {
                f.state == State::Attack
                    && (prev.state != State::Attack || prev.mov.as_ref().is_none_or(|p| m.elapsed < p.elapsed))
            });
            let ended = attempt[i].as_ref().is_some_and(|a| {
                f.state != State::Attack || f.mov.as_ref().is_none_or(|m| m.slot != a.slot) || started
            });
            if ended && let Some(a) = attempt[i].take() {
                let m = stats[i].moves.entry(a.slot).or_default();
                let contact = a.hit || f.state == State::Hold && f.sd.victim == Some(1 - i);
                m.whiffs += usize::from(!contact);
                m.punished_whiffs +=
                    usize::from(!contact && (a.punished || w.hits.iter().any(|h| h.victim == i && !h.shielded)));
            }
            if started {
                let slot = f.mov.as_ref().unwrap().slot.clone();
                let o = &before[1 - i];
                let punish = o.state == State::Attack
                    && o.mov.as_ref().is_some_and(|m| {
                        let p = sim::moves::phase(d, o);
                        !p.hitboxes.is_empty() && p.hitboxes.iter().all(|h| h.end.is_some_and(|end| m.t > end))
                    });
                let m = stats[i].moves.entry(slot.clone()).or_default();
                m.starts += 1;
                m.start_distance += (f.x - w.fighters[1 - i].x).abs();
                m.punish_starts += usize::from(punish);
                attempt[i] = Some(Attempt { slot, punish, ..Default::default() });
            }
            let offstage = !d.stages[w.stage]
                .blocks
                .iter()
                .any(|b| f.x >= b.left && f.x <= b.right && f.y <= b.top + d.global.ai.thresholds.offstage_pad);
            if f.ground || f.state == State::Ledge {
                recovery[i] = None;
            } else if offstage
                && (f.state == State::Helpless
                    || f.state == State::Attack && f.mov.as_ref().is_some_and(|m| m.slot == "uspec"))
            {
                recovery[i] = Some(tick);
            }
        }
        for h in &w.hits {
            let Some(a) = h.attacker else { continue };
            if h.shielded {
                if let Some(att) = &mut attempt[a] {
                    att.hit = true;
                    stats[a].moves.entry(att.slot.clone()).or_default().blocked += 1;
                }
                continue;
            }
            let v = h.victim;
            last_hit[v] = Some(tick);
            let slot = move_label(d, &w, &before[a], a, v);
            last_move[v] = slot.clone();
            stats[a].dealt += h.damage;
            {
                let m = stats[a].moves.entry(slot.clone()).or_default();
                m.hits += 1;
                m.damage += h.damage;
                let fighter = &d.fighters[w.fighters[a].data];
                if let Some((_, def)) = fighter.moves.iter().find(|(s, _)| *s == slot) {
                    let charge = w.fighters[a].mov.as_ref().or(before[a].mov.as_ref()).map_or(0, |m| m.charge);
                    let mult = tip_mult(d, charge);
                    m.tips += usize::from(
                        slot != "uspec"
                            && slot != "dspec"
                            && def
                                .phases
                                .iter()
                                .any(|p| p.hitboxes.len() > 1 && (p.hitboxes[0].damage * mult - h.damage).abs() < 1e-6),
                    );
                }
                if let Some(att) = &mut attempt[a] {
                    att.hit = true;
                    m.punish_hits += usize::from(att.punish);
                }
            }
            stats[v].taken += h.damage;
            if let Some(att) = &mut attempt[v] {
                att.punished = true;
            }
        }
        for i in 0..2 {
            if w.fighters[i].stocks >= stocks_before[i] {
                continue;
            }
            stocks_before[i] = w.fighters[i].stocks;
            stats[i].deaths += 1;
            let self_ko = last_hit[i].is_none_or(|t| tick - t > 240);
            stats[i].self_kos += usize::from(self_ko);
            stats[i].recovery_deaths += usize::from(recovery[i].is_some_and(|t| tick - t < 240));
            recovery[i] = None;
            if !self_ko {
                let a = 1 - i;
                stats[a].kos += 1;
                stats[a].ko_percent += dmg_before[i];
                if !last_move[i].is_empty() {
                    let m = stats[a].moves.entry(last_move[i].clone()).or_default();
                    m.kos += 1;
                    m.ko_percent += dmg_before[i];
                }
            }
        }
        for (i, before) in dmg_before.iter_mut().enumerate() {
            if w.fighters[i].dmg > 0. {
                *before = w.fighters[i].dmg;
            }
        }
        w.hits.clear();
        w.fx.clear();
        w.acts.clear();
        if w.over {
            break;
        }
    }
    let unfinished = usize::from(!w.over);
    for (i, s) in stats.iter_mut().enumerate() {
        s.matches += 1;
        s.unfinished += unfinished;
        s.ticks += ticks;
        s.wins += usize::from(w.fighters[i].state == State::Win);
    }
    if unfinished == 1 {
        println!(
            "UNFINISHED {stage} {ids:?} seed {seed}: {:?}",
            w.fighters.iter().map(|f| (f.x, f.y, f.dmg, f.stocks, f.state)).collect::<Vec<_>>()
        );
    }
    if std::env::var("BALANCE_VERBOSE").is_ok() {
        println!(
            "MATCH {stage} {} vs {} seed {seed}: ticks {ticks}, winner {:?}",
            ids[0],
            ids[1],
            w.fighters.iter().map(|f| f.state).collect::<Vec<_>>()
        );
    }
}

fn ids(env: &str, fallback: &[&str]) -> Vec<String> {
    std::env::var(env).ok().filter(|s| !s.is_empty()).map_or_else(
        || fallback.iter().map(|s| s.to_string()).collect(),
        |s| s.split(',').map(|s| s.trim().to_string()).collect(),
    )
}

fn record(kind: &str, fighter: &str, opponent: &str, stage: &str, seeds: u32, stats: &FighterStats, vs: &FighterStats) {
    let v = serde_json::json!({
        "kind": kind,
        "fighter": fighter,
        "opponent": opponent,
        "stage": stage,
        "seeds": seeds,
        "stats": stats,
        "opponent_stats": vs,
    });
    println!("BALANCE {}", serde_json::to_string(&v).unwrap());
}

#[test]
#[ignore]
fn original_vs_reference_balance() {
    let d = common::data();
    let seeds: u32 = std::env::var("BALANCE_SEEDS").ok().map_or(4, |v| v.parse().unwrap());
    let first_seed: u32 = std::env::var("BALANCE_FIRST_SEED").ok().map_or(1, |v| v.parse().unwrap());
    assert!(seeds > 0);
    let originals = ids("BALANCE_ORIGINALS", ORIGINALS);
    let references = ids("BALANCE_REFERENCES", REFERENCES);
    let stages = ids("BALANCE_STAGES", &d.stages.iter().map(|s| s.id.as_str()).collect::<Vec<_>>());
    for id in &originals {
        assert!(d.fighters.iter().any(|f| f.id == *id), "{id}: not loaded");
    }
    let mut reference_totals: std::collections::BTreeMap<String, FighterStats> = Default::default();
    for id in &originals {
        let mut total = FighterStats::default();
        for opponent in &references {
            let mut vs_total = FighterStats::default();
            for stage in &stages {
                let mut stats = FighterStats::default();
                let mut vs = FighterStats::default();
                for seed in first_seed..first_seed + seeds {
                    // Both seats sample every seed; the pairing is exact, not two independent samples.
                    for own in 0..2 {
                        let mut pair = [FighterStats::default(), FighterStats::default()];
                        let names =
                            if own == 0 { [id.as_str(), opponent.as_str()] } else { [opponent.as_str(), id.as_str()] };
                        duel(&d, stage, names, seed, &mut pair);
                        stats.merge(&pair[own]);
                        vs.merge(&pair[1 - own]);
                    }
                }
                record("matchup", id, opponent, stage, seeds, &stats, &vs);
                total.merge(&stats);
                vs_total.merge(&vs);
            }
            record("opponent", id, opponent, "all", seeds, &vs_total, &FighterStats::default());
            reference_totals.entry(opponent.clone()).or_default().merge(&vs_total);
        }
        record("total", id, "references", "all", seeds, &total, &FighterStats::default());
    }
    for (reference, stats) in &reference_totals {
        record("reference_total", reference, "originals", "all", seeds, stats, &FighterStats::default());
    }
}

/// A short deterministic snapshot used by focused regression checks; unchanged by the diagnostic env vars.
#[test]
#[ignore]
fn original_reference_matchup_is_reproducible() {
    let d = common::data();
    for (own, names) in [(0, ["brawler", "mike"]), (1, ["mike", "brawler"])] {
        let mut pair = [FighterStats::default(), FighterStats::default()];
        duel(&d, "arena", names, 3, &mut pair);
        let stats = &pair[own];
        assert_eq!(stats.matches, 1);
        assert_eq!(stats.unfinished, 0, "{names:?} should finish");
        println!("REPRO {names:?} {stats:?}");
    }
}
