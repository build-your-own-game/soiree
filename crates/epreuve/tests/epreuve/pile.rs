//! Besoin 1 des jeux (`conception/BESOINS-DES-JEUX.md` ; Punchline n° 2, Sidequest B1,
//! Tier list n° 1) : une lecture faite entre deux écritures ne change pas l'empreinte
//! de l'état. Les octets qu'une écriture recopie de la pile (bourrage, variant vide,
//! mot jamais écrit) ne dépendent plus de ce que l'appel d'avant y a laissé : la passe
//! `pile` de la réécriture la remet à zéro au début de chaque écriture.
//!
//! Deux instances d'`ardoise`, dont l'état garde des valeurs à bourrage copiées depuis
//! la pile, reçoivent les mêmes écritures ; l'une seulement lit sa scène, son état et
//! son bot entre deux écritures. Leurs empreintes restent égales après chacune.

use ardoise::ActionEcran;
use jeu_abi::{
    Appel, EcranFil, EntreeFil, QuandFil, RegardFil, Retour, StyleFil, TexteCourt, decoder, encoder,
};
use partie::{Fabrique, Instance};

use crate::modules;

fn encode<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(v, &mut o).expect("encoder");
    o
}

fn appeler(i: &mut impl Instance, a: &Appel<'_>) -> Vec<u8> {
    i.appeler(a.sorte(), &encode(a), u64::MAX / 4)
        .unwrap_or_else(|e| panic!("{a:?} : {e}"))
}

fn lire(i: &mut impl Instance) {
    let _: Vec<u8> = appeler(
        i,
        &Appel::Regarder {
            regards: vec![RegardFil::Ecran],
        },
    );
    let _: Vec<u8> = appeler(
        i,
        &Appel::Etat {
            connues: Vec::new(),
        },
    );
    let _: Vec<u8> = appeler(
        i,
        &Appel::Bot {
            regard: RegardFil::Ecran,
            style: StyleFil::Sage,
            graine: 3,
            depuis_vue: 600,
        },
    );
    let _: Vec<u8> = appeler(i, &Appel::Canonique { morceau: 0 });
}

#[test]
fn une_lecture_entre_deux_ecritures_ne_change_pas_l_empreinte() {
    let (moteur, octets) = modules::module("ardoise");
    let charge = moteur.charger(&octets).expect("le moteur charge ardoise");
    let mut f = charge.fabrique();
    let mut avec = f.neuve().expect("instance");
    let mut sans = f.neuve().expect("instance");
    let presents = encode(&Vec::<jeu_abi::Presence>::new());
    let creer = Appel::Creer {
        graine: [9; 32],
        pas: 0,
        heure: 0,
        salle: TexteCourt::new("ABCD"),
        presents: &presents,
        garde: None,
    };
    for i in [&mut avec, &mut sans] {
        let _: Vec<u8> = appeler(i, &Appel::Bonjour);
        let _: Vec<u8> = appeler(i, &Appel::Decrire);
        let _: Vec<u8> = appeler(i, &creer);
    }
    let actions = (0..120u64).map(|k| match k % 17 {
        5 | 11 => ActionEcran::Annuler,
        16 => ActionEcran::Effacer,
        n => ActionEcran::Ecrire(ardoise::Chiffre::new((n % 10).cast_signed()).expect("chiffre")),
    });
    for (ordre, a) in (0u64..).zip(actions) {
        let action = encode(&a);
        let entree = encode(&EntreeFil::Ecran {
            quoi: EcranFil::Action(&action),
            quand: QuandFil {
                fait: ordre + 1,
                fraction: 0,
                recu: ordre + 1,
                vu: ordre,
                id: ordre,
            },
        });
        let appel = Appel::Entree {
            ordre,
            place: ordre + 1,
            entree: &entree,
        };
        lire(&mut avec);
        let ra = appeler(&mut avec, &appel);
        let rs = appeler(&mut sans, &appel);
        assert_eq!(ra, rs, "les deux instances rendent la même issue");
        assert!(matches!(decoder::<Retour<'_>>(&ra), Ok(Retour::Issue(_))));
        assert_eq!(
            avec.empreinte().expect("empreinte"),
            sans.empreinte().expect("empreinte"),
            "après l'entrée {ordre} ({a:?}), une lecture a changé l'état"
        );
    }
}
