//! La forme des messages est figée par des octets écrits à la main (PUN-90).
//!
//! Un changement de la forme d'un `Appel`, d'un `Retour` ou d'une `EntreeFil` fait
//! rougir ce test. S'il est voulu, il monte `jeu_abi::ABI`, et les octets attendus
//! se corrigent ici, à la main.

use std::collections::BTreeSet;

use jeu_abi::{ABI, SorteAppel};

use crate::commun::{Message, corpus};

#[test]
fn forme_figee() {
    let mut ecarts = Vec::new();
    for c in corpus() {
        let encode = c.message.encoder();
        if encode != c.attendu {
            ecarts.push(format!(
                "{} : encodé {:02X?}, attendu {:02X?}",
                c.nom, encode, c.attendu
            ));
        }
        match c.message.relire(&c.attendu) {
            Ok(true) => {}
            Ok(false) => ecarts.push(format!(
                "{} : les octets écrits se relisent autrement",
                c.nom
            )),
            Err(e) => ecarts.push(format!(
                "{} : les octets écrits ne se relisent pas : {e}",
                c.nom
            )),
        }
    }
    assert!(ecarts.is_empty(), "{}", ecarts.join("\n"));
}

#[test]
fn abi_est_celle_des_octets_figes() {
    // Les octets de ce fichier sont ceux de l'ABI 2. Changer la forme demande de
    // monter `ABI` et de réécrire ces octets ensemble. L'ABI 2 a ajouté, à la fin
    // d'`Appel` et de `Retour`, `Proprietes` et `Hypothese` (E10, E11) : les octets
    // de l'ABI 1 n'ont pas bougé.
    assert_eq!(ABI, 2);
}

/// Le corpus couvre chaque variant d'`Appel`, de `Retour` et d'`EntreeFil` : un
/// variant ajouté sans octets figés fait rougir ce test.
#[test]
fn le_corpus_couvre_chaque_variant() {
    let noms: BTreeSet<String> = corpus()
        .iter()
        .map(|c| c.nom.split("::").take(2).collect::<Vec<_>>().join("::"))
        .collect();
    let appels = noms.iter().filter(|n| n.starts_with("Appel::")).count();
    let retours = noms.iter().filter(|n| n.starts_with("Retour::")).count();
    let entrees = noms.iter().filter(|n| n.starts_with("EntreeFil::")).count();
    assert_eq!(appels, 21, "les 21 variants d'Appel : {noms:?}");
    assert_eq!(retours, 17, "les 17 variants de Retour : {noms:?}");
    assert_eq!(entrees, 7, "les 7 variants d'EntreeFil : {noms:?}");
}

/// La table du § 3.4, écrite à part, contre `Appel::sorte()`.
#[test]
fn chaque_appel_a_sa_sorte() {
    const ECRITURES: [&str; 5] = [
        "Appel::Creer",
        "Appel::Entree",
        "Appel::Charger",
        "Appel::DepuisCanonique",
        "Appel::PireCas",
    ];
    let mut vus = 0;
    for c in corpus() {
        let Message::Appel(a) = &c.message else {
            continue;
        };
        let attendue = if ECRITURES.contains(&c.nom) {
            SorteAppel::Ecriture
        } else {
            SorteAppel::Lecture
        };
        assert_eq!(a.sorte(), attendue, "{}", c.nom);
        vus += 1;
    }
    assert_eq!(vus, 21);
    assert_eq!(SorteAppel::Lecture.code(), 0);
    assert_eq!(SorteAppel::Ecriture.code(), 1);
    assert_eq!(SorteAppel::depuis_code(1), Some(SorteAppel::Ecriture));
    assert_eq!(SorteAppel::depuis_code(2), None);
}
