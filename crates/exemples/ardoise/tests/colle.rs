//! La colle de `jeu` devant un jeu sans téléphone (besoin 3 des jeux,
//! `conception/BESOINS-DES-JEUX.md`) : un regard d'appareil y est illisible, et le jeu
//! n'est jamais appelé. Sans cela, `vue_telephone` d'`ardoise` panique (il n'a aucune
//! `Montre<Aucun>` à rendre), et un appel absurde de E13 devient une faute du jeu.

#![expect(
    clippy::unwrap_used,
    reason = "code de test : un échec s'y lit comme l'échec du test"
)]

use ardoise::Ardoise;
use jeu_abi::{
    Appel, EntreeFil, Illisible, RegardFil, Retour, StyleFil, TexteCourt, decoder, encoder,
};
use partie::{Fabrique, Instance};

fn appeler(i: &mut jeu_natif::InstanceNative<Ardoise>, a: &Appel<'_>) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(a, &mut o).unwrap();
    i.appeler(a.sorte(), &o, u64::MAX).unwrap()
}

fn creee() -> jeu_natif::InstanceNative<Ardoise> {
    let mut i = jeu_natif::FabriqueNative::<Ardoise>::new().neuve().unwrap();
    let mut presents = Vec::new();
    encoder(&Vec::<jeu_abi::Presence>::new(), &mut presents).unwrap();
    let r = appeler(
        &mut i,
        &Appel::Creer {
            graine: [7; 32],
            pas: 0,
            heure: 0,
            salle: TexteCourt::new("ABCD"),
            presents: &presents,
            garde: None,
        },
    );
    assert!(matches!(
        decoder::<Retour<'_>>(&r).unwrap(),
        Retour::Issue(_)
    ));
    i
}

fn illisible(r: &[u8]) -> bool {
    matches!(
        decoder::<Retour<'_>>(r).unwrap(),
        Retour::Illisible(e) if e.quoi == Illisible::Valeur
    )
}

#[test]
fn regarder_un_appareil_sans_telephone_est_illisible() {
    let mut i = creee();
    let r = appeler(
        &mut i,
        &Appel::Regarder {
            regards: vec![RegardFil::Appareil(5)],
        },
    );
    assert!(illisible(&r), "une vue d'appareil sans téléphone");
    // L'écran, lui, se regarde.
    let r = appeler(
        &mut i,
        &Appel::Regarder {
            regards: vec![RegardFil::Ecran],
        },
    );
    assert!(matches!(decoder::<Retour<'_>>(&r).unwrap(), Retour::Vues(v) if v.len() == 1));
}

#[test]
fn un_bot_de_telephone_sans_telephone_est_illisible() {
    let mut i = creee();
    let r = appeler(
        &mut i,
        &Appel::Bot {
            regard: RegardFil::Appareil(5),
            style: StyleFil::Sage,
            graine: 1,
            depuis_vue: 1000,
        },
    );
    assert!(illisible(&r), "un bot d'appareil sans téléphone");
    let r = appeler(
        &mut i,
        &Appel::Bot {
            regard: RegardFil::Ecran,
            style: StyleFil::Sage,
            graine: 1,
            depuis_vue: 1000,
        },
    );
    assert!(matches!(decoder::<Retour<'_>>(&r).unwrap(), Retour::Bot(_)));
}

/// Un geste ou un tenu de téléphone ne se décode pas : `Aucun` n'a pas de valeur.
#[test]
fn un_geste_de_telephone_sans_telephone_ne_se_decode_pas() {
    let mut i = creee();
    let mut entree = Vec::new();
    encoder(
        &EntreeFil::Geste {
            de: 5,
            octets: &[0],
            quand: jeu_abi::QuandFil {
                fait: 1,
                fraction: 0,
                recu: 1,
                vu: 0,
                id: 1,
            },
        },
        &mut entree,
    )
    .unwrap();
    let r = appeler(
        &mut i,
        &Appel::Entree {
            ordre: 0,
            place: 1,
            entree: &entree,
        },
    );
    assert!(matches!(
        decoder::<Retour<'_>>(&r).unwrap(),
        Retour::Illisible(_)
    ));
}

/// Besoin 3, à l'ABI 4 (PHASE-3, § 2.10) : un jeu sans téléphone n'a pas de page ;
/// `Page` y est illisible, sans appeler le jeu, sur une instance où rien n'est créé.
#[test]
fn la_page_d_un_jeu_sans_telephone_est_illisible() {
    let mut i = jeu_natif::InstanceNative::<Ardoise>::new();
    let r = appeler(
        &mut i,
        &Appel::Page {
            montre: &[0],
            pas: jeu_abi::Pas::du_fil(1),
            locaux: jeu_abi::LocauxFil::vides(),
        },
    );
    assert!(illisible(&r), "{r:?}");
}
