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
