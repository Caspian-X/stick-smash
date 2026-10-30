use sim::rng::Rng;

#[test]
fn rng_matches_typescript_mulberry32() {
    // Values from the TS `rand()` with seed 12345.
    let mut r = Rng(12345);
    assert_eq!(r.next_f64(), 0.9797282677609473);
    assert_eq!(r.next_f64(), 0.3067522644996643);
    assert_eq!(r.next_f64(), 0.484205421525985);
    assert_eq!(r.0, 1199742488);
}
