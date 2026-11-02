mod common;
use sim::{
    Input,
    data::global::InputRules,
    input::{Controls, History, Intent, Strike},
};
fn rules() -> InputRules {
    common::data().global.input
}
fn derive(h: &mut History, raw: Input, tick: u64, tap_jump: bool) -> Intent {
    h.derive(raw, tick, &rules(), 5, tap_jump, Controls::default())
}
fn stick(x: i8, y: i8) -> Input {
    Input { stick: (x, y), ..Default::default() }
}
#[test]
fn threshold_presses_holds_releases_and_taps() {
    let r = rules();
    let (t, f) = (r.stick_threshold, r.flick_threshold);
    let mut h = History::default();
    let a = derive(&mut h, stick(t - 1, -(t - 1)), 1, false);
    assert!(!a.any_p && !a.right && !a.down && a.x > 0.);
    let a = derive(&mut h, Input { stick: (t, -t), buttons: 31, ..Default::default() }, 2, false);
    assert!(a.right && a.down && a.down_p && a.jump && a.jump_held && a.atk && a.atk_held && a.sp && a.sp_held);
    assert!(a.grab && a.shield && a.shield_p && a.any_p);
    // Past the threshold but below the flick threshold is a hold, not a tap.
    assert!(!a.tap_r && !a.tap_d && !a.flick_r && !a.flick_d);
    let a = derive(&mut h, Input { stick: (t, -t), buttons: 31, ..Default::default() }, 3, false);
    assert!(!a.jump && !a.atk && !a.sp && !a.grab && !a.shield_p && !a.down_p && !a.any_p);
    derive(&mut h, Input::default(), 4, false);
    let a = derive(&mut h, stick(-f, f), 5, false);
    assert!(a.left && a.up && a.tap_l && a.tap_u && a.flick_l && a.any_p && !a.jump);
    let a = derive(&mut h, stick(-f, f), 10, false);
    assert!(a.tap_l && a.tap_u && !a.flick_l, "taps last the tap window, flicks one tick");
    let a = derive(&mut h, stick(-f, f), 11, false);
    assert!(!a.tap_l && !a.tap_u);
}
#[test]
fn flicks_need_a_fast_rise_from_the_deadzone() {
    let r = rules();
    let (dz, f, n) = (r.deadzone, r.flick_threshold, u64::from(r.flick_frames));
    // Abrupt: deadzone to flick threshold within flickFrames.
    let mut h = History::default();
    derive(&mut h, stick(0, 0), 1, false);
    for k in 1..n {
        assert!(!derive(&mut h, stick(dz, 0), 1 + k, false).flick_r);
    }
    assert!(derive(&mut h, stick(f, 0), 1 + n, false).flick_r);
    // Slow ramp: one tick too long.
    let mut h = History::default();
    derive(&mut h, stick(0, 0), 1, false);
    for k in 1..=n {
        derive(&mut h, stick(dz + 1, 0), 1 + k, false);
    }
    let a = derive(&mut h, stick(127, 0), 2 + n, false);
    assert!(!a.flick_r && !a.tap_r && a.right && a.x == 1.);
    // Just below the flick threshold never flicks.
    let mut h = History::default();
    derive(&mut h, stick(0, 0), 1, false);
    assert!(!derive(&mut h, stick(f - 1, 0), 2, false).flick_r);
    // From the opposite side (dash back) it counts.
    let mut h = History::default();
    derive(&mut h, stick(-127, 0), 1, false);
    assert!(derive(&mut h, stick(127, 0), 2, false).flick_r);
}
#[test]
fn analog_values_scale_past_the_deadzone() {
    let dz = rules().deadzone;
    let mut h = History::default();
    assert_eq!(derive(&mut h, stick(dz, -dz), 1, false).x, 0.);
    let a = derive(&mut h, stick(-127, 127), 2, false);
    assert_eq!((a.x, a.y), (-1., 1.));
    let a = derive(&mut h, stick(dz + (127 - dz) / 2, 0), 3, false);
    assert!((a.x - 0.5).abs() < 0.01);
}
#[test]
fn tap_jump_combines_independent_button_edges() {
    let mut h = History::default();
    let raw = Input { stick: (0, 127), buttons: 0, ..Default::default() };
    let a = derive(&mut h, raw, 1, true);
    assert!(a.jump && a.jump_held);
    let a = derive(&mut h, raw, 2, true);
    assert!(!a.jump && a.jump_held);
    let a = derive(&mut h, Input { buttons: 1, ..raw }, 3, true);
    assert!(a.jump && a.jump_held);
    let a = derive(&mut h, Input { stick: (0, 0), buttons: 1, ..Default::default() }, 4, true);
    assert!(!a.jump && a.jump_held);
    let mut disabled = History::default();
    let a = derive(&mut disabled, raw, 1, false);
    assert!(!a.jump && !a.jump_held);
}
#[test]
fn second_jump_control_is_its_own_press() {
    use sim::input::{JUMP, JUMP2};
    let mut h = History::default();
    let a = derive(&mut h, Input { buttons: JUMP | JUMP2, ..Default::default() }, 1, false);
    assert!(a.jump && a.short_hop);
    let mut h = History::default();
    assert!(!derive(&mut h, Input { buttons: JUMP, ..Default::default() }, 1, false).short_hop);
    let a = derive(&mut h, Input { buttons: JUMP | JUMP2, ..Default::default() }, 2, false);
    assert!(a.jump && a.short_hop && a.jump_held);
}
#[test]
fn attack_stick_is_independent_and_rearms_only_at_neutral() {
    let mut h = History::default();
    let raw = Input { stick: (-127, 0), attack_stick: (0, 127), ..Default::default() };
    let a = derive(&mut h, raw, 1, true);
    assert!(a.atk && a.stick_held && !a.atk_held && a.left && !a.up && !a.jump);
    assert_eq!(a.strike, Strike::Stick { x: 0, y: 1, smash: true });
    let a = derive(&mut h, raw, 2, true);
    assert!(!a.atk && a.stick_held);
    let a = derive(&mut h, Input { attack_stick: (127, 0), ..raw }, 3, true);
    assert!(!a.atk);
    let a = derive(&mut h, Input { attack_stick: (63, 0), ..raw }, 4, true);
    assert!(!a.stick_held);
    assert!(derive(&mut h, raw, 5, true).atk);
    // The attack button has its own edge while the stick is held, and picks by the movement stick.
    let a = derive(&mut h, Input { buttons: 2, ..raw }, 6, true);
    assert!(a.atk && a.atk_held && a.strike == Strike::Button);
}
#[test]
fn attack_stick_diagonals_choose_the_largest_axis_and_ties_go_sideways() {
    for (raw, expected) in [((90, 100), (0, 1)), ((100, -90), (1, 0)), ((-128, 127), (-1, 0)), ((64, 64), (1, 0))] {
        let a = derive(&mut History::default(), Input { attack_stick: raw, ..Default::default() }, 1, true);
        assert_eq!(a.strike, Strike::Stick { x: expected.0, y: expected.1, smash: true });
        assert!(a.atk && !a.jump && !a.down_p && !a.tap_u);
    }
}
#[test]
fn control_options_pick_stick_tilts_and_attack_special_smash() {
    let r = rules();
    let c = Controls { stick_tilts: true, ab_smash: true };
    let a = History::default().derive(Input { attack_stick: (127, 0), ..Default::default() }, 1, &r, 5, true, c);
    assert_eq!(a.strike, Strike::Stick { x: 1, y: 0, smash: false });
    let a = History::default().derive(Input { buttons: 6, ..Default::default() }, 1, &r, 5, true, c);
    assert!(a.atk && !a.sp && a.strike == Strike::Combo);
    let a = History::default().derive(Input { buttons: 6, ..Default::default() }, 1, &r, 5, true, Controls::default());
    assert!(a.atk && a.sp && a.strike == Strike::Button);
}
#[test]
fn fast_fall_buffer_remembers_a_recent_held_down_press() {
    let r = rules();
    let mut h = History::default();
    assert!(derive(&mut h, stick(0, -127), 1, false).down_recent);
    assert!(derive(&mut h, stick(0, -127), 1 + u64::from(r.fast_fall_buffer), false).down_recent);
    assert!(!derive(&mut h, stick(0, -127), 2 + u64::from(r.fast_fall_buffer), false).down_recent);
}
