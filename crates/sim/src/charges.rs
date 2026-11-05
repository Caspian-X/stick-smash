//! Limited-use charge resources (weapons and tomes): remaining uses, spending, and timed recharge.
//!
//! A fighter's pools come from content (`charges`); the runtime state is plain fingerprintable data on the
//! rollback snapshot. Nothing here is fighter-specific: moves ask for a pool by id through `ChargeNeed`.

use crate::{GameData, data::ChargeNeed, world::Fighter};

/// Remaining uses of `id`, or `None` for an unknown pool.
pub fn remaining(f: &Fighter, d: &GameData, id: &str) -> Option<u32> {
    let at = d.fighters[f.data].charges.iter().position(|c| c.id == id)?;
    Some(f.charges.get(at).copied().unwrap_or(0))
}

/// Does the fighter have this many uses left (a missing pool always passes, so content without resources is
/// unaffected)?
pub fn has(f: &Fighter, d: &GameData, need: &ChargeNeed) -> bool {
    remaining(f, d, &need.charge).is_none_or(|n| n >= need.uses)
}

/// Full uses for every pool, in content order.
pub fn full(d: &GameData, data: usize) -> Vec<u32> {
    d.fighters[data].charges.iter().map(|c| c.uses).collect()
}

/// Spend `need` from the fighter's pools, returning the sound to play when a pool empties. A pool with fewer
/// uses than asked spends what it has left (the item is used up), and an emptied pool starts its recharge.
/// Unknown ids and zero-cost needs do nothing.
pub fn spend(d: &GameData, f: &mut Fighter, need: &ChargeNeed) -> Option<String> {
    let (at, pool) = d.fighters[f.data]
        .charges
        .iter()
        .enumerate()
        .find(|(_, c)| c.id == need.charge)
        .map(|(a, c)| (a, c.clone()))?;
    let before = f.charges.get(at).copied().unwrap_or(0);
    if need.uses == 0 || before == 0 {
        return None;
    }
    let left = before.saturating_sub(need.uses);
    if f.charges.len() <= at {
        f.charges.resize(at + 1, 0);
        f.charge_wait.resize(at + 1, 0);
    }
    f.charges[at] = left;
    // `recharge: 0` keeps the pool spent until respawn, whatever the discard delay says. Otherwise the first
    // use comes back after the discard delay plus one tick step, then the rest follow on that step.
    if left == 0 && f.charge_wait[at] == 0 && pool.recharge > 0 {
        f.charge_wait[at] = pool.discard_delay.saturating_add(ticks_per_use(&pool));
    }
    if left == 0 {
        return pool.empty_sfx.clone();
    }
    None
}

/// Ticks between restored uses for a pool: `recharge` spread over its capacity. Integer arithmetic keeps the
/// schedule exact, and the pool is full after `uses × ticks_per_use` (at most one tick per use longer than
/// `recharge`).
pub fn ticks_per_use(pool: &crate::data::ChargePool) -> u32 {
    pool.recharge.div_ceil(pool.uses.max(1)).max(1)
}

/// Advance every recharging pool by one tick, restoring one use at a time on a fixed schedule. Spending during
/// the recharge never refunds a use: the countdown to the next restored use keeps running, so a pool can only
/// recover at its own rate.
pub fn tick(d: &GameData, f: &mut Fighter) -> Vec<String> {
    let pools = &d.fighters[f.data].charges;
    if pools.is_empty() {
        return Vec::new();
    }
    f.charges.resize(pools.len(), 0);
    f.charge_wait.resize(pools.len(), 0);
    let mut refilled = Vec::new();
    for (at, pool) in pools.iter().enumerate() {
        if f.charge_wait[at] == 0 {
            continue;
        }
        f.charge_wait[at] -= 1;
        if f.charge_wait[at] > 0 {
            continue;
        }
        if f.charges[at] < pool.uses {
            f.charges[at] += 1;
        }
        if f.charges[at] >= pool.uses {
            refilled.push(pool.refill_sfx.clone());
        } else {
            f.charge_wait[at] = ticks_per_use(pool);
        }
    }
    refilled.into_iter().flatten().collect()
}

/// Restore every pool to full (on respawn).
pub fn refill(d: &GameData, f: &mut Fighter) {
    f.charges = full(d, f.data);
    f.charge_wait = vec![0; d.fighters[f.data].charges.len()];
}

/// Add uses without resetting an ongoing recharge (including empty-to-nonempty). Full pools stop recharging.
/// Play the refill sound only when this restoration reaches capacity.
pub fn restore(d: &GameData, f: &mut Fighter, need: &ChargeNeed) -> Option<String> {
    let (at, pool) = d.fighters[f.data].charges.iter().enumerate().find(|(_, c)| c.id == need.charge)?;
    f.charges.resize(d.fighters[f.data].charges.len(), 0);
    f.charge_wait.resize(f.charges.len(), 0);
    let before = f.charges[at];
    f.charges[at] = before.saturating_add(need.uses).min(pool.uses);
    if f.charges[at] == pool.uses {
        f.charge_wait[at] = 0;
        if before < pool.uses {
            return pool.refill_sfx.clone();
        }
    }
    None
}
