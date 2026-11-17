//! Un hasard par graine, sans système : même graine, mêmes tirages, ici comme
//! dans un navigateur. Le banc s'en sert ; une machine peut l'emprunter.

use serde::{Deserialize, Serialize};

/// Un générateur `SplitMix64`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    /// Le tirage suivant, sur 64 bits.
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut mixed = self.0;
        mixed = (mixed ^ (mixed >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        mixed = (mixed ^ (mixed >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        mixed ^ (mixed >> 31)
    }

    /// Un nombre de 0 à `bound - 1`. 0 si `bound` vaut 0.
    pub fn below(&mut self, bound: u64) -> u64 {
        self.next_u64().checked_rem(bound).unwrap_or(0)
    }

    /// Un rang dans une liste de cette longueur. 0 si elle est vide.
    pub fn index(&mut self, len: usize) -> usize {
        let bound = u64::try_from(len).unwrap_or(u64::MAX);
        usize::try_from(self.below(bound)).unwrap_or(0)
    }

    /// Vrai `times` fois sur `out_of`.
    pub fn chance(&mut self, times: u64, out_of: u64) -> bool {
        self.below(out_of) < times
    }

    /// Un des éléments de la liste, ou `None` si elle est vide.
    pub fn pick<'list, T>(&mut self, list: &'list [T]) -> Option<&'list T> {
        list.get(self.index(list.len()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_same_seed_gives_the_same_draws() {
        let draws = |seed| {
            let mut rng = Rng::new(seed);
            [rng.next_u64(), rng.below(10), rng.below(0)]
        };
        assert_eq!(draws(7), draws(7));
        assert_ne!(draws(7), draws(8));
    }

    #[test]
    fn a_pick_stays_in_the_list() {
        let mut rng = Rng::new(1);
        let none: [u8; 0] = [];
        assert_eq!(rng.pick(&none), None);
        for _ in 0..100 {
            assert!(rng.pick(&[1, 2, 3]).is_some());
            assert!(rng.index(3) < 3);
        }
    }
}
