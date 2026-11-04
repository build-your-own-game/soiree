//! Chaque type de `jeu` qui passe la frontière se relit comme sa forme `…Fil` de
//! `jeu-abi`, et réciproquement (INTERFACE.md, § 3.5).
//!
//! Deux sortes de types passent la frontière :
//!
//! - ceux que `jeu` convertit en leur forme sur le fil avant d'encoder
//!   (`Declaration::fil`, `Scene::fil`, et `Montre<V>`, que la colle écrit comme un
//!   `MontreFil` dont la vue est encodée à part) : le module n'encode jamais que la
//!   forme `…Fil`, que le moteur décode strictement ;
//! - ceux qu'un message du jeu encode directement par serde (`Texte<L>`,
//!   `Appareils<V>`) : leur encodage doit être celui de la forme que le commun lit.
//!
//! Le test vit dans `devine` : il faut un vrai jeu, et une dépendance de `jeu` vers
//! `devine` ferait un cycle.

#![expect(
    clippy::unwrap_used,
    clippy::wildcard_enum_match_arm,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

mod commun;

use commun::{A, B, Pilote};
use devine::{Devine, Prenom, VueTel};
use jeu::Jeu;
use jeu::prelude::*;
use jeu_abi::{DeclarationFil, MontreFil, RegardFil, Retour, SceneFil, decoder, encoder};
use jeu_natif::InstanceNative;

fn octets<T: serde::Serialize + ?Sized>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(v, &mut o).unwrap();
    o
}

/// `Declaration::fil` s'encode, se relit strictement comme `DeclarationFil`, et c'est
/// ce que la colle rend à `Decrire`.
#[test]
fn declaration_se_relit_comme_son_fil() {
    let fil = Devine::DECLARATION.fil();
    let o = octets(&fil);
    let relu: DeclarationFil = decoder(&o).unwrap();
    assert_eq!(relu, fil);
    assert_eq!(octets(&relu), o, "réencodée, les mêmes octets");
    let mut p = Pilote::new(InstanceNative::<Devine>::new());
    p.avec(&jeu_abi::Appel::Decrire, |r| match r {
        Retour::Decrit(d) => assert_eq!(*d, fil),
        autre => panic!("{autre:?}"),
    });
}

/// La scène de l'écran et la vue d'un téléphone, à chaque phase, se relisent
/// strictement comme `SceneFil` et `MontreFil`, et se réencodent aux mêmes octets.
#[test]
fn scene_et_montre_se_relisent_comme_leur_fil() {
    let mut p = Pilote::new(InstanceNative::<Devine>::new());
    p.creer([1; 32], None);
    let fin = 3 + Duree::s(20).en_pas();
    let verifier = |p: &mut Pilote<InstanceNative<Devine>>| {
        let vues = p.vues(&[
            RegardFil::Ecran,
            RegardFil::Appareil(A),
            RegardFil::Appareil(B),
        ]);
        let scene: SceneFil = decoder(&vues[0]).unwrap();
        assert_eq!(octets(&scene), vues[0]);
        for v in &vues[1..] {
            let m: MontreFil<'_> = decoder(v).unwrap();
            assert_eq!(&octets(&m), v);
            let _: VueTel = decoder(m.vue).unwrap();
        }
    };
    verifier(&mut p);
    p.rejoindre(A, "Léa", 1);
    verifier(&mut p);
    p.lancer(2, 2);
    p.curseur(A, 5, 3);
    verifier(&mut p);
    p.temps(fin);
    verifier(&mut p);
}

/// `Texte<L>` s'encode comme une chaîne ; une chaîne propre se relit comme un
/// `Texte<L>`, une chaîne sale est refusée (PUN-22, SAL-117).
#[test]
fn texte_se_relit_comme_une_chaine() {
    let t = Texte::<Prenom>::nettoyer("  Léa\u{200B} ");
    let o = octets(&t);
    let s: String = decoder(&o).unwrap();
    assert_eq!(s, t.as_str());
    let relu: Texte<Prenom> = decoder(&octets(&s)).unwrap();
    assert_eq!(relu, t);
    assert!(decoder::<Texte<Prenom>>(&octets("Léa\u{200B}")).is_err());
}

/// `Appareils<V>` s'encode comme la suite de ses lignes, `(Appareil, V)` ; une suite
/// où un appareil revient est refusée.
#[test]
fn appareils_se_relit_comme_ses_lignes() {
    let mut t = Appareils::<u8>::new();
    let a = jeu_abi::Appareil::du_fil(A);
    let b = jeu_abi::Appareil::du_fil(B);
    let _: Option<u8> = t.inserer(a, 4);
    let _: Option<u8> = t.inserer(b, 9);
    let o = octets(&t);
    let lignes: Vec<(u64, u8)> = decoder(&o).unwrap();
    assert_eq!(lignes, vec![(A, 4), (B, 9)]);
    let relu: Appareils<u8> = decoder(&octets(&lignes)).unwrap();
    assert_eq!(octets(&relu), o);
    assert!(decoder::<Appareils<u8>>(&octets(&vec![(A, 1u8), (A, 2u8)])).is_err());
}
