mod common;
use sim::{GameData, Input, input::JUMP};

/// Rise of a full (held) or short (tapped) hop; with `air`, of a midair jump pressed at the full hop's apex.
fn jump_height(d: &GameData, held: bool, air: bool) -> f64 {
    let mut w = common::fighters(d, "mike", &["brawler"]);
    w.fighters[0].x = 640.;
    w.fighters[1].x = 950.;
    let mut base = w.fighters[0].y;
    let mut peak = base;
    let mut jumped = false;
    for tick in 0..120 {
        let apex = tick > 4 && w.fighters[0].vy >= 0. && !w.fighters[0].ground;
        let press = air && apex && !jumped;
        let buttons = if tick == 0 || press || (held && !air) { JUMP } else { 0 };
        if press {
            (jumped, base, peak) = (true, w.fighters[0].y, w.fighters[0].y);
        }
        common::step_draining(&mut w, d, &[Input { buttons, ..Default::default() }]);
        if !air || jumped {
            peak = peak.min(w.fighters[0].y);
        }
    }
    base - peak
}

#[test]
fn mike_jumps_reach_ikes_source_heights() {
    // Ike's full hop, short hop and double jump (ssbwiki jump height table, SSBU) at 5 px per unit.
    let d = common::data();
    for (held, air, units) in [(true, false, 29.9), (false, false, 16.), (false, true, 31.)] {
        let h = jump_height(&d, held, air);
        assert!((h - units * 5.).abs() < 1., "held {held} air {air}: {h} px vs {}", units * 5.);
    }
}
