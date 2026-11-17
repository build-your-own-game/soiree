//! L'état de la machine est une simple donnée : il s'écrit en octets et se
//! relit. La console en tire la photo, la reprise et l'empreinte.

use alloc::vec::Vec;

use serde::Serialize;
use serde::de::DeserializeOwned;

/// L'état de la machine s'écrit en octets et se relit.
///
/// Rien à écrire quand tout l'état dérive `Serialize` et `Deserialize` : la
/// brique le fait, dans un format exact au bit près (un flottant y entre par
/// ses bits, une longueur par sa valeur), le même sur l'ordinateur et dans un
/// navigateur.
///
/// Une machine qui porte un état qu'elle n'a pas écrit (une bibliothèque de
/// physique) écrit `save` et `load` elle-même. Deux états égaux donnent alors
/// les mêmes octets, et `load(save(état))` rend un état qui continue comme
/// l'original : le banc le vérifie.
pub trait State: Sized {
    /// Tout l'état, en octets.
    fn save(&self) -> Vec<u8>;
    /// L'état relu. `None` si ces octets ne sont pas un état.
    fn load(bytes: &[u8]) -> Option<Self>;
}

impl<T: Serialize + DeserializeOwned> State for T {
    fn save(&self) -> Vec<u8> {
        // L'écriture en mémoire n'échoue que si un type refuse de s'écrire
        // (une longueur inconnue) : l'état vide ne se relira pas, et le banc
        // le dira.
        postcard::to_allocvec(self).unwrap_or_default()
    }

    fn load(bytes: &[u8]) -> Option<Self> {
        postcard::from_bytes(bytes).ok()
    }
}

/// L'empreinte d'un état : un nombre calculé sur tous ses octets. Deux
/// parties dans le même état ont la même empreinte, ici comme dans un
/// navigateur.
pub fn fingerprint<S: State>(state: &S) -> u64 {
    fnv(&state.save())
}

/// FNV-1a sur 64 bits : la même sur toutes les machines.
pub(crate) fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |print, byte| {
        (print ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use alloc::borrow::ToOwned;
    use alloc::collections::BTreeMap;
    use alloc::string::String;

    use serde::Deserialize;

    use super::*;

    #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
    struct Game {
        names: BTreeMap<(u8, u64), String>,
        ball: (f32, f64),
        count: usize,
    }

    fn game() -> Game {
        let mut names = BTreeMap::new();
        let _: Option<String> = names.insert((1, u64::MAX), "Zoé 🎉".to_owned());
        Game {
            names,
            ball: (f32::NAN, f64::NEG_INFINITY),
            count: 3,
        }
    }

    #[test]
    fn a_state_read_back_writes_the_same_bytes() {
        let bytes = game().save();
        let back = Game::load(&bytes).unwrap();
        // Un flottant qui n'est pas un nombre n'est égal à rien : on compare
        // les octets, pas les valeurs.
        assert_eq!(back.save(), bytes);
        assert_eq!(fingerprint(&back), fingerprint(&game()));
    }

    #[test]
    fn two_different_states_have_different_fingerprints() {
        let mut other = game();
        other.count = 4;
        assert_ne!(fingerprint(&other), fingerprint(&game()));
    }

    #[test]
    fn bytes_that_are_not_a_state_are_refused() {
        assert_eq!(Game::load(&[0xff]), None);
    }
}
