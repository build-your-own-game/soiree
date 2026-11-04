//! La copie native comme `partie::Instance` (INTERFACE.md, § 8.4) :
//! `panique_devient_faute`, `instantane_puis_restaurer_rend_la_meme_empreinte`,
//! `meme_journal_meme_empreinte`.

#![expect(
    clippy::arithmetic_side_effects,
    clippy::many_single_char_names,
    clippy::unwrap_used,
    reason = "code de test : un échec s'y lit comme l'échec du test qui l'appelle"
)]

use devine::{ActionEcran, Devine, GesteTel, Prenom};
use jeu::prelude::*;
use jeu_abi::{Appel, EcranFil, EntreeFil, QuandFil, Retour, decoder, encoder};
use jeu_natif::{FabriqueNative, InstanceNative};
use partie::{Echec, Fabrique, Faute, Instance};

/// Le jeu minimal des tests de compilation de `jeu` : sa création panique (`todo!`).
mod paniqueur {
    include!("../../jeu/tests/ui/commun/minimal.rs");

    jeu_minimal! {
        telephones: Applicable::Oui(Telephones { repartition: Repartition::UnParJoueur, tenu: TENU }),
        geste: u8,
        tenu: Aucun,
        vue: u8,
        secrets: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
        prediction: Applicable::SansObjet(jeu::raison!("un jeu de test minimal")),
        champ: u8,
    }
}

fn octets(a: &Appel<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(a, &mut o).unwrap();
    o
}

fn appeler<I: Instance>(i: &mut I, a: &Appel<'_>) -> Result<Vec<u8>, Echec> {
    i.appeler(a.sorte(), &octets(a), 1_000)
}

fn creer() -> Vec<u8> {
    let mut presents = Vec::new();
    encoder(&Vec::<Presence>::new(), &mut presents).unwrap();
    octets(&Appel::Creer {
        graine: [8; 32],
        pas: 0,
        heure: 0,
        salle: TexteCourt::new("S"),
        presents: &presents,
        garde: None,
    })
}

/// Le journal d'une petite soirée : des gestes, un tenu, une manche lancée et révélée.
fn journal() -> Vec<Vec<u8>> {
    let mut j = vec![creer()];
    let mut pas = 0;
    let mut entree = |e: &EntreeFil<'_>, j: &mut Vec<Vec<u8>>| {
        pas += 1;
        let mut o = Vec::new();
        encoder(e, &mut o).unwrap();
        j.push(octets(&Appel::Entree {
            ordre: pas,
            place: pas,
            entree: &o,
        }));
    };
    let q = |n: u64| QuandFil {
        fait: n,
        fraction: 0,
        recu: n,
        vu: n,
        id: n,
    };
    let mut g = Vec::new();
    encoder(
        &GesteTel::Rejoindre(Texte::<Prenom>::nettoyer("Léa")),
        &mut g,
    )
    .unwrap();
    entree(
        &EntreeFil::Geste {
            de: 1,
            octets: &g,
            quand: q(1),
        },
        &mut j,
    );
    let mut a = Vec::new();
    encoder(&ActionEcran::Lancer, &mut a).unwrap();
    entree(
        &EntreeFil::Ecran {
            quoi: EcranFil::Action(&a),
            quand: q(2),
        },
        &mut j,
    );
    let mut c = Vec::new();
    encoder(&devine::Curseur(Borne::new(5).unwrap()), &mut c).unwrap();
    entree(
        &EntreeFil::Tenu {
            de: 1,
            octets: &c,
            cause: CauseTenu::Geste,
            quand: q(3),
        },
        &mut j,
    );
    let mut p = Vec::new();
    encoder(&GesteTel::Proposer, &mut p).unwrap();
    entree(
        &EntreeFil::Geste {
            de: 1,
            octets: &p,
            quand: q(4),
        },
        &mut j,
    );
    entree(
        &EntreeFil::Geste {
            de: 1,
            octets: &p,
            quand: q(5),
        },
        &mut j,
    );
    for _ in 0..1300 {
        entree(&EntreeFil::Temps { saut: 0, heure: 0 }, &mut j);
    }
    j
}

fn rejouer(i: &mut InstanceNative<Devine>, j: &[Vec<u8>]) {
    for a in j {
        let sorte = decoder::<Appel<'_>>(a).unwrap().sorte();
        let r = i.appeler(sorte, a, 0).unwrap();
        assert!(matches!(
            decoder::<Retour<'_>>(&r).unwrap(),
            Retour::Issue(_)
        ));
    }
}

/// Une panique du jeu devient `Faute::Panique`, avec son message (§ 8.4, § 4.6).
#[test]
fn panique_devient_faute() {
    let mut i = InstanceNative::<paniqueur::Minimal>::new();
    match i.appeler(jeu_abi::SorteAppel::Ecriture, &creer(), 1) {
        Err(Echec::Faute(Faute::Panique { message })) => {
            assert!(message.contains("not yet implemented"), "{message}");
        }
        autre => panic!("{autre:?}"),
    }
    assert_eq!(i.carburant(), None, "la copie native n'a pas de carburant");
}

/// Prendre un instantané, avancer, restaurer : même empreinte, et la même suite.
#[test]
fn instantane_puis_restaurer_rend_la_meme_empreinte() {
    let j = journal();
    let mut i = FabriqueNative::<Devine>::new().neuve().unwrap();
    rejouer(&mut i, &j[..4]);
    let e = i.empreinte().unwrap();
    let s = i.instantane().unwrap();
    rejouer(&mut i, &j[4..]);
    let fin = i.empreinte().unwrap();
    assert_ne!(fin, e);
    i.restaurer(&s).unwrap();
    assert_eq!(i.empreinte().unwrap(), e);
    rejouer(&mut i, &j[4..]);
    assert_eq!(i.empreinte().unwrap(), fin);
}

/// Deux copies natives, le même journal : la même empreinte après chaque entrée. Une
/// graine différente donne une autre empreinte : le hasard est dans la forme canonique.
#[test]
fn meme_journal_meme_empreinte() {
    let j = journal();
    let mut a = InstanceNative::<Devine>::new();
    let mut b = InstanceNative::<Devine>::new();
    for x in &j {
        rejouer(&mut a, std::slice::from_ref(x));
        rejouer(&mut b, std::slice::from_ref(x));
        assert_eq!(a.empreinte().unwrap(), b.empreinte().unwrap());
    }
    let mut c = InstanceNative::<Devine>::new();
    let mut autre = j.clone();
    let mut presents = Vec::new();
    encoder(&Vec::<Presence>::new(), &mut presents).unwrap();
    autre[0] = octets(&Appel::Creer {
        graine: [9; 32],
        pas: 0,
        heure: 0,
        salle: TexteCourt::new("S"),
        presents: &presents,
        garde: None,
    });
    rejouer(&mut c, &autre);
    assert_ne!(c.empreinte().unwrap(), a.empreinte().unwrap());
    // Une enveloppe absente a sa propre empreinte.
    let mut vide = InstanceNative::<Devine>::new();
    assert_ne!(vide.empreinte().unwrap(), a.empreinte().unwrap());
    let r = appeler(
        &mut vide,
        &Appel::Entree {
            ordre: 0,
            place: 0,
            entree: &[],
        },
    )
    .unwrap();
    assert!(
        matches!(decoder::<Retour<'_>>(&r).unwrap(), Retour::Illisible(_)),
        "une entrée avant la création"
    );
}
