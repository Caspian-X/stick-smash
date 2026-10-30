/// Deterministic RNG (mulberry32), same algorithm and output as the TypeScript version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rng(pub u32);

impl Rng {
    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x6d2b_79f5);
        let mut t = self.0;
        t = (t ^ (t >> 15)).wrapping_mul(t | 1);
        t ^= t.wrapping_add((t ^ (t >> 7)).wrapping_mul(t | 61));
        f64::from(t ^ (t >> 14)) / 4_294_967_296.0
    }

    /// Uniform in `[a, b)`.
    pub fn range(&mut self, a: f64, b: f64) -> f64 {
        a + self.next_f64() * (b - a)
    }
}
