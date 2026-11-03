mod common;
use sim::{GameData, Input, input::JUMP};

fn jump_height(d: &GameData, held: bool) -> f64 {
    let mut w = common::fighters(d, "mike", &["brawler"]);
    w.fighters[0].x = 640.;
    w.fighters[1].x = 950.;
    let floor = w.fighters[0].y;
    let mut peak = floor;
    for tick in 0..100 {
        let buttons = if tick == 0 || held { JUMP } else { 0 };
        common::step_draining(&mut w, d, &[Input { buttons, ..Default::default() }]);
        peak = peak.min(w.fighters[0].y);
    }
    floor - peak
}

#[test]
fn mike_short_hop_reaches_half_its_previous_height_without_changing_full_hop() {
    let d = common::data();
    let mut old = d.clone();
    old.fighters.iter_mut().find(|f| f.id == "mike").unwrap().stats.short_hop_speed = 10.1;
    let height = jump_height(&d, false);
    let previous = jump_height(&old, false);
    assert!((height / previous - 0.5).abs() < 0.01, "new {height}, previous {previous}");
    assert_eq!(jump_height(&d, true), jump_height(&old, true));
    assert!(jump_height(&d, true) > height * 2.5);
}
