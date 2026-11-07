//! Une seconde lecture indépendante du format (§ 3.7, SAL-88, SAL-90).
//!
//! Le lecteur de ce fichier n'utilise ni `jeu_abi`, ni postcard, ni serde : il lit
//! les octets champ par champ, d'après la description du format. Il lit ce que
//! `jeu_abi::encoder` écrit, sur des messages tirés au hasard : un changement de
//! forme sans montée d'ABI le fait rougir, même si encodeur et décodeur changent
//! ensemble.

use jeu_abi::{
    Appel, Capacites, CauseDepart, CauseTenu, EchecDemande, EntreeFil, IssueFil, QuandFil,
    RegardFil, Reponse, Retour, ReveilFil, SortieFil, StyleFil, encoder,
};
use proptest::prelude::*;

/// Le lecteur indépendant.
struct Lecteur<'a> {
    octets: &'a [u8],
}

impl<'a> Lecteur<'a> {
    fn new(octets: &'a [u8]) -> Self {
        Lecteur { octets }
    }

    fn octet(&mut self) -> u8 {
        let (premier, reste) = self.octets.split_first().expect("fin des octets");
        self.octets = reste;
        *premier
    }

    /// Un varint LEB128 : sept bits par octet, poids faible d'abord.
    fn varint(&mut self) -> u64 {
        let mut v: u64 = 0;
        for decalage in (0..70).step_by(7) {
            let b = self.octet();
            v |= u64::from(b & 0x7f) << decalage;
            if b & 0x80 == 0 {
                return v;
            }
        }
        panic!("varint trop long");
    }

    fn tranche(&mut self) -> &'a [u8] {
        let n = usize::try_from(self.varint()).unwrap();
        let (t, reste) = self.octets.split_at(n);
        self.octets = reste;
        t
    }

    fn brut<const N: usize>(&mut self) -> [u8; N] {
        let (t, reste) = self.octets.split_at(N);
        self.octets = reste;
        t.try_into().unwrap()
    }

    fn quand(&mut self) -> QuandFil {
        QuandFil {
            fait: self.varint(),
            fraction: u16::try_from(self.varint()).unwrap(),
            recu: self.varint(),
            vu: self.varint(),
            id: self.varint(),
        }
    }

    fn fini(&self) -> bool {
        self.octets.is_empty()
    }
}

fn encode<T: serde::Serialize>(v: &T) -> Vec<u8> {
    let mut o = Vec::new();
    encoder(v, &mut o).unwrap();
    o
}

/// Ce que le lecteur indépendant retrouve d'une entrée.
#[derive(Debug, PartialEq)]
enum Lu {
    Temps {
        saut: u64,
        heure: u64,
    },
    Geste {
        de: u64,
        octets: Vec<u8>,
        quand: QuandFil,
    },
    Tenu {
        de: u64,
        octets: Vec<u8>,
        cause: u64,
        quand: QuandFil,
    },
    Depart {
        appareil: u64,
        cause: u64,
    },
    Fait {
        id: u64,
        echec: Option<u64>,
    },
}

/// Lit une `EntreeFil` d'après le format seul.
fn lire_entree(l: &mut Lecteur<'_>) -> Lu {
    match l.varint() {
        0 => Lu::Temps {
            saut: l.varint(),
            heure: l.varint(),
        },
        2 => Lu::Geste {
            de: l.varint(),
            octets: l.tranche().to_vec(),
            quand: l.quand(),
        },
        3 => Lu::Tenu {
            de: l.varint(),
            octets: l.tranche().to_vec(),
            cause: l.varint(),
            quand: l.quand(),
        },
        5 => {
            assert_eq!(l.varint(), 1, "Presence::Depart");
            Lu::Depart {
                appareil: l.varint(),
                cause: l.varint(),
            }
        }
        6 => {
            let id = l.varint();
            assert_eq!(l.varint(), 3, "Reponse::Fait");
            let echec = match l.varint() {
                0 => None,
                1 => Some(l.varint()),
                autre => panic!("Result inconnu {autre}"),
            };
            Lu::Fait { id, echec }
        }
        autre => panic!("variant d'EntreeFil inattendu {autre}"),
    }
}

