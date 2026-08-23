#[derive(Clone, Debug)]
pub struct BattleRng {
    state: u64,
}

impl BattleRng {
    pub const fn seeded(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    pub fn roll_percent(&mut self) -> u8 {
        (self.next_u64() % 100 + 1) as u8
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splitmix_rolls_are_stable_for_known_seeds() {
        assert_eq!(BattleRng::seeded(2).roll_percent(), 11);
        assert_eq!(BattleRng::seeded(6).roll_percent(), 93);
        let mut crit_seed = BattleRng::seeded(0);
        assert_eq!(
            (crit_seed.roll_percent(), crit_seed.roll_percent()),
            (36, 1)
        );
    }
}
