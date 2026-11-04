//! Le hasard de la soirée simulée : latences, pannes, appareils, octets absurdes.
//!
//! Il est à part du hasard du jeu, qui vit dans son module : xoshiro256**, semé par
//! BLAKE3 de la graine du plan et d'une étiquette. Même graine, même étiquette, même
//! suite : une soirée se rejoue pas pour pas (OUT-37).

/// Un générateur xoshiro256** (Blackman et Vigna).
#[derive(Debug, Clone)]
pub(crate) struct Tirage {
    s: [u64; 4],
}

impl Tirage {
    /// Le générateur de cette étiquette, pour cette graine.
    pub(crate) fn new(graine: &[u8; 32], etiquette: &str) -> Tirage {
        let mut h = blake3::Hasher::new();
        let _: &mut blake3::Hasher = h.update(b"soiree-epreuve, tirage, 1");
        let _: &mut blake3::Hasher = h.update(graine);
        let _: &mut blake3::Hasher = h.update(etiquette.as_bytes());
        let o = h.finalize();
        let b = o.as_bytes();
        let mut s = [0u64; 4];
        for (mot, morceau) in s.iter_mut().zip(b.as_chunks::<8>().0) {
            *mot = u64::from_le_bytes(*morceau);
        }
        if s == [0; 4] {
            s = [1, 2, 3, 4];
        }
        Tirage { s }
    }

    /// Un générateur dérivé, indépendant de celui-ci pour la suite.
    pub(crate) fn deriver(&mut self, etiquette: &str) -> Tirage {
        let mut g = [0u8; 32];
        for morceau in g.as_chunks_mut::<8>().0 {
            *morceau = self.entier().to_le_bytes();
        }
        Tirage::new(&g, etiquette)
    }

    /// Un entier sur 64 bits.
    pub(crate) fn entier(&mut self) -> u64 {
        let [s0, s1, s2, s3] = self.s;
        let r = s1.wrapping_mul(5).rotate_left(7).wrapping_mul(9);
        let t = s1 << 17;
        let s2 = s2 ^ s0;
        let s3 = s3 ^ s1;
        let s1 = s1 ^ s2;
        let s0 = s0 ^ s3;
        self.s = [s0, s1, s2 ^ t, s3.rotate_left(45)];
        r
    }

    /// Un entier dans `[0, n)`, `n` non nul ; 0 sinon. Le léger biais du modulo est
    /// sans conséquence ici : rien ne se compte sur ce hasard.
    pub(crate) fn sous(&mut self, n: u64) -> u64 {
        self.entier().checked_rem(n).unwrap_or(0)
    }

    /// Un entier dans `[a, b]`.
    pub(crate) fn entre(&mut self, a: u64, b: u64) -> u64 {
        let (a, b) = if a <= b { (a, b) } else { (b, a) };
        a.saturating_add(self.sous(b.saturating_sub(a).saturating_add(1)))
    }

    /// Vrai une fois sur `n`.
    pub(crate) fn une_sur(&mut self, n: u64) -> bool {
        self.sous(n) == 0
    }

    /// Un octet.
    pub(crate) fn octet(&mut self) -> u8 {
        u8::try_from(self.entier() & 0xff).unwrap_or(0)
    }

    /// `n` octets.
    pub(crate) fn octets(&mut self, n: usize) -> Vec<u8> {
        (0..n).map(|_| self.octet()).collect()
    }

    /// Une fraction de pas, en 1/65 536.
    pub(crate) fn fraction(&mut self) -> u16 {
        u16::from_le_bytes([self.octet(), self.octet()])
    }
}

#[cfg(test)]
mod tests {
    use super::Tirage;

    #[test]
    fn meme_graine_meme_suite() {
        let mut a = Tirage::new(&[3; 32], "x");
        let mut b = Tirage::new(&[3; 32], "x");
        let mut c = Tirage::new(&[3; 32], "y");
        let va: Vec<u64> = (0..8).map(|_| a.entier()).collect();
        let vb: Vec<u64> = (0..8).map(|_| b.entier()).collect();
        let vc: Vec<u64> = (0..8).map(|_| c.entier()).collect();
        assert_eq!(va, vb);
        assert_ne!(va, vc);
    }

    #[test]
    fn entre_reste_dans_ses_bornes() {
        let mut t = Tirage::new(&[9; 32], "bornes");
        for _ in 0..1000 {
            let x = t.entre(3, 7);
            assert!((3..=7).contains(&x));
        }
    }
}
