use sim::{Input, input::History};
#[test]
fn threshold_presses_holds_releases_and_taps() {
    let mut h = History::default();
    let a = h.derive(Input { stick: (63, -63), buttons: 0 }, 1, 64, 5, false);
    assert!(!a.any_p);
    let a = h.derive(Input { stick: (64, -64), buttons: 31 }, 2, 64, 5, false);
    assert!(
        a.right
            && a.down
            && a.down_p
            && a.tap_r
            && a.tap_d
            && a.jump
            && a.jump_held
            && a.atk
            && a.atk_held
            && a.sp
            && a.sp_held
            && a.grab
            && a.shield
            && a.shield_p
            && a.any_p
    );
    let a = h.derive(Input { stick: (64, -64), buttons: 31 }, 7, 64, 5, false);
    assert!(a.tap_r && a.tap_d && a.jump_held && a.shield);
    assert!(!a.jump && !a.atk && !a.sp && !a.grab && !a.shield_p && !a.down_p && !a.any_p);
    let a = h.derive(Input { stick: (64, -64), buttons: 31 }, 8, 64, 5, false);
    assert!(!a.tap_r && !a.tap_d);
    let a = h.derive(Input::default(), 9, 64, 5, false);
    assert!(!a.any_p);
    let a = h.derive(Input { stick: (-64, 64), buttons: 0 }, 10, 64, 5, false);
    assert!(a.left && a.up && a.tap_l && a.tap_u && a.any_p);
    assert!(!a.jump);
}
#[test]
fn tap_jump_combines_independent_button_edges() {
    let mut h = History::default();
    let raw = Input { stick: (0, 127), buttons: 0 };
    let a = h.derive(raw, 1, 64, 5, true);
    assert!(a.jump && a.jump_held);
    let a = h.derive(raw, 2, 64, 5, true);
    assert!(!a.jump && a.jump_held);
    let a = h.derive(Input { buttons: 1, ..raw }, 3, 64, 5, true);
    assert!(a.jump && a.jump_held);
    let a = h.derive(Input { stick: (0, 0), buttons: 1 }, 4, 64, 5, true);
    assert!(!a.jump && a.jump_held);
    let mut disabled = History::default();
    let a = disabled.derive(raw, 1, 64, 5, false);
    assert!(!a.jump && !a.jump_held);
}