/// Le cas « journal » : une suite d'entrées telle que le cœur la tiendra, chaque
/// entrée dans un `Appel::Entree`.
fn entree() -> impl Strategy<Value = (EntreeFil<'static>, Lu)> {
    let quand = (
        any::<u64>(),
        any::<u16>(),
        any::<u64>(),
        any::<u64>(),
        any::<u64>(),
    )
        .prop_map(|(fait, fraction, recu, vu, id)| QuandFil {
            fait,
            fraction,
            recu,
            vu,
            id,
        });
    let octets = proptest::collection::vec(any::<u8>(), 0..40);
    let cause_tenu = prop_oneof![
        Just((CauseTenu::Geste, 0)),
        Just((CauseTenu::Silence, 1)),
        Just((CauseTenu::Depart, 2)),
        Just((CauseTenu::ArrierePlan, 3)),
        Just((CauseTenu::Pause, 4)),
    ];
    let cause_depart = prop_oneof![
        Just((CauseDepart::Silence, 0)),
        Just((CauseDepart::Fermeture, 1)),
        Just((CauseDepart::Voulu, 2)),
    ];
    let echec = prop_oneof![
        Just(None),
        Just(Some((EchecDemande::TropGros, 0))),
        Just(Some((EchecDemande::Interne, 4))),
    ];
    prop_oneof![
        (any::<u64>(), any::<u64>()).prop_map(|(saut, heure)| (
            EntreeFil::Temps { saut, heure },
            Lu::Temps { saut, heure }
        )),
        (any::<u64>(), octets.clone(), quand.clone()).prop_map(|(de, o, quand)| {
            let o: &'static [u8] = Box::leak(o.into_boxed_slice());
            (
                EntreeFil::Geste {
                    de,
                    octets: o,
                    quand,
                },
                Lu::Geste {
                    de,
                    octets: o.to_vec(),
                    quand,
                },
            )
        }),
        (any::<u64>(), octets, cause_tenu, quand).prop_map(|(de, o, (cause, c), quand)| {
            let o: &'static [u8] = Box::leak(o.into_boxed_slice());
            (
                EntreeFil::Tenu {
                    de,
                    octets: o,
                    cause,
                    quand,
                },
                Lu::Tenu {
                    de,
                    octets: o.to_vec(),
                    cause: c,
                    quand,
                },
            )
        }),
        (any::<u64>(), cause_depart).prop_map(|(a, (cause, c))| (
            EntreeFil::Presence(jeu_abi::Presence::Depart {
                appareil: jeu_abi::Appareil::du_fil(a),
                cause
            }),
            Lu::Depart {
                appareil: a,
                cause: c
            },
        )),
        (any::<u64>(), echec).prop_map(|(id, e)| (
            EntreeFil::Reponse {
                id,
                reponse: Reponse::Fait(e.map_or(Ok(()), |(e, _)| Err(e))),
            },
            Lu::Fait {
                id,
                echec: e.map(|(_, c)| c)
            },
        )),
    ]
}

proptest! {
    /// Un journal d'entrées tirées au hasard, encodé par `jeu_abi`, se relit en
    /// entier par le lecteur indépendant (SAL-90).
    #[test]
    fn seconde_lecture_d_un_journal(journal in proptest::collection::vec((entree(), any::<u64>(), any::<u64>()), 0..32)) {
        let mut octets = Vec::new();
        for ((e, _), ordre, place) in &journal {
            let entree = encode(e);
            octets.extend(encode(&Appel::Entree { ordre: *ordre, place: *place, entree: &entree }));
        }
        let mut l = Lecteur::new(&octets);
        for ((_, attendu), ordre, place) in &journal {
            prop_assert_eq!(l.varint(), 4, "Appel::Entree");
            prop_assert_eq!(l.varint(), *ordre);
            prop_assert_eq!(l.varint(), *place);
            let mut dedans = Lecteur::new(l.tranche());
            prop_assert_eq!(&lire_entree(&mut dedans), attendu);
            prop_assert!(dedans.fini());
        }
        prop_assert!(l.fini());
    }
}

/// `Appel::Creer` et `Appel::Bot`, lus champ par champ.
#[test]
fn seconde_lecture_des_appels() {
    let presents = encode(&vec![jeu_abi::Presence::Arrivee {
        appareil: jeu_abi::Appareil::du_fil(300),
        role: jeu_abi::RoleAppareil::Telephone,
        capacites: Capacites::AUCUNE,
    }]);
    let creer = encode(&Appel::Creer {
        graine: [9; 32],
        pas: 1 << 40,
        heure: 1_700_000_000_000,
        salle: jeu_abi::TexteCourt::new("K7Q2"),
        presents: &presents,
        garde: None,
    });
    let mut l = Lecteur::new(&creer);
    assert_eq!(l.varint(), 2);
    assert_eq!(l.brut::<32>(), [9; 32]);
    assert_eq!(l.varint(), 1 << 40);
    assert_eq!(l.varint(), 1_700_000_000_000);
    assert_eq!(l.tranche(), b"K7Q2");
    let mut p = Lecteur::new(l.tranche());
    assert_eq!(l.octet(), 0, "garde : None");
    assert!(l.fini());
    assert_eq!(p.varint(), 1, "un présent");
    assert_eq!(p.varint(), 0, "Arrivee");
    assert_eq!(p.varint(), 300);
    assert_eq!(p.varint(), 0, "Telephone");
    for _ in 0..6 {
        assert_eq!((p.varint(), p.varint()), (1, 0), "Non(NonSupportee)");
    }
    assert_eq!(p.varint(), 0, "Portrait");
    assert_eq!(p.octet(), 0, "pas de prénom");
    assert!(p.fini());

    let bot = encode(&Appel::Bot {
        regard: RegardFil::Appareil(u64::MAX),
        style: StyleFil::Lent,
        graine: 77,
        depuis_vue: 0,
    });
    let mut l = Lecteur::new(&bot);
    assert_eq!(
        (
            l.varint(),
            l.varint(),
            l.varint(),
            l.varint(),
            l.varint(),
            l.varint()
        ),
        (16, 1, u64::MAX, 2, 77, 0)
    );
    assert!(l.fini());
}

