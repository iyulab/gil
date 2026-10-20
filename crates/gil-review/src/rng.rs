//! The one random number generator every draw uses, so the same seed gives the same sample on every target: SplitMix64
//! spreads the seed into the state of xoshiro256** (Blackman and Vigna), and an integer below `n` comes from Lemire's
//! multiply-and-reject method, which has no bias.

/// SplitMix64: a 64-bit seed spread into a stream of well-mixed words.
pub(crate) struct SplitMix64(u64);

impl SplitMix64 {
    pub(crate) fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// xoshiro256**, its state the first four words of SplitMix64 from the seed.
pub(crate) struct Xoshiro256StarStar([u64; 4]);

impl Xoshiro256StarStar {
    pub(crate) fn new(seed: u64) -> Self {
        let mut spread = SplitMix64::new(seed);
        Self([
            spread.next_u64(),
            spread.next_u64(),
            spread.next_u64(),
            spread.next_u64(),
        ])
    }

    pub(crate) fn next_u64(&mut self) -> u64 {
        let s = &mut self.0;
        let result = s[1].wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s[1] << 17;
        s[2] ^= s[0];
        s[3] ^= s[1];
        s[1] ^= s[2];
        s[0] ^= s[3];
        s[2] ^= t;
        s[3] = s[3].rotate_left(45);
        result
    }

    /// An integer in `[0, n)`, uniformly (Lemire 2019). `n` is at least 1.
    pub(crate) fn below(&mut self, n: u64) -> u64 {
        debug_assert!(n > 0);
        let mut m = u128::from(self.next_u64()) * u128::from(n);
        let mut low = m as u64;
        if low < n {
            let threshold = n.wrapping_neg() % n;
            while low < threshold {
                m = u128::from(self.next_u64()) * u128::from(n);
                low = m as u64;
            }
        }
        (m >> 64) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix64_matches_its_published_sequence() {
        // The reference sequence for seed 1234567.
        let mut r = SplitMix64::new(1_234_567);
        let got: Vec<u64> = (0..5).map(|_| r.next_u64()).collect();
        assert_eq!(
            got,
            [
                6_457_827_717_110_365_317,
                3_203_168_211_198_807_973,
                9_817_491_932_198_370_423,
                4_593_380_528_125_082_431,
                16_408_922_859_458_223_821,
            ]
        );
    }

    #[test]
    fn xoshiro256starstar_matches_the_reference_algorithm() {
        // Words from a line-by-line translation of the reference C code, seeded through SplitMix64 the same way.
        let mut r = Xoshiro256StarStar::new(0);
        let got: Vec<u64> = (0..4).map(|_| r.next_u64()).collect();
        assert_eq!(
            got,
            [
                11_091_344_671_253_066_420,
                13_793_997_310_169_335_082,
                1_900_383_378_846_508_768,
                7_684_712_102_626_143_532,
            ]
        );
        let mut r = Xoshiro256StarStar::new(7);
        assert_eq!(r.next_u64(), 12_923_355_070_828_475_994);
    }

    #[test]
    fn below_stays_in_range_and_reaches_every_value() {
        let mut r = Xoshiro256StarStar::new(7);
        let mut seen = [0u32; 7];
        for _ in 0..7_000 {
            seen[r.below(7) as usize] += 1;
        }
        assert!(seen.iter().all(|&c| c > 800), "{seen:?}");
        assert_eq!(r.below(1), 0);
    }
}
