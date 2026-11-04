//! Les propriétés du format et des valeurs, sur des entrées tirées au hasard.

#![expect(
    clippy::arithmetic_side_effects,
    reason = "des indices pris modulo une longueur non nulle, et des bornes de propriétés"
)]

use jeu_abi::{
    Appel, Axe2, DeclarationFil, Duree, EntreeFil, Fini, MontreFil, Retour, SceneFil, TexteCourt,
    decoder, encoder, fnv1a64, fnv1a64_suite,
};
use proptest::prelude::*;

use crate::commun::corpus;

/// Décode sous tous les types de l'enveloppe ; aucun ne doit paniquer.
fn decoder_partout(octets: &[u8]) {
    let _: Result<Appel<'_>, _> = decoder(octets);
    let _: Result<Retour<'_>, _> = decoder(octets);
    let _: Result<EntreeFil<'_>, _> = decoder(octets);
    let _: Result<MontreFil<'_>, _> = decoder(octets);
    let _: Result<SceneFil, _> = decoder(octets);
    let _: Result<DeclarationFil, _> = decoder(octets);
}

/// Une modification d'une suite d'octets : remplacer, insérer ou retirer un octet.
#[derive(Debug, Clone)]
enum Mutation {
    Remplacer(usize, u8),
    Inserer(usize, u8),
    Retirer(usize),
}

fn muter(octets: &mut Vec<u8>, m: &Mutation) {
    if octets.is_empty() {
        return;
    }
    match *m {
        Mutation::Remplacer(i, b) => {
            let n = octets.len();
            octets[i % n] = b;
        }
        Mutation::Inserer(i, b) => {
            let n = octets.len();
            octets.insert(i % (n + 1), b);
        }
        Mutation::Retirer(i) => {
            let n = octets.len();
            let _: u8 = octets.remove(i % n);
        }
    }
}

fn mutation() -> impl Strategy<Value = Mutation> {
    prop_oneof![
        (any::<usize>(), any::<u8>()).prop_map(|(i, b)| Mutation::Remplacer(i, b)),
        (any::<usize>(), any::<u8>()).prop_map(|(i, b)| Mutation::Inserer(i, b)),
        any::<usize>().prop_map(Mutation::Retirer),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(4096))]

    /// Des octets au hasard ne font jamais paniquer le décodage (§ 8.1, E13).
    #[test]
    fn des_octets_au_hasard_ne_font_jamais_paniquer(octets in proptest::collection::vec(any::<u8>(), 0..512)) {
        decoder_partout(&octets);
    }

    /// Des messages valides, abîmés, ne font jamais paniquer le décodage.
    #[test]
    fn des_messages_abimes_ne_font_jamais_paniquer(
        i in any::<usize>(),
        mutations in proptest::collection::vec(mutation(), 1..8),
    ) {
        let c = corpus();
        let mut octets = c[i % c.len()].attendu.clone();
        for m in &mutations {
            muter(&mut octets, m);
        }
        decoder_partout(&octets);
    }

    /// Ce qui se décode se réencode en une valeur qui se relit à l'identique.
    #[test]
    fn un_decodage_reussi_fait_l_aller_retour(
        i in any::<usize>(),
        mutations in proptest::collection::vec(mutation(), 0..4),
    ) {
        let c = corpus();
        let mut octets = c[i % c.len()].attendu.clone();
        for m in &mutations {
            muter(&mut octets, m);
        }
        if let Ok(a) = decoder::<Appel<'_>>(&octets) {
            let mut o = Vec::new();
            encoder(&a, &mut o).unwrap();
            prop_assert_eq!(decoder::<Appel<'_>>(&o).unwrap(), a);
        }
        if let Ok(r) = decoder::<Retour<'_>>(&octets) {
            let mut o = Vec::new();
            encoder(&r, &mut o).unwrap();
            prop_assert_eq!(decoder::<Retour<'_>>(&o).unwrap(), r);
        }
        if let Ok(e) = decoder::<EntreeFil<'_>>(&octets) {
            let mut o = Vec::new();
            encoder(&e, &mut o).unwrap();
            prop_assert_eq!(decoder::<EntreeFil<'_>>(&o).unwrap(), e);
        }
    }
}

proptest! {
    /// Un `Fini` n'est jamais NaN, infini, sous-normal ou zéro négatif.
    #[test]
    fn fini_n_a_que_des_valeurs_normales_ou_zero(bits in any::<u32>()) {
        if let Some(f) = Fini::new(f32::from_bits(bits)) {
            let x = f.get();
            prop_assert!(x.is_normal() || x.to_bits() == 0);
        } else {
            prop_assert!(!f32::from_bits(bits).is_finite());
        }
    }

    /// Ramener deux flottants finis donne toujours un `Axe2` valide (CAT-22).
    #[test]
    fn ramener_donne_toujours_un_axe2(x in any::<f32>(), y in any::<f32>()) {
        match Axe2::ramener(x, y) {
            Some(a) => {
                prop_assert!(Axe2::new(a.x(), a.y()).is_some());
                prop_assert!(a.x() * a.x() + a.y() * a.y() <= 1.0 + 1e-6);
            }
            None => prop_assert!(!x.is_finite() || !y.is_finite()),
        }
    }

    /// Un `Axe2` encodé se relit à l'identique.
    #[test]
    fn axe2_fait_l_aller_retour(x in -1.0f32..=1.0, y in -1.0f32..=1.0) {
        if let Some(a) = Axe2::new(x, y) {
            let mut o = Vec::new();
            encoder(&a, &mut o).unwrap();
            prop_assert_eq!(decoder::<Axe2>(&o).unwrap(), a);
        }
    }

    /// `Duree::ms` arrondit vers le haut, et reste monotone (CAT-15).
    #[test]
    fn duree_ms_arrondit_vers_le_haut(a in 0u64..1_000_000_000_000, b in 0u64..1_000_000_000_000) {
        prop_assert!(Duree::ms(a).en_ms() >= a);
        prop_assert!(Duree::ms(a).en_ms() < a + 17);
        if a <= b {
            prop_assert!(Duree::ms(a) <= Duree::ms(b));
        }
    }

    /// Un court texte valide fait l'aller-retour ; un texte invalide ne se construit pas.
    #[test]
    fn texte_court_fait_l_aller_retour(s in "\\PC{0,40}") {
        if let Some(t) = TexteCourt::essayer(&s) {
            let mut o = Vec::new();
            encoder(&t, &mut o).unwrap();
            prop_assert_eq!(decoder::<TexteCourt>(&o).unwrap(), t);
            prop_assert_eq!(t.as_str(), s.as_str());
        } else {
            prop_assert!(s.len() > 64 || s.chars().any(char::is_control));
        }
    }

    /// FNV-1a se calcule par morceaux.
    #[test]
    fn fnv1a64_se_calcule_par_morceaux(a in proptest::collection::vec(any::<u8>(), 0..64), b in proptest::collection::vec(any::<u8>(), 0..64)) {
        let mut ab = a.clone();
        ab.extend_from_slice(&b);
        prop_assert_eq!(fnv1a64_suite(fnv1a64(&a), &b), fnv1a64(&ab));
    }
}