/// `Retour::Issue` lu champ par champ, sorties et réveil compris.
#[test]
fn seconde_lecture_d_une_issue() {
    let issue = encode(&Retour::Issue(IssueFil::Acceptee {
        sorties: vec![
            SortieFil::Retirer(12),
            SortieFil::NouvellePartie { graine: [3; 32] },
            SortieFil::Oublier(u64::MAX),
        ],
        reveil: ReveilFil::A(1 << 20),
    }));
    let mut l = Lecteur::new(&issue);
    assert_eq!((l.varint(), l.varint(), l.varint()), (2, 0, 3));
    assert_eq!((l.varint(), l.varint()), (1, 12));
    assert_eq!(l.varint(), 2);
    assert_eq!(l.brut::<32>(), [3; 32]);
    assert_eq!((l.varint(), l.varint()), (0, u64::MAX));
    assert_eq!((l.varint(), l.varint()), (1, 1 << 20));
    assert!(l.fini());
}

proptest! {
    /// `Appel::Page` (ABI 4), lu champ par champ par le lecteur indépendant, sur des
    /// locaux tirés au hasard : l'écran, les champs, les choix, les appuyés, les croix,
    /// le retenu.
    #[test]
    fn seconde_lecture_de_la_page(
        montre in proptest::collection::vec(any::<u8>(), 0..32),
        pas in any::<u64>(),
        (largeur, hauteur) in (any::<u16>(), any::<u16>()),
        champs in proptest::collection::vec((any::<u64>(), "[a-zé ]{0,12}"), 0..4),
        choix in proptest::collection::vec((any::<u64>(), any::<u32>()), 0..4),
        appuyes in proptest::collection::vec(any::<u64>(), 0..4),
        croix in proptest::collection::vec((any::<u64>(), 0usize..9), 0..4),
        retenu in proptest::option::of(proptest::collection::vec(any::<u8>(), 0..16)),
    ) {
        let champs_fil: Vec<(jeu_abi::Cle, &str)> = champs
            .iter()
            .map(|(c, t)| (jeu_abi::Cle::du_fil(*c), t.as_str()))
            .collect();
        let locaux = jeu_abi::LocauxFil {
            ecran: jeu_abi::EcranTelephone {
                largeur,
                hauteur,
                ..jeu_abi::EcranTelephone::ORDINAIRE
            },
            champs: champs_fil,
            choix: choix.iter().map(|(c, n)| (jeu_abi::Cle::du_fil(*c), *n)).collect(),
            appuyes: appuyes.iter().map(|c| jeu_abi::Cle::du_fil(*c)).collect(),
            croix: croix
                .iter()
                .map(|(c, d)| (jeu_abi::Cle::du_fil(*c), jeu_abi::Direction8::TOUTES[*d]))
                .collect(),
            retenu: retenu.as_deref(),
            capacites: Capacites::AUCUNE,
        };
        let octets = encode(&Appel::Page {
            montre: &montre,
            pas: jeu_abi::Pas::du_fil(pas),
            locaux,
        });
        let mut l = Lecteur::new(&octets);
        prop_assert_eq!(l.varint(), 21, "Appel::Page");
        prop_assert_eq!(l.tranche(), montre.as_slice());
        prop_assert_eq!(l.varint(), pas);
        prop_assert_eq!((l.varint(), l.varint()), (u64::from(largeur), u64::from(hauteur)));
        prop_assert_eq!(
            (l.varint(), l.varint(), l.varint(), l.varint(), l.varint()),
            (0, 0, 0, 0, 0),
            "sans marge, en portrait"
        );
        prop_assert_eq!(l.varint(), u64::try_from(champs.len()).unwrap());
        for (c, t) in &champs {
            prop_assert_eq!(l.varint(), *c);
            prop_assert_eq!(l.tranche(), t.as_bytes());
        }
        prop_assert_eq!(l.varint(), u64::try_from(choix.len()).unwrap());
        for (c, n) in &choix {
            prop_assert_eq!((l.varint(), l.varint()), (*c, u64::from(*n)));
        }
        prop_assert_eq!(l.varint(), u64::try_from(appuyes.len()).unwrap());
        for c in &appuyes {
            prop_assert_eq!(l.varint(), *c);
        }
        prop_assert_eq!(l.varint(), u64::try_from(croix.len()).unwrap());
        for (c, d) in &croix {
            prop_assert_eq!((l.varint(), l.varint()), (*c, u64::try_from(*d).unwrap()));
        }
        match &retenu {
            None => prop_assert_eq!(l.octet(), 0),
            Some(r) => {
                prop_assert_eq!(l.octet(), 1);
                prop_assert_eq!(l.tranche(), r.as_slice());
            }
        }
        for _ in 0..6 {
            prop_assert_eq!((l.varint(), l.varint()), (1, 0), "Non(NonSupportee)");
        }
        prop_assert_eq!((l.varint(), l.octet()), (0, 0), "Portrait, pas de prénom");
        prop_assert!(l.fini());
    }
}
