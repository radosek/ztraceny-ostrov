//! Delphi `Random`: RandSeed := RandSeed * 0x08088405 + 1; Random(n) = (seed * n) >> 32.

#[derive(Clone, Debug)]
pub struct Rng(pub u32);

impl Rng {
    pub fn below(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_mul(0x0808_8405).wrapping_add(1);
        ((self.0 as u64 * n as u64) >> 32) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::Rng;

    #[test]
    fn delphi_sequence() {
        let mut r = Rng(0);
        assert_eq!(r.0, 0);
        assert_eq!(r.below(100), 0);
        assert_eq!(r.0, 1);
        r.below(1);
        assert_eq!(r.0, 0x0808_8406);
    }
}
